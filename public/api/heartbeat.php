<?php
declare(strict_types=1);
require_once __DIR__ . '/../../config/database.php';
header('Content-Type: application/json');
$provided = $_SERVER['HTTP_X_CLAMMS_SECRET'] ?? '';
if (!hash_equals(HEARTBEAT_SECRET, $provided)) {
    http_response_code(401); echo json_encode(['ok'=>false,'error'=>'Unauthorized']); exit;
}
$input = json_decode(file_get_contents('php://input'), true) ?? [];
$tag = trim((string)($input['asset_tag'] ?? ''));
if ($tag === '') { http_response_code(422); echo json_encode(['ok'=>false,'error'=>'asset_tag required']); exit; }
$stmt = db()->prepare('SELECT id FROM assets WHERE asset_tag = ?');
$stmt->execute([$tag]); $asset = $stmt->fetch();
if (!$asset) { http_response_code(404); echo json_encode(['ok'=>false,'error'=>'Unknown asset']); exit; }
$ip = $_SERVER['REMOTE_ADDR'] ?? null;
db()->prepare('UPDATE assets SET last_seen_at = NOW() WHERE id = ?')->execute([$asset['id']]);
db()->prepare('INSERT INTO monitoring_logs(asset_id,status,ip_address,details) VALUES(?,?,?,?)')
   ->execute([$asset['id'], 'online', $ip, json_encode($input)]);
echo json_encode(['ok'=>true]);
