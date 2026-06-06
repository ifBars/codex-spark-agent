param(
    [string]$Model = "gpt-5.3-codex-spark",
    [ValidateSet("minimal", "low", "medium", "high")]
    [string]$ReasoningEffort = "medium",
    [int]$Repeat = 1,
    [int]$MaxTurns = 45,
    [ValidateSet("core", "survey", "scaffolding", "editing", "real-world")]
    [string]$Suite = "real-world",
    [string[]]$Scenario = @(
        "precise-patch",
        "multi-file-patch",
        "github-issue-bugfix",
        "rust-failing-test-bugfix",
        "typescript-reducer-bugfix",
        "github-issue-triage",
        "technical-essay",
        "config-migration",
        "ops-report",
        "rust-log-analyzer-scaffold"
    ),
    [switch]$NoTrace
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $RepoRoot
try {
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

    & cargo @benchmarkArgs
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & cargo run --bin spark -- profile-benchmark-report --suite $Suite --limit 50
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}
finally {
    Pop-Location
}
