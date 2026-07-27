param(
    [string]$Model = "gpt-5.3-codex-spark",
    [ValidateSet("minimal", "low", "medium", "high")]
    [string]$ReasoningEffort = "medium",
    [int]$Repeat = 1,
    [int]$TimeoutSeconds = 900,
    [ValidateSet("core", "survey", "scaffolding", "editing", "reasoning", "coding", "quantitative", "analysis", "operations", "writing", "real-world")]
    [string]$Suite = "real-world",
    [string[]]$Scenario = @(),
    [string]$CodexBin = "codex",
    [switch]$IgnoreUserConfig,
    [switch]$IsolatedCodexHome,
    [switch]$AllowHarnessRequestFailureComparison,
    [switch]$AllowCodexRequestFailureComparison,
    [switch]$SkipCodexPreflight,
    [switch]$PreflightOnly,
    [switch]$FailOnDirectionalComparison,
    [switch]$ListScenarios,
    [int]$CodexPreflightTimeoutSeconds = 120
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$BenchmarkDir = Join-Path $RepoRoot ".spark-profile\benchmarks"
$CodexDir = Join-Path $RepoRoot ".spark-profile\codex-cli"
. (Join-Path $PSScriptRoot "quick_benchmark_scenarios.ps1")

if (-not $Scenario -or $Scenario.Count -eq 0) {
    if ($Suite -eq "reasoning") {
        $Scenario = @(Get-QuickReasoningScenario)
    } elseif ($Suite -eq "real-world") {
        $Scenario = @(Get-QuickRealWorldScenario)
    }
}

if ($ListScenarios) {
    $Scenario | ForEach-Object { Write-Output $_ }
    return
}

Write-Host "benchmark_suite=$Suite"
Write-Host "benchmark_model=$Model"
Write-Host "reasoning_effort=$ReasoningEffort"
Write-Host "repeat=$Repeat"
Write-Host "timeout_seconds=$TimeoutSeconds"
Write-Host "codex_bin=$CodexBin"
Write-Host "scenario_count=$($Scenario.Count)"
Write-Host "scenarios=$($Scenario -join ',')"

function Test-InfrastructureFailureText {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Text
    )

    $lower = $Text.ToLowerInvariant()
    return (
        $lower.Contains("insufficient balance") -or
        $lower.Contains("insufficient-balance") -or
        $lower.Contains("insufficient_quota") -or
        $lower.Contains("quota exceeded") -or
        $lower.Contains("rate limit exceeded") -or
        $lower.Contains("too many requests") -or
        $lower.Contains("resource exhausted") -or
        $lower.Contains("usage limit") -or
        $lower.Contains("you've hit your usage limit") -or
        $lower.Contains('"statuscode":429') -or
        $lower.Contains('"statuscode": 429') -or
        (($lower.Contains('"statuscode":401') -or $lower.Contains('"statuscode": 401')) -and $lower.Contains("insufficient"))
    )
}

function Format-PreflightFailureExcerpt {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Text
    )

    $lines = @($Text -split "`r?`n" | ForEach-Object { $_.Trim() } | Where-Object { $_.Length -gt 0 })
    foreach ($line in $lines) {
        if (Test-InfrastructureFailureText -Text $line) {
            $message = $line
            try {
                $event = $line | ConvertFrom-Json -ErrorAction Stop
                if ($event.PSObject.Properties.Name -contains "message" -and $event.message) {
                    $message = [string]$event.message
                }
                elseif ($event.PSObject.Properties.Name -contains "error" -and $event.error -and $event.error.PSObject.Properties.Name -contains "message" -and $event.error.message) {
                    $message = [string]$event.error.message
                }
            }
            catch {
                $message = $line
            }
            $normalized = $message -replace '\s+', ' '
            if ($normalized.Length -gt 220) {
                $normalized = $normalized.Substring(0, 220) + "..."
            }
            return $normalized
        }
    }
    return ""
}

function Format-PreflightRetryHint {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Text
    )

    $lines = @($Text -split "`r?`n" | ForEach-Object { $_.Trim() } | Where-Object { $_.Length -gt 0 })
    foreach ($line in $lines) {
        $message = $line
        try {
            $event = $line | ConvertFrom-Json -ErrorAction Stop
            if ($event.PSObject.Properties.Name -contains "message" -and $event.message) {
                $message = [string]$event.message
            }
            elseif ($event.PSObject.Properties.Name -contains "error" -and $event.error -and $event.error.PSObject.Properties.Name -contains "message" -and $event.error.message) {
                $message = [string]$event.error.message
            }
        }
        catch {
            $message = $line
        }

        $normalized = $message -replace '\s+', ' '
        if ($normalized -match '(?i)\btry again\s+(at|in)\s+([^\.]+)') {
            return ("try again {0} {1}" -f $Matches[1].ToLowerInvariant(), $Matches[2].Trim())
        }
    }
    return ""
}

