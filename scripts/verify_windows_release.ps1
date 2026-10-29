# Build/release validation only. Never disables protection, adds exclusions or restores threats.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ArchivePath,
    [Parameter(Mandatory = $true)][string]$ReportPath
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$report = [ordered]@{
    schema = 1
    status = 'failed'
    verified_at = [DateTime]::UtcNow.ToString('o')
    archive_sha256 = $null
    defender = $null
    executables = @()
    static_scan = 'not_run'
    startup_smoke = 'not_run'
    startup_observation_seconds = 20
    detections = @()
    limitations = @('Startup observation only; no account, gameplay or full preparation validation.',
        'A clean result does not establish that the reported detection is a false positive.')
    error = $null
}
$ownedRoot = $null
$process = $null
$started = [DateTime]::Now
$failure = $null
$reportFile = [IO.Path]::GetFullPath($ReportPath)

function Read-ProtectionState {
    $state = Get-MpComputerStatus
    if (-not $state.AMServiceEnabled -or -not $state.AntivirusEnabled -or -not $state.RealTimeProtectionEnabled -or -not $state.BehaviorMonitorEnabled) {
        throw 'Microsoft Defender and real-time protection must already be active. No protection settings will be changed.'
    }
    return [ordered]@{
        service_enabled = [bool]$state.AMServiceEnabled
        antivirus_enabled = [bool]$state.AntivirusEnabled
        realtime_enabled = [bool]$state.RealTimeProtectionEnabled
        behavior_enabled = [bool]$state.BehaviorMonitorEnabled
        engine_version = [string]$state.AMEngineVersion
        signature_version = [string]$state.AntivirusSignatureVersion
        signature_updated = $state.AntivirusSignatureLastUpdated.ToString('o')
    }
}

function Read-RelatedDetections([string]$CandidateRoot, [string]$Archive) {
    $found = @()
    foreach ($detection in @(Get-MpThreatDetection)) {
        $resources = @($detection.Resources)
        $related = @($resources | Where-Object {
            $_.IndexOf($CandidateRoot, [StringComparison]::OrdinalIgnoreCase) -ge 0 -or
            $_.IndexOf($Archive, [StringComparison]::OrdinalIgnoreCase) -ge 0
        })
        if ($related.Count -gt 0 -and ($detection.InitialDetectionTime -ge $started -or $detection.LastThreatStatusChangeTime -ge $started)) {
            $found += [ordered]@{
                threat_id = [string]$detection.ThreatID
                process = [string]$detection.ProcessName
                resources = $related
                detected_at = $detection.InitialDetectionTime.ToString('o')
            }
        }
    }
    return $found
}

