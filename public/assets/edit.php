<?php
declare(strict_types=1);
require_once __DIR__ . '/../../includes/bootstrap.php'; require_login();
$id=(int)($_GET['id']??0); $q=db()->prepare('SELECT * FROM assets WHERE id=?'); $q->execute([$id]); $asset=$q->fetch();
if(!$asset){http_response_code(404);exit('Asset not found');}
$labs=db()->query('SELECT id,name FROM laboratories ORDER BY id')->fetchAll(); $cats=db()->query('SELECT id,name FROM asset_categories ORDER BY name')->fetchAll(); $error='';
if($_SERVER['REQUEST_METHOD']==='POST'){
 verify_csrf();
 try{
  db()->prepare('UPDATE assets SET lab_id=?,category_id=?,asset_tag=?,name=?,serial_number=?,status=?,notes=? WHERE id=?')->execute([(int)$_POST['lab_id'],($_POST['category_id']??'')!==''?(int)$_POST['category_id']:null,trim($_POST['asset_tag']??''),trim($_POST['name']??''),trim($_POST['serial_number']??''),$_POST['status']??'working',trim($_POST['notes']??''),$id]);
  audit('asset.updated',$asset['asset_tag']);header('Location: index.php');exit;
 }catch(PDOException $ex){$error='Unable to update. Check that the asset tag is unique.';}
}
$page_title='Edit asset';require __DIR__ . '/../../includes/header.php';
?><h1>Edit asset</h1><?php if($error): ?><p class="error"><?= e($error) ?></p><?php endif; ?>
<form method="post" class="panel"><input type="hidden" name="csrf_token" value="<?= e(csrf_token()) ?>">
<label>Laboratory<select name="lab_id"><?php foreach($labs as $l): ?><option value="<?= (int)$l['id'] ?>" <?= $asset['lab_id']==$l['id']?'selected':'' ?>><?= e($l['name']) ?></option><?php endforeach; ?></select></label>
<label>Category<select name="category_id"><option value="">Uncategorized</option><?php foreach($cats as $c): ?><option value="<?= (int)$c['id'] ?>" <?= $asset['category_id']==$c['id']?'selected':'' ?>><?= e($c['name']) ?></option><?php endforeach; ?></select></label>
<label>Asset tag<input name="asset_tag" required value="<?= e($asset['asset_tag']) ?>"></label><label>Asset name<input name="name" required value="<?= e($asset['name']) ?>"></label>
<label>Serial number<input name="serial_number" value="<?= e($asset['serial_number']) ?>"></label><label>Status<select name="status"><?php foreach(['working','needs_repair','under_maintenance','retired'] as $s): ?><option value="<?= e($s) ?>" <?= $asset['status']===$s?'selected':'' ?>><?= e($s) ?></option><?php endforeach; ?></select></label>
<label>Notes<textarea name="notes"><?= e($asset['notes']) ?></textarea></label><button>Update asset</button> <a href="index.php">Cancel</a></form>
<?php require __DIR__ . '/../../includes/footer.php'; ?>
