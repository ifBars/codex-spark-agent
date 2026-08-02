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
    root_identity_verified = $true; cleanup_complete = $true
}
$fiveByFive = New-ProoflineNativeAggregate -Samples @($sample, $sample) -BuildSha256 ('b' * 64) -ColdCount 5 -WarmCount 5
Assert-True ($fiveByFive.denominator -eq 2) 'aggregate dropped attempts'
Assert-True ($fiveByFive.privacy_gate -eq 'pending') 'five by five sampled TCP approved privacy'
Assert-True ($fiveByFive.release_protocol.eligible -eq $false) 'five by five became release eligible'
Assert-True ($fiveByFive.evidence.event_based_network_evidence -eq $false) 'sampled TCP was labeled event based'

$tenByTen = New-ProoflineNativeAggregate -Samples @($sample) -BuildSha256 ('c' * 64) -ColdCount 10 -WarmCount 10
Assert-True ($tenByTen.release_protocol.ten_cold_ten_warm_attempts -eq $false) 'incomplete ten by ten passed protocol'
Assert-True ($tenByTen.release_protocol.eligible -eq $false) 'incomplete ten by ten became eligible'

'proofline_native_lifecycle_gate tests passed'
