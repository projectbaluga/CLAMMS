use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{Path as AxumPath, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::{env, net::SocketAddr, path::Path, str::FromStr};
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    agent_secret: String,
}

#[derive(Deserialize)]
struct HeartbeatRequest {
    asset_tag: String,
    hostname: Option<String>,
    os_version: Option<String>,
    cpu_percent: Option<f32>,
    memory_percent: Option<f32>,
    disk_free_bytes: Option<i64>,
    uptime_seconds: Option<i64>,
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct ApiMessage {
    status: &'static str,
    message: &'static str,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    token: String,
    username: String,
    full_name: String,
    role: String,
}

#[derive(Debug, Serialize)]
struct CurrentUser {
    id: i64,
    username: String,
    full_name: String,
    role: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--check-db") {
        let database_path = args
            .get(2)
            .ok_or_else(|| anyhow::anyhow!("Usage: clamms-server --check-db <database-path>"))?;
        verify_database_integrity(database_path).await?;
        println!("SQLite integrity check passed: {database_path}");
        return Ok(());
    }
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let bind: SocketAddr = env::var("CLAMMS_BIND")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        .parse()?;
    let database_url =
        env::var("CLAMMS_DATABASE_URL").unwrap_or_else(|_| "sqlite://data/clamms.db".to_string());
    let agent_secret = env::var("CLAMMS_AGENT_SECRET").map_err(|_| {
        anyhow::anyhow!("CLAMMS_AGENT_SECRET is required; configure a unique secret")
    })?;
    anyhow::ensure!(
        agent_secret.len() >= 32,
        "CLAMMS_AGENT_SECRET must be at least 32 characters"
    );

    if let Some(parent) = Path::new("data/clamms.db").parent() {
        std::fs::create_dir_all(parent)?;
    }
    if database_url.starts_with("sqlite://") {
        let db_path = database_url.trim_start_matches("sqlite://");
        if let Some(parent) = Path::new(db_path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
    }

    let connect_options = SqliteConnectOptions::from_str(&database_url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await?;
    sqlx::migrate!("./migrations").run(&db).await?;
    bootstrap_admin(&db).await?;

    let state = AppState { db, agent_secret };
    let app = Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(me))
        .route("/api/auth/logout", post(logout))
        .route("/api/assets", get(list_assets).post(create_asset))
        .route("/api/assets/:id", put(update_asset))
        .route(
            "/api/laboratories",
            get(list_laboratories).post(create_laboratory),
        )
        .route("/api/laboratories/:id", put(update_laboratory))
        .route(
            "/api/maintenance",
            get(list_maintenance).post(create_maintenance),
        )
        .route("/api/maintenance/:id", put(update_maintenance))
        .route("/api/monitoring", get(list_monitoring))
        .route("/api/reports/assets.csv", get(export_assets_csv))
        .route("/api/reports/maintenance.csv", get(export_maintenance_csv))
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/:id", put(update_user))
        .route("/api/audit", get(list_audit))
        .route("/api/admin/backup", get(download_database_backup))
        .route("/api/heartbeat", post(heartbeat))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    info!("CLAMMS Rust V2 listening on http://{bind}");
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn verify_database_integrity(database_path: &str) -> anyhow::Result<()> {
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(database_path)
        .read_only(true)
        .foreign_keys(true);
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    let result: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&db)
        .await?;
    db.close().await;
    anyhow::ensure!(result == "ok", "SQLite integrity check failed: {result}");
    Ok(())
}

async fn bootstrap_admin(db: &SqlitePool) -> anyhow::Result<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(db)
        .await?;
    if count > 0 {
        return Ok(());
    }
    let (username, password) = match (
        env::var("CLAMMS_BOOTSTRAP_ADMIN_USERNAME"),
        env::var("CLAMMS_BOOTSTRAP_ADMIN_PASSWORD"),
    ) {
        (Ok(u), Ok(p)) if !u.trim().is_empty() && p.len() >= 12 => (u, p),
        _ => {
            info!("No users exist. Set CLAMMS_BOOTSTRAP_ADMIN_USERNAME and CLAMMS_BOOTSTRAP_ADMIN_PASSWORD to create the first administrator.");
            return Ok(());
        }
    };
    let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes())
        .map_err(|err| anyhow::anyhow!("Could not encode bootstrap password salt: {err}"))?;
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| anyhow::anyhow!("Could not hash bootstrap administrator password: {err}"))?
        .to_string();
    sqlx::query("INSERT INTO users (full_name, username, password_hash, role) VALUES (?, ?, ?, 'administrator')")
        .bind("CLAMMS Administrator").bind(username.trim()).bind(hash)
        .execute(db).await?;
    info!("Initial administrator account created from environment configuration.");
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn health() -> Json<ApiMessage> {
    Json(ApiMessage {
        status: "ok",
        message: "CLAMMS Rust V2 is running",
    })
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<ApiMessage>)> {
    let row = sqlx::query_as::<_, (i64, String, String, String, i64)>(
        "SELECT id, username, full_name, password_hash, is_active FROM users WHERE username = ?",
    )
    .bind(payload.username.trim())
    .fetch_optional(&state.db)
    .await
    .map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Login could not be processed",
        )
    })?;

    let Some((user_id, username, full_name, password_hash, active)) = row else {
        return Err(api_error(
            StatusCode::UNAUTHORIZED,
            "Invalid username or password",
        ));
    };
    let parsed = PasswordHash::new(&password_hash).map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Stored password hash is invalid",
        )
    })?;
    if active != 1
        || Argon2::default()
            .verify_password(payload.password.as_bytes(), &parsed)
            .is_err()
    {
        return Err(api_error(
            StatusCode::UNAUTHORIZED,
            "Invalid username or password",
        ));
    }

    sqlx::query("DELETE FROM sessions WHERE expires_at <= CURRENT_TIMESTAMP")
        .execute(&state.db)
        .await
        .map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not clean expired sessions",
            )
        })?;
    let token = Uuid::new_v4().to_string() + &Uuid::new_v4().to_string();
    let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
    sqlx::query("INSERT INTO sessions (user_id, token_hash, expires_at) VALUES (?, ?, datetime('now', '+12 hours'))")
        .bind(user_id).bind(token_hash).execute(&state.db).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not create session"))?;
    let role: String = sqlx::query_scalar("SELECT role FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not load user role",
            )
        })?;
    Ok(Json(LoginResponse {
        token,
        username,
        full_name,
        role,
    }))
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CurrentUser>, (StatusCode, Json<ApiMessage>)> {
    let user = authenticated_user(&state.db, &headers).await?;
    Ok(Json(user))
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiMessage>, (StatusCode, Json<ApiMessage>)> {
    let token = bearer_token(&headers)?;
    let hash = hex::encode(Sha256::digest(token.as_bytes()));
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(hash)
        .execute(&state.db)
        .await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not end session"))?;
    Ok(Json(ApiMessage {
        status: "ok",
        message: "Logged out",
    }))
}

async fn authenticated_user(
    db: &SqlitePool,
    headers: &HeaderMap,
) -> Result<CurrentUser, (StatusCode, Json<ApiMessage>)> {
    let token = bearer_token(headers)?;
    let hash = hex::encode(Sha256::digest(token.as_bytes()));
    sqlx::query_as::<_, (i64, String, String, String)>(
        "SELECT u.id, u.username, u.full_name, u.role FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = ? AND s.expires_at > CURRENT_TIMESTAMP AND u.is_active = 1"
    )
    .bind(hash).fetch_optional(db).await
    .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not verify session"))?
    .map(|(id, username, full_name, role)| CurrentUser { id, username, full_name, role })
    .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "Login required or session expired"))
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, (StatusCode, Json<ApiMessage>)> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| !v.is_empty())
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "Bearer token required"))
}