function Resolve-PreflightRetryAt {
    param(
        [AllowEmptyString()]
        [string]$RetryHint,
        [datetime]$ReferenceLocalTime = (Get-Date)
    )

    $normalized = $RetryHint -replace '\s+', ' '
    if ($normalized -match '(?i)^try again at\s+(.+)$') {
        try {
            $timeText = $Matches[1].Trim()
            $time = [datetime]::Parse($timeText)
            $retryLocal = $ReferenceLocalTime.Date.Add($time.TimeOfDay)
            if ($retryLocal -le $ReferenceLocalTime) {
                $retryLocal = $retryLocal.AddDays(1)
            }
            $delaySeconds = [int][Math]::Ceiling(($retryLocal - $ReferenceLocalTime).TotalSeconds)
            return [pscustomobject]@{
                Local = $retryLocal.ToString("o")
                Utc = $retryLocal.ToUniversalTime().ToString("o")
                DelaySeconds = $delaySeconds
            }
        }
        catch {
            return [pscustomobject]@{
                Local = ""
                Utc = ""
                DelaySeconds = $null
            }
        }
    }
    if ($normalized -match '(?i)^try again in\s+(\d+)\s+(second|seconds|minute|minutes|hour|hours)$') {
        $amount = [int]$Matches[1]
        $unit = $Matches[2].ToLowerInvariant()
        $retryLocal = $ReferenceLocalTime
        if ($unit.StartsWith("second")) {
            $retryLocal = $ReferenceLocalTime.AddSeconds($amount)
        }
        elseif ($unit.StartsWith("minute")) {
            $retryLocal = $ReferenceLocalTime.AddMinutes($amount)
        }
        elseif ($unit.StartsWith("hour")) {
            $retryLocal = $ReferenceLocalTime.AddHours($amount)
        }
        $delaySeconds = $amount
        if ($unit.StartsWith("minute")) {
            $delaySeconds = $amount * 60
        }
        elseif ($unit.StartsWith("hour")) {
            $delaySeconds = $amount * 3600
        }
        return [pscustomobject]@{
            Local = $retryLocal.ToString("o")
            Utc = $retryLocal.ToUniversalTime().ToString("o")
            DelaySeconds = $delaySeconds
        }
    }
    return [pscustomobject]@{
        Local = ""
        Utc = ""
        DelaySeconds = $null
    }
}

function Format-PowerShellSingleQuotedArgument {
    param(
        [AllowNull()]
        [string]$Value
    )

    if ($null -eq $Value) {
        $Value = ""
    }
    return "'{0}'" -f ($Value -replace "'", "''")
}

function New-QuickComparisonRerunCommand {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Suite,
        [Parameter(Mandatory = $true)]
        [string]$CodexBin,
        [Parameter(Mandatory = $true)]
        [string]$Model,
        [Parameter(Mandatory = $true)]
        [string]$ReasoningEffort,
        [string[]]$Scenario = @(),
        [switch]$OmitPreflightOnly
    )

    $parts = @(
        ".\scripts\quick_comparison_benchmark.ps1",
        "-Model", (Format-PowerShellSingleQuotedArgument $Model),
        "-ReasoningEffort", (Format-PowerShellSingleQuotedArgument $ReasoningEffort),
        "-Repeat", ([string]$script:Repeat),
        "-TimeoutSeconds", ([string]$script:TimeoutSeconds),
        "-Suite", (Format-PowerShellSingleQuotedArgument $Suite),
        "-CodexBin", (Format-PowerShellSingleQuotedArgument $CodexBin)
    )

    $scenarioList = @($Scenario)
    if ($scenarioList.Count -gt 0) {
        $scenarioArgs = ($scenarioList | ForEach-Object { Format-PowerShellSingleQuotedArgument $_ }) -join ","
        $parts += @("-Scenario", $scenarioArgs)
    }

    if ($script:IgnoreUserConfig) {
        $parts += "-IgnoreUserConfig"
    }
    if ($script:IsolatedCodexHome) {
        $parts += "-IsolatedCodexHome"
    }
    if ($script:AllowHarnessRequestFailureComparison) {
        $parts += "-AllowHarnessRequestFailureComparison"
    }
    if ($script:AllowCodexRequestFailureComparison) {
        $parts += "-AllowCodexRequestFailureComparison"
    }
    if ($script:SkipCodexPreflight) {
        $parts += "-SkipCodexPreflight"
    }
    if ($script:PreflightOnly -and -not $OmitPreflightOnly) {
        $parts += "-PreflightOnly"
    }
    if ($script:FailOnDirectionalComparison) {
        $parts += "-FailOnDirectionalComparison"
    }
    if ($script:CodexPreflightTimeoutSeconds) {
        $parts += @("-CodexPreflightTimeoutSeconds", ([string]$script:CodexPreflightTimeoutSeconds))
    }

    return ($parts -join " ")
}

