<?php
declare(strict_types=1);
require_once __DIR__ . '/../../includes/bootstrap.php'; require_login();
$page_title='Maintenance'; $rows=db()->query('SELECT m.*,a.asset_tag,a.name AS asset_name,l.name AS lab_name,u.name AS reporter FROM maintenance_logs m JOIN assets a ON a.id=m.asset_id JOIN laboratories l ON l.id=a.lab_id LEFT JOIN users u ON u.id=m.reported_by ORDER BY m.reported_at DESC')->fetchAll();
require __DIR__ . '/../../includes/header.php';
?><div class="page-head"><h1>Maintenance records</h1><a class="button" href="create.php">Report issue</a></div>
<table><thead><tr><th>Reported</th><th>Asset</th><th>Laboratory</th><th>Issue</th><th>Status</th><th></th></tr></thead><tbody>
<?php foreach($rows as $r): ?><tr><td><?= e($r['reported_at']) ?></td><td><?= e($r['asset_tag'].' — '.$r['asset_name']) ?></td><td><?= e($r['lab_name']) ?></td><td><?= e($r['issue']) ?></td><td><?= e($r['status']) ?></td><td><a href="edit.php?id=<?= (int)$r['id'] ?>">Update</a></td></tr><?php endforeach; ?>
<?php if(!$rows): ?><tr><td colspan="6">No maintenance records yet.</td></tr><?php endif; ?></tbody></table>
<?php require __DIR__ . '/../../includes/footer.php'; ?>
