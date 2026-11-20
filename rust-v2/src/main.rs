use axum::{
    extract::{Path as AxumPath, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, SqlitePool};
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

#[derive(Serialize)]
struct ApiMessage {
    status: &'static str,
    message: &'static str,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
    username: String,
    full_name: String,
    role: String,
}

#[derive(Serialize)]
struct CurrentUser {
    username: String,
    full_name: String,
    role: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
    let database_url = env::var("CLAMMS_DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://data/clamms.db".to_string());
    let agent_secret = env::var("CLAMMS_AGENT_SECRET")
        .map_err(|_| anyhow::anyhow!("CLAMMS_AGENT_SECRET is required; configure a unique secret"))?;
    anyhow::ensure!(agent_secret.len() >= 32, "CLAMMS_AGENT_SECRET must be at least 32 characters");

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
        .route("/api/laboratories", get(list_laboratories).post(create_laboratory))
        .route("/api/laboratories/:id", put(update_laboratory))
        .route("/api/maintenance", get(list_maintenance).post(create_maintenance))
        .route("/api/maintenance/:id", put(update_maintenance))
        .route("/api/monitoring", get(list_monitoring))
        .route("/api/reports/assets.csv", get(export_assets_csv))
        .route("/api/reports/maintenance.csv", get(export_maintenance_csv))
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/:id", put(update_user))
        .route("/api/heartbeat", post(heartbeat))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    info!("CLAMMS Rust V2 listening on http://{bind}");
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn bootstrap_admin(db: &SqlitePool) -> anyhow::Result<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(db).await?;
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
    let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes())?;
    let hash = Argon2::default().hash_password(password.as_bytes(), &salt)?.to_string();
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
    Json(ApiMessage { status: "ok", message: "CLAMMS Rust V2 is running" })
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<ApiMessage>)> {
    let row = sqlx::query_as::<_, (i64, String, String, String, i64)>(
        "SELECT id, username, full_name, password_hash, is_active FROM users WHERE username = ?"
    )
    .bind(payload.username.trim())
    .fetch_optional(&state.db).await
    .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Login could not be processed"))?;

    let Some((user_id, username, full_name, password_hash, active)) = row else {
        return Err(api_error(StatusCode::UNAUTHORIZED, "Invalid username or password"));
    };
    let parsed = PasswordHash::new(&password_hash)
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Stored password hash is invalid"))?;
    if active != 1 || Argon2::default().verify_password(payload.password.as_bytes(), &parsed).is_err() {
        return Err(api_error(StatusCode::UNAUTHORIZED, "Invalid username or password"));
    }

    let token = Uuid::new_v4().to_string() + &Uuid::new_v4().to_string();
    let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
    sqlx::query("INSERT INTO sessions (user_id, token_hash, expires_at) VALUES (?, ?, datetime('now', '+12 hours'))")
        .bind(user_id).bind(token_hash).execute(&state.db).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not create session"))?;
    let role: String = sqlx::query_scalar("SELECT role FROM users WHERE id = ?")
        .bind(user_id).fetch_one(&state.db).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not load user role"))?;
    Ok(Json(LoginResponse { token, username, full_name, role }))
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
        .bind(hash).execute(&state.db).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not end session"))?;
    Ok(Json(ApiMessage { status: "ok", message: "Logged out" }))
}

async fn authenticated_user(
    db: &SqlitePool,
    headers: &HeaderMap,
) -> Result<CurrentUser, (StatusCode, Json<ApiMessage>)> {
    let token = bearer_token(headers)?;
    let hash = hex::encode(Sha256::digest(token.as_bytes()));
    sqlx::query_as::<_, (String, String, String)>(
        "SELECT u.username, u.full_name, u.role FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = ? AND s.expires_at > CURRENT_TIMESTAMP AND u.is_active = 1"
    )
    .bind(hash).fetch_optional(db).await
    .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not verify session"))?
    .map(|(username, full_name, role)| CurrentUser { username, full_name, role })
    .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "Login required or session expired"))
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, (StatusCode, Json<ApiMessage>)> {
    headers.get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| !v.is_empty())
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "Bearer token required"))
}

