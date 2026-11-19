<?php
declare(strict_types=1);
require_once __DIR__ . '/../../includes/bootstrap.php'; require_login();
$page_title='Asset inventory';
$sql='SELECT a.*, l.name AS lab_name, c.name AS category_name FROM assets a JOIN laboratories l ON l.id=a.lab_id LEFT JOIN asset_categories c ON c.id=a.category_id WHERE 1=1';
$params=[];
if (!empty($_GET['lab_id'])) { $sql.=' AND a.lab_id=?'; $params[]=(int)$_GET['lab_id']; }
if (!empty($_GET['status'])) { $sql.=' AND a.status=?'; $params[]=$_GET['status']; }
$sql.=' ORDER BY l.id,a.asset_tag';
$stmt=db()->prepare($sql); $stmt->execute($params); $assets=$stmt->fetchAll();
$labs=db()->query('SELECT id,name FROM laboratories ORDER BY id')->fetchAll();
require __DIR__ . '/../../includes/header.php';
?><div class="page-head"><h1>Asset inventory</h1><a class="button" href="create.php">Register asset</a></div>
<form method="get" class="filters"><select name="lab_id"><option value="">All laboratories</option><?php foreach($labs as $l): ?><option value="<?= (int)$l['id'] ?>" <?= (($_GET['lab_id']??'')==$l['id'])?'selected':'' ?>><?= e($l['name']) ?></option><?php endforeach; ?></select>
<select name="status"><option value="">All statuses</option><?php foreach(['working','needs_repair','under_maintenance','retired'] as $s): ?><option <?= (($_GET['status']??'')===$s)?'selected':'' ?>><?= e($s) ?></option><?php endforeach; ?></select><button>Filter</button></form>
<table><thead><tr><th>Asset tag</th><th>Name</th><th>Laboratory</th><th>Category</th><th>Status</th><th>Last seen</th><th></th></tr></thead><tbody>
<?php foreach($assets as $a): ?><tr><td><?= e($a['asset_tag']) ?></td><td><?= e($a['name']) ?></td><td><?= e($a['lab_name']) ?></td><td><?= e($a['category_name']) ?></td><td><?= e($a['status']) ?></td><td><?= e($a['last_seen_at'] ?? 'Never') ?></td><td><a href="edit.php?id=<?= (int)$a['id'] ?>">Edit</a></td></tr><?php endforeach; ?>
<?php if(!$assets): ?><tr><td colspan="7">No assets found.</td></tr><?php endif; ?></tbody></table>
<?php require __DIR__ . '/../../includes/footer.php'; ?>
