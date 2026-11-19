<?php
declare(strict_types=1);
require_once __DIR__ . '/../config/database.php';
if (empty($_SESSION['user_id'])) { header('Location: login.php'); exit; }
$stats = db()->query("SELECT COUNT(*) assets, SUM(status='working') working, SUM(status='needs_repair') repair FROM assets")->fetch();
$labs = db()->query("SELECT l.name, COUNT(a.id) asset_count FROM laboratories l LEFT JOIN assets a ON a.lab_id=l.id GROUP BY l.id ORDER BY l.id")->fetchAll();
?><!doctype html><html lang="en"><meta charset="utf-8"><title>CLAMMS Dashboard</title>
<h1>CLAMMS Dashboard</h1><p>Welcome, <?= htmlspecialchars($_SESSION['user_name']) ?>.</p>
<p>Total assets: <?= (int)$stats['assets'] ?> | Working: <?= (int)$stats['working'] ?> | Needs repair: <?= (int)$stats['repair'] ?></p>
<h2>Laboratories</h2><ul><?php foreach ($labs as $lab): ?><li><?= htmlspecialchars($lab['name']) ?> — <?= (int)$lab['asset_count'] ?> assets</li><?php endforeach; ?></ul>
<a href="logout.php">Sign out</a></html>
