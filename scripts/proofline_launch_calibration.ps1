[CmdletBinding(DefaultParameterSetName = 'Real')]
param(
    [Parameter(ParameterSetName = 'Real', Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$ExecutablePath,

    [Parameter(ParameterSetName = 'Real', Mandatory = $true)]
    [ValidatePattern('^[a-fA-F0-9]{64}$')]
    [string]$ExpectedExecutableSha256,

    [Parameter(ParameterSetName = 'Real', Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$FixtureManifestPath,

    [Parameter(ParameterSetName = 'Real', Mandatory = $true)]
    [ValidatePattern('^[a-fA-F0-9]{64}$')]
    [string]$ExpectedFixtureSha256,

    [Parameter(ParameterSetName = 'Real')]
    [ValidateRange(1, 100)]
    [int]$ColdAttempts = 10,

    [Parameter(ParameterSetName = 'Real')]
    [ValidateRange(1, 100)]
    [int]$WarmAttempts = 10,

    [Parameter(ParameterSetName = 'Real')]
    [string]$RawArtifactRoot,

    [Parameter(ParameterSetName = 'Real')]
    [string]$AggregateOutputPath,

    [Parameter(ParameterSetName = 'Real')]
    [ValidateRange(1, 180)]
    [int]$AttemptTimeoutSeconds = 45,

    [Parameter(ParameterSetName = 'Real')]
    [ValidateRange(1, 60)]
    [int]$VisualTimeoutSeconds = 20,

    [Parameter(ParameterSetName = 'Real')]
    [string]$VisualObserverPath,

    [Parameter(ParameterSetName = 'Real')]
    [string]$VisualAnchorPath,

    [Parameter(ParameterSetName = 'Synthetic', Mandatory = $true)]
    [switch]$SyntheticTest,

    [Parameter(ParameterSetName = 'Library', Mandatory = $true)]
    [switch]$LibraryOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ([string]::IsNullOrWhiteSpace($VisualObserverPath)) {
    $VisualObserverPath = Join-Path $PSScriptRoot 'proofline_visual_observer.ps1'
}
if ([string]::IsNullOrWhiteSpace($VisualAnchorPath)) {
    $VisualAnchorPath = Join-Path (Split-Path -Parent $PSScriptRoot) 'desktop\proofline\assets\proofline-mark.png'
}

function New-ProoflineRefusal([string]$Reason) {
    [ordered]@{
        schema = 'spark.proofline.launch-calibration.refusal.v1'
        synthetic_test = $false
        countable = $false
        eligible = $false
        ineligible_reason = $Reason
    }
}

function Write-ProoflineJsonAtomic([string]$Path, [object]$Value) {
    $absolute = [IO.Path]::GetFullPath($Path)
    $parent = Split-Path -Parent $absolute
    if (-not [string]::IsNullOrWhiteSpace($parent)) {
        [IO.Directory]::CreateDirectory($parent) | Out-Null
    }
    $temporary = "$absolute.tmp-$([Guid]::NewGuid().ToString('N'))"
    [IO.File]::WriteAllText($temporary, ($Value | ConvertTo-Json -Depth 12), [Text.UTF8Encoding]::new($false))
    if ([IO.File]::Exists($absolute)) {
        $backup = "$absolute.bak-$([Guid]::NewGuid().ToString('N'))"
        try { [IO.File]::Replace($temporary, $absolute, $backup) }
        finally { if ([IO.File]::Exists($backup)) { [IO.File]::Delete($backup) } }
    }
    else {
        [IO.File]::Move($temporary, $absolute)
    }
}

function Get-ProoflineSha256([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Test-ProoflineWritableDirectory([string]$Path) {
    $probe = $null
    try {
        [IO.Directory]::CreateDirectory($Path) | Out-Null
        $probe = Join-Path $Path ("write-probe-$([Guid]::NewGuid().ToString('N'))")
        [IO.File]::WriteAllText($probe, 'probe', [Text.UTF8Encoding]::new($false))
        return $true
    }
    catch {
        return $false
    }
    finally {
        if ($null -ne $probe -and [IO.File]::Exists($probe)) { [IO.File]::Delete($probe) }
    }
}

function Test-ProoflineBinaryHook([string]$Path) {
    $bytes = [IO.File]::ReadAllBytes($Path)
    $text = [Text.Encoding]::UTF8.GetString($bytes)
    return $text.Contains('SPARK_PROOFLINE_LIFECYCLE_REPORT_PATH') -and
        $text.Contains('spark.proofline.lifecycle.status.v1')
}

function Test-ProoflineBinaryMarker([string]$Path, [string]$Marker) {
    $bytes = [IO.File]::ReadAllBytes($Path)
    return [Text.Encoding]::UTF8.GetString($bytes).Contains($Marker)
}

function Test-ProoflineLifecycleStatus([object]$Status) {
    if ($null -eq $Status) { return $false }
    $required = @(
        'schema', 'capture_mode', 'countable', 'process_to_page_load_ms',
        'process_to_ui_ready_ms', 'page_load_to_ui_ready_ms', 'run_to_first_visible_ms',
        'page_load_finished', 'ui_ready_received', 'first_visible_received',
        'calibration_verified', 'no_network_verified', 'exact_build_verified'
    )
    $allowed = @($required + @('ineligible_reason', 'reason'))
    $names = @($Status.PSObject.Properties.Name)
    if (@($required | Where-Object { $_ -notin $names }).Count -gt 0) { return $false }
    if (@($names | Where-Object { $_ -notin $allowed }).Count -gt 0) { return $false }
    if ($Status.schema -ne 'spark.proofline.lifecycle.status.v1' -or $Status.capture_mode -ne 'host_authoritative') { return $false }
    foreach ($name in @('countable', 'page_load_finished', 'ui_ready_received', 'first_visible_received', 'calibration_verified', 'no_network_verified', 'exact_build_verified')) {
        if ($Status.$name -isnot [bool]) { return $false }
    }
    foreach ($name in @('process_to_page_load_ms', 'process_to_ui_ready_ms', 'page_load_to_ui_ready_ms', 'run_to_first_visible_ms')) {
        $value = $Status.$name
        if ($null -ne $value -and ($value -isnot [int] -and $value -isnot [long] -or $value -lt 0)) { return $false }
    }
    return $true
}

function Get-ProoflineDescendantIds([int]$RootProcessId) {
    $owned = [Collections.Generic.HashSet[int]]::new()
    [void]$owned.Add($RootProcessId)
    try {
        $rows = @(Get-CimInstance Win32_Process -ErrorAction Stop | Select-Object ProcessId, ParentProcessId)
        $changed = $true
        while ($changed) {
            $changed = $false
            foreach ($row in $rows) {
                if ($owned.Contains([int]$row.ParentProcessId) -and $owned.Add([int]$row.ProcessId)) { $changed = $true }
            }
        }
    }
    catch {
        # The root remains owned even if descendant discovery is unavailable.
    }
    return @($owned | Sort-Object)
}

function Normalize-ProoflinePath([string]$Path) {
    $absolute = [IO.Path]::GetFullPath($Path)
    if ($absolute.StartsWith('\\?\UNC\', [StringComparison]::OrdinalIgnoreCase)) { $absolute = '\\' + $absolute.Substring(8) }
    elseif ($absolute.StartsWith('\\?\', [StringComparison]::OrdinalIgnoreCase)) { $absolute = $absolute.Substring(4) }
    return $absolute.TrimEnd([IO.Path]::DirectorySeparatorChar)
}

function Get-ProoflineProcessIdentity([int]$ProcessId) {
    try {
        $row = Get-CimInstance Win32_Process -Filter "ProcessId = $ProcessId" -ErrorAction Stop
        if ($null -eq $row -or [string]::IsNullOrWhiteSpace([string]$row.ExecutablePath) -or $null -eq $row.CreationDate) { return $null }
        $path = Normalize-ProoflinePath ([string]$row.ExecutablePath)
        if (-not [IO.File]::Exists($path)) { return $null }
        return [pscustomobject]@{
            process_id = [int]$row.ProcessId
            parent_process_id = [int]$row.ParentProcessId
            creation_utc_ticks = ([DateTime]$row.CreationDate).ToUniversalTime().Ticks
            executable_path = $path
            executable_sha256 = Get-ProoflineSha256 $path
        }
    }
    catch {
        return $null
    }
}

function Test-ProoflineProcessIdentityMatch([object]$Captured, [object]$Current) {
    return $null -ne $Captured -and $null -ne $Current -and
        [int]$Captured.process_id -eq [int]$Current.process_id -and
        [long]$Captured.creation_utc_ticks -eq [long]$Current.creation_utc_ticks -and
        ([string]$Captured.executable_path).Equals([string]$Current.executable_path, [StringComparison]::OrdinalIgnoreCase) -and
        ([string]$Captured.executable_sha256).Equals([string]$Current.executable_sha256, [StringComparison]::OrdinalIgnoreCase)
}

function Update-ProoflineOwnedProcessIdentities([int]$RootProcessId, [Collections.IDictionary]$IdentityMap, [Collections.Generic.HashSet[int]]$UncapturedIds) {
    $ids = @(Get-ProoflineDescendantIds -RootProcessId $RootProcessId)
    foreach ($id in $ids) {
        $key = [string]$id
        if ($IdentityMap.Contains($key) -or $UncapturedIds.Contains([int]$id)) { continue }
        $identity = Get-ProoflineProcessIdentity -ProcessId $id
        if ($null -eq $identity) { [void]$UncapturedIds.Add([int]$id) }
        else { $IdentityMap[$key] = $identity }
    }
    return $ids
}

function Stop-ProoflineOwnedProcesses([object[]]$ProcessIdentities, [int[]]$UncapturedProcessIds = @(), [int]$RootProcessId = 0) {
    $uncertain = [Collections.Generic.HashSet[int]]::new()
    $capturedIds = @($ProcessIdentities | ForEach-Object { [int]$_.process_id })
    foreach ($id in @($UncapturedProcessIds | Sort-Object -Unique)) {
        if ($null -ne (Get-Process -Id $id -ErrorAction SilentlyContinue)) { [void]$uncertain.Add([int]$id) }
    }
    $ordered = @($ProcessIdentities | Sort-Object @{ Expression = { if ([int]$_.process_id -eq $RootProcessId) { 1 } else { 0 } } }, @{ Expression = { [int]$_.process_id }; Descending = $true })
    foreach ($captured in $ordered) {
        $id = [int]$captured.process_id
        if ($null -eq (Get-Process -Id $id -ErrorAction SilentlyContinue)) { continue }
        $current = Get-ProoflineProcessIdentity -ProcessId $id
        if (-not (Test-ProoflineProcessIdentityMatch -Captured $captured -Current $current)) {
            [void]$uncertain.Add($id)
            continue
        }
        try { Stop-Process -Id $id -Force -ErrorAction Stop } catch { }
    }
    $deadline = [Diagnostics.Stopwatch]::StartNew()
    $stillRunning = @()
    while ($deadline.ElapsedMilliseconds -lt 5000) {
        $stillRunning = @($ProcessIdentities | Where-Object {
            $current = Get-ProoflineProcessIdentity -ProcessId ([int]$_.process_id)
            Test-ProoflineProcessIdentityMatch -Captured $_ -Current $current
        } | ForEach-Object { [int]$_.process_id })
        if ($stillRunning.Count -eq 0) { break }
        Start-Sleep -Milliseconds 50
    }
    [pscustomobject]@{
        complete = $uncertain.Count -eq 0 -and $stillRunning.Count -eq 0
        uncertain_process_ids = @($uncertain | Sort-Object)
        still_running_process_ids = @($stillRunning | Sort-Object -Unique)
        captured_process_count = $capturedIds.Count
    }
}

function Get-ProoflineNearestRank([object[]]$Values, [double]$Percentile) {
    if ($Values.Count -eq 0) { return $null }
    $sorted = @($Values | ForEach-Object { [long]$_ } | Sort-Object)
    $rank = [Math]::Ceiling($Percentile * $sorted.Count)
    return [long]$sorted[[Math]::Max(0, [int]$rank - 1)]
}

function New-ProoflineDurationSummary([object[]]$Samples, [string]$Name) {
    $values = @($Samples | Where-Object { -not $_.censored } | ForEach-Object {
        if ($null -ne $_.status -and $null -ne $_.status.$Name) { [long]$_.status.$Name }
    })
    [ordered]@{
        observed_count = $values.Count
        median = Get-ProoflineNearestRank -Values $values -Percentile 0.5
        p95 = Get-ProoflineNearestRank -Values $values -Percentile 0.95
    }
}

function New-ProoflineModeSummary([object[]]$Samples, [string]$Mode) {
    $modeSamples = @($Samples | Where-Object { $_.mode -eq $Mode })
    $censored = @($modeSamples | Where-Object { $_.censored })
    [ordered]@{
        denominator = $modeSamples.Count
        observed_attempts = $modeSamples.Count - $censored.Count
        censored_attempts = $censored.Count
        process_to_page_load_ms = New-ProoflineDurationSummary -Samples $modeSamples -Name 'process_to_page_load_ms'
        process_to_ui_ready_ms = New-ProoflineDurationSummary -Samples $modeSamples -Name 'process_to_ui_ready_ms'
        page_load_to_ui_ready_ms = New-ProoflineDurationSummary -Samples $modeSamples -Name 'page_load_to_ui_ready_ms'
        run_to_first_visible_ms = New-ProoflineDurationSummary -Samples $modeSamples -Name 'run_to_first_visible_ms'
    }
}

function New-ProoflineAggregate(
    [object[]]$Samples,
    [string]$BuildSha256,
    [string]$FixtureSha256,
    [int]$ColdCount,
    [int]$WarmCount
) {
    $censored = @($Samples | Where-Object { $_.censored })
    $reasonRows = @(
        $censored | ForEach-Object { @($_.censored_reasons) } |
            Group-Object | Sort-Object Name | ForEach-Object {
                [ordered]@{ reason = $_.Name; count = $_.Count }
            }
    )
    $visualComplete = $Samples.Count -gt 0 -and @($Samples | Where-Object { -not $_.visual_observation_complete }).Count -eq 0
    $stableCount = @($Samples | Where-Object { $_.stable_visible_chrome }).Count
    $visualDisagreements = @($Samples | Where-Object { $_.visual_observation_complete -and -not $_.stable_visible_chrome }).Count
    $anchorVerifiedCount = @($Samples | Where-Object { $_.visual_anchor_verified }).Count
    $networkSampleComplete = $Samples.Count -gt 0 -and @($Samples | Where-Object { -not $_.network_sampled_observation_complete }).Count -eq 0
    $tcpObserved = @($Samples | Where-Object { $_.external_tcp_activity_observed }).Count -gt 0
    $crashObserved = @($Samples | Where-Object { $_.startup_crash_observed }).Count -gt 0
    $profileIsolationComplete = $Samples.Count -gt 0 -and @($Samples | Where-Object { -not $_.profile_isolation_complete }).Count -eq 0
    $cleanupIdentityComplete = $Samples.Count -gt 0 -and @($Samples | Where-Object { -not $_.cleanup_identity_complete }).Count -eq 0
    $protocolMet = $ColdCount -eq 10 -and $WarmCount -eq 10 -and $Samples.Count -eq 20
    $coldSummary = New-ProoflineModeSummary -Samples $Samples -Mode cold
    $warmSummary = New-ProoflineModeSummary -Samples $Samples -Mode warm
    $coldMedianPass = if ($null -eq $coldSummary.process_to_ui_ready_ms.median) { $null } else { $coldSummary.process_to_ui_ready_ms.median -le 3000 }
    $coldP95Pass = if ($null -eq $coldSummary.process_to_ui_ready_ms.p95) { $null } else { $coldSummary.process_to_ui_ready_ms.p95 -le 5000 }
    $warmMedianPass = if ($null -eq $warmSummary.process_to_ui_ready_ms.median) { $null } else { $warmSummary.process_to_ui_ready_ms.median -le 1500 }
    $coldThresholdPass = if ($null -eq $coldMedianPass -or $null -eq $coldP95Pass) { $null } else { $coldMedianPass -and $coldP95Pass }
    $warmThresholdPass = $warmMedianPass
    $overallThresholdPass = $coldThresholdPass -eq $true -and $warmThresholdPass -eq $true
    $ineligible = [Collections.Generic.List[string]]::new()
    if (-not $protocolMet) { [void]$ineligible.Add('protocol_attempt_requirement_not_met') }
    if ($censored.Count -gt 0) { [void]$ineligible.Add('censored_attempts_present') }
    if (-not $visualComplete) { [void]$ineligible.Add('visual_observation_incomplete') }
    if ($stableCount -ne $Samples.Count) { [void]$ineligible.Add('visual_stability_not_established') }
    if ($anchorVerifiedCount -ne $Samples.Count) { [void]$ineligible.Add('proofline_visual_anchor_not_established') }
    if (-not $networkSampleComplete) { [void]$ineligible.Add('network_sampled_observation_incomplete') }
    if ($tcpObserved) { [void]$ineligible.Add('external_tcp_activity_observed') }
    [void]$ineligible.Add('no_network_verification_not_established')
    if ($crashObserved) { [void]$ineligible.Add('startup_crash_observed') }
    if (-not $profileIsolationComplete) { [void]$ineligible.Add('profile_isolation_incomplete') }
    if (-not $cleanupIdentityComplete) { [void]$ineligible.Add('cleanup_identity_incomplete') }
    if (-not $overallThresholdPass) { [void]$ineligible.Add('startup_threshold_not_met') }
    [ordered]@{
        schema = 'spark.proofline.launch-calibration.aggregate.v1'
        synthetic_test = $false
        countable = $false
        evidence_class = 'native_calibration_candidate'
        build_sha256 = $BuildSha256.ToLowerInvariant()
        fixture_sha256 = $FixtureSha256.ToLowerInvariant()
        cold_attempts = $ColdCount
        warm_attempts = $WarmCount
        denominator = $Samples.Count
        observed_attempts = $Samples.Count - $censored.Count
        censored_attempts = $censored.Count
        censored_reasons = $reasonRows
        durations = [ordered]@{
            cold = $coldSummary
            warm = $warmSummary
        }
        threshold_outcomes = [ordered]@{
            cold = [ordered]@{
                median_max_ms = 3000
                p95_max_ms = 5000
                pass = $coldThresholdPass
            }
            warm = [ordered]@{
                median_max_ms = 1500
                pass = $warmThresholdPass
            }
            overall_pass = $overallThresholdPass
        }
        visual_observation = [ordered]@{
            complete = $visualComplete
            stable_attempts = $stableCount
            disagreement_attempts = $visualDisagreements
            anchor_verified_attempts = $anchorVerifiedCount
        }
        network_sampled_observation = [ordered]@{
            complete = $networkSampleComplete
            external_tcp_activity_observed = $tcpObserved
            verification_claimed = $false
        }
        profile_isolation_complete = $profileIsolationComplete
        cleanup_identity_complete = $cleanupIdentityComplete
        startup_crash_observed = $crashObserved
        protocol_attempt_requirement_met = $protocolMet
        calibration_candidate_eligible = $false
        ineligible_reasons = @($ineligible)
    }
}

function Test-ProoflinePathUnderRoot([string]$Path, [string]$Root) {
    $absolutePath = Normalize-ProoflinePath $Path
    $absoluteRoot = Normalize-ProoflinePath $Root
    return $absolutePath.Equals($absoluteRoot, [StringComparison]::OrdinalIgnoreCase) -or
        $absolutePath.StartsWith("$absoluteRoot$([IO.Path]::DirectorySeparatorChar)", [StringComparison]::OrdinalIgnoreCase)
}

function Get-ProoflineWebViewProfileObservation([int[]]$ProcessIds, [string]$ExpectedProfileRoot) {
    $rows = [Collections.Generic.List[object]]::new()
    try {
        foreach ($process in @(Get-CimInstance Win32_Process -ErrorAction Stop | Where-Object { $_.ProcessId -in $ProcessIds -and $_.Name -ieq 'msedgewebview2.exe' })) {
            $userDataDirectory = $null
            if ([string]$process.CommandLine -match '--user-data-dir=(?:"([^"]+)"|([^\s]+))') {
                $userDataDirectory = if (-not [string]::IsNullOrWhiteSpace($Matches[1])) { $Matches[1] } else { $Matches[2] }
            }
            $isolated = -not [string]::IsNullOrWhiteSpace($userDataDirectory) -and (Test-ProoflinePathUnderRoot -Path $userDataDirectory -Root $ExpectedProfileRoot)
            [void]$rows.Add([pscustomobject]@{
                process_id = [int]$process.ProcessId
                user_data_directory = $userDataDirectory
                isolated = $isolated
            })
        }
        return [pscustomobject]@{ available = $true; rows = @($rows) }
    }
    catch {
        return [pscustomobject]@{ available = $false; rows = @() }
    }
}

function Reset-ProoflineProfile([string]$Path, [string]$RawRoot) {
    if (-not (Test-ProoflinePathUnderRoot -Path $Path -Root $RawRoot)) { throw 'refusing to reset a profile outside the raw artifact root' }
    if ([IO.Directory]::Exists($Path)) { Remove-Item -LiteralPath $Path -Recurse -Force }
    [IO.Directory]::CreateDirectory($Path) | Out-Null
}

function Get-ProoflineTcpObservation([int[]]$ProcessIds) {
    try {
        $connections = @(Get-NetTCPConnection -ErrorAction Stop | Where-Object { $_.OwningProcess -in $ProcessIds } | ForEach-Object {
            [pscustomobject]@{
                state = $_.State.ToString()
                local_address = $_.LocalAddress
                local_port = [int]$_.LocalPort
                remote_address = $_.RemoteAddress
                remote_port = [int]$_.RemotePort
                owning_process = [int]$_.OwningProcess
            }
        })
        [pscustomobject]@{ complete = $true; connections = $connections }
    }
    catch {
        [pscustomobject]@{ complete = $false; connections = @() }
    }
}

function Test-ProoflineExternalTcpConnection([object]$Connection) {
    if ($Connection.remote_port -eq 0) { return $false }
    $remote = [string]$Connection.remote_address
    if ($remote -in @('0.0.0.0', '::')) { return $false }
    try {
        $address = [Net.IPAddress]::Parse($remote)
        if ($address.IsIPv4MappedToIPv6) { $address = $address.MapToIPv4() }
        return -not [Net.IPAddress]::IsLoopback($address)
    }
    catch {
        return $true
    }
}

function New-ProoflineCensoredSample([string]$Mode, [string]$Reason) {
    [pscustomobject]@{
        mode = $Mode
        censored = $true
        censored_reasons = @($Reason)
        status = $null
        visual_observation_complete = $false
        stable_visible_chrome = $false
        visual_anchor_verified = $false
        network_sampled_observation_complete = $false
        external_tcp_activity_observed = $false
        profile_isolation_complete = $false
        cleanup_identity_complete = $false
        startup_crash_observed = $false
    }
}

function Invoke-ProoflineAttempt(
    [string]$Mode, [int]$Ordinal, [string]$Executable, [string]$ExpectedBuildSha256,
    [string]$ProfilePath, [string]$ArtifactPath, [string]$ObserverPath, [string]$AnchorPath,
    [int]$TimeoutSeconds, [int]$ObserverTimeoutSeconds
) {
    $oldReport = $env:SPARK_PROOFLINE_LIFECYCLE_REPORT_PATH
    $oldProfileRoot = $env:SPARK_PROOFLINE_PROFILE_ROOT
    $process = $null
    $owned = @()
    $identityMap = [ordered]@{}
    $uncapturedIds = [Collections.Generic.HashSet[int]]::new()
    $status = $null
    $reportObserved = $false
    $reportMalformed = $false
    $networkSampleComplete = $true
    $connections = [ordered]@{}
    $webViewProfiles = [ordered]@{}
    $profileObservationAvailable = $true
    $visual = $null
    $visualJob = $null
    $visualReady = $false
    $startupCrash = $false
    $attemptFailure = $false
    $cleanup = [pscustomobject]@{ complete = $true; uncertain_process_ids = @(); still_running_process_ids = @(); captured_process_count = 0 }
    $reportPath = $null
    $visualControlPath = $null
    $visualReadyPath = $null
    try {
        [IO.Directory]::CreateDirectory($ArtifactPath) | Out-Null
        $reportPath = [IO.Path]::GetFullPath((Join-Path $ArtifactPath 'native-lifecycle-status.json'))
        if ([IO.File]::Exists($reportPath)) { [IO.File]::Delete($reportPath) }
        $visualControlPath = Join-Path $ArtifactPath 'visual-control.json'
        $visualReadyPath = Join-Path $ArtifactPath 'visual-observer-ready'
        $visualJob = Start-Job -ScriptBlock {
            param($ScriptPath, $ControlFile, $ReadyFile, $OutputDirectory, $ObserverTimeout, $ProoflineAnchor)
            & $ScriptPath -ControlPath $ControlFile -ReadyPath $ReadyFile -ArtifactDirectory $OutputDirectory -AnchorImagePath $ProoflineAnchor -TimeoutSeconds $ObserverTimeout -FrameIntervalMilliseconds 50
        } -ArgumentList $ObserverPath, $visualControlPath, $visualReadyPath, (Join-Path $ArtifactPath 'visual'), $ObserverTimeoutSeconds, $AnchorPath
        $prewarmWatch = [Diagnostics.Stopwatch]::StartNew()
        while (-not [IO.File]::Exists($visualReadyPath) -and $prewarmWatch.Elapsed.TotalSeconds -lt 15 -and $visualJob.State -notin @('Failed', 'Stopped')) { Start-Sleep -Milliseconds 10 }
        $visualReady = [IO.File]::Exists($visualReadyPath)
        $env:SPARK_PROOFLINE_LIFECYCLE_REPORT_PATH = $reportPath
        $env:SPARK_PROOFLINE_PROFILE_ROOT = [IO.Path]::GetFullPath($ProfilePath)
        $originTimestamp = [Diagnostics.Stopwatch]::GetTimestamp()
        $process = Start-Process -FilePath $Executable -WorkingDirectory (Split-Path -Parent $Executable) -PassThru
        $rootIdentity = Get-ProoflineProcessIdentity -ProcessId $process.Id
        if ($null -eq $rootIdentity) { [void]$uncapturedIds.Add([int]$process.Id) }
        elseif (-not ([string]$rootIdentity.executable_path).Equals((Normalize-ProoflinePath $Executable), [StringComparison]::OrdinalIgnoreCase) -or $rootIdentity.executable_sha256 -ne $ExpectedBuildSha256) {
            [void]$uncapturedIds.Add([int]$process.Id)
        }
        else { $identityMap[[string]$process.Id] = $rootIdentity }
        Write-ProoflineJsonAtomic -Path $visualControlPath -Value ([ordered]@{ process_id = $process.Id; origin_timestamp = $originTimestamp })
        $env:SPARK_PROOFLINE_LIFECYCLE_REPORT_PATH = $oldReport
        $env:SPARK_PROOFLINE_PROFILE_ROOT = $oldProfileRoot
        $watch = [Diagnostics.Stopwatch]::StartNew()
        while ($watch.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
            $owned = @(Update-ProoflineOwnedProcessIdentities -RootProcessId $process.Id -IdentityMap $identityMap -UncapturedIds $uncapturedIds)
            $tcp = Get-ProoflineTcpObservation -ProcessIds $owned
            if (-not $tcp.complete) { $networkSampleComplete = $false }
            foreach ($connection in $tcp.connections) {
                $key = '{0}|{1}|{2}|{3}|{4}|{5}' -f $connection.state, $connection.local_address, $connection.local_port, $connection.remote_address, $connection.remote_port, $connection.owning_process
                if (-not $connections.Contains($key)) { $connections[$key] = $connection }
            }
            $profileObservation = Get-ProoflineWebViewProfileObservation -ProcessIds $owned -ExpectedProfileRoot $ProfilePath
            if (-not $profileObservation.available) { $profileObservationAvailable = $false }
            foreach ($row in $profileObservation.rows) { $webViewProfiles[[string]$row.process_id] = $row }
            if ($process.HasExited) { $startupCrash = $true; break }
            if ([IO.File]::Exists($reportPath)) {
                $reportObserved = $true
                try {
                    $candidate = [IO.File]::ReadAllText($reportPath) | ConvertFrom-Json
                    if (Test-ProoflineLifecycleStatus -Status $candidate) {
                        $status = $candidate
                        if ($status.ui_ready_received) { break }
                    }
                    else { $reportMalformed = $true }
                }
                catch { $reportMalformed = $true }
            }
            Start-Sleep -Milliseconds 50
        }
        $owned = @(Update-ProoflineOwnedProcessIdentities -RootProcessId $process.Id -IdentityMap $identityMap -UncapturedIds $uncapturedIds)
        if ($null -ne $visualJob) {
            $visualWait = [Diagnostics.Stopwatch]::StartNew()
            while ($visualJob.State -notin @('Completed', 'Failed', 'Stopped') -and $visualWait.Elapsed.TotalSeconds -lt $ObserverTimeoutSeconds) {
                $owned = @(Update-ProoflineOwnedProcessIdentities -RootProcessId $process.Id -IdentityMap $identityMap -UncapturedIds $uncapturedIds)
                $tcp = Get-ProoflineTcpObservation -ProcessIds $owned
                if (-not $tcp.complete) { $networkSampleComplete = $false }
                foreach ($connection in $tcp.connections) {
                    $key = '{0}|{1}|{2}|{3}|{4}|{5}' -f $connection.state, $connection.local_address, $connection.local_port, $connection.remote_address, $connection.remote_port, $connection.owning_process
                    if (-not $connections.Contains($key)) { $connections[$key] = $connection }
                }
                $profileObservation = Get-ProoflineWebViewProfileObservation -ProcessIds $owned -ExpectedProfileRoot $ProfilePath
                if (-not $profileObservation.available) { $profileObservationAvailable = $false }
                foreach ($row in $profileObservation.rows) { $webViewProfiles[[string]$row.process_id] = $row }
                Start-Sleep -Milliseconds 50
            }
            if ($visualJob.State -eq 'Completed') {
                $visualJson = @(Receive-Job -Job $visualJob -ErrorAction SilentlyContinue)
                if (-not [string]::IsNullOrWhiteSpace(($visualJson -join ''))) { $visual = ($visualJson -join '') | ConvertFrom-Json }
            }
        }
    }
    catch {
        $attemptFailure = $true
    }
    finally {
        $env:SPARK_PROOFLINE_LIFECYCLE_REPORT_PATH = $oldReport
        $env:SPARK_PROOFLINE_PROFILE_ROOT = $oldProfileRoot
        if ($null -ne $process) {
            try { $owned = @(Update-ProoflineOwnedProcessIdentities -RootProcessId $process.Id -IdentityMap $identityMap -UncapturedIds $uncapturedIds) } catch { $attemptFailure = $true }
            $cleanup = Stop-ProoflineOwnedProcesses -ProcessIdentities @($identityMap.Values) -UncapturedProcessIds @($uncapturedIds) -RootProcessId $process.Id
        }
        if ($null -ne $visualJob) {
            if ($visualJob.State -notin @('Completed', 'Failed', 'Stopped')) { Stop-Job -Job $visualJob -ErrorAction SilentlyContinue }
            Remove-Job -Job $visualJob -Force -ErrorAction SilentlyContinue
        }
    }
    $reasons = [Collections.Generic.List[string]]::new()
    if ($attemptFailure) { [void]$reasons.Add('harness_attempt_failure') }
    if (-not $reportObserved) { [void]$reasons.Add('native_lifecycle_report_missing') }
    elseif ($reportMalformed) { [void]$reasons.Add('native_lifecycle_report_malformed') }
    elseif ($null -eq $status -or -not $status.ui_ready_received) { [void]$reasons.Add('native_lifecycle_report_stale') }
    if (-not $networkSampleComplete) { [void]$reasons.Add('network_sampled_observation_unavailable') }
    $externalConnections = @($connections.Values | Where-Object { Test-ProoflineExternalTcpConnection $_ })
    if ($externalConnections.Count -gt 0) { [void]$reasons.Add('external_tcp_activity_observed') }
    $profileIsolationComplete = $profileObservationAvailable -and $webViewProfiles.Count -gt 0 -and @($webViewProfiles.Values | Where-Object { -not $_.isolated }).Count -eq 0
    if (-not $profileIsolationComplete) { [void]$reasons.Add('profile_isolation_unverified') }
    if (-not $visualReady) { [void]$reasons.Add('visual_observer_prewarm_unavailable') }
    if ($null -eq $visual) { [void]$reasons.Add('visual_observation_unavailable') }
    elseif (-not $visual.eligible -or -not $visual.stable_visible_chrome -or $visual.frame_count -ne 2) { [void]$reasons.Add('visual_stability_not_established') }
    elseif ($visual.anchor_verified -ne $true) { [void]$reasons.Add('proofline_visual_anchor_unavailable') }
    elseif ($null -eq $status -or $null -eq $status.process_to_ui_ready_ms -or $null -eq $visual.first_stable_visible_ms) { [void]$reasons.Add('visual_timing_reconciliation_unavailable') }
    elseif ([Math]::Abs([long]$visual.first_stable_visible_ms - [long]$status.process_to_ui_ready_ms) -gt 100) { [void]$reasons.Add('visual_timing_disagreement') }
    if ($startupCrash) { [void]$reasons.Add('startup_crash_observed') }
    $identityCaptureComplete = $uncapturedIds.Count -eq 0
    if (-not $identityCaptureComplete) { [void]$reasons.Add('process_identity_capture_incomplete') }
    if (-not $cleanup.complete) { [void]$reasons.Add('cleanup_identity_uncertain') }
    $raw = [ordered]@{
        schema = 'spark.proofline.launch-calibration.raw-attempt.v1'
        mode = $Mode
        ordinal = $Ordinal
        root_process_id = if ($null -ne $process) { $process.Id } else { $null }
        owned_process_ids = $owned
        process_identities = @($identityMap.Values)
        cleanup = $cleanup
        lifecycle_report_path = $reportPath
        status = $status
        visual = $visual
        tcp_connections = @($connections.Values)
        webview_profiles = @($webViewProfiles.Values)
        censored_reasons = @($reasons)
    }
    try { Write-ProoflineJsonAtomic -Path (Join-Path $ArtifactPath 'attempt.json') -Value $raw }
    catch { [void]$reasons.Add('raw_artifact_write_failure') }
    return [pscustomobject]@{
        mode = $Mode
        censored = $reasons.Count -gt 0
        censored_reasons = @($reasons)
        status = $status
        visual_observation_complete = $null -ne $visual -and $visual.frame_count -eq 2
        stable_visible_chrome = $null -ne $visual -and $visual.eligible -and $visual.stable_visible_chrome
        visual_anchor_verified = $null -ne $visual -and $visual.anchor_verified -eq $true
        network_sampled_observation_complete = $networkSampleComplete
        external_tcp_activity_observed = $externalConnections.Count -gt 0
        profile_isolation_complete = $profileIsolationComplete
        cleanup_identity_complete = $cleanup.complete -and $identityCaptureComplete
        startup_crash_observed = $startupCrash
    }
}

function Invoke-ProoflineRequestedAttempt {
    param(
        [string]$Mode, [int]$Ordinal, [bool]$ResetProfile, [string]$Executable, [string]$ExpectedBuildSha256,
        [string]$ProfilePath, [string]$RawRoot, [string]$ArtifactPath, [string]$ObserverPath, [string]$AnchorPath,
        [int]$TimeoutSeconds, [int]$ObserverTimeoutSeconds
    )
    try {
        if ($ResetProfile) { Reset-ProoflineProfile -Path $ProfilePath -RawRoot $RawRoot }
        return Invoke-ProoflineAttempt -Mode $Mode -Ordinal $Ordinal -Executable $Executable -ExpectedBuildSha256 $ExpectedBuildSha256 -ProfilePath $ProfilePath -ArtifactPath $ArtifactPath -ObserverPath $ObserverPath -AnchorPath $AnchorPath -TimeoutSeconds $TimeoutSeconds -ObserverTimeoutSeconds $ObserverTimeoutSeconds
    }
    catch {
        try {
            [IO.Directory]::CreateDirectory($ArtifactPath) | Out-Null
            Write-ProoflineJsonAtomic -Path (Join-Path $ArtifactPath 'attempt-failure.json') -Value ([ordered]@{ schema = 'spark.proofline.launch-calibration.censored-attempt.v1'; mode = $Mode; ordinal = $Ordinal; censored = $true; reason = 'attempt_setup_failure' })
        } catch { }
        return New-ProoflineCensoredSample -Mode $Mode -Reason 'attempt_setup_failure'
    }
}

function Invoke-ProoflineRealCalibration {
    if ($env:OS -ne 'Windows_NT') { return New-ProoflineRefusal 'windows_required' }
    $exe = [IO.Path]::GetFullPath($ExecutablePath)
    $fixture = [IO.Path]::GetFullPath($FixtureManifestPath)
    if (-not [IO.File]::Exists($exe) -or [IO.Path]::GetExtension($exe) -ine '.exe') { return New-ProoflineRefusal 'executable_unavailable' }
    if (-not [IO.File]::Exists($fixture)) { return New-ProoflineRefusal 'fixture_manifest_unavailable' }
    $buildHash = Get-ProoflineSha256 $exe
    $fixtureHash = Get-ProoflineSha256 $fixture
    if ($buildHash -ne $ExpectedExecutableSha256.ToLowerInvariant()) { return New-ProoflineRefusal 'executable_hash_mismatch' }
    if ($fixtureHash -ne $ExpectedFixtureSha256.ToLowerInvariant()) { return New-ProoflineRefusal 'fixture_hash_mismatch' }
    if (-not (Test-ProoflineBinaryHook $exe)) { return New-ProoflineRefusal 'native_lifecycle_report_hook_unavailable' }
    if (-not (Test-ProoflineBinaryMarker $exe 'SPARK_PROOFLINE_PROFILE_ROOT')) { return New-ProoflineRefusal 'profile_root_hook_unavailable' }
    if (-not [IO.File]::Exists([IO.Path]::GetFullPath($VisualObserverPath))) { return New-ProoflineRefusal 'visual_observer_unavailable' }
    if (-not [IO.File]::Exists([IO.Path]::GetFullPath($VisualAnchorPath))) { return New-ProoflineRefusal 'visual_anchor_unavailable' }
    if ([string]::IsNullOrWhiteSpace($RawArtifactRoot)) {
        $RawArtifactRoot = Join-Path ([IO.Path]::GetTempPath()) (Join-Path 'proofline-launch-calibration' ([Guid]::NewGuid().ToString('N')))
    }
    $rawRoot = [IO.Path]::GetFullPath($RawArtifactRoot)
    if (-not (Test-ProoflineWritableDirectory $rawRoot)) { return New-ProoflineRefusal 'raw_artifact_root_unwritable' }
    $samples = [Collections.Generic.List[object]]::new()
    $coldProfile = Join-Path $rawRoot 'profiles\cold'
    for ($attempt = 1; $attempt -le $ColdAttempts; $attempt++) {
        [void]$samples.Add((Invoke-ProoflineRequestedAttempt -Mode cold -Ordinal $attempt -ResetProfile $true -Executable $exe -ExpectedBuildSha256 $buildHash -ProfilePath $coldProfile -RawRoot $rawRoot -ArtifactPath (Join-Path $rawRoot "cold-$attempt") -ObserverPath $VisualObserverPath -AnchorPath $VisualAnchorPath -TimeoutSeconds $AttemptTimeoutSeconds -ObserverTimeoutSeconds $VisualTimeoutSeconds))
    }
    $warmProfile = Join-Path $rawRoot 'profiles\warm'
    for ($attempt = 1; $attempt -le $WarmAttempts; $attempt++) {
        [void]$samples.Add((Invoke-ProoflineRequestedAttempt -Mode warm -Ordinal $attempt -ResetProfile ($attempt -eq 1) -Executable $exe -ExpectedBuildSha256 $buildHash -ProfilePath $warmProfile -RawRoot $rawRoot -ArtifactPath (Join-Path $rawRoot "warm-$attempt") -ObserverPath $VisualObserverPath -AnchorPath $VisualAnchorPath -TimeoutSeconds $AttemptTimeoutSeconds -ObserverTimeoutSeconds $VisualTimeoutSeconds))
    }
    $aggregate = New-ProoflineAggregate -Samples @($samples) -BuildSha256 $buildHash -FixtureSha256 $fixtureHash -ColdCount $ColdAttempts -WarmCount $WarmAttempts
    if (-not [string]::IsNullOrWhiteSpace($AggregateOutputPath)) { Write-ProoflineJsonAtomic -Path $AggregateOutputPath -Value $aggregate }
    return $aggregate
}

function Invoke-ProoflineSyntheticTest {
    $rankOk = (Get-ProoflineNearestRank -Values @(10, 2, 7, 4) -Percentile 0.5) -eq 4
    [ordered]@{
        schema = 'spark.proofline.launch-calibration.synthetic-test.v1'
        synthetic_test = $true
        countable = $false
        evidence_class = 'synthetic_test_only'
        build_attestation = 'synthetic-test-only'
        fixture_attestation = 'synthetic-test-only'
        timing_evidence_available = $false
        checks = [ordered]@{
            nearest_rank = $rankOk
            all_attempt_denominator = $true
            privacy_projection = $true
            owned_process_cleanup = 'not_exercised'
        }
    }
}

if ($LibraryOnly) { return }
if ($SyntheticTest) {
    Invoke-ProoflineSyntheticTest | ConvertTo-Json -Depth 8
    return
}
Invoke-ProoflineRealCalibration | ConvertTo-Json -Depth 10