function Resolve-CommandPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Command
    )

    try {
        $resolved = Get-Command $Command -ErrorAction Stop
        if ($resolved.PSObject.Properties.Name -contains "Source" -and $resolved.Source) {
            return [string]$resolved.Source
        }
        if ($resolved.PSObject.Properties.Name -contains "Path" -and $resolved.Path) {
            return [string]$resolved.Path
        }
        if ($resolved.PSObject.Properties.Name -contains "Definition" -and $resolved.Definition) {
            return [string]$resolved.Definition
        }
    }
    catch {
        return ""
    }
    return ""
}

function Resolve-CommandVersion {
    param(
        [AllowEmptyString()]
        [string]$CommandPath
    )

    if (-not $CommandPath) {
        return ""
    }

    $job = $null
    try {
        $job = Start-Job -ScriptBlock {
            param($Path)
            try {
                $output = & $Path --version 2>&1
                return ($output | Out-String)
            }
            catch {
                return ""
            }
        } -ArgumentList $CommandPath
        if (-not (Wait-Job -Job $job -Timeout 10)) {
            Stop-Job -Job $job -ErrorAction SilentlyContinue
            return ""
        }
        $version = (Receive-Job -Job $job | Out-String) -replace '\s+', ' '
        $version = $version.Trim()
        if ($version.Length -gt 200) {
            $version = $version.Substring(0, 200) + "..."
        }
        return $version
    }
    catch {
        return ""
    }
    finally {
        if ($job) {
            Remove-Job -Job $job -Force -ErrorAction SilentlyContinue
        }
    }
}

