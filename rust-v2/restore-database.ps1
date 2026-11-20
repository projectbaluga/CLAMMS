param(
    [Parameter(Mandatory=$true)][string]$BackupPath,
    [string]$InstallPath = "$env:ProgramData\CLAMMS\Server",
    [string]$TaskName = "CLAMMS Server"
)
$ErrorActionPreference = "Stop"
if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw "Run elevated PowerShell." }
$exe = Join-Path $InstallPath "clamms-server.exe"
$db = Join-Path $InstallPath "data\clamms.db"
if (-not (Test-Path $exe)) { throw "CLAMMS server executable not found: $exe" }
if (-not (Test-Path $BackupPath)) { throw "Backup file not found: $BackupPath" }

# Verify the source backup before stopping the live service.
& $exe --check-db $BackupPath
if ($LASTEXITCODE -ne 0) { throw "Selected backup failed SQLite integrity validation. Live database was not changed." }

$wasRunning = (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue).State -eq "Running"
if ($wasRunning) {
    Stop-ScheduledTask -TaskName $TaskName
    $deadline = (Get-Date).AddSeconds(30)
    do {
        Start-Sleep -Milliseconds 500
        $state = (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue).State
    } while ($state -eq "Running" -and (Get-Date) -lt $deadline)
    if ($state -eq "Running") { throw "Server task did not stop; no restore was performed." }
}

$dataDir = Split-Path $db -Parent
New-Item -ItemType Directory -Force -Path $dataDir | Out-Null
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$currentBackup = Join-Path $dataDir "clamms-before-restore-$stamp.db"
$tempDb = Join-Path $dataDir "clamms-restore-$stamp.tmp"
$oldDb = Join-Path $dataDir "clamms-previous-$stamp.db"
try {
    if (Test-Path $db) { Copy-Item $db $currentBackup -Force }
    Copy-Item $BackupPath $tempDb -Force
    & $exe --check-db $tempDb
    if ($LASTEXITCODE -ne 0) { throw "Staged restore failed integrity validation." }
    Remove-Item "$db-wal","$db-shm" -Force -ErrorAction SilentlyContinue
    if (Test-Path $db) { Move-Item $db $oldDb -Force }
    Move-Item $tempDb $db -Force
    if ($wasRunning) { Start-ScheduledTask -TaskName $TaskName }
    Write-Host "Restore completed. Previous database retained at: $oldDb"
    if (Test-Path $currentBackup) { Write-Host "Pre-restore file copy: $currentBackup" }
}
catch {
    if (Test-Path $tempDb) { Remove-Item $tempDb -Force }
    if ((-not (Test-Path $db)) -and (Test-Path $oldDb)) { Move-Item $oldDb $db -Force }
    if ($wasRunning) { Start-ScheduledTask -TaskName $TaskName }
    throw
}
