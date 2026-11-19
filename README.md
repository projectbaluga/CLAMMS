# CLAMMS
Computer Laboratory Asset & Maintenance Management System

A PHP 8.1+ / MySQL starter application for managing six computer laboratories, assets, custodianship, maintenance records, and device heartbeats.

## Setup
1. Create a MySQL database and import `database/clamms.sql`.
2. Set database credentials and `APP_URL` in `config/config.php`.
3. Configure your web server document root to the `public/` directory.
4. Set a unique `HEARTBEAT_SECRET` in the config and use it in device heartbeat requests.
5. Create an administrator account using your own secure password hash and insert it into the `users` table.

Use HTTPS, a dedicated least-privilege database account, and change all sample secrets before deployment. This is a starter implementation; test and harden it before production use.