fn api_error(status: StatusCode, message: &'static str) -> (StatusCode, Json<ApiMessage>) {
    (
        status,
        Json(ApiMessage {
            status: "error",
            message,
        }),
    )
}

async fn list_laboratories(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<
    Json<Vec<(i64, String, String, Option<String>, Option<String>)>>,
    (StatusCode, Json<ApiMessage>),
> {
    authenticated_user(&state.db, &headers).await?;
    let rows = sqlx::query_as::<_, (i64, String, String, Option<String>, Option<String>)>(
        "SELECT id, code, name, location, description FROM laboratories ORDER BY code",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load laboratories",
        )
    })?;
    Ok(Json(rows))
}

async fn list_assets(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<
    Json<
        Vec<(
            i64,
            String,
            i64,
            Option<String>,
            Option<String>,
            String,
            Option<i64>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        )>,
    >,
    (StatusCode, Json<ApiMessage>),
> {
    authenticated_user(&state.db, &headers).await?;
    let rows = sqlx::query_as::<_, (i64, String, i64, Option<String>, Option<String>, String, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, asset_tag, laboratory_id, brand, model, status, category_id, serial_number, hostname, ip_address, purchase_date, warranty_end, notes FROM assets ORDER BY asset_tag"
    ).fetch_all(&state.db).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not load assets"))?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct UserInput {
    full_name: String,
    username: String,
    role: String,
    password: Option<String>,
    is_active: Option<bool>,
}

fn valid_role(role: &str) -> bool {
    matches!(
        role,
        "administrator" | "custodian" | "technician" | "viewer"
    )
}

fn password_hash(password: &str) -> Result<String, (StatusCode, Json<ApiMessage>)> {
    if password.len() < 12 || password.len() > 1024 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Password must be 12 to 1024 bytes long",
        ));
    }
    let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes()).map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not prepare password hash",
        )
    })?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not hash password"))
}

async fn list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<(i64, String, String, String, i64)>>, (StatusCode, Json<ApiMessage>)> {
    let user = authenticated_user(&state.db, &headers).await?;
    if user.role != "administrator" {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "Administrator role required",
        ));
    }
    let rows = sqlx::query_as::<_, (i64, String, String, String, i64)>(
        "SELECT id,full_name,username,role,is_active FROM users ORDER BY full_name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not load users"))?;
    Ok(Json(rows))
}

async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<UserInput>,
) -> Result<(StatusCode, Json<ApiMessage>), (StatusCode, Json<ApiMessage>)> {
    let user = authenticated_user(&state.db, &headers).await?;
    if user.role != "administrator" {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "Administrator role required",
        ));
    }
    let full = input.full_name.trim();
    let username = input.username.trim();
    if full.is_empty()
        || full.len() > 150
        || username.is_empty()
        || username.len() > 80
        || !valid_role(&input.role)
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "User details or role are invalid",
        ));
    }
    let password = input
        .password
        .as_deref()
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "Initial password is required"))?;
    let hash = password_hash(password)?;
    let r = sqlx::query(
        "INSERT INTO users (full_name,username,password_hash,role,is_active) VALUES (?,?,?,?,?)",
    )
    .bind(full)
    .bind(username)
    .bind(hash)
    .bind(&input.role)
    .bind(input.is_active.unwrap_or(true) as i64)
    .execute(&state.db)
    .await;
    match r {
        Ok(r) => {
            write_audit(
                &state.db,
                user.id,
                "create",
                "user",
                Some(r.last_insert_rowid()),
                username,
            )
            .await;
            Ok((
                StatusCode::CREATED,
                Json(ApiMessage {
                    status: "ok",
                    message: "User created",
                }),
            ))
        }
        Err(_) => Err(api_error(
            StatusCode::CONFLICT,
            "Username is already in use or user data is invalid",
        )),
    }
}

