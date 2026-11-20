# CLAMMS Windows Agent

The agent sends workstation telemetry to the CLAMMS server. It does not create inventory records: create the asset in CLAMMS first and set the exact matching asset tag in the agent configuration.

## Configure

Copy `.env.example` to `.env` and set:

- `CLAMMS_SERVER_URL`: server base URL reachable over the trusted school LAN or VPN.
- `CLAMMS_AGENT_SECRET`: exactly the same secret as the server's `CLAMMS_AGENT_SECRET`.
- `CLAMMS_ASSET_TAG`: exact asset tag registered in CLAMMS.
- `CLAMMS_INTERVAL_SECONDS`: 15–3600 seconds; default 60.

Keep the secret private. Do not expose the server port publicly. For production, use HTTPS or a trusted VPN tunnel; do not send the shared secret across an untrusted network.

## Build on Windows

Install stable Rust, open PowerShell in this directory, then run:

```powershell
Copy-Item .env.example .env
# Edit .env with the correct server URL, secret and asset tag.
cargo build --release
.\target\release\clamms-agent.exe
```

Install and test one workstation first before deploying to the remaining computers. Configure Windows Task Scheduler to start the agent at system startup, using a dedicated low-privilege account. A Windows service wrapper can be introduced after the pilot is verified.

The agent reports CPU usage, memory usage, total free disk bytes, uptime, hostname and Windows OS version. Heartbeats are stored by the server in monitoring logs. The agent has not yet been validated on physical school workstations.
