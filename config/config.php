<?php
declare(strict_types=1);

define('APP_NAME', 'CLAMMS');
define('APP_URL', 'http://localhost');
define('DB_HOST', '127.0.0.1');
define('DB_NAME', 'clamms_db');
define('DB_USER', 'clamms_user');
define('DB_PASS', 'CHANGE_THIS_DATABASE_PASSWORD');
define('HEARTBEAT_SECRET', 'CHANGE_THIS_TO_A_LONG_RANDOM_SECRET');

if (session_status() !== PHP_SESSION_ACTIVE) {
    session_start();
}
