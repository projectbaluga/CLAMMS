<?php
declare(strict_types=1);
require_once __DIR__ . '/../config/database.php';

function e(mixed $value): string {
    return htmlspecialchars((string)$value, ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8');
}
function require_login(): void {
    if (empty($_SESSION['user_id'])) {
        header('Location: ' . APP_URL . '/login.php');
        exit;
    }
}
function csrf_token(): string {
    if (empty($_SESSION['csrf_token'])) $_SESSION['csrf_token'] = bin2hex(random_bytes(32));
    return $_SESSION['csrf_token'];
}
function verify_csrf(): void {
    $token = $_POST['csrf_token'] ?? '';
    if (!is_string($token) || !hash_equals(csrf_token(), $token)) {
        http_response_code(419);
        exit('Invalid or expired form token. Please go back and try again.');
    }
}
function audit(string $action, string $details = ''): void {
    $stmt = db()->prepare('INSERT INTO audit_logs(user_id, action, details) VALUES(?,?,?)');
    $stmt->execute([$_SESSION['user_id'] ?? null, $action, $details]);
}
