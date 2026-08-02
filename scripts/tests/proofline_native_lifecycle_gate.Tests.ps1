$ErrorActionPreference = 'Stop'
$scriptPath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\proofline_native_lifecycle_gate.ps1'))

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

. $scriptPath -LibraryOnly

$synthetic = & $scriptPath -SyntheticTest | ConvertFrom-Json
Assert-True ($synthetic.schema -eq 'spark.proofline.native-lifecycle-gate.synthetic-test.v1') 'synthetic schema changed'
Assert-True ($synthetic.countable -eq $false) 'synthetic result became countable'
Assert-True ($synthetic.privacy_gate -eq 'pending') 'sampled TCP approved privacy'
Assert-True ($synthetic.checks.all_attempts_retained -eq $true) 'synthetic denominator was dropped'

$sample = [pscustomobject]@{
    mode = 'cold'; censored = $false; censored_reasons = @(); first_uia_anchor_ms = 200
    uia_anchor_observed = $true; tcp_observation_complete = $true; non_loopback_tcp_observed = $false
    root_identity_verified = $true; process_identity_capture_complete = $true
    process_tree_observation_complete = $true; cleanup_complete = $true
}
$fiveByFive = New-ProoflineNativeAggregate -Samples @($sample, $sample) -BuildSha256 ('b' * 64) -ColdCount 5 -WarmCount 5
Assert-True ($fiveByFive.denominator -eq 2) 'aggregate dropped attempts'
Assert-True ($fiveByFive.privacy_gate -eq 'pending') 'five by five sampled TCP approved privacy'
Assert-True ($fiveByFive.release_protocol.eligible -eq $false) 'five by five became release eligible'
Assert-True ($fiveByFive.evidence.event_based_network_evidence -eq $false) 'sampled TCP was labeled event based'

$tenByTen = New-ProoflineNativeAggregate -Samples @($sample) -BuildSha256 ('c' * 64) -ColdCount 10 -WarmCount 10
Assert-True ($tenByTen.release_protocol.ten_cold_ten_warm_attempts -eq $false) 'incomplete ten by ten passed protocol'
Assert-True ($tenByTen.release_protocol.eligible -eq $false) 'incomplete ten by ten became eligible'

$warmSample = [pscustomobject]@{
    mode = 'warm'; censored = $false; censored_reasons = @(); first_uia_anchor_ms = 90
    uia_anchor_observed = $true; tcp_observation_complete = $true; non_loopback_tcp_observed = $false
    root_identity_verified = $true; process_identity_capture_complete = $true
    process_tree_observation_complete = $true; cleanup_complete = $true
}
$censoredSample = [pscustomobject]@{
    mode = 'warm'; censored = $true; censored_reasons = @('root_binary_identity_unverified'); first_uia_anchor_ms = $null
    uia_anchor_observed = $false; tcp_observation_complete = $false; non_loopback_tcp_observed = $false
    root_identity_verified = $false; process_identity_capture_complete = $false
    process_tree_observation_complete = $false; cleanup_complete = $true
}
$mixed = New-ProoflineNativeAggregate -Samples @($sample, $warmSample, $censoredSample) -BuildSha256 ('d' * 64) -ColdCount 1 -WarmCount 2
Assert-True ($mixed.denominator -eq 3) 'censored attempt left the denominator'
Assert-True ($mixed.observed_attempts -eq 2) 'observed attempt count is wrong'
Assert-True ($mixed.censored_attempts -eq 1) 'censored attempt count is wrong'
Assert-True ($mixed.timings.warm.denominator -eq 2) 'warm denominator dropped the censored attempt'
Assert-True ($mixed.timings.warm.first_uia_anchor_ms.observed_count -eq 1) 'warm anchor count is wrong'
Assert-True ($mixed.evidence.exact_binary_identity_complete -eq $false) 'unverified identity was reported complete'
Assert-True ($mixed.evidence.process_identity_capture_complete -eq $false) 'incomplete process identity capture was reported complete'
Assert-True ($mixed.evidence.process_tree_observation_complete -eq $false) 'incomplete process tree observation was reported complete'
Assert-True ($mixed.evidence.tcp_observation_complete -eq $false) 'incomplete TCP observation was reported complete'
Assert-True ('censored_attempts_present' -in $mixed.release_protocol.blocked_reasons) 'censored attempts were not a release blocker'
Assert-True ('tcp_observation_incomplete' -in $mixed.release_protocol.blocked_reasons) 'incomplete TCP was not a release blocker'

'proofline_native_lifecycle_gate tests passed'