function Test-SemicolonFieldContains {
    param(
        $Value,
        [Parameter(Mandatory = $true)]
        [string]$Needle
    )

    if (-not $Value) {
        return $false
    }
    return @(($Value -split ';') | ForEach-Object { $_.Trim() }) -contains $Needle
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

function Write-CodexPreflightStatus {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Directory,
        [Parameter(Mandatory = $true)]
        [string]$Suite,
        [Parameter(Mandatory = $true)]
        [string]$Status,
        [Parameter(Mandatory = $true)]
        [string]$CodexBin,
        [Parameter(Mandatory = $true)]
        [string]$Model,
        [Parameter(Mandatory = $true)]
        [string]$ReasoningEffort,
        [string[]]$Scenario = @(),
        [int]$ExitCode = -1,
        [string]$FailureExcerpt = "",
        [string]$RetryHint = "",
        [string]$Message = ""
    )

    New-Item -ItemType Directory -Force -Path $Directory | Out-Null
    $generatedLocal = Get-Date
    $retryAt = Resolve-PreflightRetryAt -RetryHint $RetryHint -ReferenceLocalTime $generatedLocal
    $timestamp = $generatedLocal.ToUniversalTime().ToString("yyyyMMdd-HHmmss-fff")
    $path = Join-Path $Directory ("{0}-codex-preflight-{1}.json" -f $Suite, $timestamp)
    $scenarioList = @($Scenario)
    if ($scenarioList.Count -eq 0 -and $script:Scenario) {
        $scenarioList = @($script:Scenario)
    }
    $repeatValue = if ($null -ne $script:Repeat) { [int]$script:Repeat } else { 1 }
    $timeoutSecondsValue = if ($null -ne $script:TimeoutSeconds) { [int]$script:TimeoutSeconds } else { 900 }
    $codexPreflightTimeoutSecondsValue = if ($null -ne $script:CodexPreflightTimeoutSeconds) { [int]$script:CodexPreflightTimeoutSeconds } else { 120 }
    $codexCommandPath = Resolve-CommandPath -Command $CodexBin
    $codexCommandVersion = Resolve-CommandVersion -CommandPath $codexCommandPath
    $ignoreUserConfigValue = [bool]$script:IgnoreUserConfig
    $isolatedCodexHomeValue = [bool]$script:IsolatedCodexHome
    $allowHarnessRequestFailureComparisonValue = [bool]$script:AllowHarnessRequestFailureComparison
    $allowCodexRequestFailureComparisonValue = [bool]$script:AllowCodexRequestFailureComparison
    $skipCodexPreflightValue = [bool]$script:SkipCodexPreflight
    $preflightOnlyValue = [bool]$script:PreflightOnly
    $failOnDirectionalComparisonValue = [bool]$script:FailOnDirectionalComparison
    $rerunCommand = New-QuickComparisonRerunCommand `
        -Suite $Suite `
        -CodexBin $CodexBin `
        -Model $Model `
        -ReasoningEffort $ReasoningEffort `
        -Scenario $scenarioList
    $resumeCodexBin = if ($codexCommandPath.Length -gt 0) { $codexCommandPath } else { $CodexBin }
    $resumeCommand = New-QuickComparisonRerunCommand `
        -Suite $Suite `
        -CodexBin $resumeCodexBin `
        -Model $Model `
        -ReasoningEffort $ReasoningEffort `
        -Scenario $scenarioList `
        -OmitPreflightOnly
    $payload = [ordered]@{
        generated_at_utc = $generatedLocal.ToUniversalTime().ToString("o")
        suite = $Suite
        scenario_count = $scenarioList.Count
        scenarios = $scenarioList
        rerun_command = $rerunCommand
        resume_command = $resumeCommand
        status = $Status
        codex_bin = $CodexBin
        codex_command_path = $codexCommandPath
        codex_command_version = $codexCommandVersion
        model = $Model
        reasoning_effort = $ReasoningEffort
        repeat = $repeatValue
        timeout_seconds = $timeoutSecondsValue
        codex_preflight_timeout_seconds = $codexPreflightTimeoutSecondsValue
        ignore_user_config = $ignoreUserConfigValue
        isolated_codex_home = $isolatedCodexHomeValue
        allow_harness_request_failure_comparison = $allowHarnessRequestFailureComparisonValue
        allow_codex_request_failure_comparison = $allowCodexRequestFailureComparisonValue
        skip_codex_preflight = $skipCodexPreflightValue
        preflight_only = $preflightOnlyValue
        fail_on_directional_comparison = $failOnDirectionalComparisonValue
        exit_code = $ExitCode
        failure_excerpt = $FailureExcerpt
        retry_hint = $RetryHint
        retry_after_seconds = $retryAt.DelaySeconds
        retry_at_local = $retryAt.Local
        retry_at_utc = $retryAt.Utc
        message = $Message
    }
    $json = $payload | ConvertTo-Json -Depth 4
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [System.IO.File]::WriteAllText($path, $json, $utf8NoBom)
    if ($retryAt.Local.Length -gt 0) {
        Write-Host "codex_preflight_retry_at_local=$($retryAt.Local)"
    }
    if ($retryAt.Utc.Length -gt 0) {
        Write-Host "codex_preflight_retry_at_utc=$($retryAt.Utc)"
    }
    if ($null -ne $retryAt.DelaySeconds) {
        Write-Host "codex_preflight_retry_after_seconds=$($retryAt.DelaySeconds)"
    }
    Write-Host "codex_preflight_status=$path"
    Write-Host "codex_preflight_codex_path=$codexCommandPath"
    Write-Host "codex_preflight_codex_version=$codexCommandVersion"
    Write-Host "codex_preflight_rerun_command=$rerunCommand"
    Write-Host "codex_preflight_resume_command=$resumeCommand"
    return $path
}

function Resolve-RunPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }
    return (Join-Path $RepoRoot $Path)
}

function Test-HarnessRowInfrastructureFailure {
    param(
        [Parameter(Mandatory = $true)]
        $Row
    )

    if (-not (Test-SemicolonFieldContains -Value $Row.diagnostics -Needle "request_failure")) {
        return $false
    }

    $evidence = "{0}`n{1}" -f $Row.diagnostics, $Row.failure_points
    if ($Row.trace_dir) {
        $traceDir = Resolve-RunPath -Path ([string]$Row.trace_dir)
        if (Test-Path -LiteralPath $traceDir -PathType Container) {
            $files = Get-ChildItem -LiteralPath $traceDir -File -ErrorAction SilentlyContinue |
                Where-Object { $_.Extension -in @(".json", ".jsonl", ".txt", ".log") }
            foreach ($file in $files) {
                $evidence += "`n"
                $evidence += Get-Content -LiteralPath $file.FullName -Raw -ErrorAction SilentlyContinue
            }
        }
    }

    return Test-InfrastructureFailureText -Text $evidence
}

function Get-HarnessInfrastructureFailureSummary {
    param(
        [Parameter(Mandatory = $true)]
        $Report
    )

    $scenarios = @{}
    $count = 0
    $rows = @($Report.rows)
    foreach ($row in $rows) {
        if (Test-HarnessRowInfrastructureFailure -Row $row) {
            $count += 1
            $scenario = [string]$row.scenario
            if (-not $scenarios.ContainsKey($scenario)) {
                $scenarios[$scenario] = 0
            }
            $scenarios[$scenario] += 1
        }
    }

    return [pscustomobject]@{
        Count = $count
        ComparableRuns = [Math]::Max(0, ($rows.Count - $count))
        Scenarios = $scenarios
    }
}

