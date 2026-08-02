[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$scriptPath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\proofline_launch_calibration.ps1'))
. $scriptPath -LibraryOnly

$passed = 0
$failed = 0

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Invoke-Test([string]$Name, [scriptblock]$Body) {
    try {
        & $Body
        $script:passed++
        Write-Host "PASS $Name"
    }
    catch {
        $script:failed++
        Write-Host "FAIL $Name -- $($_.Exception.Message)"
    }
}

function New-TestStatus([int]$Ready, [int]$Page = 100, [int]$Visible = 20) {
    [pscustomobject]@{
        schema = 'spark.proofline.lifecycle.status.v1'
        capture_mode = 'host_authoritative'
        countable = $false
        process_to_page_load_ms = $Page
        process_to_ui_ready_ms = $Ready
        page_load_to_ui_ready_ms = $Ready - $Page
        run_to_first_visible_ms = $Visible
        page_load_finished = $true
        ui_ready_received = $true
        first_visible_received = $true
        calibration_verified = $false
        no_network_verified = $false
        exact_build_verified = $false
    }
}

function New-TestSample([string]$Mode, [int]$Ready, [bool]$Censored = $false, [string[]]$Reasons = @()) {
    [pscustomobject]@{
        mode = $Mode
        censored = $Censored
        censored_reasons = $Reasons
        status = New-TestStatus -Ready $Ready
        visual_observation_complete = -not $Censored
        stable_visible_chrome = -not $Censored
        visual_anchor_verified = -not $Censored
        network_sampled_observation_complete = $true
        external_tcp_activity_observed = $false
        profile_isolation_complete = $true
        cleanup_identity_complete = $true
        startup_crash_observed = $false
    }
}

Invoke-Test 'nearest-rank is deterministic' {
    Assert-True ((Get-ProoflineNearestRank -Values @(10, 1, 9, 2, 8, 3, 7, 4, 6, 5) -Percentile 0.5) -eq 5) 'median rank mismatch'
    Assert-True ((Get-ProoflineNearestRank -Values @(1, 2, 3, 4, 5, 6, 7, 8, 9, 10) -Percentile 0.95) -eq 10) 'p95 rank mismatch'
    Assert-True ($null -eq (Get-ProoflineNearestRank -Values @() -Percentile 0.95)) 'empty input must remain null'
}

Invoke-Test 'aggregate keeps cold and warm bands separate' {
    $samples = @(
        New-TestSample cold 1000
        New-TestSample cold 3000
        New-TestSample warm 500
        New-TestSample warm 1500
    )
    $aggregate = New-ProoflineAggregate -Samples $samples -BuildSha256 ('a' * 64) -FixtureSha256 ('b' * 64) -ColdCount 2 -WarmCount 2
    Assert-True ($aggregate.denominator -eq 4) 'overall denominator mismatch'
    Assert-True ($aggregate.durations.cold.denominator -eq 2) 'cold denominator mismatch'
    Assert-True ($aggregate.durations.warm.denominator -eq 2) 'warm denominator mismatch'
    Assert-True ($aggregate.durations.cold.process_to_ui_ready_ms.median -eq 1000) 'cold median mismatch'
    Assert-True ($aggregate.durations.warm.process_to_ui_ready_ms.p95 -eq 1500) 'warm p95 mismatch'
    Assert-True ($aggregate.threshold_outcomes.cold.median_max_ms -eq 3000 -and $aggregate.threshold_outcomes.cold.p95_max_ms -eq 5000) 'cold threshold contract mismatch'
    Assert-True ($aggregate.threshold_outcomes.warm.median_max_ms -eq 1500) 'warm threshold contract mismatch'
    Assert-True ($aggregate.threshold_outcomes.overall_pass) 'expected threshold gate to pass'
    Assert-True ($aggregate.profile_isolation_complete) 'expected isolated profiles'
    Assert-True (-not $aggregate.calibration_candidate_eligible) 'sampled network polling must not establish candidate eligibility'
    Assert-True (-not $aggregate.network_sampled_observation.verification_claimed) 'sampled network evidence claimed verification'
}

Invoke-Test 'censored attempts remain in every denominator' {
    $samples = @(
        New-TestSample cold 1000
        New-TestSample cold 2000 $true @('visual_stability_not_established')
        New-TestSample warm 500
    )
    $aggregate = New-ProoflineAggregate -Samples $samples -BuildSha256 ('a' * 64) -FixtureSha256 ('b' * 64) -ColdCount 2 -WarmCount 1
    Assert-True ($aggregate.denominator -eq 3 -and $aggregate.censored_attempts -eq 1 -and $aggregate.observed_attempts -eq 2) 'overall censored denominator mismatch'
    Assert-True ($aggregate.durations.cold.denominator -eq 2 -and $aggregate.durations.cold.censored_attempts -eq 1) 'cold censored denominator mismatch'
    Assert-True ($aggregate.durations.cold.process_to_ui_ready_ms.observed_count -eq 1) 'censored duration entered observed statistics'
    Assert-True ($aggregate.durations.cold.process_to_ui_ready_ms.median -eq 1000) 'censored duration changed the median'
    Assert-True ($aggregate.censored_reasons[0].reason -eq 'visual_stability_not_established' -and $aggregate.censored_reasons[0].count -eq 1) 'censored reason count mismatch'
}

Invoke-Test 'aggregate projection excludes raw and identifying fields' {
    $aggregate = New-ProoflineAggregate -Samples @(
        New-TestSample cold 1000
        New-TestSample warm 500
    ) -BuildSha256 ('a' * 64) -FixtureSha256 ('b' * 64) -ColdCount 1 -WarmCount 1
    $json = $aggregate | ConvertTo-Json -Depth 12 -Compress
    foreach ($forbidden in @('launch_id', 'timestamp', 'process_id', 'window_handle', 'local_address', 'remote_address', 'url', 'command', 'path', 'screenshot', 'sentinel-secret.example')) {
        Assert-True ($json.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -lt 0) "aggregate leaked $forbidden"
    }
    $schema = Get-Content -LiteralPath (Join-Path $PSScriptRoot '..\schemas\proofline-launch-aggregate.schema.json') -Raw | ConvertFrom-Json
    Assert-True ($schema.additionalProperties -eq $false) 'aggregate schema must reject extra top-level fields'
    Assert-True ($schema.properties.durations.additionalProperties -eq $false) 'duration schema must reject extra bands'
}

Invoke-Test 'synthetic mode is structurally non-countable' {
    $synthetic = Invoke-ProoflineSyntheticTest
    $json = $synthetic | ConvertTo-Json -Depth 8 -Compress
    Assert-True ($synthetic.synthetic_test -and -not $synthetic.countable -and -not $synthetic.timing_evidence_available) 'synthetic classification mismatch'
    Assert-True ($synthetic.schema -ne 'spark.proofline.launch-calibration.aggregate.v1') 'synthetic output reused real schema'
    Assert-True ($json -notmatch '[a-f0-9]{64}') 'synthetic output contains a real-looking SHA'
    Assert-True ($json -notmatch '"[^"]+_ms"\s*:') 'synthetic output contains timing evidence fields'
}

Invoke-Test 'censored extreme values cannot change nearest-rank summaries' {
    $samples = @(
        New-TestSample cold 100
        New-TestSample cold 200
        New-TestSample cold 999999 $true @('native_lifecycle_report_stale')
    )
    $aggregate = New-ProoflineAggregate -Samples $samples -BuildSha256 ('a' * 64) -FixtureSha256 ('b' * 64) -ColdCount 3 -WarmCount 0
    Assert-True ($aggregate.durations.cold.process_to_ui_ready_ms.observed_count -eq 2) 'censored extreme was counted'
    Assert-True ($aggregate.durations.cold.process_to_ui_ready_ms.median -eq 100) 'censored extreme changed median'
    Assert-True ($aggregate.durations.cold.process_to_ui_ready_ms.p95 -eq 200) 'censored extreme changed p95'
}

Invoke-Test 'atomic JSON output supports replacement on Windows PowerShell' {
    $temporary = Join-Path ([IO.Path]::GetTempPath()) "proofline-atomic-test-$([Guid]::NewGuid().ToString('N')).json"
    try {
        Write-ProoflineJsonAtomic -Path $temporary -Value ([ordered]@{ value = 'first' })
        Write-ProoflineJsonAtomic -Path $temporary -Value ([ordered]@{ value = 'second' })
        $value = Get-Content -LiteralPath $temporary -Raw | ConvertFrom-Json
        Assert-True ($value.value -eq 'second') 'atomic replacement retained stale content'
        Assert-True (@(Get-ChildItem -LiteralPath (Split-Path -Parent $temporary) -Filter "$(Split-Path -Leaf $temporary).tmp-*" -ErrorAction SilentlyContinue).Count -eq 0) 'atomic replacement left a temporary file'
    }
    finally {
        if ([IO.File]::Exists($temporary)) { Remove-Item -LiteralPath $temporary -Force }
    }
}

Invoke-Test 'network classification separates loopback from external endpoints' {
    $loopback = [pscustomobject]@{ remote_address = '127.0.0.1'; remote_port = 43123 }
    $mappedLoopback = [pscustomobject]@{ remote_address = '::ffff:127.0.0.1'; remote_port = 43123 }
    $unspecified = [pscustomobject]@{ remote_address = '::'; remote_port = 0 }
    $external = [pscustomobject]@{ remote_address = '203.0.113.20'; remote_port = 443 }
    Assert-True (-not (Test-ProoflineExternalTcpConnection $loopback)) 'loopback was classified external'
    Assert-True (-not (Test-ProoflineExternalTcpConnection $mappedLoopback)) 'mapped loopback was classified external'
    Assert-True (-not (Test-ProoflineExternalTcpConnection $unspecified)) 'bound socket was classified external'
    Assert-True (Test-ProoflineExternalTcpConnection $external) 'remote endpoint was not classified external'
}

Invoke-Test 'profile roots reject sibling and prefix-confusion paths' {
    $root = 'C:\proofline-test\profiles\cold'
    Assert-True (Test-ProoflinePathUnderRoot -Path $root -Root $root) 'exact profile root was rejected'
    Assert-True (Test-ProoflinePathUnderRoot -Path (Join-Path $root 'EBWebView') -Root $root) 'profile child was rejected'
    Assert-True (Test-ProoflinePathUnderRoot -Path '\\?\C:\proofline-test\profiles\cold\EBWebView' -Root $root) 'Win32 long-path profile child was rejected'
    Assert-True (-not (Test-ProoflinePathUnderRoot -Path 'C:\proofline-test\profiles\cold-escape\EBWebView' -Root $root)) 'prefix-confusion path was accepted'
    Assert-True (-not (Test-ProoflinePathUnderRoot -Path 'C:\proofline-test\profiles\warm\EBWebView' -Root $root)) 'sibling profile was accepted'
}

Invoke-Test 'real mode refuses a binary without the native hook before launch' {
    $temporary = Join-Path ([IO.Path]::GetTempPath()) "proofline-refusal-test-$([Guid]::NewGuid().ToString('N'))"
    [IO.Directory]::CreateDirectory($temporary) | Out-Null
    try {
        $dummyExe = Join-Path $temporary 'Proofline.exe'
        $fixture = Join-Path $temporary 'fixture.json'
        [IO.File]::WriteAllText($dummyExe, 'not an executable and no lifecycle hook', [Text.UTF8Encoding]::new($false))
        [IO.File]::WriteAllText($fixture, '{"schema":"test-only"}', [Text.UTF8Encoding]::new($false))
        $raw = Join-Path $temporary 'raw-should-not-exist'
        $resultJson = & $scriptPath -ExecutablePath $dummyExe -ExpectedExecutableSha256 (Get-ProoflineSha256 $dummyExe) -FixtureManifestPath $fixture -ExpectedFixtureSha256 (Get-ProoflineSha256 $fixture) -RawArtifactRoot $raw
        $result = ($resultJson -join '') | ConvertFrom-Json
        Assert-True ($result.ineligible_reason -eq 'native_lifecycle_report_hook_unavailable') 'unexpected refusal reason'
        Assert-True (-not [IO.Directory]::Exists($raw)) 'refusal must happen before artifact creation or process launch'
        [IO.File]::WriteAllText($dummyExe, 'SPARK_PROOFLINE_LIFECYCLE_REPORT_PATH spark.proofline.lifecycle.status.v1', [Text.UTF8Encoding]::new($false))
        $profileRefusalJson = & $scriptPath -ExecutablePath $dummyExe -ExpectedExecutableSha256 (Get-ProoflineSha256 $dummyExe) -FixtureManifestPath $fixture -ExpectedFixtureSha256 (Get-ProoflineSha256 $fixture) -RawArtifactRoot $raw
        $profileRefusal = ($profileRefusalJson -join '') | ConvertFrom-Json
        Assert-True ($profileRefusal.ineligible_reason -eq 'profile_root_hook_unavailable') 'missing profile hook was not refused before launch'
        Assert-True (-not [IO.Directory]::Exists($raw)) 'profile-hook refusal created artifacts'
    }
    finally {
        if ([IO.Directory]::Exists($temporary)) { Remove-Item -LiteralPath $temporary -Recurse -Force }
    }
}

Invoke-Test 'owned-process cleanup is PID scoped and does not require admin' {
    $child = Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -ArgumentList @('-NoProfile', '-Command', 'Start-Sleep -Seconds 30') -PassThru
    try {
        $identity = Get-ProoflineProcessIdentity -ProcessId $child.Id
        $cleanup = Stop-ProoflineOwnedProcesses -ProcessIdentities @($identity) -RootProcessId $child.Id
        Assert-True ($cleanup.complete) 'cleanup reported a failure'
        Assert-True ($null -eq (Get-Process -Id $child.Id -ErrorAction SilentlyContinue)) 'owned process survived cleanup'
        Assert-True ($null -ne (Get-Process -Id $PID -ErrorAction SilentlyContinue)) 'cleanup affected the test process'
    }
    finally {
        if ($null -ne (Get-Process -Id $child.Id -ErrorAction SilentlyContinue)) { Stop-Process -Id $child.Id -Force }
    }
}

Invoke-Test 'owned-process cleanup accepts already-exited and racing children' {
    $exited = Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -ArgumentList @('-NoProfile', '-Command', 'Start-Sleep -Milliseconds 100') -PassThru
    $exitedIdentity = Get-ProoflineProcessIdentity -ProcessId $exited.Id
    $exited.WaitForExit()
    Assert-True ((Stop-ProoflineOwnedProcesses -ProcessIdentities @($exitedIdentity) -RootProcessId $exited.Id).complete) 'already-exited child was reported as a cleanup failure'
    $racing = Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -ArgumentList @('-NoProfile', '-Command', 'Start-Sleep -Milliseconds 50') -PassThru
    $racingIdentity = Get-ProoflineProcessIdentity -ProcessId $racing.Id
    Start-Sleep -Milliseconds 25
    Assert-True ((Stop-ProoflineOwnedProcesses -ProcessIdentities @($racingIdentity) -RootProcessId $racing.Id).complete) 'racing child was reported as a cleanup failure'
}

Invoke-Test 'cleanup refuses mismatched PID identity without terminating it' {
    $current = Get-ProoflineProcessIdentity -ProcessId $PID
    $mismatched = $current.PSObject.Copy()
    $mismatched.creation_utc_ticks = [long]$current.creation_utc_ticks - 1
    $cleanup = Stop-ProoflineOwnedProcesses -ProcessIdentities @($mismatched) -RootProcessId $PID
    Assert-True (-not $cleanup.complete -and $PID -in $cleanup.uncertain_process_ids) 'PID-reuse uncertainty was not reported'
    Assert-True ($null -ne (Get-Process -Id $PID -ErrorAction SilentlyContinue)) 'identity-mismatched process was terminated'
}

Invoke-Test 'requested attempt failures remain one censored row each' {
    $temporary = Join-Path ([IO.Path]::GetTempPath()) "proofline-attempt-failure-$([Guid]::NewGuid().ToString('N'))"
    [IO.Directory]::CreateDirectory($temporary) | Out-Null
    try {
        $setupFailure = Invoke-ProoflineRequestedAttempt -Mode cold -Ordinal 1 -ResetProfile $true -Executable 'missing.exe' -ExpectedBuildSha256 ('a' * 64) -ProfilePath 'C:\outside-profile' -RawRoot $temporary -ArtifactPath (Join-Path $temporary 'cold-1') -ObserverPath 'missing-observer.ps1' -AnchorPath 'missing-anchor.png' -TimeoutSeconds 1 -ObserverTimeoutSeconds 1
        $startupFailure = Invoke-ProoflineRequestedAttempt -Mode warm -Ordinal 1 -ResetProfile $false -Executable (Join-Path $temporary 'missing.exe') -ExpectedBuildSha256 ('a' * 64) -ProfilePath (Join-Path $temporary 'profile') -RawRoot $temporary -ArtifactPath (Join-Path $temporary 'warm-1') -ObserverPath (Join-Path $temporary 'missing-observer.ps1') -AnchorPath (Join-Path $temporary 'missing-anchor.png') -TimeoutSeconds 1 -ObserverTimeoutSeconds 1
        $aggregate = New-ProoflineAggregate -Samples @($setupFailure, $startupFailure) -BuildSha256 ('a' * 64) -FixtureSha256 ('b' * 64) -ColdCount 1 -WarmCount 1
        Assert-True ($aggregate.denominator -eq 2 -and $aggregate.censored_attempts -eq 2) 'failed requested attempts disappeared from the denominator'
        Assert-True (@($setupFailure).Count -eq 1 -and @($startupFailure).Count -eq 1) 'a failed request emitted multiple sample rows'
        $failureJson = $setupFailure | ConvertTo-Json -Depth 6 -Compress
        Assert-True ($failureJson -notmatch 'exception|stack|command|path') 'censored sample leaked failure internals'
        $failureRow = Get-Content -LiteralPath (Join-Path $temporary 'cold-1\attempt-failure.json') -Raw | ConvertFrom-Json
        Assert-True (@($failureRow.PSObject.Properties.Name | Where-Object { $_ -notin @('schema', 'mode', 'ordinal', 'censored', 'reason') }).Count -eq 0) 'failure artifact contains non-allowlisted fields'
    }
    finally {
        if ([IO.Directory]::Exists($temporary)) { Remove-Item -LiteralPath $temporary -Recurse -Force }
    }
}

Invoke-Test 'all public schemas reject additional top-level fields' {
    foreach ($file in Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot '..\schemas') -Filter 'proofline-*.schema.json') {
        $schema = Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json
        Assert-True ($schema.additionalProperties -eq $false) "$($file.Name) permits additional fields"
    }
}

