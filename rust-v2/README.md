# CLAMMS Rust V2

Rust-based Windows-hosted rebuild of the Computer Laboratory Asset & Maintenance Management System.

## Deployment architecture

- The laboratory custodian's Windows PC hosts `clamms-server.exe` and the local SQLite database.
- Each workstation runs the separate `clamms-agent.exe` process built from `agent/`.
- The server stores data in a local SQLite database at `data/clamms.db`.
- Users access the dashboard through a browser over the school LAN.
- Remote users connect through an approved VPN (for example, Tailscale or WireGuard).


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

This branch is an actively developed Rust V2 implementation and is not yet a production replacement for the PHP system. Current implemented server capabilities include:

- Axum/Tokio HTTP server
- SQLite connection and SQLx migration runner
- Initial schema for six labs, users, assets, categories, maintenance and monitoring
- Health endpoint at `GET /api/health`
- Secret-protected heartbeat endpoint at `POST /api/heartbeat`
- Argon2 password verification, hashed bearer-token sessions, login/logout and current-user endpoint
- Authenticated laboratory and asset inventory listing
- Laboratory create/update APIs (administrator and custodian)
- Asset create/update APIs with status validation (administrator and custodian); use status `retired` rather than hard-deleting inventory
- Maintenance list/create/update APIs (administrator, custodian and technician)
- Administrator-only user listing, creation, role changes, activation/deactivation and password reset; password changes revoke that user's active sessions
- Browser dashboard at `/` with login, inventory, laboratory, maintenance, monitoring-history, audit history and administrator user views
- Separate Rust Windows monitoring agent under `agent/`, configured per workstation with its asset tag and server secret
- Audit trail table with actor attribution for management operations
- Administrator-only audit history API and dashboard view
- Windows CI workflow for formatting, compilation, linting, tests and downloadable build artifacts

Still outstanding: tested backup/restore workflow, stronger audit attribution, rate limiting, session expiry cleanup, automated API/database integration tests, installer/service packaging, and deployment hardening. Dashboard forms and agent are implemented but still need real Windows build and pilot validation. Role checks currently distinguish inventory and maintenance writes; they are not yet a complete school-specific authorization policy. Do not use this build for production or irreplaceable asset data until the outstanding controls and successful Windows build/test validation are complete.

## API routes currently implemented

All routes below except health and heartbeat require a bearer token where noted.

| Method | Route | Access |
|---|---|---|
| GET | `/api/health` | Public |
| POST | `/api/auth/login` | Public |
| GET | `/api/auth/me` | Authenticated |
| POST | `/api/auth/logout` | Authenticated |
| GET / POST | `/api/laboratories` | Authenticated / administrator or custodian for POST |
| PUT | `/api/laboratories/:id` | Administrator or custodian |
| GET / POST | `/api/assets` | Authenticated / administrator or custodian for POST |
| PUT | `/api/assets/:id` | Administrator or custodian |
| GET / POST | `/api/maintenance` | Authenticated / administrator, custodian or technician for POST |
| PUT | `/api/maintenance/:id` | Administrator, custodian or technician |
| GET / POST | `/api/users` | Administrator only |
| PUT | `/api/users/:id` | Administrator only |
| GET | `/api/monitoring` | Authenticated |
| GET | `/api/reports/assets.csv` | Authenticated CSV export |
| GET | `/api/reports/maintenance.csv` | Authenticated CSV export |
| POST | `/api/heartbeat` | Shared agent secret |

Send `Authorization: Bearer <token>` for authenticated routes. The login response returns the token once; keep it private. The heartbeat endpoint uses `X-CLAMMS-SECRET`, not a user session.

Write APIs accept JSON. Asset status values: `working`, `needs_repair`, `under_maintenance`, `retired`, `missing`. Maintenance status values: `open`, `in_progress`, `resolved`, `cancelled`. User roles: `administrator`, `custodian`, `technician`, `viewer`. User creation and password changes require a password of at least 12 characters. User updates must explicitly send `is_active`.

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
- The server and agent binaries are built by CI when the workflow succeeds; they still require a successful run and a real Windows pilot before production use.

## Browser dashboard

Open `http://127.0.0.1:8080/` and sign in using the administrator account configured during bootstrap. The dashboard provides read views and role-gated create/update forms for assets, laboratories, maintenance records, and users. The monitoring tab displays received heartbeat history. Inventory and maintenance views provide CSV exports; CSV values are quoted and formula-like prefixes are neutralized. PDF exports and a full alerting workflow are not included yet.

## Build the workstation agent

From PowerShell:

```powershell
cd agent
Copy-Item .env.example .env
# Edit .env: server URL, matching agent secret, and the workstation's exact asset tag.
cargo build --release
.\target\release\clamms-agent.exe
```

Pilot on one workstation first. Create its asset record in the dashboard before configuring the agent. Then verify that new entries appear in Monitoring. Deploy to the other workstations only after validating network reachability, telemetry and Windows startup behavior.


## Windows CI artifacts

When the `CLAMMS Rust V2` workflow completes successfully, it publishes a `clamms-windows-x64` artifact containing `clamms-server.exe` and `clamms-agent.exe` for 14 days. A green workflow run is required before treating those binaries as usable builds.
