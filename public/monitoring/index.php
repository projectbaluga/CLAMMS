<?php
declare(strict_types=1);
require_once __DIR__ . '/../../includes/bootstrap.php'; require_login();
$page_title='Monitoring';
$rows=db()->query("SELECT a.id,a.asset_tag,a.name,a.status,a.last_seen_at,l.name lab_name FROM assets a JOIN laboratories l ON l.id=a.lab_id WHERE a.status <> 'retired' ORDER BY l.id,a.asset_tag")->fetchAll();
require __DIR__ . '/../../includes/header.php';
?>
<div class="page-head"><h1>Computer monitoring</h1><span>Automatic heartbeat + manual inspection</span></div>
<p>Automatic status is based on the last device heartbeat. Devices with no heartbeat in the last five minutes are marked as not recently seen; this does not prove the device is powered off.</p>
<table><thead><tr><th>Asset</th><th>Laboratory</th><th>Inventory condition</th><th>Last heartbeat</th><th>Connection estimate</th><th>Manual check</th></tr></thead><tbody>
<?php foreach($rows as $a): $recent=$a['last_seen_at'] && strtotime($a['last_seen_at']) >= time()-300; ?>
<tr><td><?= e($a['asset_tag'].' — '.$a['name']) ?></td><td><?= e($a['lab_name']) ?></td><td><?= e($a['status']) ?></td><td><?= e($a['last_seen_at'] ?? 'Never') ?></td><td><?= $recent?'<span class="ok">Recently seen</span>':'<span class="error">Not recently seen</span>' ?></td>
<td><form method="post" action="manual_check.php" class="inline-form"><input type="hidden" name="csrf_token" value="<?= e(csrf_token()) ?>"><input type="hidden" name="asset_id" value="<?= (int)$a['id'] ?>"><select name="result"><option value="manual_check_ok">Working</option><option value="manual_check_issue">Issue found</option></select><input name="note" placeholder="Optional note"><button>Record</button></form></td></tr>
<?php endforeach; ?>
<?php if(!$rows): ?><tr><td colspan="6">No assets registered.</td></tr><?php endif; ?></tbody></table>
<?php require __DIR__ . '/../../includes/footer.php'; ?>
