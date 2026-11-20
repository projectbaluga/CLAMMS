param([string]$InstallPath = "$env:ProgramData\CLAMMS\Server", [string]$TaskName = "CLAMMS Server")
$ErrorActionPreference = "Stop"
if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw "Run elevated PowerShell." }
$exe = Join-Path $PSScriptRoot "target\release\clamms-server.exe"
if (-not (Test-Path $exe)) { $exe = Join-Path $PSScriptRoot "clamms-server.exe" }
if (-not (Test-Path $exe)) { throw "clamms-server.exe not found. Build the server first." }
New-Item -ItemType Directory -Force -Path $InstallPath | Out-Null
Copy-Item $exe (Join-Path $InstallPath "clamms-server.exe") -Force
New-Item -ItemType Directory -Force -Path (Join-Path $InstallPath "data") | Out-Null
& icacls.exe $InstallPath /inheritance:r /grant:r "*S-1-5-18:(OI)(CI)F" "*S-1-5-32-544:(OI)(CI)F" | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Could not secure server directory permissions." }
$sourceEnv = Join-Path $PSScriptRoot ".env"
$targetEnv = Join-Path $InstallPath ".env"
if (-not (Test-Path $targetEnv)) {
    if (Test-Path $sourceEnv) { Copy-Item $sourceEnv $targetEnv }
    else { @("CLAMMS_BIND=0.0.0.0:8080","CLAMMS_DATABASE_URL=sqlite://data/clamms.db","CLAMMS_AGENT_SECRET=REPLACE_WITH_LONG_RANDOM_SECRET","CLAMMS_BOOTSTRAP_ADMIN_USERNAME=admin","CLAMMS_BOOTSTRAP_ADMIN_PASSWORD=REPLACE_WITH_UNIQUE_PASSWORD_12_CHARS_MINIMUM","RUST_LOG=info") | Set-Content $targetEnv }
}
$action = New-ScheduledTaskAction -Execute (Join-Path $InstallPath "clamms-server.exe") -WorkingDirectory $InstallPath
$trigger = New-ScheduledTaskTrigger -AtStartup
$principal = New-ScheduledTaskPrincipal -UserId "SYSTEM" -LogonType ServiceAccount -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -RestartCount 5 -RestartInterval (New-TimeSpan -Minutes 1)
Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description "CLAMMS local inventory and monitoring server" -Force | Out-Null
Write-Host "Installed. Secure $targetEnv before starting."
Write-Warning "Restrict TCP 8080 to school LAN/VPN only; never expose it to the public internet."
Write-Host "Start with: Start-ScheduledTask -TaskName '$TaskName'"
