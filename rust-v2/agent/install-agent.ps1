param([string]$InstallPath = "$env:ProgramData\CLAMMS\Agent", [string]$TaskName = "CLAMMS Workstation Monitoring Agent")
$ErrorActionPreference = "Stop"
if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw "Run elevated PowerShell." }
$exe = Join-Path $PSScriptRoot "target\release\clamms-agent.exe"
if (-not (Test-Path $exe)) { throw "clamms-agent.exe not found. Build the agent first." }
New-Item -ItemType Directory -Force -Path $InstallPath | Out-Null
& icacls.exe $InstallPath /inheritance:r /grant:r "*S-1-5-18:(OI)(CI)F" "*S-1-5-32-544:(OI)(CI)F" | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Could not secure agent directory permissions." }
Copy-Item $exe (Join-Path $InstallPath "clamms-agent.exe") -Force
$sourceEnv = Join-Path $PSScriptRoot ".env"
$targetEnv = Join-Path $InstallPath ".env"
if (-not (Test-Path $targetEnv)) {
    if (Test-Path $sourceEnv) { Copy-Item $sourceEnv $targetEnv }
    else { @("CLAMMS_SERVER_URL=http://127.0.0.1:8080","CLAMMS_AGENT_SECRET=REPLACE_WITH_SERVER_SECRET","CLAMMS_ASSET_TAG=SET_ASSET_TAG","CLAMMS_INTERVAL_SECONDS=60") | Set-Content $targetEnv }
}
$action = New-ScheduledTaskAction -Execute (Join-Path $InstallPath "clamms-agent.exe") -WorkingDirectory $InstallPath
$trigger = New-ScheduledTaskTrigger -AtStartup
$principal = New-ScheduledTaskPrincipal -UserId "SYSTEM" -LogonType ServiceAccount -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -RestartCount 5 -RestartInterval (New-TimeSpan -Minutes 1)
Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description "CLAMMS asset telemetry agent" -Force | Out-Null
Write-Host "Installed. Edit $targetEnv with the real server URL, secret and registered asset tag."
Write-Host "Then run: Start-ScheduledTask -TaskName '$TaskName'"
