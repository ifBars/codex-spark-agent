param(
    [string]$Model = "gpt-5.3-codex-spark",
    [ValidateSet("minimal", "low", "medium", "high")]
    [string]$ReasoningEffort = "medium",
    [int]$Repeat = 1,
    [int]$TimeoutSeconds = 900,
    [int]$JudgeTimeoutSeconds = 900,
    [string]$CodexBin = "codex",
    [switch]$IgnoreUserConfig,
    [switch]$IsolatedCodexHome
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$BenchmarkDir = Join-Path $RepoRoot ".spark-profile\benchmarks"
$CodexDir = Join-Path $RepoRoot ".spark-profile\codex-cli"
$Scenarios = @(
    "asset-ripper-exploration",
    "fivem-exploration",
    "cpp2il-exploration",
    "il2cpp-interop-exploration"
)
$ReadRoots = @(
    "C:\Users\ghost\Desktop\Coding\ScheduleOne\AssetRipper_export_20260718_070918",
    "C:\Users\ghost\Desktop\Coding\FiveM\fivem-master",
    "C:\Users\ghost\Desktop\Coding\Cpp2IL",
    "C:\Users\ghost\Desktop\Coding\Il2CppInterop"
)

foreach ($root in $ReadRoots) {
    if (-not (Test-Path -LiteralPath $root -PathType Container)) {
        throw "Exploration reference root does not exist: $root"
    }
}

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
        throw "Expected generated file '$Filter' under '$Directory' after $($SinceUtc.ToString('o'))."
    }
    return $file.FullName
}

$runStartedAt = [datetime]::UtcNow
$comparisonArgs = @{
    Suite = "survey"
    Model = $Model
    ReasoningEffort = $ReasoningEffort
    Repeat = $Repeat
    TimeoutSeconds = $TimeoutSeconds
    CodexBin = $CodexBin
    Scenario = $Scenarios
}
if ($IgnoreUserConfig) {
    $comparisonArgs.IgnoreUserConfig = $true
}
if ($IsolatedCodexHome) {
    $comparisonArgs.IsolatedCodexHome = $true
}

& (Join-Path $PSScriptRoot "quick_comparison_benchmark.ps1") @comparisonArgs
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$comparisonReport = Get-LatestGeneratedFile `
    -Directory $BenchmarkDir `
    -Filter "survey-comparison-*.json" `
    -SinceUtc $runStartedAt
$harnessManifest = Get-LatestGeneratedFile `
    -Directory $BenchmarkDir `
    -Filter "survey-run-*.json" `
    -SinceUtc $runStartedAt
$codexReport = Get-LatestGeneratedFile `
    -Directory $CodexDir `
    -Filter "survey-codex-cli-*.json" `
    -SinceUtc $runStartedAt

Push-Location $RepoRoot
try {
    $judgeStartedAt = [datetime]::UtcNow
    & cargo run --bin spark -- benchmark-judge `
        --comparison-report $comparisonReport `
        --model "gpt-5.6-terra" `
        --reasoning-effort medium `
        --codex-bin $CodexBin `
        --timeout-seconds $JudgeTimeoutSeconds
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    $judgeReport = Get-LatestGeneratedFile `
        -Directory $BenchmarkDir `
        -Filter "survey-llm-judge-*.json" `
        -SinceUtc $judgeStartedAt

    $judgedComparisonStartedAt = [datetime]::UtcNow
    & cargo run --bin spark -- benchmark-compare `
        --suite survey `
        --harness-report $harnessManifest `
        --codex-cli-report $codexReport `
        --llm-judge-report $judgeReport `
        --group-by-reasoning
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    $judgedComparison = Get-LatestGeneratedFile `
        -Directory $BenchmarkDir `
        -Filter "survey-comparison-*.json" `
        -SinceUtc $judgedComparisonStartedAt

    Write-Host "exploration_scenarios=$($Scenarios -join ',')"
    Write-Host "spark_tasks_per_scenario=4"
    Write-Host "judge_backend=codex-cli"
    Write-Host "judge_model=gpt-5.6-terra"
    Write-Host "judge_reasoning_effort=medium"
    Write-Host "judge_report=$judgeReport"
    Write-Host "judged_comparison_report=$judgedComparison"
}
finally {
    Pop-Location
}
