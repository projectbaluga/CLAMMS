param(
    [string]$InstallPath = "$env:ProgramData\CLAMMS\Server",
    [string]$BackupDirectory = "$env:ProgramData\CLAMMS\Backups",
    [string]$TaskName = "CLAMMS Server"
)
$ErrorActionPreference = "Stop"
$DatabasePath = Join-Path $InstallPath "data\clamms.db"
$ServerExe = Join-Path $InstallPath "clamms-server.exe"
if (-not (Test-Path $DatabasePath)) { throw "Database not found: $DatabasePath" }
if (-not (Test-Path $ServerExe)) { throw "Server executable not found: $ServerExe" }
New-Item -ItemType Directory -Force -Path $BackupDirectory | Out-Null
$task = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
$wasRunning = $null -ne $task -and $task.State -eq "Running"
$serverProcess = Get-Process -Name "clamms-server" -ErrorAction SilentlyContinue
if ($null -ne $serverProcess -and -not $wasRunning) { throw "Server process is running outside the expected scheduled task; stop it before backing up." }
$destination = Join-Path $BackupDirectory ("clamms-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".db")
try {
    if ($wasRunning) {
        Stop-ScheduledTask -TaskName $TaskName
        $deadline = (Get-Date).AddSeconds(30)
        do {
            Start-Sleep -Milliseconds 500
            $state = (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue).State
        } while ($state -eq "Running" -and (Get-Date) -lt $deadline)
        if ($state -eq "Running") { throw "Server task did not stop; backup was cancelled." }
    }
    Copy-Item $DatabasePath $destination
    & $ServerExe --check-db $destination
    if ($LASTEXITCODE -ne 0) { Remove-Item $destination -Force; throw "Backup failed SQLite integrity validation." }
    $hash = Get-FileHash $destination -Algorithm SHA256
    "$($hash.Hash)  $([IO.Path]::GetFileName($destination))" | Set-Content "$destination.sha256"
    Write-Host "Verified backup saved: $destination"
    Write-Host "SHA256: $($hash.Hash)"
}
finally {
    if ($wasRunning) { Start-ScheduledTask -TaskName $TaskName }
}
