#requires -PSEdition Desktop

[CmdletBinding(DefaultParameterSetName = 'Real')]
param(
    [Parameter(ParameterSetName = 'Real', Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$ExecutablePath,

    [Parameter(ParameterSetName = 'Real', Mandatory = $true)]
    [ValidatePattern('^[a-fA-F0-9]{64}$')]
    [string]$ExpectedExecutableSha256,

    [Parameter(ParameterSetName = 'Real')]
    [ValidateRange(1, 100)]
    [int]$ColdAttempts = 5,

    [Parameter(ParameterSetName = 'Real')]
    [ValidateRange(1, 100)]
    [int]$WarmAttempts = 5,

    [Parameter(ParameterSetName = 'Real')]
    [ValidateNotNullOrEmpty()]
    [string]$AnchorName = 'Proofline for Spark',

    [Parameter(ParameterSetName = 'Real')]
    [ValidateRange(1, 120)]
    [int]$AttemptTimeoutSeconds = 20,

    [Parameter(ParameterSetName = 'Real')]
    [ValidateRange(100, 10000)]
    [int]$NetworkObservationMilliseconds = 1000,

    [Parameter(ParameterSetName = 'Real')]
    [string]$RawArtifactRoot,

    [Parameter(ParameterSetName = 'Real')]
    [string]$AggregateOutputPath,

    [Parameter(ParameterSetName = 'Synthetic', Mandatory = $true)]
    [switch]$SyntheticTest,

    [Parameter(ParameterSetName = 'Library', Mandatory = $true)]
    [switch]$LibraryOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-ProoflineNativeJsonAtomic([string]$Path, [object]$Value) {
    $absolute = [IO.Path]::GetFullPath($Path)
    $parent = Split-Path -Parent $absolute
    [IO.Directory]::CreateDirectory($parent) | Out-Null
    $temporary = "$absolute.tmp-$([Guid]::NewGuid().ToString('N'))"
    [IO.File]::WriteAllText($temporary, ($Value | ConvertTo-Json -Depth 12), [Text.UTF8Encoding]::new($false))
    if ([IO.File]::Exists($absolute)) {
        $backup = "$absolute.bak-$([Guid]::NewGuid().ToString('N'))"
        try { [IO.File]::Replace($temporary, $absolute, $backup) }
        catch {
            if ([IO.File]::Exists($temporary)) { [IO.File]::Delete($temporary) }
            throw
        }
        finally { if ([IO.File]::Exists($backup)) { [IO.File]::Delete($backup) } }
    }
    else { [IO.File]::Move($temporary, $absolute) }
}

function Get-ProoflineNativeSha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Normalize-ProoflineNativePath([string]$Path) {
    $absolute = [IO.Path]::GetFullPath($Path)
    if ($absolute.StartsWith('\\?\UNC\', [StringComparison]::OrdinalIgnoreCase)) { $absolute = '\\' + $absolute.Substring(8) }
    elseif ($absolute.StartsWith('\\?\', [StringComparison]::OrdinalIgnoreCase)) { $absolute = $absolute.Substring(4) }
    return $absolute.TrimEnd([IO.Path]::DirectorySeparatorChar)
}

function Get-ProoflineNativeProcessIdentity([int]$ProcessId) {
    try {
        $row = Get-CimInstance Win32_Process -Filter "ProcessId = $ProcessId" -ErrorAction Stop
        if ($null -eq $row -or [string]::IsNullOrWhiteSpace([string]$row.ExecutablePath) -or $null -eq $row.CreationDate) { return $null }
        $path = Normalize-ProoflineNativePath ([string]$row.ExecutablePath)
        if (-not [IO.File]::Exists($path)) { return $null }
        return [pscustomobject]@{
            process_id = [int]$row.ProcessId
            parent_process_id = [int]$row.ParentProcessId
            creation_utc_ticks = ([DateTime]$row.CreationDate).ToUniversalTime().Ticks
            executable_path = $path
            executable_sha256 = Get-ProoflineNativeSha256 $path
        }
    }
    catch { return $null }
}

function Test-ProoflineNativeProcessIdentity([object]$Captured, [object]$Current) {
    return $null -ne $Captured -and $null -ne $Current -and
        [int]$Captured.process_id -eq [int]$Current.process_id -and
        [long]$Captured.creation_utc_ticks -eq [long]$Current.creation_utc_ticks -and
        ([string]$Captured.executable_path).Equals([string]$Current.executable_path, [StringComparison]::OrdinalIgnoreCase) -and
        ([string]$Captured.executable_sha256).Equals([string]$Current.executable_sha256, [StringComparison]::OrdinalIgnoreCase)
}

function Get-ProoflineNativeDescendantIds(
    [int]$RootProcessId,
    [object]$RootIdentity,
    [Collections.Generic.List[string]]$ObservationFailures
) {
    $owned = [Collections.Generic.HashSet[int]]::new()
    [void]$owned.Add($RootProcessId)
    try {
        $rows = @(Get-CimInstance Win32_Process -ErrorAction Stop | Select-Object ProcessId, ParentProcessId, CreationDate)
        $createdAt = @{}
        foreach ($row in $rows) {
            if ($null -ne $row.CreationDate) {
                $createdAt[[int]$row.ProcessId] = ([DateTime]$row.CreationDate).ToUniversalTime().Ticks
            }
        }
        if ($null -ne $RootIdentity) { $createdAt[$RootProcessId] = [long]$RootIdentity.creation_utc_ticks }
        if (-not $createdAt.ContainsKey($RootProcessId)) {
            [void]$ObservationFailures.Add('process_tree_root_creation_time_unavailable')
            return @($owned | Sort-Object)
        }
        $changed = $true
        while ($changed) {
            $changed = $false
            foreach ($row in $rows) {
                $parentId = [int]$row.ParentProcessId
                $childId = [int]$row.ProcessId
                if ($owned.Contains($parentId) -and $createdAt.ContainsKey($parentId) -and $createdAt.ContainsKey($childId) -and
                    [long]$createdAt[$childId] -ge [long]$createdAt[$parentId] -and $owned.Add($childId)) {
                    $changed = $true
                }
            }
        }
    }
    catch { [void]$ObservationFailures.Add('process_tree_cim_enumeration_failed') }
    return @($owned | Sort-Object)
}

function Update-ProoflineNativeProcessTree(
    [int]$RootProcessId,
    [Collections.IDictionary]$Identities,
    [Collections.Generic.HashSet[int]]$UncapturedIds,
    [Collections.Generic.HashSet[int]]$PendingIdentityIds,
    [Collections.Generic.List[string]]$ObservationFailures
) {
    $rootIdentity = $Identities[[string]$RootProcessId]
    if ($null -eq $rootIdentity) { $rootIdentity = Get-ProoflineNativeProcessIdentity $RootProcessId }
    $ids = @(Get-ProoflineNativeDescendantIds -RootProcessId $RootProcessId -RootIdentity $rootIdentity -ObservationFailures $ObservationFailures)
    foreach ($id in $ids) {
        $key = [string]$id
        if ($Identities.Contains($key)) { continue }
        $identity = Get-ProoflineNativeProcessIdentity $id
        if ($null -eq $identity) {
            if ($null -ne (Get-Process -Id $id -ErrorAction SilentlyContinue)) { [void]$PendingIdentityIds.Add($id) }
            else { [void]$UncapturedIds.Add($id) }
        }
        else {
            [void]$PendingIdentityIds.Remove($id)
            [void]$UncapturedIds.Remove($id)
            $Identities[$key] = $identity
        }
    }
    return $ids
}

function Test-ProoflineNativeNonLoopback([object]$Connection) {
    if ([int]$Connection.remote_port -eq 0) { return $false }
    try {
        $address = [Net.IPAddress]::Parse([string]$Connection.remote_address)
        if ($address.IsIPv4MappedToIPv6) { $address = $address.MapToIPv4() }
        return -not [Net.IPAddress]::IsLoopback($address)
    }
    catch { return $true }
}

function Get-ProoflineNativeTcpSample([int[]]$ProcessIds) {
    try {
        $connections = @(Get-NetTCPConnection -ErrorAction Stop | Where-Object { $_.OwningProcess -in $ProcessIds } | ForEach-Object {
            [pscustomobject]@{
                state = $_.State.ToString()
                local_address = [string]$_.LocalAddress
                local_port = [int]$_.LocalPort
                remote_address = [string]$_.RemoteAddress
                remote_port = [int]$_.RemotePort
                owning_process = [int]$_.OwningProcess
            }
        })
        return [pscustomobject]@{ available = $true; connections = $connections }
    }
    catch { return [pscustomobject]@{ available = $false; connections = @() } }
}

function Find-ProoflineNativeUiAutomationAnchor([IntPtr]$WindowHandle, [string]$ExpectedAnchor) {
    try {
        Add-Type -AssemblyName UIAutomationClient -ErrorAction Stop
        Add-Type -AssemblyName UIAutomationTypes -ErrorAction Stop
        $window = [System.Windows.Automation.AutomationElement]::FromHandle($WindowHandle)
        if ($null -eq $window) { return [pscustomobject]@{ available = $true; observed = $false; matched_name = $null } }
        $windowName = [string]$window.Current.Name
        if ($windowName.IndexOf($ExpectedAnchor, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            return [pscustomobject]@{ available = $true; observed = $true; matched_name = $windowName }
        }
        $condition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty, $ExpectedAnchor)
        $element = $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
        if ($null -ne $element) { return [pscustomobject]@{ available = $true; observed = $true; matched_name = [string]$element.Current.Name } }
        return [pscustomobject]@{ available = $true; observed = $false; matched_name = $null }
    }
    catch { return [pscustomobject]@{ available = $false; observed = $false; matched_name = $null } }
}

function Stop-ProoflineNativeProcessTree(
    [object[]]$ProcessIdentities,
    [int[]]$UncapturedProcessIds,
    [int]$RootProcessId,
    [Collections.Generic.List[string]]$ObservationFailures
) {
    $uncertain = [Collections.Generic.HashSet[int]]::new()
    foreach ($id in $UncapturedProcessIds) {
        if ($null -ne (Get-Process -Id $id -ErrorAction SilentlyContinue)) { [void]$uncertain.Add($id) }
    }
    $root = @($ProcessIdentities | Where-Object { [int]$_.process_id -eq $RootProcessId })
    $descendants = @($ProcessIdentities | Where-Object { [int]$_.process_id -ne $RootProcessId } | Sort-Object { [int]$_.process_id })
    foreach ($captured in @($root + $descendants)) {
        $id = [int]$captured.process_id
        if ($null -eq (Get-Process -Id $id -ErrorAction SilentlyContinue)) { continue }
        if (-not (Test-ProoflineNativeProcessIdentity $captured (Get-ProoflineNativeProcessIdentity $id))) {
            [void]$uncertain.Add($id)
            continue
        }
        try { Stop-Process -Id $id -Force -ErrorAction Stop } catch { [void]$uncertain.Add($id) }
    }
    $rootIdentity = $root | Select-Object -First 1
    $rescannedIds = @(Get-ProoflineNativeDescendantIds -RootProcessId $RootProcessId -RootIdentity $rootIdentity -ObservationFailures $ObservationFailures)
    $knownById = [ordered]@{}
    foreach ($identity in $ProcessIdentities) { $knownById[[string]$identity.process_id] = $identity }
    foreach ($id in $rescannedIds) {
        $key = [string]$id
        if ($knownById.Contains($key)) { continue }
        $identity = Get-ProoflineNativeProcessIdentity $id
        if ($null -eq $identity) { [void]$uncertain.Add($id) }
        else { $knownById[$key] = $identity; try { Stop-Process -Id $id -Force -ErrorAction Stop } catch { [void]$uncertain.Add($id) } }
    }
    $deadline = [Diagnostics.Stopwatch]::StartNew()
    $stillRunning = @()
    while ($deadline.Elapsed.TotalSeconds -lt 5) {
        $stillRunning = @($knownById.Values | Where-Object {
            Test-ProoflineNativeProcessIdentity $_ (Get-ProoflineNativeProcessIdentity ([int]$_.process_id))
        } | ForEach-Object { [int]$_.process_id })
        if ($stillRunning.Count -eq 0) { break }
        Start-Sleep -Milliseconds 100
    }
    return [pscustomobject]@{
        complete = $uncertain.Count -eq 0 -and $stillRunning.Count -eq 0
        uncertain_process_ids = @($uncertain | Sort-Object)
        still_running_process_ids = @($stillRunning | Sort-Object -Unique)
    }
}

function New-ProoflineNativeAttempt(
    [string]$Mode,
    [int]$Ordinal,
    [string]$Executable,
    [string]$ExpectedSha256,
    [string]$Anchor,
    [int]$TimeoutSeconds,
    [int]$ConnectionObservationMilliseconds,
    [string]$ArtifactDirectory
) {
    $process = $null
    $identities = [ordered]@{}
    $uncaptured = [Collections.Generic.HashSet[int]]::new()
    $pendingIdentity = [Collections.Generic.HashSet[int]]::new()
    $treeObservationFailures = [Collections.Generic.List[string]]::new()
    $connections = [ordered]@{}
    $tcpAvailable = $true
    $firstWindowHandleMs = $null
    $firstUiaAnchorMs = $null
    $uiaAvailable = $null
    $uiaAnchor = $null
    $startupCrash = $false
    $observedProcessIds = @()
    $nextAnchorProbeMs = 0
    $rootIdentityVerifiedBeforeCleanup = $false
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $reasons = [Collections.Generic.List[string]]::new()
    $cleanup = [pscustomobject]@{ complete = $false; uncertain_process_ids = @(); still_running_process_ids = @() }
    try {
        [IO.Directory]::CreateDirectory($ArtifactDirectory) | Out-Null
        $process = Start-Process -FilePath $Executable -PassThru -ErrorAction Stop
        while ($clock.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
            $observedProcessIds = @(Update-ProoflineNativeProcessTree -RootProcessId $process.Id -Identities $identities -UncapturedIds $uncaptured -PendingIdentityIds $pendingIdentity -ObservationFailures $treeObservationFailures)
            $sample = Get-ProoflineNativeTcpSample -ProcessIds $observedProcessIds
            if (-not $sample.available) { $tcpAvailable = $false }
            foreach ($connection in $sample.connections) {
                $key = '{0}|{1}|{2}|{3}|{4}|{5}' -f $connection.state, $connection.local_address, $connection.local_port, $connection.remote_address, $connection.remote_port, $connection.owning_process
                $connections[$key] = $connection
            }
            if ($process.HasExited) { $startupCrash = $true; break }
            $process.Refresh()
            if ($firstWindowHandleMs -eq $null -and $process.MainWindowHandle -ne 0) {
                $firstWindowHandleMs = [long]$clock.ElapsedMilliseconds
            }
            if ($firstWindowHandleMs -ne $null -and $firstUiaAnchorMs -eq $null -and $clock.ElapsedMilliseconds -ge $nextAnchorProbeMs) {
                $nextAnchorProbeMs = $clock.ElapsedMilliseconds + 150
                $uia = Find-ProoflineNativeUiAutomationAnchor -WindowHandle ([IntPtr]$process.MainWindowHandle) -ExpectedAnchor $Anchor
                $uiaAvailable = $uia.available
                if ($uia.observed) {
                    $firstUiaAnchorMs = [long]$clock.ElapsedMilliseconds
                    $uiaAnchor = $uia.matched_name
                    $until = $clock.ElapsedMilliseconds + $ConnectionObservationMilliseconds
                    while ($clock.ElapsedMilliseconds -lt $until) {
                        $observedProcessIds = @(Update-ProoflineNativeProcessTree -RootProcessId $process.Id -Identities $identities -UncapturedIds $uncaptured -PendingIdentityIds $pendingIdentity -ObservationFailures $treeObservationFailures)
                        $sample = Get-ProoflineNativeTcpSample -ProcessIds $observedProcessIds
                        if (-not $sample.available) { $tcpAvailable = $false }
                        foreach ($connection in $sample.connections) {
                            $key = '{0}|{1}|{2}|{3}|{4}|{5}' -f $connection.state, $connection.local_address, $connection.local_port, $connection.remote_address, $connection.remote_port, $connection.owning_process
                            $connections[$key] = $connection
                        }
                        Start-Sleep -Milliseconds 25
                    }
                    break
                }
            }
            Start-Sleep -Milliseconds 25
        }
    }
    catch { [void]$reasons.Add('attempt_start_or_observation_failure') }
    finally {
        if ($null -ne $process) {
            $observedProcessIds = @(Update-ProoflineNativeProcessTree -RootProcessId $process.Id -Identities $identities -UncapturedIds $uncaptured -PendingIdentityIds $pendingIdentity -ObservationFailures $treeObservationFailures)
            $capturedRoot = $identities[[string]$process.Id]
            $currentRoot = Get-ProoflineNativeProcessIdentity $process.Id
            $rootIdentityVerifiedBeforeCleanup = Test-ProoflineNativeProcessIdentity $capturedRoot $currentRoot
            $unverifiedIds = @($uncaptured | Sort-Object) + @($pendingIdentity | Sort-Object)
            $cleanup = Stop-ProoflineNativeProcessTree -ProcessIdentities @($identities.Values) -UncapturedProcessIds $unverifiedIds -RootProcessId $process.Id -ObservationFailures $treeObservationFailures
        }
    }
    $rootIdentity = if ($null -eq $process) { $null } else { $identities[[string]$process.Id] }
    $rootIdentityMatches = $rootIdentityVerifiedBeforeCleanup
    if ($null -eq $process) { [void]$reasons.Add('process_not_started') }
    if (-not $rootIdentityMatches -or $null -eq $rootIdentity -or $rootIdentity.executable_sha256 -ne $ExpectedSha256) { [void]$reasons.Add('root_binary_identity_unverified') }
    if ($firstWindowHandleMs -eq $null) { [void]$reasons.Add('first_window_handle_not_observed') }
    if ($uiaAvailable -ne $true) { [void]$reasons.Add('uia_anchor_observer_unavailable') }
    elseif ($firstUiaAnchorMs -eq $null) { [void]$reasons.Add('named_uia_anchor_not_observed') }
    if (-not $tcpAvailable) { [void]$reasons.Add('tcp_table_observation_unavailable') }
    $externalConnections = @($connections.Values | Where-Object { Test-ProoflineNativeNonLoopback $_ })
    if ($externalConnections.Count -gt 0) { [void]$reasons.Add('non_loopback_tcp_observed') }
    if ($startupCrash) { [void]$reasons.Add('process_exited_before_anchor') }
    if ($uncaptured.Count -gt 0 -or $pendingIdentity.Count -gt 0) { [void]$reasons.Add('process_identity_capture_incomplete') }
    if ($treeObservationFailures.Count -gt 0) { [void]$reasons.Add('process_tree_observation_incomplete') }
    if (-not $cleanup.complete) { [void]$reasons.Add('safe_cleanup_incomplete') }
    $raw = [ordered]@{
        schema = 'spark.proofline.native-lifecycle-gate.attempt.v1'
        mode = $Mode
        ordinal = $Ordinal
        binary_sha256 = $ExpectedSha256
        root_process_id = if ($null -eq $process) { $null } else { $process.Id }
        process_identities = @($identities.Values)
        uncaptured_process_ids = @($uncaptured | Sort-Object)
        pending_identity_process_ids = @($pendingIdentity | Sort-Object)
        process_tree_observation_failures = @($treeObservationFailures | Sort-Object -Unique)
        root_identity_verified_before_cleanup = $rootIdentityVerifiedBeforeCleanup
        first_window_handle_ms = $firstWindowHandleMs
        first_uia_anchor_ms = $firstUiaAnchorMs
        uia_anchor_name = $uiaAnchor
        visual_proof_claimed = $false
        timing_limitations = @(
            'window_handle_is_not_a_pixel_or_stable-frame_proof',
            'uia_anchor_is_an_accessibility-tree_observation_not_a_visual-frame-proof'
        )
        network_observation = [ordered]@{
            method = 'sampled_windows_tcp_table'
            event_based = $false
            enforcement_claimed = $false
            complete = $tcpAvailable
            non_loopback_tcp_observed = $externalConnections.Count -gt 0
            connections = @($connections.Values)
        }
        cleanup = $cleanup
        censored_reasons = @($reasons)
    }
    Write-ProoflineNativeJsonAtomic -Path (Join-Path $ArtifactDirectory 'attempt.json') -Value $raw
    return [pscustomobject]@{
        mode = $Mode
        censored = $reasons.Count -gt 0
        censored_reasons = @($reasons)
        first_window_handle_ms = $firstWindowHandleMs
        first_uia_anchor_ms = $firstUiaAnchorMs
        uia_anchor_observed = $firstUiaAnchorMs -ne $null
        tcp_observation_complete = $tcpAvailable
        non_loopback_tcp_observed = $externalConnections.Count -gt 0
        root_identity_verified = $rootIdentityMatches -and $null -ne $rootIdentity -and $rootIdentity.executable_sha256 -eq $ExpectedSha256
        process_identity_capture_complete = $uncaptured.Count -eq 0 -and $pendingIdentity.Count -eq 0
        process_tree_observation_complete = $treeObservationFailures.Count -eq 0
        cleanup_complete = $cleanup.complete
    }
}

function Get-ProoflineNativeNearestRank([object[]]$Values, [double]$Percentile) {
    if ($Values.Count -eq 0) { return $null }
    $sorted = @($Values | ForEach-Object { [long]$_ } | Sort-Object)
    return [long]$sorted[[Math]::Max(0, [Math]::Ceiling($Percentile * $sorted.Count) - 1)]
}

function Get-ProoflineNativeModeSummary([object[]]$Samples, [string]$Mode) {
    $modeSamples = @($Samples | Where-Object { $_.mode -eq $Mode })
    $anchorTimes = @($modeSamples | Where-Object { -not $_.censored -and $null -ne $_.first_uia_anchor_ms } | ForEach-Object { $_.first_uia_anchor_ms })
    return [ordered]@{
        denominator = $modeSamples.Count
        observed_attempts = @($modeSamples | Where-Object { -not $_.censored }).Count
        censored_attempts = @($modeSamples | Where-Object { $_.censored }).Count
        first_uia_anchor_ms = [ordered]@{
            observed_count = $anchorTimes.Count
            median = Get-ProoflineNativeNearestRank -Values $anchorTimes -Percentile 0.5
            p95 = Get-ProoflineNativeNearestRank -Values $anchorTimes -Percentile 0.95
        }
    }
}

function New-ProoflineNativeAggregate([object[]]$Samples, [string]$BuildSha256, [int]$ColdCount, [int]$WarmCount) {
    $censored = @($Samples | Where-Object { $_.censored })
    $reasons = @($censored | ForEach-Object { @($_.censored_reasons) } | Group-Object | Sort-Object Name | ForEach-Object { [ordered]@{ reason = $_.Name; count = $_.Count } })
    $protocolTenByTen = $ColdCount -eq 10 -and $WarmCount -eq 10 -and $Samples.Count -eq 20
    $allAnchor = $Samples.Count -gt 0 -and @($Samples | Where-Object { -not $_.uia_anchor_observed }).Count -eq 0
    $allIdentity = $Samples.Count -gt 0 -and @($Samples | Where-Object { -not $_.root_identity_verified }).Count -eq 0
    $allIdentityCapture = $Samples.Count -gt 0 -and @($Samples | Where-Object { -not $_.process_identity_capture_complete }).Count -eq 0
    $allTreeObservation = $Samples.Count -gt 0 -and @($Samples | Where-Object { -not $_.process_tree_observation_complete }).Count -eq 0
    $allTcpObserved = $Samples.Count -gt 0 -and @($Samples | Where-Object { -not $_.tcp_observation_complete }).Count -eq 0
    $externalObserved = @($Samples | Where-Object { $_.non_loopback_tcp_observed }).Count -gt 0
    $allCleanup = $Samples.Count -gt 0 -and @($Samples | Where-Object { -not $_.cleanup_complete }).Count -eq 0
    $blocked = [Collections.Generic.List[string]]::new()
    [void]$blocked.Add('event_based_or_enforceable_network_boundary_not_established')
    [void]$blocked.Add('stable_visual_frame_not_established')
    if (-not $protocolTenByTen) { [void]$blocked.Add('protocol_attempt_requirement_not_met') }
    if ($censored.Count -gt 0) { [void]$blocked.Add('censored_attempts_present') }
    if (-not $allIdentity) { [void]$blocked.Add('exact_binary_identity_incomplete') }
    if (-not $allIdentityCapture) { [void]$blocked.Add('process_identity_capture_incomplete') }
    if (-not $allTreeObservation) { [void]$blocked.Add('process_tree_observation_incomplete') }
    if (-not $allTcpObserved) { [void]$blocked.Add('tcp_observation_incomplete') }
    if ($externalObserved) { [void]$blocked.Add('non_loopback_tcp_observed') }
    if (-not $allCleanup) { [void]$blocked.Add('safe_cleanup_incomplete') }
    return [ordered]@{
        schema = 'spark.proofline.native-lifecycle-gate.aggregate.v1'
        synthetic_test = $false
        countable = $false
        binary_sha256 = $BuildSha256
        requested_attempts = [ordered]@{ cold = $ColdCount; warm = $WarmCount }
        mode_definitions = [ordered]@{
            cold = 'fresh process launch; no OS cache purge or application-profile reset is claimed'
            warm = 'subsequent launch of the same binary in the same diagnostic session; retained OS cache is possible'
        }
        denominator = $Samples.Count
        observed_attempts = $Samples.Count - $censored.Count
        censored_attempts = $censored.Count
        censored_reasons = $reasons
        timings = [ordered]@{
            cold = Get-ProoflineNativeModeSummary -Samples $Samples -Mode cold
            warm = Get-ProoflineNativeModeSummary -Samples $Samples -Mode warm
        }
        evidence = [ordered]@{
            exact_binary_identity_complete = $allIdentity
            process_identity_capture_complete = $allIdentityCapture
            process_tree_observation_complete = $allTreeObservation
            safe_cleanup_complete = $allCleanup
            named_uia_anchor_complete = $allAnchor
            stable_visual_frame_proven = $false
            network_observation_method = 'sampled_windows_tcp_table'
            event_based_network_evidence = $false
            network_enforcement_claimed = $false
            tcp_observation_complete = $allTcpObserved
            non_loopback_tcp_observed = $externalObserved
        }
        release_protocol = [ordered]@{
            ten_cold_ten_warm_attempts = $protocolTenByTen
            eligible = $false
            blocked_reasons = @($blocked)
        }
        privacy_gate = 'pending'
        limitations = @(
            'sampled_tcp_table_cannot_prove_no_short_lived_connection',
            'sampled_tcp_table_is_not_enforcement',
            'window_handle_and_uia_anchor_are_not_stable_pixel-frame_proof'
        )
    }
}

function Invoke-ProoflineNativeLifecycleGate {
    if ($env:OS -ne 'Windows_NT') { throw 'windows_required' }
    $exe = [IO.Path]::GetFullPath($ExecutablePath)
    if (-not [IO.File]::Exists($exe) -or [IO.Path]::GetExtension($exe) -ine '.exe') { throw 'executable_unavailable' }
    $buildSha = Get-ProoflineNativeSha256 $exe
    if ($buildSha -ne $ExpectedExecutableSha256.ToLowerInvariant()) { throw 'executable_hash_mismatch' }
    if ([string]::IsNullOrWhiteSpace($RawArtifactRoot)) {
        $RawArtifactRoot = Join-Path ([IO.Path]::GetTempPath()) (Join-Path 'proofline-native-lifecycle-gate' ([Guid]::NewGuid().ToString('N')))
    }
    $rawRoot = [IO.Path]::GetFullPath($RawArtifactRoot)
    [IO.Directory]::CreateDirectory($rawRoot) | Out-Null
    $samples = [Collections.Generic.List[object]]::new()
    foreach ($mode in @('cold', 'warm')) {
        $count = if ($mode -eq 'cold') { $ColdAttempts } else { $WarmAttempts }
        for ($ordinal = 1; $ordinal -le $count; $ordinal++) {
            $artifactPath = Join-Path $rawRoot "$mode-$ordinal"
            try {
                [void]$samples.Add((New-ProoflineNativeAttempt -Mode $mode -Ordinal $ordinal -Executable $exe -ExpectedSha256 $buildSha -Anchor $AnchorName -TimeoutSeconds $AttemptTimeoutSeconds -ConnectionObservationMilliseconds $NetworkObservationMilliseconds -ArtifactDirectory $artifactPath))
            }
            catch {
                [IO.Directory]::CreateDirectory($artifactPath) | Out-Null
                $failure = [pscustomobject]@{ mode = $mode; censored = $true; censored_reasons = @('attempt_harness_failure'); first_window_handle_ms = $null; first_uia_anchor_ms = $null; uia_anchor_observed = $false; tcp_observation_complete = $false; non_loopback_tcp_observed = $false; root_identity_verified = $false; process_identity_capture_complete = $false; process_tree_observation_complete = $false; cleanup_complete = $false }
                Write-ProoflineNativeJsonAtomic -Path (Join-Path $artifactPath 'attempt-failure.json') -Value ([ordered]@{ schema = 'spark.proofline.native-lifecycle-gate.censored-attempt.v1'; mode = $mode; ordinal = $ordinal; reason = 'attempt_harness_failure' })
                [void]$samples.Add($failure)
            }
        }
    }
    $aggregate = New-ProoflineNativeAggregate -Samples @($samples) -BuildSha256 $buildSha -ColdCount $ColdAttempts -WarmCount $WarmAttempts
    if (-not [string]::IsNullOrWhiteSpace($AggregateOutputPath)) { Write-ProoflineNativeJsonAtomic -Path $AggregateOutputPath -Value $aggregate }
    return $aggregate
}

function Invoke-ProoflineNativeLifecycleSyntheticTest {
    $samples = @(
        [pscustomobject]@{ mode = 'cold'; censored = $false; censored_reasons = @(); first_uia_anchor_ms = 100; uia_anchor_observed = $true; tcp_observation_complete = $true; non_loopback_tcp_observed = $false; root_identity_verified = $true; process_identity_capture_complete = $true; process_tree_observation_complete = $true; cleanup_complete = $true },
        [pscustomobject]@{ mode = 'warm'; censored = $false; censored_reasons = @(); first_uia_anchor_ms = 50; uia_anchor_observed = $true; tcp_observation_complete = $true; non_loopback_tcp_observed = $false; root_identity_verified = $true; process_identity_capture_complete = $true; process_tree_observation_complete = $true; cleanup_complete = $true }
    )
    $aggregate = New-ProoflineNativeAggregate -Samples $samples -BuildSha256 ('a' * 64) -ColdCount 1 -WarmCount 1
    return [ordered]@{
        schema = 'spark.proofline.native-lifecycle-gate.synthetic-test.v1'
        synthetic_test = $true
        countable = $false
        privacy_gate = $aggregate.privacy_gate
        checks = [ordered]@{
            sampled_network_never_approves_privacy = $aggregate.privacy_gate -eq 'pending'
            short_protocol_is_not_release_eligible = -not $aggregate.release_protocol.ten_cold_ten_warm_attempts
            all_attempts_retained = $aggregate.denominator -eq 2
        }
    }
}

if ($LibraryOnly) { return }
if ($SyntheticTest) {
    Invoke-ProoflineNativeLifecycleSyntheticTest | ConvertTo-Json -Depth 12
    return
}
Invoke-ProoflineNativeLifecycleGate | ConvertTo-Json -Depth 12