fn api_error(status: StatusCode, message: &'static str) -> (StatusCode, Json<ApiMessage>) {
    (status, Json(ApiMessage { status: "error", message }))
}

async fn list_laboratories(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<(i64, String, String, Option<String>, Option<String>)>>, (StatusCode, Json<ApiMessage>)> {
    authenticated_user(&state.db, &headers).await?;
    let rows = sqlx::query_as::<_, (i64, String, String, Option<String>, Option<String>)>(
        "SELECT id, code, name, location, description FROM laboratories ORDER BY code"
    ).fetch_all(&state.db).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not load laboratories"))?;
    Ok(Json(rows))
}

async fn list_assets(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<(i64, String, i64, Option<String>, Option<String>, String, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>>, (StatusCode, Json<ApiMessage>)> {
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
    matches!(role, "administrator" | "custodian" | "technician" | "viewer")
}

fn password_hash(password: &str) -> Result<String, (StatusCode, Json<ApiMessage>)> {
    if password.len() < 12 || password.len() > 1024 {
        return Err(api_error(StatusCode::BAD_REQUEST, "Password must be 12 to 1024 bytes long"));
    }
    let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes())
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not prepare password hash"))?;
    Argon2::default().hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not hash password"))
}

async fn list_users(
    State(state): State<AppState>, headers: HeaderMap,
) -> Result<Json<Vec<(i64,String,String,String,i64)>>,(StatusCode,Json<ApiMessage>)> {
    let user=authenticated_user(&state.db,&headers).await?;
    if user.role!="administrator" { return Err(api_error(StatusCode::FORBIDDEN,"Administrator role required")); }
    let rows=sqlx::query_as::<_,(i64,String,String,String,i64)>(
        "SELECT id,full_name,username,role,is_active FROM users ORDER BY full_name"
    ).fetch_all(&state.db).await
        .map_err(|_|api_error(StatusCode::INTERNAL_SERVER_ERROR,"Could not load users"))?;
    Ok(Json(rows))
}

async fn create_user(
    State(state): State<AppState>, headers: HeaderMap, Json(input): Json<UserInput>,
) -> Result<(StatusCode,Json<ApiMessage>),(StatusCode,Json<ApiMessage>)> {
    let user=authenticated_user(&state.db,&headers).await?;
    if user.role!="administrator" { return Err(api_error(StatusCode::FORBIDDEN,"Administrator role required")); }
    let full=input.full_name.trim(); let username=input.username.trim();
    if full.is_empty() || full.len()>150 || username.is_empty() || username.len()>80 || !valid_role(&input.role) {
        return Err(api_error(StatusCode::BAD_REQUEST,"User details or role are invalid"));
    }
    let password=input.password.as_deref().ok_or_else(||api_error(StatusCode::BAD_REQUEST,"Initial password is required"))?;
    let hash=password_hash(password)?;
    let r=sqlx::query("INSERT INTO users (full_name,username,password_hash,role,is_active) VALUES (?,?,?,?,?)")
        .bind(full).bind(username).bind(hash).bind(&input.role).bind(input.is_active.unwrap_or(true) as i64)
        .execute(&state.db).await;
    match r {
        Ok(r) => { write_audit(&state.db,"create","user",Some(r.last_insert_rowid()),username).await;
            Ok((StatusCode::CREATED,Json(ApiMessage{status:"ok",message:"User created"}))) }
        Err(_) => Err(api_error(StatusCode::CONFLICT,"Username is already in use or user data is invalid")),
    }
}

async fn update_user(
    State(state): State<AppState>, headers: HeaderMap, AxumPath(id): AxumPath<i64>, Json(input): Json<UserInput>,
) -> Result<Json<ApiMessage>,(StatusCode,Json<ApiMessage>)> {
    let actor=authenticated_user(&state.db,&headers).await?;
    if actor.role!="administrator" { return Err(api_error(StatusCode::FORBIDDEN,"Administrator role required")); }
    let full=input.full_name.trim(); let username=input.username.trim();
    if id<=0 || full.is_empty() || full.len()>150 || username.is_empty() || username.len()>80 || !valid_role(&input.role) {
        return Err(api_error(StatusCode::BAD_REQUEST,"User details or role are invalid"));
    }
    let active=input.is_active.ok_or_else(||api_error(StatusCode::BAD_REQUEST,"is_active is required when updating a user"))? as i64;
    if let Some(password)=input.password.as_deref() {
        let hash=password_hash(password)?;
        sqlx::query("UPDATE users SET full_name=?,username=?,role=?,is_active=?,password_hash=? WHERE id=?")
            .bind(full).bind(username).bind(&input.role).bind(active).bind(hash).bind(id)
            .execute(&state.db).await.map_err(|_|api_error(StatusCode::CONFLICT,"Could not update user; username may be in use"))?;
        sqlx::query("DELETE FROM sessions WHERE user_id=?").bind(id).execute(&state.db).await
            .map_err(|_|api_error(StatusCode::INTERNAL_SERVER_ERROR,"User updated but sessions could not be revoked"))?;
    } else {
        sqlx::query("UPDATE users SET full_name=?,username=?,role=?,is_active=? WHERE id=?")
            .bind(full).bind(username).bind(&input.role).bind(active).bind(id)
            .execute(&state.db).await.map_err(|_|api_error(StatusCode::CONFLICT,"Could not update user; username may be in use"))?;
    }
    write_audit(&state.db,"update","user",Some(id),username).await;
    Ok(Json(ApiMessage{status:"ok",message:"User updated; active sessions revoked when password changed"}))
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
    matches!(value, "working" | "needs_repair" | "under_maintenance" | "retired" | "missing")
}

fn valid_maintenance_status(value: &str) -> bool {
    matches!(value, "open" | "in_progress" | "resolved" | "cancelled")
}

async fn write_audit(db: &SqlitePool, action: &str, entity: &str, id: Option<i64>, details: &str) {
    let _ = sqlx::query("INSERT INTO audit_logs (action, entity_type, entity_id, details) VALUES (?, ?, ?, ?)")
        .bind(action).bind(entity).bind(id.map(|v| v.to_string())).bind(details)
        .execute(db).await;
}

async fn create_laboratory(
    State(state): State<AppState>, headers: HeaderMap, Json(input): Json<LaboratoryInput>,
) -> Result<(StatusCode, Json<ApiMessage>), (StatusCode, Json<ApiMessage>)> {
    let user = authenticated_user(&state.db, &headers).await?;
    if !can_manage_inventory(&user.role) { return Err(api_error(StatusCode::FORBIDDEN, "Insufficient permissions")); }
    let code=input.code.trim(); let name=input.name.trim();
    if code.is_empty() || code.len()>30 || name.is_empty() || name.len()>150 {
        return Err(api_error(StatusCode::BAD_REQUEST, "Laboratory code or name is invalid"));
    }
    let result=sqlx::query("INSERT INTO laboratories (code,name,location,description) VALUES (?,?,?,?)")
        .bind(code).bind(name).bind(input.location).bind(input.description).execute(&state.db).await;
    match result {
        Ok(r) => { write_audit(&state.db,"create","laboratory",Some(r.last_insert_rowid()),code).await;
            Ok((StatusCode::CREATED,Json(ApiMessage{status:"ok",message:"Laboratory created"}))) }
        Err(_) => Err(api_error(StatusCode::CONFLICT,"Laboratory code already exists or data is invalid")),
    }
}

async fn update_laboratory(
    State(state): State<AppState>, headers: HeaderMap, AxumPath(id): AxumPath<i64>, Json(input): Json<LaboratoryInput>,
) -> Result<Json<ApiMessage>, (StatusCode, Json<ApiMessage>)> {
    let user=authenticated_user(&state.db,&headers).await?;
    if !can_manage_inventory(&user.role) { return Err(api_error(StatusCode::FORBIDDEN,"Insufficient permissions")); }
    let code=input.code.trim(); let name=input.name.trim();
    if id<=0 || code.is_empty() || code.len()>30 || name.is_empty() || name.len()>150 {
        return Err(api_error(StatusCode::BAD_REQUEST,"Laboratory fields are invalid"));
    }
    let r=sqlx::query("UPDATE laboratories SET code=?,name=?,location=?,description=? WHERE id=?")
        .bind(code).bind(name).bind(input.location).bind(input.description).bind(id).execute(&state.db).await
        .map_err(|_|api_error(StatusCode::CONFLICT,"Could not update laboratory; code may already exist"))?;
    if r.rows_affected()==0 { return Err(api_error(StatusCode::NOT_FOUND,"Laboratory not found")); }
    write_audit(&state.db,"update","laboratory",Some(id),code).await;
    Ok(Json(ApiMessage{status:"ok",message:"Laboratory updated"}))
}

async fn create_asset(
    State(state): State<AppState>, headers: HeaderMap, Json(input): Json<AssetInput>,
) -> Result<(StatusCode, Json<ApiMessage>), (StatusCode, Json<ApiMessage>)> {
    let user=authenticated_user(&state.db,&headers).await?;
    if !can_manage_inventory(&user.role) { return Err(api_error(StatusCode::FORBIDDEN,"Insufficient permissions")); }
    let tag=input.asset_tag.trim();
    let status=input.status.as_deref().unwrap_or("working");
    if tag.is_empty() || tag.len()>100 || input.laboratory_id<=0 || !valid_asset_status(status) {
        return Err(api_error(StatusCode::BAD_REQUEST,"Asset tag, laboratory or status is invalid"));
    }
    let r=sqlx::query("INSERT INTO assets (asset_tag,laboratory_id,category_id,brand,model,serial_number,hostname,ip_address,status,purchase_date,warranty_end,notes) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)")
        .bind(tag).bind(input.laboratory_id).bind(input.category_id).bind(input.brand).bind(input.model)
        .bind(input.serial_number).bind(input.hostname).bind(input.ip_address).bind(status)
        .bind(input.purchase_date).bind(input.warranty_end).bind(input.notes).execute(&state.db).await;
    match r {
        Ok(r) => { write_audit(&state.db,"create","asset",Some(r.last_insert_rowid()),tag).await;
            Ok((StatusCode::CREATED,Json(ApiMessage{status:"ok",message:"Asset created"}))) }
        Err(_) => Err(api_error(StatusCode::CONFLICT,"Asset tag is already used or referenced data is invalid")),
    }
}

async fn update_asset(
    State(state): State<AppState>, headers: HeaderMap, AxumPath(id): AxumPath<i64>, Json(input): Json<AssetInput>,
) -> Result<Json<ApiMessage>, (StatusCode, Json<ApiMessage>)> {
    let user=authenticated_user(&state.db,&headers).await?;
    if !can_manage_inventory(&user.role) { return Err(api_error(StatusCode::FORBIDDEN,"Insufficient permissions")); }
    let tag=input.asset_tag.trim(); let status=input.status.as_deref().unwrap_or("working");
    if id<=0 || tag.is_empty() || tag.len()>100 || input.laboratory_id<=0 || !valid_asset_status(status) {
        return Err(api_error(StatusCode::BAD_REQUEST,"Asset fields are invalid"));
    }
    let r=sqlx::query("UPDATE assets SET asset_tag=?,laboratory_id=?,category_id=?,brand=?,model=?,serial_number=?,hostname=?,ip_address=?,status=?,purchase_date=?,warranty_end=?,notes=?,updated_at=CURRENT_TIMESTAMP WHERE id=?")
        .bind(tag).bind(input.laboratory_id).bind(input.category_id).bind(input.brand).bind(input.model)
        .bind(input.serial_number).bind(input.hostname).bind(input.ip_address).bind(status)
        .bind(input.purchase_date).bind(input.warranty_end).bind(input.notes).bind(id).execute(&state.db).await
        .map_err(|_|api_error(StatusCode::CONFLICT,"Could not update asset; tag may already exist"))?;
    if r.rows_affected()==0 { return Err(api_error(StatusCode::NOT_FOUND,"Asset not found")); }
    write_audit(&state.db,"update","asset",Some(id),tag).await;
    Ok(Json(ApiMessage{status:"ok",message:"Asset updated"}))
}

async fn list_maintenance(
    State(state): State<AppState>, headers: HeaderMap,
) -> Result<Json<Vec<(i64,i64,String,Option<String>,String,String)>>, (StatusCode,Json<ApiMessage>)> {
    authenticated_user(&state.db,&headers).await?;
    let rows=sqlx::query_as::<_,(i64,i64,String,Option<String>,String,String)>(
        "SELECT id,asset_id,issue,action_taken,status,reported_at FROM maintenance_logs ORDER BY reported_at DESC LIMIT 500"
    ).fetch_all(&state.db).await.map_err(|_|api_error(StatusCode::INTERNAL_SERVER_ERROR,"Could not load maintenance records"))?;
    Ok(Json(rows))
}

async fn create_maintenance(
    State(state): State<AppState>, headers: HeaderMap, Json(input): Json<MaintenanceInput>,
) -> Result<(StatusCode,Json<ApiMessage>),(StatusCode,Json<ApiMessage>)> {
    let user=authenticated_user(&state.db,&headers).await?;
    if !can_manage_maintenance(&user.role) { return Err(api_error(StatusCode::FORBIDDEN,"Insufficient permissions")); }
    let issue=input.issue.trim(); let status=input.status.as_deref().unwrap_or("open");
    if input.asset_id<=0 || issue.is_empty() || issue.len()>4000 || !valid_maintenance_status(status) {
        return Err(api_error(StatusCode::BAD_REQUEST,"Maintenance details are invalid"));
    }
    let r=sqlx::query("INSERT INTO maintenance_logs (asset_id,issue,action_taken,status) VALUES (?,?,?,?)")
        .bind(input.asset_id).bind(issue).bind(input.action_taken).bind(status).execute(&state.db).await;
    match r {
        Ok(r) => { write_audit(&state.db,"create","maintenance",Some(r.last_insert_rowid()),issue).await;
            Ok((StatusCode::CREATED,Json(ApiMessage{status:"ok",message:"Maintenance record created"}))) }
        Err(_) => Err(api_error(StatusCode::BAD_REQUEST,"Asset does not exist or maintenance data is invalid")),
    }
}

async fn update_maintenance(
    State(state): State<AppState>, headers: HeaderMap, AxumPath(id): AxumPath<i64>, Json(input): Json<MaintenanceInput>,
) -> Result<Json<ApiMessage>,(StatusCode,Json<ApiMessage>)> {
    let user=authenticated_user(&state.db,&headers).await?;
    if !can_manage_maintenance(&user.role) { return Err(api_error(StatusCode::FORBIDDEN,"Insufficient permissions")); }
    let issue=input.issue.trim(); let status=input.status.as_deref().unwrap_or("open");
    if id<=0 || input.asset_id<=0 || issue.is_empty() || issue.len()>4000 || !valid_maintenance_status(status) {
        return Err(api_error(StatusCode::BAD_REQUEST,"Maintenance details are invalid"));
    }
    let r=if status=="resolved" {
        sqlx::query("UPDATE maintenance_logs SET asset_id=?,issue=?,action_taken=?,status=?,resolved_at=CURRENT_TIMESTAMP WHERE id=?")
            .bind(input.asset_id).bind(issue).bind(input.action_taken).bind(status).bind(id).execute(&state.db).await
    } else {
        sqlx::query("UPDATE maintenance_logs SET asset_id=?,issue=?,action_taken=?,status=?,resolved_at=NULL WHERE id=?")
            .bind(input.asset_id).bind(issue).bind(input.action_taken).bind(status).bind(id).execute(&state.db).await
    }.map_err(|_|api_error(StatusCode::BAD_REQUEST,"Could not update maintenance record"))?;
    if r.rows_affected()==0 { return Err(api_error(StatusCode::NOT_FOUND,"Maintenance record not found")); }
    write_audit(&state.db,"update","maintenance",Some(id),status).await;
    Ok(Json(ApiMessage{status:"ok",message:"Maintenance record updated"}))
}

fn csv_cell(value: &str) -> String {
    let safe = if value.chars().next().is_some_and(|c| matches!(c, '=' | '+' | '-' | '@' | '\t' | '\r')) {
        format!("'{}", value)
    } else {
        value.to_string()
    };
    format!("\"{}\"", safe.replace('"', "\"\""))
}

async fn export_assets_csv(
    State(state): State<AppState>, headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiMessage>)> {
    authenticated_user(&state.db, &headers).await?;
    let rows=sqlx::query_as::<_,(i64,String,String,i64,Option<String>,Option<String>,String)>(
        "SELECT a.id,a.asset_tag,l.code,a.laboratory_id,a.brand,a.model,a.status FROM assets a JOIN laboratories l ON l.id=a.laboratory_id ORDER BY l.code,a.asset_tag"
    ).fetch_all(&state.db).await
        .map_err(|_|api_error(StatusCode::INTERNAL_SERVER_ERROR,"Could not generate asset report"))?;
    let mut csv=String::from("id,asset_tag,laboratory_code,laboratory_id,brand,model,status\r\n");
    for row in rows {
        let cells=[row.0.to_string(),row.1,row.2,row.3.to_string(),row.4.unwrap_or_default(),row.5.unwrap_or_default(),row.6];
        csv.push_str(&cells.iter().map(|v|csv_cell(v)).collect::<Vec<_>>().join(","));
        csv.push_str("\r\n");
    }
    Ok(([(header::CONTENT_TYPE,"text/csv; charset=utf-8"),(header::CONTENT_DISPOSITION,"attachment; filename=clamms-assets.csv")],csv).into_response())
}

async fn export_maintenance_csv(
    State(state): State<AppState>, headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ApiMessage>)> {
    authenticated_user(&state.db, &headers).await?;
    let rows=sqlx::query_as::<_,(i64,String,String,Option<String>,String,String,Option<String>)>(
        "SELECT m.id,a.asset_tag,m.issue,m.action_taken,m.status,m.reported_at,m.resolved_at FROM maintenance_logs m JOIN assets a ON a.id=m.asset_id ORDER BY m.reported_at DESC"
    ).fetch_all(&state.db).await
        .map_err(|_|api_error(StatusCode::INTERNAL_SERVER_ERROR,"Could not generate maintenance report"))?;
    let mut csv=String::from("id,asset_tag,issue,action_taken,status,reported_at,resolved_at\r\n");
    for row in rows {
        let cells=[row.0.to_string(),row.1,row.2,row.3.unwrap_or_default(),row.4,row.5,row.6.unwrap_or_default()];
        csv.push_str(&cells.iter().map(|v|csv_cell(v)).collect::<Vec<_>>().join(","));
        csv.push_str("\r\n");
    }
    Ok(([(header::CONTENT_TYPE,"text/csv; charset=utf-8"),(header::CONTENT_DISPOSITION,"attachment; filename=clamms-maintenance.csv")],csv).into_response())
}

async fn list_monitoring(
    State(state): State<AppState>, headers: HeaderMap,
) -> Result<Json<Vec<(i64,String,Option<String>,Option<String>,Option<f32>,Option<f32>,Option<i64>,Option<i64>,String)>>,(StatusCode,Json<ApiMessage>)> {
    authenticated_user(&state.db,&headers).await?;
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
    let supplied = headers.get("x-clamms-secret")
        .and_then(|value| value.to_str().ok()).unwrap_or_default();
    if supplied != state.agent_secret {
        return Err(api_error(StatusCode::UNAUTHORIZED, "Unauthorized agent"));
    }
    let asset_tag = payload.asset_tag.trim();
    if asset_tag.is_empty() || asset_tag.len() > 100 {
        return Err(api_error(StatusCode::BAD_REQUEST, "Invalid asset tag"));
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
        return Err(api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not save heartbeat"));
    }
    Ok((StatusCode::ACCEPTED, Json(ApiMessage { status: "ok", message: "Heartbeat received" })))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for status in ["working", "needs_repair", "under_maintenance", "retired", "missing"] {
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
    }

    #[test]
    fn user_roles_are_allowlisted() {
        for role in ["administrator", "custodian", "technician", "viewer"] {
            assert!(valid_role(role));
        }
        assert!(!valid_role("superuser"));
    }
}
