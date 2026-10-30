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
    resize_smoke = 'not_run'
    microsoft_code_popup = 'not_run'
    popup_diagnostics = $null
    startup_observation_seconds = 20
    detections = @()
    limitations = @('Startup observation only; no account, gameplay or full preparation validation.',
        'A clean result does not establish that the reported detection is a false positive.')
    error = $null
    error_location = $null
    stage = 'initialization'
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
    # Use the signed Defender console for signature updates as well as scans.
    # Update-MpSignature's CIM provider returned RPC failures on hosted Windows.
    # Any console failure or inactive protection still stops validation.
    $report.stage = 'signature_update'
    $signatureLog = [IO.Path]::ChangeExtension($reportFile, 'signature-update.log')
    New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName($signatureLog)) | Out-Null
    & $scanner -SignatureUpdate *> $signatureLog
    if ($LASTEXITCODE -ne 0) { throw ('Defender signature update failed; exit code ' + $LASTEXITCODE) }
    $report.defender = Read-ProtectionState
    $report.stage = 'archive_integrity'
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
    $report.stage = 'static_scan'
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
    $report.stage = 'gui_smoke'
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
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class RustedGuiSmoke {
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int height, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern IntPtr FindWindow(string cls, string title);
    // PowerShell converts $null to an empty .NET string. Construct the native
    // null title here so FindWindow does not search only for untitled windows.
    public static IntPtr MicrosoftPopup() { return FindWindow("RustedMicrosoftCodePopup", null); }
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder text, int max);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    public delegate bool WindowEnumerator(IntPtr window, IntPtr parameter);
    [DllImport("user32.dll")] public static extern bool EnumWindows(WindowEnumerator callback, IntPtr parameter);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr window, StringBuilder text, int max);
    public static string[] ProcessWindows(uint target) {
        var windows = new System.Collections.Generic.List<string>();
        EnumWindows((window, parameter) => {
            uint processId;
            GetWindowThreadProcessId(window, out processId);
            if (processId == target) {
                var cls = new StringBuilder(200);
                GetClassName(window, cls, 200);
                windows.Add(cls.ToString() + ":visible=" + IsWindowVisible(window));
            }
            return true;
        }, IntPtr.Zero);
        return windows.ToArray();
    }
}
'@
    $process.Refresh()
    $mainWindow = $process.MainWindowHandle
    for ($resize = 0; $resize -lt 40; $resize++) {
        if (-not [RustedGuiSmoke]::SetWindowPos($mainWindow, [IntPtr]::Zero, 0, 0,
            (1120 + ($resize * 73) % 480), (760 + ($resize * 31) % 240), 0x16)) {
            throw 'Launcher resize failed.'
        }
    }
    [void][RustedGuiSmoke]::SetWindowPos($mainWindow, [IntPtr]::Zero, 0, 0, 1200, 820, 0x16)
    $process.Refresh()
    if (-not $process.Responding -or [RustedGuiSmoke]::GetDlgItem($mainWindow, 1) -eq [IntPtr]::Zero) {
        throw 'Launcher did not remain responsive with its Play action after resizing.'
    }
    $report.resize_smoke = 'passed'
    # Request a genuine Microsoft code, but never enter a user account or log the code.
    # Exercise actual button notifications (BM_CLICK), not synthetic menu commands.
    # The dialog message manager can consume queued WM_COMMAND messages on Windows.
    $loginButton = [IntPtr]::Zero
    for ($attempt = 0; $attempt -lt 40; $attempt++) {
        [void][RustedGuiSmoke]::PostMessage([RustedGuiSmoke]::GetDlgItem($mainWindow, 104), 0xF5, [IntPtr]::Zero, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 250
        $loginButton = [RustedGuiSmoke]::GetDlgItem($mainWindow, 501)
        if ($loginButton -ne [IntPtr]::Zero) { break }
    }
    if ($loginButton -eq [IntPtr]::Zero) { throw 'Accounts navigation did not display the Microsoft sign-in button.' }
    [void][RustedGuiSmoke]::PostMessage($loginButton, 0xF5, [IntPtr]::Zero, [IntPtr]::Zero)
    $popup = [IntPtr]::Zero
    $popupReady = $false
    for ($attempt = 0; $attempt -lt 240; $attempt++) {
        $popup = [RustedGuiSmoke]::MicrosoftPopup()
        $codeLength = 0
        if ($popup -ne [IntPtr]::Zero) {
            $codeLength = [RustedGuiSmoke]::SendMessage([RustedGuiSmoke]::GetDlgItem($popup, 6001), 0xE, [IntPtr]::Zero, [IntPtr]::Zero).ToInt64()
        }
        $homeVisible = [RustedGuiSmoke]::GetDlgItem($mainWindow, 1010) -ne [IntPtr]::Zero
        $popupTitle = New-Object Text.StringBuilder 100
        [void][RustedGuiSmoke]::GetWindowText($popup, $popupTitle, 100)
        $popupVisible = [RustedGuiSmoke]::IsWindowVisible($popup)
        if ($codeLength -gt 0 -and $homeVisible -and $popupVisible -and $popupTitle.ToString() -eq 'Connexion Microsoft - Rusted') { $popupReady = $true; break }
        if ($process.HasExited) { throw 'Launcher exited while requesting Microsoft sign-in.' }
        Start-Sleep -Milliseconds 250
    }
    if (-not $popupReady) {
        [void][RustedGuiSmoke]::PostMessage([RustedGuiSmoke]::GetDlgItem($mainWindow, 104), 0xF5, [IntPtr]::Zero, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 500
    }
    $notice = New-Object Text.StringBuilder 500
    [void][RustedGuiSmoke]::GetWindowText([RustedGuiSmoke]::GetDlgItem($mainWindow, 550), $notice, 500)
    # Only lengths/flags and the generic account status: never save the device code.
    $report.popup_diagnostics = [ordered]@{ window_found = ($popup -ne [IntPtr]::Zero); visible = $popupVisible; title = $popupTitle.ToString(); code_present = ($codeLength -gt 0); home_visible = $homeVisible; account_notice = $notice.ToString(); account_code_controls = ([RustedGuiSmoke]::GetDlgItem($mainWindow, 509) -ne [IntPtr]::Zero) }
    $report.popup_diagnostics['process_windows'] = [RustedGuiSmoke]::ProcessWindows([uint32]$process.Id)
    if (-not $popupReady) { throw ('Microsoft code popup was not ready above the home page. Account status: ' + $notice.ToString()) }
    [void][RustedGuiSmoke]::PostMessage($popup, 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
    for ($attempt = 0; $attempt -lt 40 -and [RustedGuiSmoke]::IsWindow($popup); $attempt++) {
        Start-Sleep -Milliseconds 100
    }
    $signInAvailable = $false
    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        $button = [RustedGuiSmoke]::GetDlgItem($mainWindow, 501)
        if ($button -ne [IntPtr]::Zero -and [RustedGuiSmoke]::IsWindowEnabled($button)) { $signInAvailable = $true; break }
        Start-Sleep -Milliseconds 100
    }
    $report.popup_diagnostics['closed_after_cancel'] = -not [RustedGuiSmoke]::IsWindow($popup)
    $report.popup_diagnostics['sign_in_available_after_cancel'] = $signInAvailable
    $report.popup_diagnostics['account_files_after_cancel'] = @(Get-ChildItem -LiteralPath (Join-Path $data 'accounts') -File).Count
    if ([RustedGuiSmoke]::IsWindow($popup) -or -not $signInAvailable -or $report.popup_diagnostics['account_files_after_cancel'] -ne 0) {
        throw 'Microsoft popup did not cancel cleanly without storing an account.'
    }
    $report.microsoft_code_popup = 'passed; real device request, code displayed, cancelled without account'
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
    $report.stage = 'complete'
}
catch {
    $failure = $_.Exception.Message
    $report.error = $failure
    $report.error_location = $_.InvocationInfo.PositionMessage
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