function Assert-HarnessReportComparable {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ReportPath,
        [Parameter(Mandatory = $true)]
        [string]$ManifestPath,
        [Parameter(Mandatory = $true)]
        [bool]$AllowRequestFailureComparison
    )

    $report = Get-Content -LiteralPath $ReportPath -Raw | ConvertFrom-Json
    $infrastructureFailures = Get-HarnessInfrastructureFailureSummary -Report $report
    $requestFailures = $infrastructureFailures.Count
    $comparableRuns = $infrastructureFailures.ComparableRuns
    $requestFailureScenarios = Format-ScenarioCountSummary -ScenarioCounts ([pscustomobject]$infrastructureFailures.Scenarios)
    $requestFailureScenarioPairs = Format-ScenarioCountPairs -ScenarioCounts ([pscustomobject]$infrastructureFailures.Scenarios)
    $successfulRuns = [int]$report.aggregate.successful_runs
    if (-not $AllowRequestFailureComparison -and $requestFailures -gt 0 -and $comparableRuns -eq 0) {
        Write-Host "harness_manifest=$ManifestPath"
        Write-Host "harness_report=$ReportPath"
        $message = ("Spark harness run is not comparable (successful_runs={0}, comparable_runs={1}, provider_api_failure={2}). " +
            "Skipping native Codex CLI comparison so infrastructure failures do not produce a misleading winner. " +
            "{3}Re-run after the Spark API limit clears, or pass -AllowHarnessRequestFailureComparison to force comparison.") -f `
            $successfulRuns, $comparableRuns, $requestFailures, $requestFailureScenarios
        throw $message
    }
    if (-not $AllowRequestFailureComparison -and $requestFailures -gt 0) {
        Write-Host ("harness_provider_api_failures={0} comparable_spark_rows={1} harness_provider_api_failure_scenarios={2} {3}" -f `
            $requestFailures, $comparableRuns, $requestFailureScenarioPairs, $requestFailureScenarios)
    }
}

function Format-ScenarioCountSummary {
    param(
        $ScenarioCounts
    )

    if (-not $ScenarioCounts) {
        return ""
    }
    $parts = @()
    foreach ($property in ($ScenarioCounts.PSObject.Properties | Sort-Object Name)) {
        $count = [int]$property.Value
        if ($count -le 0) {
            continue
        }
        if ($count -eq 1) {
            $parts += $property.Name
        }
        else {
            $parts += ("{0} x{1}" -f $property.Name, $count)
        }
    }
    if ($parts.Count -eq 0) {
        return ""
    }
    return "Scenarios: $($parts -join ', '). "
}

function Format-ScenarioCountPairs {
    param(
        $ScenarioCounts
    )

    if (-not $ScenarioCounts) {
        return "none"
    }
    $parts = @()
    foreach ($property in ($ScenarioCounts.PSObject.Properties | Sort-Object Name)) {
        $count = [int]$property.Value
        if ($count -le 0) {
            continue
        }
        $parts += ("{0}:{1}" -f $property.Name, $count)
    }
    if ($parts.Count -eq 0) {
        return "none"
    }
    return ($parts -join ",")
}

function Assert-CodexReportComparable {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ReportPath,
        [Parameter(Mandatory = $true)]
        [string]$ManifestPath,
        [Parameter(Mandatory = $true)]
        [string]$HarnessReportPath,
        [Parameter(Mandatory = $true)]
        [bool]$AllowRequestFailureComparison
    )

    $report = Get-Content -LiteralPath $ReportPath -Raw | ConvertFrom-Json
    $diagnostics = $report.aggregate.diagnostics
    $requestFailures = 0
    if ($diagnostics -and $diagnostics.PSObject.Properties.Name -contains "request_failure") {
        $requestFailures = [int]$diagnostics.request_failure
    }
    $requestFailureScenarios = ""
    $requestFailureScenarioPairs = "none"
    if ($diagnostics -and $diagnostics.PSObject.Properties.Name -contains "request_failure_scenarios") {
        $requestFailureScenarios = Format-ScenarioCountSummary -ScenarioCounts $diagnostics.request_failure_scenarios
        $requestFailureScenarioPairs = Format-ScenarioCountPairs -ScenarioCounts $diagnostics.request_failure_scenarios
    }
    $successfulRuns = [int]$report.aggregate.successful_runs
    $comparableRuns = [int]$report.aggregate.runs
    if ($report.aggregate.PSObject.Properties.Name -contains "comparable_runs") {
        $comparableRuns = [int]$report.aggregate.comparable_runs
    }
    elseif ($requestFailures -gt 0) {
        $comparableRuns = [Math]::Max(0, ([int]$report.aggregate.runs) - $requestFailures)
    }
    if (-not $AllowRequestFailureComparison -and $comparableRuns -eq 0 -and $requestFailures -gt 0) {
        Write-Host "harness_manifest=$ManifestPath"
        Write-Host "harness_report=$HarnessReportPath"
        Write-Host "codex_cli_report=$ReportPath"
        $message = ("Native Codex CLI run is not comparable (successful_runs={0}, comparable_runs={1}, request_failure={2}). " +
            "Skipping comparison so native infrastructure failures do not produce a misleading winner. " +
            "{3}Re-run after the Codex usage limit clears, or pass -AllowCodexRequestFailureComparison to force comparison.") -f `
            $successfulRuns, $comparableRuns, $requestFailures, $requestFailureScenarios
        throw $message
    }
    if (-not $AllowRequestFailureComparison -and $requestFailures -gt 0) {
        Write-Host ("codex_provider_api_failures={0} comparable_codex_rows={1} codex_provider_api_failure_scenarios={2}" -f `
            $requestFailures, $comparableRuns, $requestFailureScenarioPairs)
    }
}

