# Ephemeral GitHub-hosted Windows runner only: enable protection for validation.
# This is not shipped or executed by the launcher. Never disables a protection,
# adds an exclusion, changes reporting policy, or restores a quarantined file.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT -or
    $env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Defender preparation is restricted to an ephemeral GitHub-hosted Windows runner.'
}
$before = Get-MpComputerStatus
$before | Select-Object AMServiceEnabled, AntivirusEnabled, RealTimeProtectionEnabled,
    BehaviorMonitorEnabled, AMEngineVersion, AntivirusSignatureVersion | ConvertTo-Json
if (-not $before.AMServiceEnabled -or -not $before.AntivirusEnabled) {
    throw 'The runner does not provide an active Defender antivirus service.'
}
if (-not $before.RealTimeProtectionEnabled -or -not $before.BehaviorMonitorEnabled) {
    # The Disable* parameters must be FALSE to turn these protections ON.
    Set-MpPreference -DisableRealtimeMonitoring $false -DisableBehaviorMonitoring $false
}
for ($attempt = 0; $attempt -lt 10; $attempt++) {
    $state = Get-MpComputerStatus
    if ($state.RealTimeProtectionEnabled -and $state.BehaviorMonitorEnabled) {
        $state | Select-Object AMServiceEnabled, AntivirusEnabled, RealTimeProtectionEnabled,
            BehaviorMonitorEnabled, AMEngineVersion, AntivirusSignatureVersion | ConvertTo-Json
        Write-Output 'Real-time and behavioral protection are active for the Windows validation.'
        exit 0
    }
    Start-Sleep -Seconds 2
}
throw 'Runner policy did not allow active protection. Validation remains blocked; do not bypass the policy.'
