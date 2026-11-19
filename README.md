# CLAMMS
**Computer Laboratory Asset & Maintenance Management System**

PHP 8.1+ and MySQL starter application for a school with six computer laboratories and one custodian.

## Included modules
- Dashboard with lab inventory counts and maintenance summary
- Six seeded laboratory records; editable room names and expected computer counts
- Asset registration, editing, filtering, categories, and condition status
- Maintenance issue reporting, action notes, and resolution tracking
- Automatic device heartbeat receiver and recent-heartbeat view
- Manual equipment inspection logging
- User login, password hashing, CSRF validation on management forms, and audit log helper

## Installation
1. Use PHP 8.1+ with PDO MySQL enabled, MySQL 8+, and Apache or another PHP web server.
2. Create/import the database: run `database/clamms.sql` in MySQL.
3. Edit `config/config.php`: set `APP_URL` to the public URL of the `public` folder, and set database name, user, password, and a long random `HEARTBEAT_SECRET`.
4. Create a database user with only the required privileges on `clamms_db`.
5. Point the web server document root to this repository's `public/` folder. Do not expose `config/`, `includes/`, `database/`, or `scripts/` directly.
6. Create the first administrator from the project root using CLI:
   `php scripts/create_admin.php "System Administrator" admin "Use-A-Unique-Password-At-Least-12-Chars"`
   Use a unique password and do not reuse the example.
7. Open `APP_URL` in a browser and sign in.

## Device heartbeat
Send an HTTPS POST request to `/api/heartbeat.php` with JSON such as:
```json
{"asset_tag":"CL1-PC001","hostname":"CL1-PC001"}
```
Include the header `X-CLAMMS-SECRET: <the configured HEARTBEAT_SECRET>`. Register the asset tag in the inventory first. The endpoint records the time and source IP. A Windows PowerShell heartbeat client is provided at `scripts/windows/clamms-heartbeat.ps1`. Copy it to each lab PC and run it continuously using a dedicated restricted account. Example: `powershell.exe -ExecutionPolicy Bypass -File .\\clamms-heartbeat.ps1 -ServerUrl https://school.example.edu -AssetTag CL1-PC001 -SharedSecret YOUR_LONG_SECRET`. Configure Task Scheduler to start it at system startup and restart it on failure. Protect the shared secret; for larger deployments, consider per-device credentials and regular secret rotation.

## Monitoring interpretation
The monitoring screen labels a device "recently seen" if its last heartbeat was within five minutes. A missing heartbeat can mean the PC is off, disconnected, blocked by a firewall, or its agent is not running; it is not definitive proof of failure. Manual inspection records are separate from automatic heartbeat logs.

## Deployment and security notes
- Deploy behind HTTPS only; change all sample credentials and secrets.
- Restrict the database account and server file permissions.
- Back up the database regularly.
- Configure PHP error logging and disable public error display.
- Review roles/permissions before multi-user deployment. The starter currently uses a simple authenticated-user access model.
- This is a starter release, not yet a fully tested production deployment. Test on a staging server and arrange a security review before using it for official school records.
