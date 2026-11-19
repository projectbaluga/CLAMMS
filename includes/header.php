<?php
require_once __DIR__ . '/bootstrap.php';
?><!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title><?= e($page_title ?? 'CLAMMS') ?> | CLAMMS</title><link rel="stylesheet" href="<?= e(APP_URL) ?>/assets/css/app.css"></head>
<body><header class="topbar"><a class="brand" href="<?= e(APP_URL) ?>/dashboard.php">CLAMMS</a>
<nav><a href="<?= e(APP_URL) ?>/dashboard.php">Dashboard</a><a href="<?= e(APP_URL) ?>/laboratories/index.php">Laboratories</a>
<a href="<?= e(APP_URL) ?>/assets/index.php">Assets</a><a href="<?= e(APP_URL) ?>/maintenance/index.php">Maintenance</a>
<a href="<?= e(APP_URL) ?>/logout.php">Sign out</a></nav></header><main class="container">
