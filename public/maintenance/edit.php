<?php
declare(strict_types=1);
require_once __DIR__ . '/../../includes/bootstrap.php'; require_login();
$id=(int)($_GET['id']??0);$q=db()->prepare('SELECT m.*,a.id AS aid FROM maintenance_logs m JOIN assets a ON a.id=m.asset_id WHERE m.id=?');$q->execute([$id]);$row=$q->fetch();
if(!$row){http_response_code(404);exit('Record not found');}
if($_SERVER['REQUEST_METHOD']==='POST'){
 verify_csrf();$status=$_POST['status']??'open';if(!in_array($status,['open','in_progress','resolved'],true))$status='open';
 db()->prepare('UPDATE maintenance_logs SET status=?,action_taken=?,resolved_at=? WHERE id=?')->execute([$status,trim($_POST['action_taken']??''),$status==='resolved'?date('Y-m-d H:i:s'):null,$id]);
 $assetStatus=$status==='resolved'?'working':($status==='in_progress'?'under_maintenance':'needs_repair');
 db()->prepare('UPDATE assets SET status=? WHERE id=?')->execute([$assetStatus,$row['aid']]);audit('maintenance.updated','Record '.$id);header('Location: index.php');exit;
}
$page_title='Update maintenance';require __DIR__ . '/../../includes/header.php';
?><h1>Update maintenance record</h1><div class="panel"><p><strong>Issue:</strong> <?= e($row['issue']) ?></p>
<form method="post"><input type="hidden" name="csrf_token" value="<?= e(csrf_token()) ?>">
<label>Status<select name="status"><?php foreach(['open','in_progress','resolved'] as $s): ?><option value="<?= e($s) ?>" <?= $row['status']===$s?'selected':'' ?>><?= e($s) ?></option><?php endforeach; ?></select></label>
<label>Action taken / remarks<textarea name="action_taken"><?= e($row['action_taken']) ?></textarea></label><button>Save update</button> <a href="index.php">Cancel</a></form></div>
<?php require __DIR__ . '/../../includes/footer.php'; ?>
