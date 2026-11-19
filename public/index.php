<?php
declare(strict_types=1);
require_once __DIR__ . '/../config/database.php';
if (!empty($_SESSION['user_id'])) {
    header('Location: dashboard.php');
    exit;
}
?><!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title><?= htmlspecialchars(APP_NAME) ?></title>
<body><h1>CLAMMS</h1><p>Computer Laboratory Asset &amp; Maintenance Management System</p>
<p><a href="login.php">Sign in</a></p></body></html>
