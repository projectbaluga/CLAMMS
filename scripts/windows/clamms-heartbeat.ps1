param(
    [Parameter(Mandatory=$true)][string]$ServerUrl,
    [Parameter(Mandatory=$true)][string]$AssetTag,
    [Parameter(Mandatory=$true)][string]$SharedSecret,
    [int]$IntervalSeconds = 60
)

# Run continuously using a dedicated, restricted Windows account.
# Configure Task Scheduler to start at system startup and restart on failure.
$endpoint = $ServerUrl.TrimEnd("/") + "/api/heartbeat.php"
$hostname = $env:COMPUTERNAME

while ($true) {
    try {
        $payload = @{
            asset_tag = $AssetTag
            hostname = $hostname
            reported_at = (Get-Date).ToUniversalTime().ToString("o")
        } | ConvertTo-Json -Compress

        Invoke-RestMethod -Uri $endpoint -Method Post -ContentType "application/json" `
            -Headers @{ "X-CLAMMS-SECRET" = $SharedSecret } -Body $payload -TimeoutSec 15 | Out-Null
    }
    catch {
        # A temporary network/server failure must not stop future heartbeat attempts.
        Write-Warning ("CLAMMS heartbeat failed for {0}: {1}" -f $AssetTag, $_.Exception.Message)
    }
    Start-Sleep -Seconds ([Math]::Max(15, $IntervalSeconds))
}
