<?php
declare(strict_types=1);
require_once __DIR__ . '/../../includes/bootstrap.php'; require_login();
$id=(int)($_GET['id'] ?? 0); $q=db()->prepare('SELECT * FROM laboratories WHERE id=?'); $q->execute([$id]); $lab=$q->fetch();
if (!$lab) { http_response_code(404); exit('Laboratory not found'); }
$error='';
if ($_SERVER['REQUEST_METHOD']==='POST') {
 verify_csrf();
 try {
  db()->prepare('UPDATE laboratories SET name=?,room_code=?,computer_count=?,description=? WHERE id=?')->execute([trim($_POST['name'] ?? ''),trim($_POST['room_code'] ?? ''),max(0,(int)($_POST['computer_count'] ?? 0)),trim($_POST['description'] ?? ''),$id]);
  audit('laboratory.updated',$lab['name']); header('Location: index.php'); exit;
 } catch(PDOException $ex) { $error='Unable to update. Check that the name is unique.'; }
}
$page_title='Edit laboratory'; require __DIR__ . '/../../includes/header.php';
?><h1>Edit laboratory</h1><?php if($error): ?><p class="error"><?= e($error) ?></p><?php endif; ?>
<form method="post" class="panel"><input type="hidden" name="csrf_token" value="<?= e(csrf_token()) ?>">
<label>Laboratory name<input name="name" required value="<?= e($lab['name']) ?>"></label>
<label>Room code<input name="room_code" value="<?= e($lab['room_code']) ?>"></label>
<label>Expected computer count<input type="number" min="0" name="computer_count" value="<?= (int)$lab['computer_count'] ?>"></label>
<label>Description<textarea name="description"><?= e($lab['description']) ?></textarea></label><button>Update laboratory</button> <a href="index.php">Cancel</a></form>
<?php require __DIR__ . '/../../includes/footer.php'; ?>