async fn update_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
    Json(input): Json<UserInput>,
) -> Result<Json<ApiMessage>, (StatusCode, Json<ApiMessage>)> {
    let actor = authenticated_user(&state.db, &headers).await?;
    if actor.role != "administrator" {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "Administrator role required",
        ));
    }
    let full = input.full_name.trim();
    let username = input.username.trim();
    if id <= 0
        || full.is_empty()
        || full.len() > 150
        || username.is_empty()
        || username.len() > 80
        || !valid_role(&input.role)
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "User details or role are invalid",
        ));
    }
    let active = input.is_active.ok_or_else(|| {
        api_error(
            StatusCode::BAD_REQUEST,
            "is_active is required when updating a user",
        )
    })? as i64;
    if let Some(password) = input.password.as_deref() {
        let hash = password_hash(password)?;
        sqlx::query(
            "UPDATE users SET full_name=?,username=?,role=?,is_active=?,password_hash=? WHERE id=?",
        )
        .bind(full)
        .bind(username)
        .bind(&input.role)
        .bind(active)
        .bind(hash)
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|_| {
            api_error(
                StatusCode::CONFLICT,
                "Could not update user; username may be in use",
            )
        })?;
        sqlx::query("DELETE FROM sessions WHERE user_id=?")
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|_| {
                api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "User updated but sessions could not be revoked",
                )
            })?;
    } else {
        sqlx::query("UPDATE users SET full_name=?,username=?,role=?,is_active=? WHERE id=?")
            .bind(full)
            .bind(username)
            .bind(&input.role)
            .bind(active)
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|_| {
                api_error(
                    StatusCode::CONFLICT,
                    "Could not update user; username may be in use",
                )
            })?;
    }
    write_audit(&state.db, actor.id, "update", "user", Some(id), username).await;
    Ok(Json(ApiMessage {
        status: "ok",
        message: "User updated; active sessions revoked when password changed",
    }))
}

#[derive(Deserialize)]
struct LaboratoryInput {
    code: String,
    name: String,
    location: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct AssetInput {
    asset_tag: String,
    laboratory_id: i64,
    category_id: Option<i64>,
    brand: Option<String>,
    model: Option<String>,
    serial_number: Option<String>,
    hostname: Option<String>,
    ip_address: Option<String>,
    status: Option<String>,
    purchase_date: Option<String>,
    warranty_end: Option<String>,
    notes: Option<String>,
}

#[derive(Deserialize)]
struct MaintenanceInput {
    asset_id: i64,
    issue: String,
    action_taken: Option<String>,
    status: Option<String>,
}

fn can_manage_inventory(role: &str) -> bool {
    matches!(role, "administrator" | "custodian")
}

fn can_manage_maintenance(role: &str) -> bool {
    matches!(role, "administrator" | "custodian" | "technician")
}

fn valid_asset_status(value: &str) -> bool {
    matches!(
        value,
        "working" | "needs_repair" | "under_maintenance" | "retired" | "missing"
    )
}

fn valid_maintenance_status(value: &str) -> bool {
    matches!(value, "open" | "in_progress" | "resolved" | "cancelled")
}

async fn write_audit(
    db: &SqlitePool,
    actor_id: i64,
    action: &str,
    entity: &str,
    id: Option<i64>,
    details: &str,
) {
    let _ = sqlx::query("INSERT INTO audit_logs (user_id, action, entity_type, entity_id, details) VALUES (?, ?, ?, ?, ?)")
        .bind(actor_id).bind(action).bind(entity).bind(id.map(|v| v.to_string())).bind(details)
        .execute(db).await;
}

async fn create_laboratory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<LaboratoryInput>,
) -> Result<(StatusCode, Json<ApiMessage>), (StatusCode, Json<ApiMessage>)> {
    let user = authenticated_user(&state.db, &headers).await?;
    if !can_manage_inventory(&user.role) {
        return Err(api_error(StatusCode::FORBIDDEN, "Insufficient permissions"));
    }
    let code = input.code.trim();
    let name = input.name.trim();
    if code.is_empty() || code.len() > 30 || name.is_empty() || name.len() > 150 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Laboratory code or name is invalid",
        ));
    }
    let result =
        sqlx::query("INSERT INTO laboratories (code,name,location,description) VALUES (?,?,?,?)")
            .bind(code)
            .bind(name)
            .bind(input.location)
            .bind(input.description)
            .execute(&state.db)
            .await;
    match result {
        Ok(r) => {
            write_audit(
                &state.db,
                user.id,
                "create",
                "laboratory",
                Some(r.last_insert_rowid()),
                code,
            )
            .await;
            Ok((
                StatusCode::CREATED,
                Json(ApiMessage {
                    status: "ok",
                    message: "Laboratory created",
                }),
            ))
        }
        Err(_) => Err(api_error(
            StatusCode::CONFLICT,
            "Laboratory code already exists or data is invalid",
        )),
    }
}

