param(
    [string]$Model = "gpt-5.3-codex-spark",
    [ValidateSet("minimal", "low", "medium", "high")]
    [string]$ReasoningEffort = "medium",
    [int]$Repeat = 1,
    [int]$MaxTurns = 45,
    [ValidateSet("core", "survey", "scaffolding", "editing", "real-world")]
    [string]$Suite = "real-world",
    [string[]]$Scenario = @(),
    [switch]$ListScenarios,
    [switch]$NoTrace
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$BenchmarkDir = Join-Path $RepoRoot ".spark-profile\benchmarks"
. (Join-Path $PSScriptRoot "quick_benchmark_scenarios.ps1")

if (-not $Scenario -or $Scenario.Count -eq 0) {
    $Scenario = @(Get-QuickRealWorldScenario)
}

if ($ListScenarios) {
    $Scenario | ForEach-Object { Write-Output $_ }
    return
}

Write-Host "benchmark_suite=$Suite"
Write-Host "benchmark_model=$Model"
Write-Host "reasoning_effort=$ReasoningEffort"
Write-Host "repeat=$Repeat"
Write-Host "max_turns=$MaxTurns"
Write-Host "scenario_count=$($Scenario.Count)"
Write-Host "scenarios=$($Scenario -join ',')"

function Get-LatestGeneratedFile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Directory,
        [Parameter(Mandatory = $true)]
        [string]$Filter,
        [Parameter(Mandatory = $true)]
        [datetime]$SinceUtc
    )

    $file = Get-ChildItem -LiteralPath $Directory -Filter $Filter -File -ErrorAction SilentlyContinue |
        Where-Object { $_.LastWriteTimeUtc -ge $SinceUtc } |
        Sort-Object LastWriteTimeUtc, Name |
        Select-Object -Last 1
    if (-not $file) {
        throw "Expected generated file '$Filter' under '$Directory' after $($SinceUtc.ToString('o')), but none was found."
    }
    return $file.FullName
}

function Get-LatestHarnessReportFile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Directory,
        [Parameter(Mandatory = $true)]
        [string]$Suite,
        [Parameter(Mandatory = $true)]
        [datetime]$SinceUtc
    )

    $file = Get-ChildItem -LiteralPath $Directory -Filter "$Suite-*.json" -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -notlike "$Suite-run-*.json" -and $_.Name -notlike "$Suite-comparison-*.json" } |
        Where-Object { $_.Name -notlike "$Suite-codex-preflight-*.json" } |
        Where-Object { $_.LastWriteTimeUtc -ge $SinceUtc } |
        Sort-Object LastWriteTimeUtc, Name |
        Select-Object -Last 1
    if (-not $file) {
        throw "Expected generated harness report for suite '$Suite' under '$Directory' after $($SinceUtc.ToString('o')), but none was found."
    }
    return $file.FullName
}

Push-Location $RepoRoot
try {
    New-Item -ItemType Directory -Force -Path $BenchmarkDir | Out-Null

    $benchmarkArgs = @(
        "run", "--bin", "spark", "--",
        "profile-benchmark", $Suite,
        "--model", $Model,
        "--reasoning-effort", $ReasoningEffort,
        "--repeat", "$Repeat",
        "--max-turns", "$MaxTurns"
    )

    foreach ($name in $Scenario) {
        $benchmarkArgs += @("--scenario", $name)
    }

    if ($NoTrace) {
        $benchmarkArgs += "--no-trace"
    }

    $sparkRunStartedAt = [datetime]::UtcNow
    & cargo @benchmarkArgs
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    if ($NoTrace) {
        Write-Host "benchmark_report=skipped reason=no-trace"
        return
    }

    $harnessManifest = Get-LatestGeneratedFile -Directory $BenchmarkDir -Filter "$Suite-run-*.json" -SinceUtc $sparkRunStartedAt
    $harnessReportStartedAt = [datetime]::UtcNow
    & cargo run --bin spark -- profile-benchmark-report --suite $Suite --run-manifest $harnessManifest --limit 50
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    $harnessReport = Get-LatestHarnessReportFile -Directory $BenchmarkDir -Suite $Suite -SinceUtc $harnessReportStartedAt
    Write-Host "harness_manifest=$harnessManifest"
    Write-Host "harness_report=$harnessReport"
}
finally {
    Pop-Location
}
