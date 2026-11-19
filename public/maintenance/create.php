<?php
declare(strict_types=1);
require_once __DIR__ . '/../../includes/bootstrap.php'; require_login();
$assets=db()->query("SELECT id,asset_tag,name FROM assets WHERE status <> 'retired' ORDER BY asset_tag")->fetchAll(); $error='';
if($_SERVER['REQUEST_METHOD']==='POST'){
 verify_csrf();
 try{
  $aid=(int)$_POST['asset_id'];$issue=trim($_POST['issue']??'');
  if($issue==='') throw new RuntimeException('Describe the issue.');
  db()->beginTransaction();
  db()->prepare('INSERT INTO maintenance_logs(asset_id,reported_by,issue) VALUES(?,?,?)')->execute([$aid,$_SESSION['user_id'],$issue]);
  db()->prepare("UPDATE assets SET status='needs_repair' WHERE id=?")->execute([$aid]);
  db()->commit();audit('maintenance.reported','Asset ID '.$aid);header('Location: index.php');exit;
 }catch(Throwable $ex){if(db()->inTransaction())db()->rollBack();$error=$ex instanceof RuntimeException?$ex->getMessage():'Unable to create maintenance record.';}
}
$page_title='Report issue';require __DIR__ . '/../../includes/header.php';
?><h1>Report maintenance issue</h1><?php if($error): ?><p class="error"><?= e($error) ?></p><?php endif; ?>
<form method="post" class="panel"><input type="hidden" name="csrf_token" value="<?= e(csrf_token()) ?>">
<label>Asset<select name="asset_id" required><?php foreach($assets as $a): ?><option value="<?= (int)$a['id'] ?>"><?= e($a['asset_tag'].' — '.$a['name']) ?></option><?php endforeach; ?></select></label>
<label>Issue description<textarea name="issue" required></textarea></label><button>Submit report</button> <a href="index.php">Cancel</a></form>
<?php require __DIR__ . '/../../includes/footer.php'; ?>
