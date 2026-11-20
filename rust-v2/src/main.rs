use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Html,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::{env, net::SocketAddr, path::Path};
use tower_http::trace::TraceLayer;
use tracing::{error, info};

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

#[derive(Serialize)]
struct ApiMessage {
    status: &'static str,
    message: &'static str,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&db).await?;

    let state = AppState { db, agent_secret };
    let app = Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/heartbeat", post(heartbeat))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    info!("CLAMMS Rust V2 listening on http://{bind}");
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
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
<p>This is the initial server foundation. Inventory, accounts, maintenance workflows and the full dashboard are scheduled for the next implementation stage.</p>
</section></main></body></html>"#)
}

async fn health() -> Json<ApiMessage> {
    Json(ApiMessage { status: "ok", message: "CLAMMS Rust V2 is running" })
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
        return Err((StatusCode::UNAUTHORIZED, Json(ApiMessage {
            status: "error",
            message: "Unauthorized agent",
        })));
    }

    let asset_tag = payload.asset_tag.trim();
    if asset_tag.is_empty() || asset_tag.len() > 100 {
        return Err((StatusCode::BAD_REQUEST, Json(ApiMessage {
            status: "error",
            message: "Invalid asset tag",
        })));
    }

    let result = sqlx::query(
        "INSERT INTO monitoring_logs (asset_tag, hostname, os_version, cpu_percent, memory_percent, disk_free_bytes, uptime_seconds, received_at) VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)"
    )
    .bind(asset_tag)
    .bind(payload.hostname)
    .bind(payload.os_version)
    .bind(payload.cpu_percent)
    .bind(payload.memory_percent)
    .bind(payload.disk_free_bytes)
    .bind(payload.uptime_seconds)
    .execute(&state.db)
    .await;

    if let Err(err) = result {
        error!("Heartbeat database insert failed: {err}");
        return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiMessage {
            status: "error",
            message: "Could not save heartbeat",
        })));
    }

    Ok((StatusCode::ACCEPTED, Json(ApiMessage {
        status: "ok",
        message: "Heartbeat received",
    })))
}
