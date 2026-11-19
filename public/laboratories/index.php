<?php
declare(strict_types=1);
require_once __DIR__ . '/../../includes/bootstrap.php'; require_login();
$page_title = 'Laboratories';
$labs = db()->query('SELECT l.*, COUNT(a.id) AS actual_assets FROM laboratories l LEFT JOIN assets a ON a.lab_id=l.id GROUP BY l.id ORDER BY l.id')->fetchAll();
require __DIR__ . '/../../includes/header.php';
?><div class="page-head"><h1>Laboratories</h1><a class="button" href="create.php">Add laboratory</a></div>
<table><thead><tr><th>Laboratory</th><th>Room</th><th>Registered assets</th><th>Expected computers</th><th></th></tr></thead><tbody>
<?php foreach ($labs as $lab): ?><tr><td><?= e($lab['name']) ?></td><td><?= e($lab['room_code']) ?></td><td><?= (int)$lab['actual_assets'] ?></td><td><?= (int)$lab['computer_count'] ?></td><td><a href="edit.php?id=<?= (int)$lab['id'] ?>">Edit</a></td></tr><?php endforeach; ?>
</tbody></table><?php require __DIR__ . '/../../includes/footer.php'; ?>