async fn update_laboratory(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
    Json(input): Json<LaboratoryInput>,
) -> Result<Json<ApiMessage>, (StatusCode, Json<ApiMessage>)> {
    let user = authenticated_user(&state.db, &headers).await?;
    if !can_manage_inventory(&user.role) {
        return Err(api_error(StatusCode::FORBIDDEN, "Insufficient permissions"));
    }
    let code = input.code.trim();
    let name = input.name.trim();
    if id <= 0 || code.is_empty() || code.len() > 30 || name.is_empty() || name.len() > 150 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Laboratory fields are invalid",
        ));
    }
    let r =
        sqlx::query("UPDATE laboratories SET code=?,name=?,location=?,description=? WHERE id=?")
            .bind(code)
            .bind(name)
            .bind(input.location)
            .bind(input.description)
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|_| {
                api_error(
                    StatusCode::CONFLICT,
                    "Could not update laboratory; code may already exist",
                )
            })?;
    if r.rows_affected() == 0 {
        return Err(api_error(StatusCode::NOT_FOUND, "Laboratory not found"));
    }
    write_audit(&state.db, user.id, "update", "laboratory", Some(id), code).await;
    Ok(Json(ApiMessage {
        status: "ok",
        message: "Laboratory updated",
    }))
}

async fn create_asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<AssetInput>,
) -> Result<(StatusCode, Json<ApiMessage>), (StatusCode, Json<ApiMessage>)> {
    let user = authenticated_user(&state.db, &headers).await?;
    if !can_manage_inventory(&user.role) {
        return Err(api_error(StatusCode::FORBIDDEN, "Insufficient permissions"));
    }
    let tag = input.asset_tag.trim();
    let status = input.status.as_deref().unwrap_or("working");
    if tag.is_empty() || tag.len() > 100 || input.laboratory_id <= 0 || !valid_asset_status(status)
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Asset tag, laboratory or status is invalid",
        ));
    }
    let r=sqlx::query("INSERT INTO assets (asset_tag,laboratory_id,category_id,brand,model,serial_number,hostname,ip_address,status,purchase_date,warranty_end,notes) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)")
        .bind(tag).bind(input.laboratory_id).bind(input.category_id).bind(input.brand).bind(input.model)
        .bind(input.serial_number).bind(input.hostname).bind(input.ip_address).bind(status)
        .bind(input.purchase_date).bind(input.warranty_end).bind(input.notes).execute(&state.db).await;
    match r {
        Ok(r) => {
            write_audit(
                &state.db,
                user.id,
                "create",
                "asset",
                Some(r.last_insert_rowid()),
                tag,
            )
            .await;
            Ok((
                StatusCode::CREATED,
                Json(ApiMessage {
                    status: "ok",
                    message: "Asset created",
                }),
            ))
        }
        Err(_) => Err(api_error(
            StatusCode::CONFLICT,
            "Asset tag is already used or referenced data is invalid",
        )),
    }
}

async fn update_asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
    Json(input): Json<AssetInput>,
) -> Result<Json<ApiMessage>, (StatusCode, Json<ApiMessage>)> {
    let user = authenticated_user(&state.db, &headers).await?;
    if !can_manage_inventory(&user.role) {
        return Err(api_error(StatusCode::FORBIDDEN, "Insufficient permissions"));
    }
    let tag = input.asset_tag.trim();
    let status = input.status.as_deref().unwrap_or("working");
    if id <= 0
        || tag.is_empty()
        || tag.len() > 100
        || input.laboratory_id <= 0
        || !valid_asset_status(status)
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Asset fields are invalid",
        ));
    }
    let r=sqlx::query("UPDATE assets SET asset_tag=?,laboratory_id=?,category_id=?,brand=?,model=?,serial_number=?,hostname=?,ip_address=?,status=?,purchase_date=?,warranty_end=?,notes=?,updated_at=CURRENT_TIMESTAMP WHERE id=?")
        .bind(tag).bind(input.laboratory_id).bind(input.category_id).bind(input.brand).bind(input.model)
        .bind(input.serial_number).bind(input.hostname).bind(input.ip_address).bind(status)
        .bind(input.purchase_date).bind(input.warranty_end).bind(input.notes).bind(id).execute(&state.db).await
        .map_err(|_|api_error(StatusCode::CONFLICT,"Could not update asset; tag may already exist"))?;
    if r.rows_affected() == 0 {
        return Err(api_error(StatusCode::NOT_FOUND, "Asset not found"));
    }
    write_audit(&state.db, user.id, "update", "asset", Some(id), tag).await;
    Ok(Json(ApiMessage {
        status: "ok",
        message: "Asset updated",
    }))
}

async fn list_maintenance(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<
    Json<Vec<(i64, i64, String, Option<String>, String, String)>>,
    (StatusCode, Json<ApiMessage>),
> {
    authenticated_user(&state.db, &headers).await?;
    let rows=sqlx::query_as::<_,(i64,i64,String,Option<String>,String,String)>(
        "SELECT id,asset_id,issue,action_taken,status,reported_at FROM maintenance_logs ORDER BY reported_at DESC LIMIT 500"
    ).fetch_all(&state.db).await.map_err(|_|api_error(StatusCode::INTERNAL_SERVER_ERROR,"Could not load maintenance records"))?;
    Ok(Json(rows))
}

async fn create_maintenance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<MaintenanceInput>,
) -> Result<(StatusCode, Json<ApiMessage>), (StatusCode, Json<ApiMessage>)> {
    let user = authenticated_user(&state.db, &headers).await?;
    if !can_manage_maintenance(&user.role) {
        return Err(api_error(StatusCode::FORBIDDEN, "Insufficient permissions"));
    }
    let issue = input.issue.trim();
    let status = input.status.as_deref().unwrap_or("open");
    if input.asset_id <= 0
        || issue.is_empty()
        || issue.len() > 4000
        || !valid_maintenance_status(status)
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Maintenance details are invalid",
        ));
    }
    let r = sqlx::query(
        "INSERT INTO maintenance_logs (asset_id,issue,action_taken,status) VALUES (?,?,?,?)",
    )
    .bind(input.asset_id)
    .bind(issue)
    .bind(input.action_taken)
    .bind(status)
    .execute(&state.db)
    .await;
    match r {
        Ok(r) => {
            write_audit(
                &state.db,
                user.id,
                "create",
                "maintenance",
                Some(r.last_insert_rowid()),
                issue,
            )
            .await;
            Ok((
                StatusCode::CREATED,
                Json(ApiMessage {
                    status: "ok",
                    message: "Maintenance record created",
                }),
            ))
        }
        Err(_) => Err(api_error(
            StatusCode::BAD_REQUEST,
            "Asset does not exist or maintenance data is invalid",
        )),
    }
}

