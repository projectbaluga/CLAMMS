<?php
declare(strict_types=1);
require_once __DIR__ . '/../../includes/bootstrap.php'; require_login();
if($_SERVER['REQUEST_METHOD']!=='POST'){http_response_code(405);exit('Method not allowed');}
verify_csrf();
$assetId=(int)($_POST['asset_id']??0);$result=$_POST['result']??'';$note=trim($_POST['note']??'');
if(!in_array($result,['manual_check_ok','manual_check_issue'],true)){http_response_code(422);exit('Invalid inspection result');}
$q=db()->prepare('SELECT id,asset_tag FROM assets WHERE id=?');$q->execute([$assetId]);$asset=$q->fetch();
if(!$asset){http_response_code(404);exit('Asset not found');}
db()->prepare('INSERT INTO monitoring_logs(asset_id,status,ip_address,details) VALUES(?,?,?,?)')->execute([$assetId,$result,$_SERVER['REMOTE_ADDR']??null,json_encode(['note'=>$note,'checked_by'=>$_SESSION['user_id']])]);
if($result==='manual_check_issue') db()->prepare("UPDATE assets SET status='needs_repair' WHERE id=?")->execute([$assetId]);
audit('monitoring.manual_check',$asset['asset_tag'].' '.$result);
header('Location: index.php');exit;
