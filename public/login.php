<?php
declare(strict_types=1);
require_once __DIR__ . '/../config/database.php';
$error = '';
if ($_SERVER['REQUEST_METHOD'] === 'POST') {
    $stmt = db()->prepare('SELECT id, name, password_hash, role FROM users WHERE username = ?');
    $stmt->execute([trim($_POST['username'] ?? '')]);
    $user = $stmt->fetch();
    if ($user && password_verify($_POST['password'] ?? '', $user['password_hash'])) {
        session_regenerate_id(true);
        $_SESSION['user_id'] = (int)$user['id'];
        $_SESSION['user_name'] = $user['name'];
        $_SESSION['role'] = $user['role'];
        header('Location: dashboard.php');
        exit;
    }
    $error = 'Invalid username or password.';
}
?><!doctype html><html lang="en"><meta charset="utf-8"><title>CLAMMS Login</title>
<h1>CLAMMS Login</h1><?= htmlspecialchars($error) ?>
<form method="post"><label>Username <input name="username" required></label><br>
<label>Password <input type="password" name="password" required></label><br>
<button type="submit">Sign in</button></form></html>