async fn update_maintenance(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
    Json(input): Json<MaintenanceInput>,
) -> Result<Json<ApiMessage>, (StatusCode, Json<ApiMessage>)> {
    let user = authenticated_user(&state.db, &headers).await?;
    if !can_manage_maintenance(&user.role) {
        return Err(api_error(StatusCode::FORBIDDEN, "Insufficient permissions"));
    }
    let issue = input.issue.trim();
    let status = input.status.as_deref().unwrap_or("open");
    if id <= 0
        || input.asset_id <= 0
        || issue.is_empty()
        || issue.len() > 4000
        || !valid_maintenance_status(status)
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Maintenance details are invalid",
        ));
    }
    let r=if status=="resolved" {
        sqlx::query("UPDATE maintenance_logs SET asset_id=?,issue=?,action_taken=?,status=?,resolved_at=CURRENT_TIMESTAMP WHERE id=?")
            .bind(input.asset_id).bind(issue).bind(input.action_taken).bind(status).bind(id).execute(&state.db).await
    } else {
        sqlx::query("UPDATE maintenance_logs SET asset_id=?,issue=?,action_taken=?,status=?,resolved_at=NULL WHERE id=?")
            .bind(input.asset_id).bind(issue).bind(input.action_taken).bind(status).bind(id).execute(&state.db).await
    }.map_err(|_|api_error(StatusCode::BAD_REQUEST,"Could not update maintenance record"))?;
    if r.rows_affected() == 0 {
        return Err(api_error(
            StatusCode::NOT_FOUND,
            "Maintenance record not found",
        ));
    }
    write_audit(
        &state.db,
        user.id,
        "update",
        "maintenance",
        Some(id),
        status,
    )
    .await;
    Ok(Json(ApiMessage {
        status: "ok",
        message: "Maintenance record updated",
    }))
}

fn csv_cell(value: &str) -> String {
    let first_non_whitespace = value
        .trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        .chars()
        .next();
    let safe = if first_non_whitespace
        .is_some_and(|c| matches!(c, '=' | '+' | '-' | '@'))
    {
        format!("'{}", value)
    } else {
        value.to_string()
    };
    format!("\"{}\"", safe.replace('"', "\"\""))
}

async fn export_assets_csv(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiMessage>)> {
    authenticated_user(&state.db, &headers).await?;
    let rows=sqlx::query_as::<_,(i64,String,String,i64,Option<String>,Option<String>,String)>(
        "SELECT a.id,a.asset_tag,l.code,a.laboratory_id,a.brand,a.model,a.status FROM assets a JOIN laboratories l ON l.id=a.laboratory_id ORDER BY l.code,a.asset_tag"
    ).fetch_all(&state.db).await
        .map_err(|_|api_error(StatusCode::INTERNAL_SERVER_ERROR,"Could not generate asset report"))?;
    let mut csv = String::from("id,asset_tag,laboratory_code,laboratory_id,brand,model,status\r\n");
    for row in rows {
        let cells = [
            row.0.to_string(),
            row.1,
            row.2,
            row.3.to_string(),
            row.4.unwrap_or_default(),
            row.5.unwrap_or_default(),
            row.6,
        ];
        csv.push_str(
            &cells
                .iter()
                .map(|v| csv_cell(v))
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push_str("\r\n");
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=clamms-assets.csv",
            ),
        ],
        csv,
    )
        .into_response())
}

async fn export_maintenance_csv(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiMessage>)> {
    authenticated_user(&state.db, &headers).await?;
    let rows=sqlx::query_as::<_,(i64,String,String,Option<String>,String,String,Option<String>)>(
        "SELECT m.id,a.asset_tag,m.issue,m.action_taken,m.status,m.reported_at,m.resolved_at FROM maintenance_logs m JOIN assets a ON a.id=m.asset_id ORDER BY m.reported_at DESC"
    ).fetch_all(&state.db).await
        .map_err(|_|api_error(StatusCode::INTERNAL_SERVER_ERROR,"Could not generate maintenance report"))?;
    let mut csv =
        String::from("id,asset_tag,issue,action_taken,status,reported_at,resolved_at\r\n");
    for row in rows {
        let cells = [
            row.0.to_string(),
            row.1,
            row.2,
            row.3.unwrap_or_default(),
            row.4,
            row.5,
            row.6.unwrap_or_default(),
        ];
        csv.push_str(
            &cells
                .iter()
                .map(|v| csv_cell(v))
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push_str("\r\n");
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=clamms-maintenance.csv",
            ),
        ],
        csv,
    )
        .into_response())
}