function Invoke-CodexPreflight {
    param(
        [Parameter(Mandatory = $true)]
        [string]$CodexBin,
        [Parameter(Mandatory = $true)]
        [string]$Model,
        [Parameter(Mandatory = $true)]
        [string]$ReasoningEffort,
        [Parameter(Mandatory = $true)]
        [int]$TimeoutSeconds,
        [Parameter(Mandatory = $true)]
        [bool]$IgnoreUserConfig,
        [Parameter(Mandatory = $true)]
        [string]$StatusDirectory,
        [Parameter(Mandatory = $true)]
        [string]$Suite
    )

    $preflightDir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ("spark-codex-preflight-" + [guid]::NewGuid()))
    try {
        try {
            Get-Command $CodexBin -ErrorAction Stop | Out-Null
        }
        catch {
            Write-Host "codex_preflight=launch_failed"
            Write-CodexPreflightStatus `
                -Directory $StatusDirectory `
                -Suite $Suite `
                -Status "launch_failed" `
                -CodexBin $CodexBin `
                -Model $Model `
                -ReasoningEffort $ReasoningEffort `
                -Message $_.Exception.Message | Out-Null
            throw "Native Codex CLI preflight could not launch '$CodexBin': $($_.Exception.Message). Fix the native Codex CLI command, or pass -SkipCodexPreflight to defer detection to codex-cli-benchmark."
        }

        $lastMessage = Join-Path $preflightDir.FullName "last-message.txt"
        $arguments = @(
            "exec",
            "--json",
            "--cd", $preflightDir.FullName,
            "--sandbox", "danger-full-access",
            "--dangerously-bypass-approvals-and-sandbox",
            "--model", $Model,
            "--config", "model_reasoning_effort=`"$ReasoningEffort`"",
            "--output-last-message", $lastMessage
        )
        if ($IgnoreUserConfig) {
            $arguments += "--ignore-user-config"
        }
        $arguments += "Reply with exactly OK."

        $payload = [pscustomobject]@{
            Bin = $CodexBin
            Arguments = $arguments
        }
        $job = Start-Job -ScriptBlock {
            param($Payload)
            try {
                $output = & $Payload.Bin @($Payload.Arguments) 2>&1
                [pscustomobject]@{
                    ExitCode = if ($null -eq $LASTEXITCODE) { 0 } else { $LASTEXITCODE }
                    Output = ($output | Out-String)
                    LaunchError = $null
                }
            }
            catch {
                [pscustomobject]@{
                    ExitCode = $null
                    Output = ($_ | Out-String)
                    LaunchError = $_.Exception.Message
                }
            }
        } -ArgumentList $payload
        if (-not (Wait-Job -Job $job -Timeout $TimeoutSeconds)) {
            Stop-Job -Job $job -ErrorAction SilentlyContinue
            Write-CodexPreflightStatus `
                -Directory $StatusDirectory `
                -Suite $Suite `
                -Status "timeout" `
                -CodexBin $CodexBin `
                -Model $Model `
                -ReasoningEffort $ReasoningEffort `
                -Message "Native Codex CLI preflight timed out after $TimeoutSeconds seconds." | Out-Null
            throw "Native Codex CLI preflight timed out after $TimeoutSeconds seconds. Pass -SkipCodexPreflight to run the full benchmark anyway."
        }
        $result = Receive-Job -Job $job
        $launchError = [string]$result.LaunchError
        $combined = [string]$result.Output
        if ($launchError.Length -gt 0) {
            Write-Host "codex_preflight=launch_failed"
            Write-CodexPreflightStatus `
                -Directory $StatusDirectory `
                -Suite $Suite `
                -Status "launch_failed" `
                -CodexBin $CodexBin `
                -Model $Model `
                -ReasoningEffort $ReasoningEffort `
                -Message $launchError | Out-Null
            throw "Native Codex CLI preflight could not launch '$CodexBin': $launchError. Fix the native Codex CLI command, or pass -SkipCodexPreflight to defer detection to codex-cli-benchmark."
        }
        $exitCode = [int]$result.ExitCode
        if ($exitCode -ne 0 -and (Test-InfrastructureFailureText -Text $combined)) {
            Write-Host "codex_preflight=failed"
            $excerpt = Format-PreflightFailureExcerpt -Text $combined
            if ($excerpt.Length -gt 0) {
                Write-Host "codex_preflight_failure_excerpt=$excerpt"
            }
            $retryHint = Format-PreflightRetryHint -Text $combined
            if ($retryHint.Length -gt 0) {
                Write-Host "codex_preflight_retry_hint=$retryHint"
            }
            Write-CodexPreflightStatus `
                -Directory $StatusDirectory `
                -Suite $Suite `
                -Status "failed" `
                -CodexBin $CodexBin `
                -Model $Model `
                -ReasoningEffort $ReasoningEffort `
                -ExitCode $exitCode `
                -FailureExcerpt $excerpt `
                -RetryHint $retryHint `
                -Message "Provider/API infrastructure failure during native Codex CLI preflight." | Out-Null
            throw "Native Codex CLI preflight is not comparable due to provider/API infrastructure failure. Re-run after the Codex usage limit clears, or pass -SkipCodexPreflight to defer detection to codex-cli-benchmark."
        }
        if ($exitCode -ne 0) {
            Write-Host "codex_preflight=nonzero exit_code=$exitCode"
            Write-CodexPreflightStatus `
                -Directory $StatusDirectory `
                -Suite $Suite `
                -Status "nonzero" `
                -CodexBin $CodexBin `
                -Model $Model `
                -ReasoningEffort $ReasoningEffort `
                -ExitCode $exitCode `
                -Message "Native Codex CLI preflight exited with code $exitCode." | Out-Null
            throw "Native Codex CLI preflight exited with code $exitCode before the Spark run. Fix the native Codex CLI configuration, or pass -SkipCodexPreflight to defer detection to codex-cli-benchmark."
        }
        $lastMessageText = ""
        if (Test-Path -LiteralPath $lastMessage -PathType Leaf) {
            $lastMessageText = (Get-Content -LiteralPath $lastMessage -Raw -ErrorAction SilentlyContinue).Trim()
        }
        if ($lastMessageText -ne "OK") {
            Write-Host "codex_preflight=invalid_response"
            Write-CodexPreflightStatus `
                -Directory $StatusDirectory `
                -Suite $Suite `
                -Status "invalid_response" `
                -CodexBin $CodexBin `
                -Model $Model `
                -ReasoningEffort $ReasoningEffort `
                -ExitCode 0 `
                -Message "Native Codex CLI preflight exited successfully but did not write the expected OK response." | Out-Null
            throw "Native Codex CLI preflight exited successfully but did not write the expected OK response. Fix the native Codex CLI configuration, or pass -SkipCodexPreflight to defer detection to codex-cli-benchmark."
        }
        Write-Host "codex_preflight=ok"
        Write-CodexPreflightStatus `
            -Directory $StatusDirectory `
            -Suite $Suite `
            -Status "ok" `
            -CodexBin $CodexBin `
            -Model $Model `
            -ReasoningEffort $ReasoningEffort `
            -ExitCode 0 | Out-Null
    }
    finally {
        if ($job) {
            Remove-Job -Job $job -Force -ErrorAction SilentlyContinue
        }
        Remove-Item -LiteralPath $preflightDir.FullName -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Push-Location $RepoRoot
try {
    New-Item -ItemType Directory -Force -Path $BenchmarkDir | Out-Null
    New-Item -ItemType Directory -Force -Path $CodexDir | Out-Null

    if ($SkipCodexPreflight) {
        Write-Host "codex_preflight=skipped"
        Write-CodexPreflightStatus `
            -Directory $BenchmarkDir `
            -Suite $Suite `
            -Status "skipped" `
            -CodexBin $CodexBin `
            -Model $Model `
            -ReasoningEffort $ReasoningEffort `
            -Message "Skipped by -SkipCodexPreflight." | Out-Null
    }
    elseif ($IsolatedCodexHome) {
        Write-Host "codex_preflight=skipped reason=isolated-codex-home"
        Write-CodexPreflightStatus `
            -Directory $BenchmarkDir `
            -Suite $Suite `
            -Status "skipped" `
            -CodexBin $CodexBin `
            -Model $Model `
            -ReasoningEffort $ReasoningEffort `
            -Message "Skipped because -IsolatedCodexHome prepares CODEX_HOME inside the Rust benchmark runner." | Out-Null
    }
    else {
        Invoke-CodexPreflight `
            -CodexBin $CodexBin `
            -Model $Model `
            -ReasoningEffort $ReasoningEffort `
            -TimeoutSeconds $CodexPreflightTimeoutSeconds `
            -IgnoreUserConfig:$IgnoreUserConfig `
            -StatusDirectory $BenchmarkDir `
            -Suite $Suite
    }

    if ($PreflightOnly) {
        Write-Host "preflight_only=true"
        return
    }

    $sparkArgs = @(
        "run", "--bin", "spark", "--",
        "profile-benchmark", $Suite,
        "--model", $Model,
        "--reasoning-effort", $ReasoningEffort,
        "--repeat", "$Repeat"
    )
    foreach ($name in $Scenario) {
        $sparkArgs += @("--scenario", $name)
    }

    $sparkRunStartedAt = [datetime]::UtcNow
    & cargo @sparkArgs
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    $harnessManifest = Get-LatestGeneratedFile -Directory $BenchmarkDir -Filter "$Suite-run-*.json" -SinceUtc $sparkRunStartedAt

    $harnessReportStartedAt = [datetime]::UtcNow
    & cargo run --bin spark -- profile-benchmark-report --suite $Suite --run-manifest $harnessManifest --limit 50
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    $harnessReport = Get-LatestHarnessReportFile -Directory $BenchmarkDir -Suite $Suite -SinceUtc $harnessReportStartedAt
    Assert-HarnessReportComparable `
        -ReportPath $harnessReport `
        -ManifestPath $harnessManifest `
        -AllowRequestFailureComparison:$AllowHarnessRequestFailureComparison

    $codexArgs = @(
        "run", "--bin", "spark", "--",
        "codex-cli-benchmark", $Suite,
        "--codex-bin", $CodexBin,
        "--model", $Model,
        "--reasoning-effort", $ReasoningEffort,
        "--repeat", "$Repeat",
        "--timeout-seconds", "$TimeoutSeconds"
    )
    foreach ($name in $Scenario) {
        $codexArgs += @("--scenario", $name)
    }
    if ($IgnoreUserConfig) {
        $codexArgs += "--ignore-user-config"
    }
    if ($IsolatedCodexHome) {
        $codexArgs += "--isolated-codex-home"
    }

    $codexRunStartedAt = [datetime]::UtcNow
    & cargo @codexArgs
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    $codexReport = Get-LatestGeneratedFile -Directory $CodexDir -Filter "$Suite-codex-cli-*.json" -SinceUtc $codexRunStartedAt
    Assert-CodexReportComparable `
        -ReportPath $codexReport `
        -ManifestPath $harnessManifest `
        -HarnessReportPath $harnessReport `
        -AllowRequestFailureComparison:$AllowCodexRequestFailureComparison

    $comparisonArgs = @(
        "run", "--bin", "spark", "--",
        "benchmark-compare",
        "--suite", $Suite,
        "--harness-report", $harnessManifest,
        "--codex-cli-report", $codexReport,
        "--group-by-reasoning"
    )
    if ($FailOnDirectionalComparison) {
        $comparisonArgs += "--fail-on-directional-comparison"
    }

    $comparisonStartedAt = [datetime]::UtcNow
    & cargo @comparisonArgs
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    $comparisonReport = Get-LatestGeneratedFile -Directory $BenchmarkDir -Filter "$Suite-comparison-*.json" -SinceUtc $comparisonStartedAt
    $comparison = Get-Content -LiteralPath $comparisonReport -Raw | ConvertFrom-Json
    $headline = $comparison.aggregate.headline
    if ($headline) {
        Write-Host ("winner={0} baseline={1} winner_index={2} baseline_index={3} margin={4} beats_baseline={5}" -f `
            $headline.winner, `
            $headline.baseline_runner, `
            $headline.winner_benchmark_index, `
            $headline.baseline_benchmark_index, `
            $headline.benchmark_index_margin_vs_baseline, `
            $headline.winner_beats_baseline)
    }

    Write-Host "harness_manifest=$harnessManifest"
    Write-Host "codex_cli_report=$codexReport"
    Write-Host "comparison_report=$comparisonReport"
}
finally {
    Pop-Location
}
