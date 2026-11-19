<?php
declare(strict_types=1);
require_once __DIR__ . '/../includes/bootstrap.php';
if (!empty($_SESSION['user_id'])) { header('Location: ' . APP_URL . '/dashboard.php'); exit; }
$error = '';
if ($_SERVER['REQUEST_METHOD'] === 'POST') {
    verify_csrf();
    $stmt = db()->prepare('SELECT id, name, password_hash, role FROM users WHERE username = ?');
    $stmt->execute([trim($_POST['username'] ?? '')]);
    $user = $stmt->fetch();
    if ($user && password_verify($_POST['password'] ?? '', $user['password_hash'])) {
        session_regenerate_id(true);
        $_SESSION['user_id'] = (int)$user['id'];
        $_SESSION['user_name'] = $user['name'];
        $_SESSION['role'] = $user['role'];
        header('Location: ' . APP_URL . '/dashboard.php');
        exit;
    }
    $error = 'Invalid username or password.';
}
?><!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>CLAMMS Login</title><link rel="stylesheet" href="<?= e(APP_URL) ?>/assets/css/app.css"></head><body><main class="container">
<div class="panel"><h1>CLAMMS Login</h1><?php if ($error): ?><p class="error"><?= e($error) ?></p><?php endif; ?>
<form method="post"><input type="hidden" name="csrf_token" value="<?= e(csrf_token()) ?>">
<label>Username<input name="username" autocomplete="username" required></label>
<label>Password<input type="password" name="password" autocomplete="current-password" required></label>
<button type="submit">Sign in</button></form></div></main></body></html>
