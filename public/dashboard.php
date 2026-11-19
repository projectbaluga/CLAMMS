<?php
declare(strict_types=1);
require_once __DIR__ . '/../includes/bootstrap.php';
require_login();
$stats=db()->query("SELECT COUNT(*) assets, COALESCE(SUM(status='working'),0) working, COALESCE(SUM(status='needs_repair'),0) repair, COALESCE(SUM(status='under_maintenance'),0) in_service FROM assets")->fetch();
$labs=db()->query("SELECT l.id,l.name,l.computer_count,COUNT(a.id) asset_count FROM laboratories l LEFT JOIN assets a ON a.lab_id=l.id GROUP BY l.id ORDER BY l.id")->fetchAll();
$open=(int)db()->query("SELECT COUNT(*) FROM maintenance_logs WHERE status <> 'resolved'")->fetchColumn();
$page_title='Dashboard';
require __DIR__ . '/../includes/header.php';
?>
<h1>Dashboard</h1><p>Welcome, <?= e($_SESSION['user_name'] ?? 'User') ?>.</p>
<div class="page-head">
<div class="panel"><strong>Total assets</strong><h2><?= (int)$stats['assets'] ?></h2></div>
<div class="panel"><strong>Working</strong><h2><?= (int)$stats['working'] ?></h2></div>
<div class="panel"><strong>Needs repair</strong><h2><?= (int)$stats['repair'] ?></h2></div>
<div class="panel"><strong>Under maintenance</strong><h2><?= (int)$stats['in_service'] ?></h2></div>
<div class="panel"><strong>Open maintenance cases</strong><h2><?= $open ?></h2></div>
</div>
<h2>Laboratory overview</h2>
<table><thead><tr><th>Laboratory</th><th>Expected computers</th><th>Registered assets</th><th>Inventory gap</th></tr></thead><tbody>
<?php foreach($labs as $lab): ?><tr><td><?= e($lab['name']) ?></td><td><?= (int)$lab['computer_count'] ?></td><td><?= (int)$lab['asset_count'] ?></td><td><?= max(0,(int)$lab['computer_count']-(int)$lab['asset_count']) ?></td></tr><?php endforeach; ?>
</tbody></table>
<?php require __DIR__ . '/../includes/footer.php'; ?>
