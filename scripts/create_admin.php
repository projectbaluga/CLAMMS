<?php
declare(strict_types=1);
if (PHP_SAPI !== 'cli') { http_response_code(403); exit("CLI only\n"); }
require_once __DIR__ . '/../config/database.php';
if ($argc < 4) { fwrite(STDERR, "Usage: php scripts/create_admin.php <name> <username> <password>\n"); exit(1); }
[$script,$name,$username,$password]=$argv;
if (strlen($password)<12) { fwrite(STDERR,"Password must be at least 12 characters.\n"); exit(1); }
$hash=password_hash($password,PASSWORD_DEFAULT);
$stmt=db()->prepare("INSERT INTO users(name,username,password_hash,role) VALUES(?,?,?,'admin')");
$stmt->execute([$name,$username,$hash]);
echo "Administrator account created.\n";
