<?php
declare(strict_types=1);
require_once __DIR__ . '/../../includes/bootstrap.php'; require_login();
$labs=db()->query('SELECT id,name FROM laboratories ORDER BY id')->fetchAll(); $cats=db()->query('SELECT id,name FROM asset_categories ORDER BY name')->fetchAll(); $error='';
if($_SERVER['REQUEST_METHOD']==='POST') {
 verify_csrf();
 try {
  db()->prepare('INSERT INTO assets(lab_id,category_id,asset_tag,name,serial_number,status,notes) VALUES(?,?,?,?,?,?,?)')->execute([(int)$_POST['lab_id'],($_POST['category_id']??'')!==''?(int)$_POST['category_id']:null,trim($_POST['asset_tag']??''),trim($_POST['name']??''),trim($_POST['serial_number']??''),$_POST['status']??'working',trim($_POST['notes']??'')]);
  audit('asset.created',trim($_POST['asset_tag']??'')); header('Location: index.php'); exit;
 } catch(PDOException $ex) { $error='Unable to save. The asset tag may already exist.'; }
}
$page_title='Register asset'; require __DIR__ . '/../../includes/header.php';
?><h1>Register asset</h1><?php if($error): ?><p class="error"><?= e($error) ?></p><?php endif; ?>
<form method="post" class="panel"><input type="hidden" name="csrf_token" value="<?= e(csrf_token()) ?>">
<label>Laboratory<select name="lab_id" required><?php foreach($labs as $l): ?><option value="<?= (int)$l['id'] ?>"><?= e($l['name']) ?></option><?php endforeach; ?></select></label>
<label>Category<select name="category_id"><option value="">Uncategorized</option><?php foreach($cats as $c): ?><option value="<?= (int)$c['id'] ?>"><?= e($c['name']) ?></option><?php endforeach; ?></select></label>
<label>Asset tag<input name="asset_tag" required maxlength="80"></label><label>Asset name<input name="name" required maxlength="160"></label>
<label>Serial number<input name="serial_number" maxlength="120"></label><label>Status<select name="status"><?php foreach(['working','needs_repair','under_maintenance','retired'] as $s): ?><option value="<?= e($s) ?>"><?= e($s) ?></option><?php endforeach; ?></select></label>
<label>Notes<textarea name="notes"></textarea></label><button>Save asset</button> <a href="index.php">Cancel</a></form>
<?php require __DIR__ . '/../../includes/footer.php'; ?>