Invoke-Test 'visual readiness loop avoids WMI process enumeration' {
    $observerSource = Get-Content -LiteralPath (Join-Path $PSScriptRoot '..\proofline_visual_observer.ps1') -Raw
    Assert-True ($observerSource -notmatch 'Get-CimInstance|Win32_Process') 'visual observer performs WMI work in its readiness path'
    Assert-True ($observerSource -match 'GetAncestor') 'visual observer lacks HWND-root occlusion validation'
    Assert-True ($observerSource -match 'AnchorImagePath' -and $observerSource -match 'anchor_verified') 'visual observer lacks a Proofline image-anchor gate'
    Assert-True ($observerSource.IndexOf('$secondFrameTimestamp') -lt $observerSource.IndexOf('Get-ProoflineAnchorSignal -FramePath $firstPath')) 'anchor analysis time contaminates stable-frame timing'
    $runnerSource = Get-Content -LiteralPath (Join-Path $PSScriptRoot '..\proofline_launch_calibration.ps1') -Raw
    Assert-True ($runnerSource.IndexOf('proofline_visual_anchor_unavailable') -lt $runnerSource.IndexOf('visual_timing_reconciliation_unavailable')) 'unanchored frames can reach readiness reconciliation'
}

Write-Host "RESULT passed=$passed failed=$failed"
if ($failed -gt 0) { exit 1 }