async fn download_database_backup(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiMessage>)> {
    let actor = authenticated_user(&state.db, &headers).await?;
    if actor.role != "administrator" {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "Administrator role required",
        ));
    }
    let backup_dir = Path::new("data/backups");
    std::fs::create_dir_all(backup_dir).map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not prepare backup directory",
        )
    })?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup_path = backup_dir.join(format!("clamms-{stamp}-{}.db", Uuid::new_v4()));
    let backup_name = backup_path.to_string_lossy().to_string();
    let result = sqlx::query("VACUUM INTO ?")
        .bind(&backup_name)
        .execute(&state.db)
        .await;
    if result.is_err() {
        let _ = std::fs::remove_file(&backup_path);
        return Err(api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not create consistent database backup",
        ));
    }
    let bytes_result = std::fs::read(&backup_path);
    let _ = std::fs::remove_file(&backup_path);
    let bytes = bytes_result.map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not read database backup",
        )
    })?;
    write_audit(
        &state.db,
        actor.id,
        "backup",
        "database",
        None,
        "Database backup downloaded",
    )
    .await;
    Ok((
        [
            (header::CONTENT_TYPE, "application/vnd.sqlite3"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=clamms-backup.db",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
        .into_response())
}

async fn list_audit(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<
    Json<
        Vec<(
            i64,
            Option<String>,
            String,
            String,
            Option<String>,
            Option<String>,
            String,
        )>,
    >,
    (StatusCode, Json<ApiMessage>),
> {
    let actor = authenticated_user(&state.db, &headers).await?;
    if actor.role != "administrator" {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "Administrator role required",
        ));
    }
    let rows=sqlx::query_as::<_,(i64,Option<String>,String,String,Option<String>,Option<String>,String)>(
        "SELECT a.id,u.full_name,a.action,a.entity_type,a.entity_id,a.details,a.created_at FROM audit_logs a LEFT JOIN users u ON u.id=a.user_id ORDER BY a.created_at DESC,a.id DESC LIMIT 500"
    ).fetch_all(&state.db).await
        .map_err(|_|api_error(StatusCode::INTERNAL_SERVER_ERROR,"Could not load audit records"))?;
    Ok(Json(rows))
}

async fn list_monitoring(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<
    Json<
        Vec<(
            i64,
            String,
            Option<String>,
            Option<String>,
            Option<f32>,
            Option<f32>,
            Option<i64>,
            Option<i64>,
            String,
        )>,
    >,
    (StatusCode, Json<ApiMessage>),
> {
    authenticated_user(&state.db, &headers).await?;
    let rows=sqlx::query_as::<_,(i64,String,Option<String>,Option<String>,Option<f32>,Option<f32>,Option<i64>,Option<i64>,String)>(
        "SELECT id,asset_tag,hostname,os_version,cpu_percent,memory_percent,disk_free_bytes,uptime_seconds,received_at FROM monitoring_logs ORDER BY received_at DESC LIMIT 500"
    ).fetch_all(&state.db).await
        .map_err(|_|api_error(StatusCode::INTERNAL_SERVER_ERROR,"Could not load monitoring records"))?;
    Ok(Json(rows))
}

async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<HeartbeatRequest>,
) -> Result<(StatusCode, Json<ApiMessage>), (StatusCode, Json<ApiMessage>)> {
    let supplied = headers
        .get("x-clamms-secret")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if supplied != state.agent_secret {
        return Err(api_error(StatusCode::UNAUTHORIZED, "Unauthorized agent"));
    }
    let asset_tag = payload.asset_tag.trim();
    if asset_tag.is_empty()
        || asset_tag.len() > 100
        || payload
            .cpu_percent
            .is_some_and(|v| !v.is_finite() || !(0.0..=100.0).contains(&v))
        || payload
            .memory_percent
            .is_some_and(|v| !v.is_finite() || !(0.0..=100.0).contains(&v))
        || payload.disk_free_bytes.is_some_and(|v| v < 0)
        || payload.uptime_seconds.is_some_and(|v| v < 0)
        || payload.hostname.as_deref().is_some_and(|v| v.len() > 255)
        || payload.os_version.as_deref().is_some_and(|v| v.len() > 255)
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Invalid heartbeat values",
        ));
    }
    let known_asset: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM assets WHERE asset_tag = ?")
        .bind(asset_tag)
        .fetch_one(&state.db)
        .await
        .map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not validate asset tag",
            )
        })?;
    if known_asset == 0 {
        return Err(api_error(
            StatusCode::NOT_FOUND,
            "Asset tag is not registered in CLAMMS",
        ));
    }
    let result = sqlx::query(
        "INSERT INTO monitoring_logs (asset_tag, hostname, os_version, cpu_percent, memory_percent, disk_free_bytes, uptime_seconds, received_at) VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)"
    )
    .bind(asset_tag).bind(payload.hostname).bind(payload.os_version)
    .bind(payload.cpu_percent).bind(payload.memory_percent)
    .bind(payload.disk_free_bytes).bind(payload.uptime_seconds)
    .execute(&state.db).await;
    if let Err(err) = result {
        error!("Heartbeat database insert failed: {err}");
        return Err(api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not save heartbeat",
        ));
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(ApiMessage {
            status: "ok",
            message: "Heartbeat received",
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_state() -> AppState {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        sqlx::migrate!("./migrations")
            .run(&db)
            .await
            .expect("apply migrations");
        let hash = password_hash("correct-horse-battery-staple").expect("hash password");
        sqlx::query(
            "INSERT INTO users (full_name, username, password_hash, role) VALUES (?, ?, ?, ?)",
        )
        .bind("Test Administrator")
        .bind("admin-test")
        .bind(&hash)
        .bind("administrator")
        .execute(&db)
        .await
        .expect("insert administrator");
        sqlx::query(
            "INSERT INTO users (full_name, username, password_hash, role) VALUES (?, ?, ?, ?)",
        )
        .bind("Test Viewer")
        .bind("viewer-test")
        .bind(&hash)
        .bind("viewer")
        .execute(&db)
        .await
        .expect("insert viewer");
        AppState {
            db,
            agent_secret: "test-agent-secret-that-is-at-least-32-bytes".to_string(),
        }
    }

    async fn login_headers(state: &AppState, username: &str) -> HeaderMap {
        let response = login(
            State(state.clone()),
            Json(LoginRequest {
                username: username.to_string(),
                password: "correct-horse-battery-staple".to_string(),
            }),
        )
        .await
        .expect("login succeeds");
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", response.0.token))
                .expect("valid bearer header"),
        );
        headers
    }

    fn sample_asset(tag: &str, lab_id: i64) -> AssetInput {
        AssetInput {
            asset_tag: tag.to_string(),
            laboratory_id: lab_id,
            category_id: None,
            brand: Some("Test brand".to_string()),
            model: Some("Test model".to_string()),
            serial_number: None,
            hostname: Some("test-host".to_string()),
            ip_address: None,
            status: Some("working".to_string()),
            purchase_date: None,
            warranty_end: None,
            notes: None,
        }
    }

    #[tokio::test]
    async fn api_workflow_covers_auth_inventory_maintenance_and_monitoring() {
        let state = test_state().await;
        let admin = login_headers(&state, "admin-test").await;

        let current = me(State(state.clone()), admin.clone())
            .await
            .expect("authenticated user");
        assert_eq!(current.0.role, "administrator");

        let labs = list_laboratories(State(state.clone()), admin.clone())
            .await
            .expect("seeded laboratories");
        assert!(!labs.0.is_empty());
        let lab_id = labs.0[0].0;

        let (asset_status, _) = create_asset(
            State(state.clone()),
            admin.clone(),
            Json(sample_asset("TEST-PC-001", lab_id)),
        )
        .await
        .expect("create asset");
        assert_eq!(asset_status, StatusCode::CREATED);

        let assets = list_assets(State(state.clone()), admin.clone())
            .await
            .expect("list assets");
        assert_eq!(assets.0.len(), 1);
        let asset_id = assets.0[0].0;

        let _ = update_asset(
            State(state.clone()),
            admin.clone(),
            AxumPath(asset_id),
            Json(sample_asset("TEST-PC-001", lab_id)),
        )
        .await
        .expect("update asset");

        let (maintenance_status, _) = create_maintenance(
            State(state.clone()),
            admin.clone(),
            Json(MaintenanceInput {
                asset_id,
                issue: "Test keyboard issue".to_string(),
                action_taken: None,
                status: None,
            }),
        )
        .await
        .expect("create maintenance record");
        assert_eq!(maintenance_status, StatusCode::CREATED);

        let maintenance = list_maintenance(State(state.clone()), admin.clone())
            .await
            .expect("list maintenance");
        assert_eq!(maintenance.0.len(), 1);
        let maintenance_id = maintenance.0[0].0;

        let _ = update_maintenance(
            State(state.clone()),
            admin.clone(),
            AxumPath(maintenance_id),
            Json(MaintenanceInput {
                asset_id,
                issue: "Test keyboard issue".to_string(),
                action_taken: Some("Replaced keyboard".to_string()),
                status: Some("resolved".to_string()),
            }),
        )
        .await
        .expect("resolve maintenance record");

        let (heartbeat_status, _) = heartbeat(
            State(state.clone()),
            {
                let mut headers = HeaderMap::new();
                headers.insert(
                    "x-clamms-secret",
                    HeaderValue::from_static("test-agent-secret-that-is-at-least-32-bytes"),
                );
                headers
            },
            Json(HeartbeatRequest {
                asset_tag: "TEST-PC-001".to_string(),
                hostname: Some("test-host".to_string()),
                os_version: Some("Windows test".to_string()),
                cpu_percent: Some(23.5),
                memory_percent: Some(41.0),
                disk_free_bytes: Some(10_000),
                uptime_seconds: Some(3_600),
            }),
        )
        .await
        .expect("accept heartbeat");
        assert_eq!(heartbeat_status, StatusCode::ACCEPTED);

        let monitoring = list_monitoring(State(state.clone()), admin.clone())
            .await
            .expect("list monitoring");
        assert_eq!(monitoring.0.len(), 1);
        assert_eq!(monitoring.0[0].1, "TEST-PC-001");

        let asset_csv = export_assets_csv(State(state.clone()), admin.clone())
            .await
            .expect("export assets");
        assert_eq!(
            asset_csv.headers()[header::CONTENT_TYPE],
            "text/csv; charset=utf-8"
        );

        let maintenance_csv = export_maintenance_csv(State(state.clone()), admin.clone())
            .await
            .expect("export maintenance");
        assert_eq!(
            maintenance_csv.headers()[header::CONTENT_TYPE],
            "text/csv; charset=utf-8"
        );

        let _ = logout(State(state.clone()), admin.clone())
            .await
            .expect("logout");
        assert_eq!(
            me(State(state.clone()), admin)
                .await
                .expect_err("logged-out session rejected")
                .0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn api_rejects_unauthorized_roles_and_invalid_agent_heartbeats() {
        let state = test_state().await;
        let viewer = login_headers(&state, "viewer-test").await;
        assert_eq!(
            list_users(State(state.clone()), viewer.clone())
                .await
                .expect_err("viewer cannot list users")
                .0,
            StatusCode::FORBIDDEN
        );

        let labs = list_laboratories(State(state.clone()), viewer.clone())
            .await
            .expect("viewer can read laboratories");
        let denied = create_asset(
            State(state.clone()),
            viewer.clone(),
            Json(sample_asset("DENIED-PC", labs.0[0].0)),
        )
        .await
        .expect_err("viewer cannot create assets");
        assert_eq!(denied.0, StatusCode::FORBIDDEN);

        let mut bad_secret = HeaderMap::new();
        bad_secret.insert("x-clamms-secret", HeaderValue::from_static("wrong-secret"));
        assert_eq!(
            heartbeat(
                State(state.clone()),
                bad_secret,
                Json(HeartbeatRequest {
                    asset_tag: "ANY-PC".to_string(),
                    hostname: None,
                    os_version: None,
                    cpu_percent: None,
                    memory_percent: None,
                    disk_free_bytes: None,
                    uptime_seconds: None,
                }),
            )
            .await
            .expect_err("reject wrong agent secret")
            .0,
            StatusCode::UNAUTHORIZED
        );

        let mut valid_secret = HeaderMap::new();
        valid_secret.insert(
            "x-clamms-secret",
            HeaderValue::from_static("test-agent-secret-that-is-at-least-32-bytes"),
        );
        assert_eq!(
            heartbeat(
                State(state),
                valid_secret,
                Json(HeartbeatRequest {
                    asset_tag: "UNREGISTERED-PC".to_string(),
                    hostname: None,
                    os_version: None,
                    cpu_percent: Some(101.0),
                    memory_percent: None,
                    disk_free_bytes: None,
                    uptime_seconds: None,
                }),
            )
            .await
            .expect_err("reject out-of-range telemetry")
            .0,
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn expired_sessions_and_deactivated_accounts_are_rejected() {
        let state = test_state().await;
        let headers = login_headers(&state, "admin-test").await;
        let token = bearer_token(&headers).expect("bearer token");
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));

        sqlx::query("UPDATE sessions SET expires_at = '2000-01-01 00:00:00' WHERE token_hash = ?")
            .bind(&token_hash)
            .execute(&state.db)
            .await
            .expect("expire session");

        assert_eq!(
            authenticated_user(&state.db, &headers)
                .await
                .expect_err("expired session rejected")
                .0,
            StatusCode::UNAUTHORIZED
        );

        let active_headers = login_headers(&state, "admin-test").await;
        sqlx::query("UPDATE users SET is_active = 0 WHERE username = 'admin-test'")
            .execute(&state.db)
            .await
            .expect("deactivate administrator");

        assert_eq!(
            authenticated_user(&state.db, &active_headers)
                .await
                .expect_err("deactivated user session rejected")
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            login(
                State(state),
                Json(LoginRequest {
                    username: "admin-test".to_string(),
                    password: "correct-horse-battery-staple".to_string(),
                }),
            )
            .await
            .expect_err("deactivated account cannot log in")
            .0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn database_backup_is_admin_only_and_contains_sqlite_data() {
        let state = test_state().await;
        let admin = login_headers(&state, "admin-test").await;
        let viewer = login_headers(&state, "viewer-test").await;

        assert_eq!(
            download_database_backup(State(state.clone()), viewer)
                .await
                .expect_err("viewer cannot download database backup")
                .0,
            StatusCode::FORBIDDEN
        );

        let response = download_database_backup(State(state), admin)
            .await
            .expect("administrator backup succeeds");
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/vnd.sqlite3"
        );
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "no-store"
        );
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read backup response body");
        assert!(bytes.starts_with(b"SQLite format 3\0"));
        assert!(bytes.len() > 100);
    }

    #[test]
    fn role_permissions_are_restricted() {
        assert!(can_manage_inventory("administrator"));
        assert!(can_manage_inventory("custodian"));
        assert!(!can_manage_inventory("technician"));
        assert!(!can_manage_inventory("viewer"));
        assert!(can_manage_maintenance("technician"));
        assert!(!can_manage_maintenance("viewer"));
    }

    #[test]
    fn asset_status_values_are_allowlisted() {
        for status in [
            "working",
            "needs_repair",
            "under_maintenance",
            "retired",
            "missing",
        ] {
            assert!(valid_asset_status(status));
        }
        assert!(!valid_asset_status("deleted"));
        assert!(!valid_asset_status("unknown"));
    }

    #[test]
    fn maintenance_status_values_are_allowlisted() {
        for status in ["open", "in_progress", "resolved", "cancelled"] {
            assert!(valid_maintenance_status(status));
        }
        assert!(!valid_maintenance_status("closed"));
    }

    #[test]
    fn csv_cells_escape_quotes_and_formula_prefixes() {
        assert_eq!(csv_cell("plain"), "\"plain\"");
        assert_eq!(csv_cell("a\"b"), "\"a\"\"b\"");
        assert_eq!(csv_cell("=1+1"), "\"'=1+1\"");
        assert_eq!(csv_cell("  =1+1"), "\"'  =1+1\"");
        assert_eq!(csv_cell("\t=1+1"), "\"'\t=1+1\"");
    }

    #[test]
    fn user_roles_are_allowlisted() {
        for role in ["administrator", "custodian", "technician", "viewer"] {
            assert!(valid_role(role));
        }
        assert!(!valid_role("superuser"));
    }
}
