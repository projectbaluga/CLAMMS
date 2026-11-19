<?php
declare(strict_types=1);
require_once __DIR__ . '/../../includes/bootstrap.php'; require_login();
$error = '';
if ($_SERVER['REQUEST_METHOD'] === 'POST') {
 verify_csrf();
 try {
  $stmt = db()->prepare('INSERT INTO laboratories(name,room_code,computer_count,description) VALUES(?,?,?,?)');
  $stmt->execute([trim($_POST['name'] ?? ''), trim($_POST['room_code'] ?? ''), max(0,(int)($_POST['computer_count'] ?? 0)), trim($_POST['description'] ?? '')]);
  audit('laboratory.created', trim($_POST['name'] ?? '')); header('Location: index.php'); exit;
 } catch (PDOException $ex) { $error = 'Unable to save. Check that the laboratory name is unique.'; }
}
$page_title='Add laboratory'; require __DIR__ . '/../../includes/header.php';
?><h1>Add laboratory</h1><?php if ($error): ?><p class="error"><?= e($error) ?></p><?php endif; ?>
<form method="post" class="panel"><input type="hidden" name="csrf_token" value="<?= e(csrf_token()) ?>">
<label>Laboratory name<input name="name" required maxlength="120"></label>
<label>Room code<input name="room_code" maxlength="40"></label>
<label>Expected computer count<input type="number" name="computer_count" min="0" value="0"></label>
<label>Description<textarea name="description"></textarea></label><button>Save laboratory</button> <a href="index.php">Cancel</a></form>
<?php require __DIR__ . '/../../includes/footer.php'; ?>
