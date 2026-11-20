use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Html,
    routing::{get, post},
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
        .route("/api/laboratories", get(list_laboratories))
        .route("/api/assets", get(list_assets))
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
    Html(r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>CLAMMS V2</title><style>
body{font:16px system-ui;margin:0;background:#f3f6fb;color:#172033}main{max-width:850px;margin:8vh auto;padding:28px}
section{background:white;border:1px solid #dce3ed;border-radius:14px;padding:24px}
h1{margin-top:0}code{background:#edf2f8;padding:3px 6px;border-radius:5px}
</style></head><body><main><section><h1>CLAMMS Rust V2</h1>
<p>Local Computer Laboratory Asset &amp; Maintenance Management System server.</p>
<p>Server status: <strong>running</strong></p><p>Health endpoint: <code>/api/health</code></p>
<p>Authentication API: <code>/api/auth/login</code>, <code>/api/auth/me</code>, <code>/api/auth/logout</code>.</p>
<p>Initial server foundation. Management APIs are being implemented in stages.</p>
</section></main></body></html>"#)
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
) -> Result<Json<Vec<(i64, String, String, Option<String>)>>, (StatusCode, Json<ApiMessage>)> {
    authenticated_user(&state.db, &headers).await?;
    let rows = sqlx::query_as::<_, (i64, String, String, Option<String>)>(
        "SELECT id, code, name, location FROM laboratories ORDER BY code"
    ).fetch_all(&state.db).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not load laboratories"))?;
    Ok(Json(rows))
}

async fn list_assets(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<(i64, String, i64, Option<String>, Option<String>, String)>>, (StatusCode, Json<ApiMessage>)> {
    authenticated_user(&state.db, &headers).await?;
    let rows = sqlx::query_as::<_, (i64, String, i64, Option<String>, Option<String>, String)>(
        "SELECT id, asset_tag, laboratory_id, brand, model, status FROM assets ORDER BY asset_tag"
    ).fetch_all(&state.db).await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Could not load assets"))?;
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