try {
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw 'This validation requires genuine Windows with Microsoft Defender; Wine is not supported.' }
    $archive = (Resolve-Path -LiteralPath $ArchivePath).Path
    $report.archive_sha256 = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    $report.defender = Read-ProtectionState
    # An update failure stops validation rather than labelling an unverified release clean.
    Update-MpSignature
    $report.defender = Read-ProtectionState
    $ownedRoot = Join-Path ([IO.Path]::GetTempPath()) ('Rusted-validation-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $ownedRoot | Out-Null
    Expand-Archive -LiteralPath $archive -DestinationPath $ownedRoot
    $product = Join-Path $ownedRoot 'Rusted Launcher'
    $manifest = Get-Content -LiteralPath (Join-Path $product 'launcher-manifest.json') -Raw | ConvertFrom-Json
    foreach ($entry in $manifest.files.PSObject.Properties) {
        $file = [IO.Path]::GetFullPath((Join-Path $product $entry.Name))
        if (-not $file.StartsWith($product + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Path outside the release in its integrity manifest.'
        }
        if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ne $entry.Value) {
            throw ('Release integrity mismatch: ' + $entry.Name)
        }
    }
    foreach ($exe in @(Get-ChildItem -LiteralPath $product -Recurse -File -Filter '*.exe')) {
        $signature = Get-AuthenticodeSignature -LiteralPath $exe.FullName
        $report.executables += [ordered]@{
            file = $exe.FullName.Substring($product.Length + 1)
            sha256 = (Get-FileHash -LiteralPath $exe.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            authenticode = [string]$signature.Status
        }
    }
    $platform = Join-Path $env:ProgramData 'Microsoft\Windows Defender\Platform'
    $scanners = @(Get-ChildItem -LiteralPath $platform -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending | ForEach-Object { Join-Path $_.FullName 'MpCmdRun.exe' })
    $scanners += Join-Path $env:ProgramFiles 'Windows Defender\MpCmdRun.exe'
    $scanner = $scanners | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
    if (-not $scanner) { throw 'Official Microsoft Defender command-line scanner unavailable.' }
    $scannerSignature = Get-AuthenticodeSignature -LiteralPath $scanner
    if ($scannerSignature.Status -ne 'Valid' -or $scannerSignature.SignerCertificate.Subject -notmatch 'Microsoft') {
        throw 'Microsoft Defender scanner signature could not be verified.'
    }
    $scanLog = [IO.Path]::ChangeExtension($reportFile, 'scan.log')
    New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName($scanLog)) | Out-Null
    # Normal scan/remediation: no exclusions, suppression, or restoration of blocked executables.
    & $scanner -Scan -ScanType 3 -File $product *> $scanLog
    if ($LASTEXITCODE -ne 0) { throw ('Defender scan failed or reported a threat; exit code ' + $LASTEXITCODE) }
    $report.detections = @(Read-RelatedDetections $ownedRoot $archive)
    if ($report.detections.Count -gt 0) { throw 'Defender detected a threat in the release.' }
    foreach ($exe in $report.executables) {
        if ((Get-FileHash -LiteralPath (Join-Path $product $exe.file) -Algorithm SHA256).Hash -ne $exe.sha256) {
            throw ('Executable missing or changed after scan: ' + $exe.file)
        }
    }
    $report.static_scan = 'passed'
    $launcher = Join-Path $product 'Rusted Launcher.exe'
    if (-not (Test-Path -LiteralPath $launcher -PathType Leaf)) { throw 'The launcher is missing, possibly quarantined.' }
    $data = Join-Path $ownedRoot 'smoke-data'
    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName = $launcher
    $start.WorkingDirectory = $product
    $start.UseShellExecute = $false
    $start.Arguments = '--data-dir "' + $data + '"'
    foreach ($name in @('HORIZON_HOME', 'HORIZON_LAUNCHER_HOME', 'HORIZON_LAUNCHER_TRASH',
        'HORIZON_LAUNCHER_CLIPBOARD', 'HORIZON_LAUNCHER_DESKTOP', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME')) {
        $start.EnvironmentVariables[$name] = Join-Path $ownedRoot $name
    }
    $start.EnvironmentVariables['HORIZON_LAUNCHER_NO_SELF_UPDATE'] = '1'
    $start.EnvironmentVariables['HORIZON_NO_AUDIO'] = '1'
    $start.EnvironmentVariables.Remove('HORIZON_SESSION_FILE')
    $process = [Diagnostics.Process]::Start($start)
    if (-not $process.WaitForInputIdle(15000)) { throw 'The launcher did not reach an idle GUI.' }
    # Observe the genuine first-run GUI with its automatic preparation, without entering an account.
    for ($second = 0; $second -lt $report.startup_observation_seconds; $second++) {
        Start-Sleep -Seconds 1
        $process.Refresh()
        if ($process.HasExited) { throw 'The launcher exited during the startup observation.' }
    }
    if (-not $process.Responding -or $process.MainWindowHandle -eq [IntPtr]::Zero) {
        throw 'The launcher GUI is not responding.'
    }
    if (-not $process.CloseMainWindow() -or -not $process.WaitForExit(15000)) {
        throw 'The launcher could not close normally.'
    }
    if ($process.ExitCode -ne 0) { throw 'The launcher returned a failure status.' }
    Start-Sleep -Seconds 2
    $report.defender = Read-ProtectionState
    $report.detections = @(Read-RelatedDetections $ownedRoot $archive)
    if ($report.detections.Count -gt 0 -or -not (Test-Path -LiteralPath $launcher -PathType Leaf)) {
        throw 'Defender detected or quarantined the launcher during the startup observation.'
    }
    foreach ($exe in $report.executables) {
        if ((Get-FileHash -LiteralPath (Join-Path $product $exe.file) -Algorithm SHA256).Hash -ne $exe.sha256) {
            throw ('Executable missing or changed during startup: ' + $exe.file)
        }
    }
    $report.startup_smoke = 'passed'
    $report.status = 'passed'
}
catch {
    $failure = $_.Exception.Message
    $report.error = $failure
    if ($null -ne $ownedRoot) {
        try { $report.detections = @(Read-RelatedDetections $ownedRoot $archive) }
        catch { $report.error += ' Defender detection history could not be read.' }
    }
}
finally {
    if ($null -ne $process) {
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
        $process.Dispose()
    }
    New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName($reportFile)) | Out-Null
    $report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $reportFile -Encoding UTF8
    # Keep the report, not the disposable test instance or downloaded data.
    if ($null -ne $ownedRoot -and (Test-Path -LiteralPath $ownedRoot)) {
        Remove-Item -LiteralPath $ownedRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
if ($null -ne $failure) { throw ('Windows release validation failed: ' + $failure) }
Write-Output ('Windows Defender scan and startup observation passed. Report: ' + $reportFile)
