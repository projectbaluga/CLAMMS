param([string]$DatabasePath = "$env:ProgramData\CLAMMS\Server\data\clamms.db", [string]$BackupDirectory = "$env:ProgramData\CLAMMS\Backups")
$ErrorActionPreference = "Stop"
if (-not (Test-Path $DatabasePath)) { throw "Database not found: $DatabasePath" }
New-Item -ItemType Directory -Force -Path $BackupDirectory | Out-Null
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$destination = Join-Path $BackupDirectory "clamms-$stamp.db"
Copy-Item $DatabasePath $destination
if ((Get-Item $destination).Length -lt 4096) { Remove-Item $destination -Force; throw "Backup appears unexpectedly small." }
$hash = Get-FileHash $destination -Algorithm SHA256
"$($hash.Hash)  $([IO.Path]::GetFileName($destination))" | Set-Content "$destination.sha256"
Write-Host "Backup saved: $destination"
Write-Host "SHA256: $($hash.Hash)"
Write-Warning "For a guaranteed consistent offline file copy, stop CLAMMS Server first. While running, use the administrator backup download endpoint."
