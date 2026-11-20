use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::Serialize;
use std::{env, time::Duration};
use sysinfo::{Disks, System};
use tokio::{signal, time};
use tracing::{error, info, warn};

#[derive(Serialize)]
struct Heartbeat {
    asset_tag: String,
    hostname: Option<String>,
    os_version: Option<String>,
    cpu_percent: f32,
    memory_percent: f32,
    disk_free_bytes: u64,
    uptime_seconds: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let env_file = dir.join(".env");
            if env_file.exists() {
                dotenvy::from_path(env_file).ok();
            }
        }
    }
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")))
        .init();

    let server = env::var("CLAMMS_SERVER_URL")
        .context("CLAMMS_SERVER_URL is required")?
        .trim_end_matches('/')
        .to_string();
    let secret = env::var("CLAMMS_AGENT_SECRET")
        .context("CLAMMS_AGENT_SECRET is required")?;
    let asset_tag = env::var("CLAMMS_ASSET_TAG")
        .context("CLAMMS_ASSET_TAG is required")?;
    let interval = env::var("CLAMMS_INTERVAL_SECONDS")
        .unwrap_or_else(|_| "60".to_string()).parse::<u64>()?;
    if secret.len() < 32 { bail!("CLAMMS_AGENT_SECRET must be at least 32 characters"); }
    if asset_tag.trim().is_empty() { bail!("CLAMMS_ASSET_TAG cannot be empty"); }
    if !(15..=3600).contains(&interval) { bail!("CLAMMS_INTERVAL_SECONDS must be between 15 and 3600"); }

    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("CLAMMS-Agent/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let endpoint = format!("{server}/api/heartbeat");
    let mut ticker = time::interval(Duration::from_secs(interval));
    let mut system = System::new_all();

    info!("CLAMMS agent started for asset {asset_tag}; sending heartbeat every {interval}s");
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                system.refresh_all();
                let total = system.total_memory();
                let used = total.saturating_sub(system.available_memory());
                let memory_percent = if total == 0 { 0.0 } else { used as f32 * 100.0 / total as f32 };
                let disks = Disks::new_with_refreshed_list();
                let disk_free_bytes = disks.iter().map(|d| d.available_space()).sum();
                let payload = Heartbeat {
                    asset_tag: asset_tag.clone(),
                    hostname: System::host_name(),
                    os_version: System::long_os_version(),
                    cpu_percent: system.global_cpu_usage(),
                    memory_percent,
                    disk_free_bytes,
                    uptime_seconds: System::uptime(),
                };
                match client.post(&endpoint)
                    .header("X-CLAMMS-SECRET", &secret)
                    .json(&payload)
                    .send().await {
                    Ok(response) if response.status().is_success() => info!("Heartbeat accepted ({})", response.status()),
                    Ok(response) => warn!("Heartbeat rejected by server: {}", response.status()),
                    Err(err) => error!("Heartbeat request failed: {err}"),
                }
            }
            _ = signal::ctrl_c() => {
                info!("Shutdown signal received; stopping agent.");
                break;
            }
        }
    }
    Ok(())
}
