# CLAMMS Rust V2

Rust-based Windows-hosted rebuild of the Computer Laboratory Asset & Maintenance Management System.

## Deployment architecture

- The laboratory custodian's Windows PC hosts `clamms-server.exe`.
- The server stores data in a local SQLite database at `data/clamms.db`.
- Users access the dashboard through a browser over the school LAN.
- Remote users connect through an approved VPN (for example, Tailscale or WireGuard).
- Each monitored workstation will run a separate `clamms-agent.exe`.

Do not expose port 8080 or the SQLite database directly to the public internet. Restrict Windows Firewall access to the school LAN and VPN interface. Use unique credentials and protect the agent secret.

## Requirements for building

- Current stable Rust toolchain (rustup)
- Windows 10/11 or Windows Server build environment

## Configure

Copy `.env.example` to `.env`, then set `CLAMMS_AGENT_SECRET` to a unique random value of at least 32 characters. Do not commit `.env` or share the secret.

The server loads a local `.env` file at startup. To create the first administrator, set `CLAMMS_BOOTSTRAP_ADMIN_USERNAME` and `CLAMMS_BOOTSTRAP_ADMIN_PASSWORD` in `.env`; the password must be at least 12 characters. The bootstrap account is created only when the users table is empty. Remove the bootstrap password from the environment after first startup. For production, run it through a Windows service wrapper or a controlled startup task with the working directory set to this folder.

## Build and run (PowerShell)

```powershell
cargo build --release
$env:CLAMMS_AGENT_SECRET = "replace-with-a-unique-random-secret-at-least-32-chars"
$env:CLAMMS_BIND = "0.0.0.0:8080"
$env:CLAMMS_DATABASE_URL = "sqlite://data/clamms.db"
.\target\release\clamms-server.exe
```

Open `http://127.0.0.1:8080` on the host PC. From a permitted school LAN client, use the custodian PC's LAN IP and port 8080.

## Current implementation status

This branch is the initial Rust V2 foundation, not a completed replacement for the PHP system yet. It currently includes:

- Axum/Tokio HTTP server
- SQLite connection and SQLx migration runner
- Initial schema for six labs, users, assets, categories, maintenance and monitoring
- Health endpoint at `GET /api/health`
- Secret-protected heartbeat endpoint at `POST /api/heartbeat`

Not implemented yet: role-specific authorization policies, user administration, complete CRUD screens/APIs, agent executable, reporting, backup/restore UI, and VPN provisioning. Login sessions are a foundation, not a complete security review. Do not use this starter build for production or real asset data until those controls and tests are completed.

## Heartbeat request

```http
POST /api/heartbeat
Content-Type: application/json
X-CLAMMS-SECRET: <same secret configured on server and agent>
```

```json
{
  "asset_tag": "CLAMMS-LAB01-PC001",
  "hostname": "LAB01-PC001",
  "os_version": "Windows 11",
  "cpu_percent": 12.5,
  "memory_percent": 48.2,
  "disk_free_bytes": 120000000000,
  "uptime_seconds": 86400
}
```

## Important deployment notes

- Keep the custodian PC powered on and prevent automatic sleep while it is serving CLAMMS.
- Use a UPS if possible.
- Configure regular, tested backups of the SQLite database. Back up using SQLite's online backup mechanism or while the server is stopped; do not simply copy a live database file without accounting for WAL files.
- Configure Windows Firewall to allow port 8080 only from trusted LAN/VPN networks.
- Remote access should be through VPN, not direct port forwarding.
- The server and agent binaries have not yet been built or tested on a Windows host.
