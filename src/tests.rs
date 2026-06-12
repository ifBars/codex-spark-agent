use crate::chat::{
    command_args, matching_slash_commands, parse_mode, slash_command_token,
    unknown_slash_command_warning,
};
mod profile_scenarios;

use crate::cli::{Cli, Command, ProfileBenchmarkSuiteKind, TraceSort};
use crate::client::output_text_delta;
use crate::profile::scenarios::validate_scenario_repeat;
use crate::prompt_commands;
use crate::session::{is_active_session, timestamp_session_name};
use crate::skill::commands::{contains_skill_mention, mentioned_skill_names};
use crate::trace::commands::{
    TraceListRecord, latest_trace_dir, list_trace_dirs, resolve_char_threshold, sort_trace_records,
    trace_export_record, trace_filter_label, trace_has_all_diagnostics,
    trace_matches_metric_filters, trace_runs_root, trace_sort_metric, trace_sort_name,
};
use crate::{DEFAULT_COMPACT_AFTER_CHARS, json_count_map_summary};
use serde_json::json;
use std::path::PathBuf;

#[test]
fn chat_cli_accepts_reasoning_effort_flag() {
    let cli = <Cli as clap::Parser>::try_parse_from([
        "spark",
        "chat",
        "--reasoning-effort",
        "low",
        "answer from relay",
    ])
    .expect("parse chat reasoning flag");

    let Command::Chat {
        reasoning_effort,
        prompt,
        ..
    } = cli.command
    else {
        panic!("expected chat command");
    };

    assert_eq!(reasoning_effort, "low");
    assert_eq!(prompt, vec!["answer from relay"]);
}

#[test]
fn chat_cli_accepts_custom_system_prompt_flag() {
    let cli = <Cli as clap::Parser>::try_parse_from([
        "spark",
        "chat",
        "--system-prompt",
        "You are Relay in Discord.",
        "answer from relay",
    ])
    .expect("parse chat system prompt flag");

    let Command::Chat {
        system_prompt,
        prompt,
        ..
    } = cli.command
    else {
        panic!("expected chat command");
    };

    assert_eq!(system_prompt.as_deref(), Some("You are Relay in Discord."));
    assert_eq!(prompt, vec!["answer from relay"]);
}

#[test]
fn reusable_prompt_commands_are_discovered_and_expanded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let commands_dir = dir.path().join(".agents").join("commands");
    std::fs::create_dir_all(&commands_dir).expect("create commands dir");
    std::fs::write(
        commands_dir.join("review.md"),
        "---\ndescription: Review a focused change\n---\nReview this change:\n\n{{args}}\n",
    )
    .expect("write command");

    let commands = prompt_commands::discover_commands(dir.path()).expect("discover commands");
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].name, "review");
    assert_eq!(commands[0].description, "Review a focused change");
    assert_eq!(commands[0].source_path, ".agents/commands/review.md");

    let expanded =
        prompt_commands::expand_command(dir.path(), "review", "src/main.rs").expect("expand");
    assert_eq!(expanded, "Review this change:\n\nsrc/main.rs");
}

#[test]
fn reusable_prompt_commands_expand_from_slash_input() {
    let dir = tempfile::tempdir().expect("tempdir");
    let commands_dir = dir.path().join(".spark").join("commands");
    std::fs::create_dir_all(&commands_dir).expect("create commands dir");
    std::fs::write(
        commands_dir.join("fix.md"),
        "Fix the issue described by these arguments.\n",
    )
    .expect("write command");

    let expanded =
        prompt_commands::expand_slash_command(dir.path(), "/fix issue 12").expect("expand slash");
    assert_eq!(
        expanded.as_deref(),
        Some("Fix the issue described by these arguments.\n\nArguments: issue 12")
    );
    assert_eq!(
        prompt_commands::expand_slash_command(dir.path(), "/unknown args")
            .expect("unknown slash command"),
        None
    );
}

#[test]
fn claude_prompt_commands_import_with_arguments_and_namespaces() {
    let dir = tempfile::tempdir().expect("tempdir");
    let commands_dir = dir.path().join(".claude").join("commands").join("db");
    std::fs::create_dir_all(&commands_dir).expect("create commands dir");
    std::fs::write(
        commands_dir.join("migrate.md"),
        "---\ndescription: Review a database migration\n---\nReview migration $ARGUMENTS.\n",
    )
    .expect("write command");

    let commands = prompt_commands::discover_commands(dir.path()).expect("discover commands");
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].name, "db:migrate");
    assert_eq!(commands[0].description, "Review a database migration");
    assert_eq!(commands[0].source_path, ".claude/commands/db/migrate.md");

    let expanded =
        prompt_commands::expand_slash_command(dir.path(), "/db:migrate 20260612_add_users")
            .expect("expand slash");
    assert_eq!(
        expanded.as_deref(),
        Some("Review migration 20260612_add_users.")
    );
}

#[test]
fn commands_cli_accepts_name_and_arguments() {
    let cli = <Cli as clap::Parser>::try_parse_from([
        "spark",
        "commands",
        "--cwd",
        ".",
        "review",
        "src/main.rs",
    ])
    .expect("parse commands cli");

    let Command::Commands {
        cwd, name, args, ..
    } = cli.command
    else {
        panic!("expected commands command");
    };

    assert_eq!(cwd, PathBuf::from("."));
    assert_eq!(name.as_deref(), Some("review"));
    assert_eq!(args, vec!["src/main.rs"]);
}

#[test]
fn spinner_preview_cli_is_available() {
    let cli = <Cli as clap::Parser>::try_parse_from(["spark", "spinner-preview"])
        .expect("parse spinner preview cli");

    assert!(matches!(cli.command, Command::SpinnerPreview));
}

#[test]
fn benchmark_report_cli_accepts_run_manifest_flag() {
    let cli = <Cli as clap::Parser>::try_parse_from([
        "spark",
        "profile-benchmark-report",
        "--suite",
        "real-world",
        "--run-manifest",
        ".spark-profile/benchmarks/real-world-run-1.json",
    ])
    .expect("parse benchmark report run manifest flag");

    let Command::ProfileBenchmarkReport {
        suite,
        run_manifests,
        ..
    } = cli.command
    else {
        panic!("expected profile-benchmark-report command");
    };

    assert_eq!(suite.name(), "real-world");
    assert_eq!(
        run_manifests,
        vec![PathBuf::from(
            ".spark-profile/benchmarks/real-world-run-1.json"
        )]
    );
}

#[test]
fn benchmark_compare_help_describes_harness_report_inputs() {
    let mut command = <Cli as clap::CommandFactory>::command();
    let help = command
        .find_subcommand_mut("benchmark-compare")
        .expect("benchmark-compare subcommand")
        .render_long_help()
        .to_string();

    assert!(help.contains(
        "Optional Spark harness run manifest or saved benchmark report JSON. Repeat to merge inputs"
    ));
    assert!(help.contains(
        "Exit nonzero after writing artifacts when inputs or provider skips make the headline directional"
    ));
}

#[test]
fn json_count_map_summary_formats_sorted_nonzero_counts() {
    let value = json!({
        "ops-report": 1,
        "config-migration": 2,
        "precise-patch": 0
    });

    assert_eq!(
        json_count_map_summary(Some(&value)).as_deref(),
        Some("config-migration:2,ops-report:1")
    );
    assert_eq!(json_count_map_summary(Some(&json!({}))), None);
    assert_eq!(json_count_map_summary(None), None);
}

#[test]
fn quick_comparison_script_guards_harness_request_failures_by_default() {
    let script = std::fs::read_to_string("scripts/quick_comparison_benchmark.ps1")
        .expect("read quick comparison script");

    assert!(script.contains("[switch]$AllowHarnessRequestFailureComparison"));
    assert!(script.contains("[switch]$AllowCodexRequestFailureComparison"));
    assert!(script.contains("[switch]$SkipCodexPreflight"));
    assert!(script.contains("[switch]$PreflightOnly"));
    assert!(script.contains("[switch]$FailOnDirectionalComparison"));
    assert!(script.contains("[switch]$ListScenarios"));
    assert!(script.contains("[string[]]$Scenario = @()"));
    assert!(script.contains("quick_benchmark_scenarios.ps1"));
    assert!(script.contains("$Scenario = @(Get-QuickRealWorldScenario)"));
    assert!(script.contains("if ($ListScenarios)"));
    assert!(script.contains("$Scenario | ForEach-Object { Write-Output $_ }"));
    assert!(script.contains("Write-Host \"benchmark_suite=$Suite\""));
    assert!(script.contains("Write-Host \"benchmark_model=$Model\""));
    assert!(script.contains("Write-Host \"reasoning_effort=$ReasoningEffort\""));
    assert!(script.contains("Write-Host \"repeat=$Repeat\""));
    assert!(script.contains("Write-Host \"max_turns=$MaxTurns\""));
    assert!(script.contains("Write-Host \"timeout_seconds=$TimeoutSeconds\""));
    assert!(script.contains("Write-Host \"codex_bin=$CodexBin\""));
    assert!(script.contains("Write-Host \"scenario_count=$($Scenario.Count)\""));
    assert!(script.contains("Write-Host \"scenarios=$($Scenario -join ',')\""));
    assert!(script.contains("[int]$CodexPreflightTimeoutSeconds = 120"));
    assert!(script.contains("function Test-InfrastructureFailureText"));
    assert!(script.contains("function Format-PreflightFailureExcerpt"));
    assert!(script.contains("function Format-PreflightRetryHint"));
    assert!(script.contains("function Resolve-PreflightRetryAt"));
    assert!(script.contains("function Format-PowerShellSingleQuotedArgument"));
    assert!(script.contains("function New-QuickComparisonRerunCommand"));
    assert!(script.contains("[switch]$OmitPreflightOnly"));
    assert!(script.contains("if ($script:FailOnDirectionalComparison)"));
    assert!(script.contains("$parts += \"-FailOnDirectionalComparison\""));
    assert!(script.contains("function Resolve-CommandPath"));
    assert!(script.contains("function Resolve-CommandVersion"));
    assert!(script.contains("function Write-CodexPreflightStatus"));
    assert!(script.contains("function Invoke-CodexPreflight"));
    assert!(script.contains("Start-Job -ScriptBlock"));
    assert!(script.contains("& $Payload.Bin @($Payload.Arguments) 2>&1"));
    assert!(script.contains("function Test-SemicolonFieldContains"));
    assert!(script.contains("function Test-HarnessRowInfrastructureFailure"));
    assert!(script.contains("function Get-HarnessInfrastructureFailureSummary"));
    assert!(script.contains("Test-InfrastructureFailureText -Text $evidence"));
    assert!(script.contains(
        "$infrastructureFailures = Get-HarnessInfrastructureFailureSummary -Report $report"
    ));
    assert!(script.contains("$requestFailures = $infrastructureFailures.Count"));
    assert!(script.contains("$comparableRuns = $infrastructureFailures.ComparableRuns"));
    assert!(script.contains("Format-ScenarioCountSummary -ScenarioCounts ([pscustomobject]$infrastructureFailures.Scenarios)"));
    assert!(script.contains("codex_preflight=launch_failed"));
    assert!(script.contains("Native Codex CLI preflight could not launch"));
    assert!(script.contains("Native Codex CLI preflight exited with code $exitCode"));
    assert!(script.contains("Wait-Job -Job $job -Timeout $TimeoutSeconds"));
    assert!(script.contains("$lastMessageText -ne \"OK\""));
    assert!(script.contains("codex_preflight=invalid_response"));
    assert!(script.contains(
        "Native Codex CLI preflight exited successfully but did not write the expected OK response."
    ));
    assert!(script.contains("codex_preflight=failed"));
    assert!(script.contains("codex_preflight_failure_excerpt=$excerpt"));
    assert!(script.contains("codex_preflight_retry_hint=$retryHint"));
    assert!(script.contains("codex_preflight_status=$path"));
    assert!(script.contains("codex_preflight=skipped"));
    assert!(script.contains("generated_at_utc"));
    assert!(script.contains("scenario_count"));
    assert!(script.contains("scenarios = $scenarioList"));
    assert!(script.contains("rerun_command = $rerunCommand"));
    assert!(script.contains("$resumeCodexBin = if ($codexCommandPath.Length -gt 0)"));
    assert!(script.contains("resume_command = $resumeCommand"));
    assert!(script.contains("codex_command_path = $codexCommandPath"));
    assert!(script.contains("codex_command_version = $codexCommandVersion"));
    assert!(script.contains("repeat = $repeatValue"));
    assert!(script.contains("max_turns = $maxTurnsValue"));
    assert!(script.contains("timeout_seconds = $timeoutSecondsValue"));
    assert!(
        script.contains("codex_preflight_timeout_seconds = $codexPreflightTimeoutSecondsValue")
    );
    assert!(script.contains("ignore_user_config = $ignoreUserConfigValue"));
    assert!(script.contains("isolated_codex_home = $isolatedCodexHomeValue"));
    assert!(script.contains(
        "allow_harness_request_failure_comparison = $allowHarnessRequestFailureComparisonValue"
    ));
    assert!(script.contains(
        "allow_codex_request_failure_comparison = $allowCodexRequestFailureComparisonValue"
    ));
    assert!(script.contains("skip_codex_preflight = $skipCodexPreflightValue"));
    assert!(script.contains("preflight_only = $preflightOnlyValue"));
    assert!(script.contains("fail_on_directional_comparison = $failOnDirectionalComparisonValue"));
    assert!(script.contains(".\\scripts\\quick_comparison_benchmark.ps1"));
    assert!(script.contains("-Scenario\", $scenarioArgs"));
    assert!(script.contains("retry_after_seconds"));
    assert!(script.contains("retry_at_local"));
    assert!(script.contains("retry_at_utc"));
    assert!(script.contains("codex_preflight_retry_at_local=$($retryAt.Local)"));
    assert!(script.contains("codex_preflight_retry_at_utc=$($retryAt.Utc)"));
    assert!(script.contains("codex_preflight_retry_after_seconds=$($retryAt.DelaySeconds)"));
    assert!(script.contains("\"{0}-codex-preflight-{1}.json\""));
    assert!(script.contains("codex_preflight_codex_path=$codexCommandPath"));
    assert!(script.contains("codex_preflight_codex_version=$codexCommandVersion"));
    assert!(script.contains("codex_preflight_rerun_command=$rerunCommand"));
    assert!(script.contains("codex_preflight_resume_command=$resumeCommand"));
    assert!(script.contains("function Assert-HarnessReportComparable"));
    assert!(script.contains("function Assert-CodexReportComparable"));
    assert!(script.contains("function Format-ScenarioCountSummary"));
    assert!(script.contains("[datetime]$SinceUtc"));
    assert!(script.contains("function Get-LatestGeneratedFile"));
    assert!(script.contains("function Get-LatestHarnessReportFile"));
    assert!(script.contains("LastWriteTimeUtc -ge $SinceUtc"));
    assert!(script.contains("Name -notlike \"$Suite-codex-preflight-*.json\""));
    assert!(script.contains("$sparkRunStartedAt = [datetime]::UtcNow"));
    assert!(script.contains("$harnessReportStartedAt = [datetime]::UtcNow"));
    assert!(script.contains("$codexRunStartedAt = [datetime]::UtcNow"));
    assert!(script.contains("$comparisonStartedAt = [datetime]::UtcNow"));
    assert!(script.contains("-SinceUtc $sparkRunStartedAt"));
    assert!(script.contains("-SinceUtc $harnessReportStartedAt"));
    assert!(script.contains("-SinceUtc $codexRunStartedAt"));
    assert!(script.contains("-SinceUtc $comparisonStartedAt"));
    assert!(script.contains("request_failure"));
    assert!(script.contains("request_failure_scenarios"));
    assert!(script.contains("$requestFailureScenarios = Format-ScenarioCountSummary"));
    assert!(script.contains("function Format-ScenarioCountPairs"));
    assert!(script.contains("$requestFailureScenarioPairs = Format-ScenarioCountPairs"));
    assert!(script.contains("comparable_runs={1}, provider_api_failure={2}"));
    assert!(script.contains("Scenarios: $($parts -join ', '). "));
    assert!(script.contains("return ($parts -join \",\")"));
    assert!(script.contains("comparable_runs"));
    assert!(script.contains("[string]$ManifestPath"));
    assert!(script.contains("[string]$HarnessReportPath"));
    assert!(script.contains("Write-Host \"harness_manifest=$ManifestPath\""));
    assert!(script.contains("Write-Host \"harness_report=$HarnessReportPath\""));
    assert!(script.contains("Skipping native Codex CLI comparison"));
    assert!(
        script.contains(
            "if (-not $AllowRequestFailureComparison -and $requestFailures -gt 0 -and $comparableRuns -eq 0)"
        )
    );
    assert!(script.contains("harness_provider_api_failures={0} comparable_spark_rows={1} harness_provider_api_failure_scenarios={2} {3}"));
    assert!(script.contains("codex_provider_api_failures={0} comparable_codex_rows={1} codex_provider_api_failure_scenarios={2}"));
    assert!(
        script.contains(
            "if (-not $AllowRequestFailureComparison -and $comparableRuns -eq 0 -and $requestFailures -gt 0)"
        )
    );
    assert!(
        script.contains("-AllowRequestFailureComparison:$AllowHarnessRequestFailureComparison")
    );
    assert!(script.contains("-AllowRequestFailureComparison:$AllowCodexRequestFailureComparison"));
    assert!(script.contains("-ManifestPath $harnessManifest"));
    assert!(script.contains("-HarnessReportPath $harnessReport"));
    assert!(script.contains("Invoke-CodexPreflight `"));
    assert!(script.contains("-TimeoutSeconds $CodexPreflightTimeoutSeconds"));
    assert!(script.contains("$comparisonArgs = @("));
    assert!(script.contains("if ($FailOnDirectionalComparison)"));
    assert!(script.contains("$comparisonArgs += \"--fail-on-directional-comparison\""));
    assert!(script.contains("& cargo @comparisonArgs"));
    assert!(script.contains("if ($PreflightOnly)"));
    assert!(script.contains("preflight_only=true"));
    let preflight_only_marker = script
        .find("preflight_only=true")
        .expect("preflight-only marker");
    let suite_marker = script
        .find("Write-Host \"benchmark_suite=$Suite\"")
        .expect("benchmark suite output marker");
    let codex_bin_marker = script
        .find("Write-Host \"codex_bin=$CodexBin\"")
        .expect("codex bin output marker");
    let scenario_count_marker = script
        .find("Write-Host \"scenario_count=$($Scenario.Count)\"")
        .expect("scenario count output marker");
    let scenario_list_marker = script
        .find("Write-Host \"scenarios=$($Scenario -join ',')\"")
        .expect("scenario list output marker");
    let preflight_marker = script
        .find("Invoke-CodexPreflight `")
        .expect("preflight marker");
    let spark_run_marker = script
        .find("\"profile-benchmark\", $Suite")
        .expect("spark benchmark marker");
    assert!(suite_marker < codex_bin_marker);
    assert!(codex_bin_marker < scenario_count_marker);
    assert!(scenario_count_marker < scenario_list_marker);
    assert!(scenario_list_marker < preflight_marker);
    assert!(preflight_only_marker < spark_run_marker);
}

#[cfg(windows)]
#[test]
fn quick_comparison_harness_guard_executes_provider_api_filter() {
    let dir = tempfile::tempdir().expect("tempdir");
    let local_trace = dir.path().join("local-max-turns");
    let infra_trace = dir.path().join("provider-api-limit");
    std::fs::create_dir_all(&local_trace).expect("local trace dir");
    std::fs::create_dir_all(&infra_trace).expect("infra trace dir");
    std::fs::write(
        local_trace.join("002-max_turns-error.json"),
        r#"{"stage":"max_turns","error":"stopped after 0 turns without completion"}"#,
    )
    .expect("write local trace");
    std::fs::write(
        infra_trace.join("002-response-error.json"),
        r#"{"error":{"message":"You've hit your usage limit for GPT-5.3-Codex-Spark."}}"#,
    )
    .expect("write infra trace");

    let local_report = dir.path().join("local-report.json");
    let infra_report = dir.path().join("infra-report.json");
    let mixed_report = dir.path().join("mixed-report.json");
    let codex_partial_report = dir.path().join("codex-partial-report.json");
    let harness_report_candidate = dir.path().join("real-world-20260610-000000.json");
    let preflight_report_candidate = dir
        .path()
        .join("real-world-codex-preflight-20260610-000001.json");
    std::fs::write(
        &local_report,
        serde_json::to_string(&json!({
            "rows": [{
                "trace_dir": local_trace.display().to_string(),
                "scenario": "precise-patch",
                "diagnostics": "request_failure",
                "failure_points": "max_turns"
            }],
            "aggregate": {"successful_runs": 0}
        }))
        .expect("local report json"),
    )
    .expect("write local report");
    std::fs::write(
        &infra_report,
        serde_json::to_string(&json!({
            "rows": [{
                "trace_dir": infra_trace.display().to_string(),
                "scenario": "config-migration",
                "diagnostics": "request_failure",
                "failure_points": "response_error"
            }],
            "aggregate": {"successful_runs": 0}
        }))
        .expect("infra report json"),
    )
    .expect("write infra report");
    std::fs::write(
        &mixed_report,
        serde_json::to_string(&json!({
            "rows": [
                {
                    "trace_dir": infra_trace.display().to_string(),
                    "scenario": "config-migration",
                    "diagnostics": "request_failure",
                    "failure_points": "response_error"
                },
                {
                    "trace_dir": local_trace.display().to_string(),
                    "scenario": "precise-patch",
                    "diagnostics": "request_failure",
                    "failure_points": "max_turns"
                }
            ],
            "aggregate": {"successful_runs": 0}
        }))
        .expect("mixed report json"),
    )
    .expect("write mixed report");
    std::fs::write(
        &codex_partial_report,
        serde_json::to_string(&json!({
            "aggregate": {
                "runs": 2,
                "successful_runs": 1,
                "comparable_runs": 1,
                "diagnostics": {
                    "request_failure": 1,
                    "request_failure_scenarios": {
                        "config-migration": 1
                    }
                }
            }
        }))
        .expect("codex partial report json"),
    )
    .expect("write codex partial report");

    let repo_root = std::env::current_dir().expect("current dir");
    let script_path = repo_root
        .join("scripts")
        .join("quick_comparison_benchmark.ps1");
    let command = format!(
        r#"
$ErrorActionPreference = 'Stop'
$RepoRoot = {repo_root}
$script = Get-Content -LiteralPath {script_path} -Raw
$start = $script.IndexOf('function Test-InfrastructureFailureText')
$end = $script.IndexOf('Push-Location $RepoRoot')
if ($start -lt 0 -or $end -lt 0) {{ throw 'failed to locate quick comparison function block' }}
Invoke-Expression $script.Substring($start, $end - $start)
$preflightExcerpt = Format-PreflightFailureExcerpt -Text "noise`n{{`"message`":`"You've hit your usage limit for GPT-5.3-Codex-Spark. Switch to another model now, or try again at 5:38 PM.`"}}"
if ($preflightExcerpt -notlike '*usage limit*') {{
    throw 'expected preflight failure excerpt'
}}
if ($preflightExcerpt -like '*{{*') {{
    throw 'expected preflight excerpt to extract JSON message text'
}}
$preflightRetryHint = Format-PreflightRetryHint -Text "noise`n{{`"message`":`"You've hit your usage limit for GPT-5.3-Codex-Spark. Switch to another model now, or try again at 5:38 PM.`"}}"
if ($preflightRetryHint -ne 'try again at 5:38 PM') {{
    throw "expected preflight retry hint, got '$preflightRetryHint'"
}}
$absoluteRetryAt = Resolve-PreflightRetryAt -RetryHint 'try again at 5:38 PM' -ReferenceLocalTime ([datetime]'2026-06-10T16:12:00')
if ($absoluteRetryAt.Local -notlike '2026-06-10T17:38:00*') {{
    throw "expected same-day absolute retry timestamp, got '$($absoluteRetryAt.Local)'"
}}
if ($absoluteRetryAt.DelaySeconds -ne 5160) {{
    throw "expected same-day retry delay seconds, got '$($absoluteRetryAt.DelaySeconds)'"
}}
$nextDayRetryAt = Resolve-PreflightRetryAt -RetryHint 'try again at 5:38 PM' -ReferenceLocalTime ([datetime]'2026-06-10T18:12:00')
if ($nextDayRetryAt.Local -notlike '2026-06-11T17:38:00*') {{
    throw "expected next-day absolute retry timestamp, got '$($nextDayRetryAt.Local)'"
}}
if ($nextDayRetryAt.DelaySeconds -ne 84360) {{
    throw "expected next-day retry delay seconds, got '$($nextDayRetryAt.DelaySeconds)'"
}}
$relativeRetryAt = Resolve-PreflightRetryAt -RetryHint 'try again in 10 minutes' -ReferenceLocalTime ([datetime]'2026-06-10T16:12:00')
if ($relativeRetryAt.Local -notlike '2026-06-10T16:22:00*') {{
    throw "expected relative retry timestamp, got '$($relativeRetryAt.Local)'"
}}
if ($relativeRetryAt.DelaySeconds -ne 600) {{
    throw "expected relative retry delay seconds, got '$($relativeRetryAt.DelaySeconds)'"
}}
Set-Content -LiteralPath {harness_report_candidate} -Value '{{"rows":[]}}'
Set-Content -LiteralPath {preflight_report_candidate} -Value '{{"status":"failed"}}'
(Get-Item -LiteralPath {harness_report_candidate}).LastWriteTimeUtc = [datetime]'2026-06-10T16:00:00Z'
(Get-Item -LiteralPath {preflight_report_candidate}).LastWriteTimeUtc = [datetime]'2026-06-10T16:01:00Z'
$selectedHarnessReport = Get-LatestHarnessReportFile -Directory {report_dir} -Suite 'real-world' -SinceUtc ([datetime]'2026-06-10T15:59:00Z')
$expectedHarnessReport = (Get-Item -LiteralPath {harness_report_candidate}).FullName
if ($selectedHarnessReport -ne $expectedHarnessReport) {{
    throw "expected harness report selection to ignore preflight artifact, got '$selectedHarnessReport'"
}}
$statusPath = Write-CodexPreflightStatus -Directory {report_dir} -Suite 'real-world' -Status 'failed' -CodexBin 'codex' -Model 'gpt-5.3-codex-spark' -ReasoningEffort 'medium' -Scenario @('precise-patch','ops-report') -ExitCode 1 -RetryHint 'try again in 10 minutes'
$status = Get-Content -LiteralPath $statusPath -Raw | ConvertFrom-Json
if (-not $status.retry_at_local -or -not $status.retry_at_utc) {{
    throw 'expected preflight status artifact to include retry timestamps'
}}
if ($status.scenario_count -ne 2) {{
    throw "expected preflight status artifact to include 2 scenarios, got '$($status.scenario_count)'"
}}
if ($status.scenarios -join ',' -ne 'precise-patch,ops-report') {{
    throw "expected preflight status artifact to include scenario names, got '$($status.scenarios -join ',')'"
}}
if (-not ($status.PSObject.Properties.Name -contains 'codex_command_path')) {{
    throw 'expected preflight status artifact to include codex_command_path'
}}
if (-not ($status.PSObject.Properties.Name -contains 'codex_command_version')) {{
    throw 'expected preflight status artifact to include codex_command_version'
}}
if ($status.repeat -ne 1 -or $status.max_turns -ne 45 -or $status.timeout_seconds -ne 900 -or $status.codex_preflight_timeout_seconds -ne 120) {{
    throw "expected preflight status artifact to include default run metadata, got repeat=$($status.repeat) max_turns=$($status.max_turns) timeout_seconds=$($status.timeout_seconds) codex_preflight_timeout_seconds=$($status.codex_preflight_timeout_seconds)"
}}
if ($status.ignore_user_config -or $status.isolated_codex_home -or $status.allow_harness_request_failure_comparison -or $status.allow_codex_request_failure_comparison -or $status.skip_codex_preflight -or $status.preflight_only -or $status.fail_on_directional_comparison) {{
    throw 'expected direct preflight status artifact to include false switch defaults'
}}
if ($status.rerun_command -notlike '*quick_comparison_benchmark.ps1*' -or $status.rerun_command -notlike '*-Scenario*' -or $status.rerun_command -notlike '*precise-patch*' -or $status.rerun_command -notlike '*ops-report*' -or $status.rerun_command -notlike "*-CodexBin 'codex'*") {{
    throw "expected preflight status artifact to include actionable rerun command, got '$($status.rerun_command)'"
}}
if ($status.codex_command_path) {{
    if ($status.resume_command -notlike "*-CodexBin '$($status.codex_command_path -replace "'", "''")'*") {{
        throw "expected preflight resume command to pin resolved codex path, got '$($status.resume_command)'"
    }}
}} elseif ($status.resume_command -ne $status.rerun_command) {{
    throw "expected preflight status artifact without a resolved codex path or -PreflightOnly to use same rerun and resume command, got resume='$($status.resume_command)' rerun='$($status.rerun_command)'"
}}
if ($status.retry_after_seconds -ne 600) {{
    throw "expected preflight status artifact to include retry delay seconds, got '$($status.retry_after_seconds)'"
}}
$resolvedStatusPath = Write-CodexPreflightStatus -Directory {report_dir} -Suite 'real-world' -Status 'ok' -CodexBin 'powershell' -Model 'gpt-5.3-codex-spark' -ReasoningEffort 'medium' -Scenario @('precise-patch') -ExitCode 0
$resolvedStatus = Get-Content -LiteralPath $resolvedStatusPath -Raw | ConvertFrom-Json
if (-not $resolvedStatus.codex_command_path) {{
    throw 'expected powershell command path to resolve'
}}
if (-not $resolvedStatus.codex_command_version) {{
    throw 'expected powershell command version to resolve'
}}
if ($resolvedStatus.rerun_command -notlike "*-CodexBin 'powershell'*") {{
    throw "expected exact rerun command to preserve original command name, got '$($resolvedStatus.rerun_command)'"
}}
if ($resolvedStatus.resume_command -notlike "*-CodexBin '$($resolvedStatus.codex_command_path -replace "'", "''")'*") {{
    throw "expected resume command to pin resolved command path, got '$($resolvedStatus.resume_command)'"
}}
$script:Repeat = 2
$script:MaxTurns = 33
$script:TimeoutSeconds = 444
$script:CodexPreflightTimeoutSeconds = 55
$script:IgnoreUserConfig = $true
$script:IsolatedCodexHome = $true
$script:AllowHarnessRequestFailureComparison = $true
$script:AllowCodexRequestFailureComparison = $true
$script:SkipCodexPreflight = $true
$script:PreflightOnly = $true
$script:FailOnDirectionalComparison = $true
$switchedStatusPath = Write-CodexPreflightStatus -Directory {report_dir} -Suite 'real-world' -Status 'skipped' -CodexBin 'custom-codex' -Model 'gpt-5.3-codex-spark' -ReasoningEffort 'high' -Scenario @('ci-failure-triage','pull-request-review') -Message 'switch-state test'
$switchedStatus = Get-Content -LiteralPath $switchedStatusPath -Raw | ConvertFrom-Json
if (-not $switchedStatus.ignore_user_config -or -not $switchedStatus.isolated_codex_home -or -not $switchedStatus.allow_harness_request_failure_comparison -or -not $switchedStatus.allow_codex_request_failure_comparison -or -not $switchedStatus.skip_codex_preflight -or -not $switchedStatus.preflight_only -or -not $switchedStatus.fail_on_directional_comparison) {{
    throw 'expected switched preflight status artifact to preserve true switch state'
}}
if ($switchedStatus.repeat -ne 2 -or $switchedStatus.max_turns -ne 33 -or $switchedStatus.timeout_seconds -ne 444 -or $switchedStatus.codex_preflight_timeout_seconds -ne 55) {{
    throw "expected switched preflight status artifact to preserve run metadata, got repeat=$($switchedStatus.repeat) max_turns=$($switchedStatus.max_turns) timeout_seconds=$($switchedStatus.timeout_seconds) codex_preflight_timeout_seconds=$($switchedStatus.codex_preflight_timeout_seconds)"
}}
if ($switchedStatus.scenarios -join ',' -ne 'ci-failure-triage,pull-request-review') {{
    throw "expected switched preflight status artifact to preserve scenarios, got '$($switchedStatus.scenarios -join ',')'"
}}
if (-not ($switchedStatus.PSObject.Properties.Name -contains 'codex_command_path')) {{
    throw 'expected switched preflight status artifact to include codex_command_path'
}}
if (-not ($switchedStatus.PSObject.Properties.Name -contains 'codex_command_version')) {{
    throw 'expected switched preflight status artifact to include codex_command_version'
}}
foreach ($needle in @('-IgnoreUserConfig','-IsolatedCodexHome','-AllowHarnessRequestFailureComparison','-AllowCodexRequestFailureComparison','-SkipCodexPreflight','-PreflightOnly','-FailOnDirectionalComparison','-CodexPreflightTimeoutSeconds 55','ci-failure-triage','pull-request-review',"-CodexBin 'custom-codex'")) {{
    if ($switchedStatus.rerun_command -notlike "*$needle*") {{
        throw "expected switched rerun command to include '$needle', got '$($switchedStatus.rerun_command)'"
    }}
}}
foreach ($needle in @('-IgnoreUserConfig','-IsolatedCodexHome','-AllowHarnessRequestFailureComparison','-AllowCodexRequestFailureComparison','-SkipCodexPreflight','-FailOnDirectionalComparison','-CodexPreflightTimeoutSeconds 55','ci-failure-triage','pull-request-review',"-CodexBin 'custom-codex'")) {{
    if ($switchedStatus.resume_command -notlike "*$needle*") {{
        throw "expected switched resume command to include '$needle', got '$($switchedStatus.resume_command)'"
    }}
}}
if ($switchedStatus.resume_command -like '*-PreflightOnly*') {{
    throw "expected switched resume command to omit -PreflightOnly, got '$($switchedStatus.resume_command)'"
}}
Assert-HarnessReportComparable -ReportPath {local_report} -ManifestPath 'manifest.json' -AllowRequestFailureComparison:$false
Assert-HarnessReportComparable -ReportPath {mixed_report} -ManifestPath 'manifest.json' -AllowRequestFailureComparison:$false
Assert-CodexReportComparable -ReportPath {codex_partial_report} -ManifestPath 'manifest.json' -HarnessReportPath 'harness-report.json' -AllowRequestFailureComparison:$false
try {{
    Assert-HarnessReportComparable -ReportPath {infra_report} -ManifestPath 'manifest.json' -AllowRequestFailureComparison:$false
    throw 'expected infrastructure failure guard to throw'
}} catch {{
    if ($_.Exception.Message -notlike '*comparable_runs=0, provider_api_failure=1*') {{ throw }}
}}
Write-Host 'quick comparison harness guard ok'
"#,
        repo_root = powershell_single_quoted_path(&repo_root),
        script_path = powershell_single_quoted_path(&script_path),
        local_report = powershell_single_quoted_path(&local_report),
        infra_report = powershell_single_quoted_path(&infra_report),
        mixed_report = powershell_single_quoted_path(&mixed_report),
        codex_partial_report = powershell_single_quoted_path(&codex_partial_report),
        harness_report_candidate = powershell_single_quoted_path(&harness_report_candidate),
        preflight_report_candidate = powershell_single_quoted_path(&preflight_report_candidate),
        report_dir = powershell_single_quoted_path(dir.path()),
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &command])
        .output()
        .expect("run powershell");

    assert!(
        output.status.success(),
        "PowerShell guard check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(
        "harness_provider_api_failures=1 comparable_spark_rows=1 harness_provider_api_failure_scenarios=config-migration:1"
    ));
    assert!(stdout.contains(
        "codex_provider_api_failures=1 comparable_codex_rows=1 codex_provider_api_failure_scenarios=config-migration:1"
    ));
    assert!(stdout.contains("codex_preflight_retry_at_local="));
    assert!(stdout.contains("codex_preflight_retry_at_utc="));
    assert!(stdout.contains("codex_preflight_retry_after_seconds=600"));
    assert!(stdout.contains("codex_preflight_codex_path="));
    assert!(stdout.contains("codex_preflight_codex_version="));
    assert!(stdout.contains("codex_preflight_rerun_command="));
    assert!(stdout.contains("codex_preflight_resume_command="));
    assert!(stdout.contains("-AllowHarnessRequestFailureComparison"));
    assert!(stdout.contains("-AllowCodexRequestFailureComparison"));
    assert!(stdout.contains("quick comparison harness guard ok"));
}

#[cfg(windows)]
#[test]
fn quick_comparison_preflight_only_skips_isolated_codex_home() {
    let script_path = std::env::current_dir()
        .expect("current dir")
        .join("scripts")
        .join("quick_comparison_benchmark.ps1");
    let output = std::process::Command::new("powershell")
        .arg("-NoProfile")
        .arg("-File")
        .arg(script_path)
        .args([
            "-PreflightOnly",
            "-IsolatedCodexHome",
            "-CodexBin",
            "definitely-not-a-real-codex-binary",
        ])
        .output()
        .expect("run quick comparison script");

    assert!(
        output.status.success(),
        "isolated preflight-only check should not launch Codex\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("codex_preflight=skipped reason=isolated-codex-home"));
    assert!(stdout.contains("preflight_only=true"));
    assert!(stdout.contains("codex_preflight_codex_path="));
    assert!(stdout.contains("codex_preflight_codex_version="));
    assert!(stdout.contains("codex_preflight_rerun_command="));
    assert!(stdout.contains("codex_preflight_resume_command="));
    assert!(stdout.contains("-IsolatedCodexHome"));
    let status_path = stdout
        .lines()
        .find_map(|line| line.strip_prefix("codex_preflight_status="))
        .expect("preflight status path")
        .trim();
    let status: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(status_path).expect("read preflight status artifact"),
    )
    .expect("parse preflight status artifact");
    assert_eq!(status["suite"], "real-world");
    assert_eq!(status["status"], "skipped");
    assert_eq!(status["codex_bin"], "definitely-not-a-real-codex-binary");
    assert_eq!(status["codex_command_path"], "");
    assert_eq!(status["codex_command_version"], "");
    assert_eq!(status["repeat"], 1);
    assert_eq!(status["max_turns"], 45);
    assert_eq!(status["timeout_seconds"], 900);
    assert_eq!(status["codex_preflight_timeout_seconds"], 120);
    assert_eq!(status["ignore_user_config"], false);
    assert_eq!(status["isolated_codex_home"], true);
    assert_eq!(status["allow_harness_request_failure_comparison"], false);
    assert_eq!(status["allow_codex_request_failure_comparison"], false);
    assert_eq!(status["skip_codex_preflight"], false);
    assert_eq!(status["preflight_only"], true);
    assert_eq!(status["fail_on_directional_comparison"], false);
    assert_eq!(status["scenario_count"], 14);
    assert_eq!(status["scenarios"][0], "precise-patch");
    let rerun_command = status["rerun_command"]
        .as_str()
        .expect("rerun command string");
    assert!(rerun_command.contains(".\\scripts\\quick_comparison_benchmark.ps1"));
    assert!(rerun_command.contains("-Scenario"));
    assert!(rerun_command.contains("precise-patch"));
    assert!(rerun_command.contains("rust-log-analyzer-scaffold"));
    assert!(rerun_command.contains("-PreflightOnly"));
    assert!(rerun_command.contains("-IsolatedCodexHome"));
    assert!(rerun_command.contains("-CodexBin 'definitely-not-a-real-codex-binary'"));
    let resume_command = status["resume_command"]
        .as_str()
        .expect("resume command string");
    assert!(resume_command.contains(".\\scripts\\quick_comparison_benchmark.ps1"));
    assert!(resume_command.contains("-Scenario"));
    assert!(resume_command.contains("precise-patch"));
    assert!(resume_command.contains("rust-log-analyzer-scaffold"));
    assert!(resume_command.contains("-IsolatedCodexHome"));
    assert!(resume_command.contains("-CodexBin 'definitely-not-a-real-codex-binary'"));
    assert!(!resume_command.contains("-PreflightOnly"));
    assert_eq!(
        status["scenarios"]
            .as_array()
            .expect("scenario array")
            .last()
            .expect("last scenario"),
        "rust-log-analyzer-scaffold"
    );
    assert!(status["retry_after_seconds"].is_null());
    assert_eq!(status["retry_at_local"], "");
    assert_eq!(status["retry_at_utc"], "");
    assert_eq!(
        status["message"],
        "Skipped because -IsolatedCodexHome prepares CODEX_HOME inside the Rust benchmark runner."
    );
}

fn quick_real_world_scenarios(helper: &str) -> Vec<String> {
    let Some(start) = helper.find("return @(") else {
        return Vec::new();
    };
    let after_start = &helper[start..];
    let Some(end) = after_start.find("\n    )") else {
        return Vec::new();
    };

    after_start[..end]
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim().trim_end_matches(',');
            if trimmed.starts_with('"') && trimmed.ends_with('"') {
                Some(trimmed.trim_matches('"').to_string())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn quick_script_default_scenarios_match_and_stay_in_real_world_suite() {
    let comparison = std::fs::read_to_string("scripts/quick_comparison_benchmark.ps1")
        .expect("read quick comparison script");
    let harness = std::fs::read_to_string("scripts/quick_harness_benchmark.ps1")
        .expect("read quick harness script");
    let helper = std::fs::read_to_string("scripts/quick_benchmark_scenarios.ps1")
        .expect("read quick scenario helper");
    let agents = std::fs::read_to_string("AGENTS.md").expect("read AGENTS.md");

    let scenarios = quick_real_world_scenarios(&helper);
    let real_world_scenarios = ProfileBenchmarkSuiteKind::RealWorld
        .scenarios()
        .iter()
        .map(|scenario| scenario.name())
        .collect::<Vec<_>>();

    assert!(comparison.contains("quick_benchmark_scenarios.ps1"));
    assert!(harness.contains("quick_benchmark_scenarios.ps1"));
    assert!(comparison.contains("$Scenario = @(Get-QuickRealWorldScenario)"));
    assert!(harness.contains("$Scenario = @(Get-QuickRealWorldScenario)"));
    assert!(comparison.contains("if ($ListScenarios)"));
    assert!(harness.contains("if ($ListScenarios)"));
    assert!(!scenarios.is_empty());
    for scenario in &scenarios {
        assert!(
            real_world_scenarios.contains(&scenario.as_str()),
            "{scenario} is not in the real-world suite"
        );
        assert!(
            agents.contains(scenario),
            "AGENTS.md quick benchmark guidance does not mention {scenario}"
        );
    }

    let mut deduped = scenarios.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(
        deduped.len(),
        scenarios.len(),
        "quick default scenarios should not contain duplicates"
    );
    assert!(scenarios.contains(&"pull-request-review".to_string()));
    assert!(scenarios.contains(&"dependency-upgrade-triage".to_string()));
    assert!(scenarios.contains(&"merge-conflict-resolution".to_string()));
    assert!(scenarios.contains(&"rust-log-analyzer-scaffold".to_string()));
    assert!(agents.contains(".\\scripts\\quick_harness_benchmark.ps1 -ListScenarios"));
}

#[test]
fn readme_documents_quick_scenario_listing_and_preflight_scenario_metadata() {
    let readme = std::fs::read_to_string("README.md").expect("read README");

    assert!(readme.contains(".\\scripts\\quick_comparison_benchmark.ps1 -ListScenarios"));
    assert!(readme.contains(".\\scripts\\quick_harness_benchmark.ps1 -ListScenarios"));
    assert!(readme.contains("benchmark_suite"));
    assert!(readme.contains("benchmark_model"));
    assert!(readme.contains("reasoning_effort"));
    assert!(readme.contains("repeat"));
    assert!(readme.contains("max_turns"));
    assert!(readme.contains("timeout_seconds"));
    assert!(readme.contains("codex_bin"));
    assert!(readme.contains("codex_command_path"));
    assert!(readme.contains("codex_command_version"));
    assert!(readme.contains("command_path"));
    assert!(readme.contains("command_version"));
    assert!(readme.contains("inputs"));
    assert!(readme.contains("Report Inputs"));
    assert!(readme.contains("input freshness warning"));
    assert!(readme.contains("scenario_count"));
    assert!(readme.contains("codex_preflight_timeout_seconds"));
    assert!(readme.contains("ignore_user_config"));
    assert!(readme.contains("isolated_codex_home"));
    assert!(readme.contains("allow_harness_request_failure_comparison"));
    assert!(readme.contains("allow_codex_request_failure_comparison"));
    assert!(readme.contains("skip_codex_preflight"));
    assert!(readme.contains("preflight_only"));
    assert!(readme.contains("fail_on_directional_comparison"));
    assert!(readme.contains("--fail-on-directional-comparison"));
    assert!(readme.contains("-FailOnDirectionalComparison"));
    assert!(readme.contains("rerun_command"));
    assert!(readme.contains("resume_command"));
    assert!(readme.contains("scenarios"));
    assert!(readme.contains("codex_preflight_status=..."));
    assert!(readme.contains("codex_preflight_codex_path=..."));
    assert!(readme.contains("codex_preflight_codex_version=..."));
    assert!(readme.contains("retry_after_seconds"));
    assert!(readme.contains("retry_at_local"));
    assert!(readme.contains("retry_at_utc"));
    assert!(readme.contains("codex_preflight_rerun_command=..."));
    assert!(readme.contains("codex_preflight_resume_command=..."));
}

#[cfg(windows)]
fn powershell_single_quoted_path(path: &std::path::Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "''"))
}

#[test]
fn quick_harness_script_reports_exact_run_manifest() {
    let script = std::fs::read_to_string("scripts/quick_harness_benchmark.ps1")
        .expect("read quick harness script");

    assert!(script.contains("$BenchmarkDir = Join-Path $RepoRoot"));
    assert!(script.contains("function Get-LatestGeneratedFile"));
    assert!(script.contains("function Get-LatestHarnessReportFile"));
    assert!(script.contains("[datetime]$SinceUtc"));
    assert!(script.contains("LastWriteTimeUtc -ge $SinceUtc"));
    assert!(script.contains("Name -notlike \"$Suite-run-*.json\""));
    assert!(script.contains("Name -notlike \"$Suite-comparison-*.json\""));
    assert!(script.contains("Name -notlike \"$Suite-codex-preflight-*.json\""));
    assert!(script.contains("$sparkRunStartedAt = [datetime]::UtcNow"));
    assert!(script.contains("$harnessReportStartedAt = [datetime]::UtcNow"));
    assert!(script.contains("New-Item -ItemType Directory -Force -Path $BenchmarkDir"));
    assert!(script.contains("benchmark_report=skipped reason=no-trace"));
    assert!(script.contains("[switch]$ListScenarios"));
    assert!(script.contains("[string[]]$Scenario = @()"));
    assert!(script.contains("quick_benchmark_scenarios.ps1"));
    assert!(script.contains("$Scenario = @(Get-QuickRealWorldScenario)"));
    assert!(script.contains("if ($ListScenarios)"));
    assert!(script.contains("$Scenario | ForEach-Object { Write-Output $_ }"));
    assert!(script.contains("Write-Host \"benchmark_suite=$Suite\""));
    assert!(script.contains("Write-Host \"benchmark_model=$Model\""));
    assert!(script.contains("Write-Host \"reasoning_effort=$ReasoningEffort\""));
    assert!(script.contains("Write-Host \"repeat=$Repeat\""));
    assert!(script.contains("Write-Host \"max_turns=$MaxTurns\""));
    assert!(script.contains("Write-Host \"scenario_count=$($Scenario.Count)\""));
    assert!(script.contains("Write-Host \"scenarios=$($Scenario -join ',')\""));
    assert!(
        script.contains(
            "$harnessManifest = Get-LatestGeneratedFile -Directory $BenchmarkDir -Filter \"$Suite-run-*.json\" -SinceUtc $sparkRunStartedAt"
        )
    );
    assert!(script.contains(
        "$harnessReport = Get-LatestHarnessReportFile -Directory $BenchmarkDir -Suite $Suite -SinceUtc $harnessReportStartedAt"
    ));
    assert!(script.contains("--run-manifest $harnessManifest"));
    assert!(script.contains("Write-Host \"harness_manifest=$harnessManifest\""));
    assert!(script.contains("Write-Host \"harness_report=$harnessReport\""));
    let no_trace_marker = script
        .find("benchmark_report=skipped reason=no-trace")
        .expect("no-trace report skip marker");
    let report_marker = script
        .find("--run-manifest $harnessManifest")
        .expect("run manifest report marker");
    let suite_marker = script
        .find("Write-Host \"benchmark_suite=$Suite\"")
        .expect("benchmark suite output marker");
    let max_turns_marker = script
        .find("Write-Host \"max_turns=$MaxTurns\"")
        .expect("max turns output marker");
    let scenario_count_marker = script
        .find("Write-Host \"scenario_count=$($Scenario.Count)\"")
        .expect("scenario count output marker");
    let scenario_list_marker = script
        .find("Write-Host \"scenarios=$($Scenario -join ',')\"")
        .expect("scenario list output marker");
    let benchmark_run_marker = script
        .find("& cargo @benchmarkArgs")
        .expect("benchmark run marker");
    assert!(no_trace_marker < report_marker);
    assert!(suite_marker < max_turns_marker);
    assert!(max_turns_marker < scenario_count_marker);
    assert!(scenario_count_marker < scenario_list_marker);
    assert!(scenario_list_marker < benchmark_run_marker);
    let manifest_output_marker = script
        .find("Write-Host \"harness_manifest=$harnessManifest\"")
        .expect("manifest output marker");
    let report_output_marker = script
        .find("Write-Host \"harness_report=$harnessReport\"")
        .expect("report output marker");
    assert!(report_marker < manifest_output_marker);
    assert!(manifest_output_marker < report_output_marker);
}

#[test]
fn slash_commands_match_exactly_or_with_whitespace() {
    assert_eq!(command_args("/skill", "/skill"), Some(""));
    assert_eq!(
        command_args("/skill load rust", "/skill"),
        Some("load rust")
    );
    assert_eq!(command_args("/compact", "/compact"), Some(""));
    assert_eq!(command_args("/compact now", "/compact"), Some("now"));
    assert_eq!(command_args("/compaction", "/compact"), None);
    assert_eq!(command_args("/profile", "/profile"), Some(""));
    assert_eq!(command_args("/profiles", "/profile"), None);
    assert_eq!(command_args("/skills", "/skill"), None);
    assert_eq!(command_args("/sessions", "/session"), None);
}

#[test]
fn slash_command_helpers_match_menu_and_unknown_warning() {
    assert_eq!(slash_command_token("/sk load rust"), Some("/sk"));
    assert_eq!(slash_command_token("hello /sk"), None);

    let matches = matching_slash_commands("/sk");
    assert!(matches.iter().any(|command| command.name == "/skill"));
    assert!(matches.iter().any(|command| command.name == "/skills"));
    assert!(unknown_slash_command_warning("/wat now").contains("unknown command: /wat"));
}

#[test]
fn parse_mode_accepts_ask_work_and_agent_alias() {
    assert_eq!(parse_mode("ask"), Some(crate::tools::AgentMode::Ask));
    assert_eq!(parse_mode("work"), Some(crate::tools::AgentMode::Work));
    assert_eq!(parse_mode("agent"), Some(crate::tools::AgentMode::Work));
    assert_eq!(parse_mode(""), None);
}

#[test]
fn output_text_delta_reads_streaming_response_events() {
    let event = serde_json::json!({
        "type": "response.output_text.delta",
        "delta": "hello"
    });

    assert_eq!(output_text_delta(&event), Some("hello"));
    assert_eq!(
        output_text_delta(&serde_json::json!({"type": "response.output_text.done"})),
        None
    );
}

#[test]
fn detects_repo_local_skill_mentions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let skill_dir = dir.path().join(".agents").join("skills").join("demo-skill");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: demo-skill\ndescription: Demo\n---\n\n# Demo\n",
    )
    .expect("write skill");

    let mentions = mentioned_skill_names(
        &dir.path().to_path_buf(),
        "Please use @demo-skill for this.",
    )
    .expect("mentions");

    assert_eq!(mentions, vec!["demo-skill"]);
}

#[test]
fn skill_mentions_require_boundaries() {
    assert!(contains_skill_mention(
        "Please use @demo-skill.",
        "@demo-skill"
    ));
    assert!(!contains_skill_mention(
        "Please use @demo-skill-extra.",
        "@demo-skill"
    ));
}

#[test]
fn trace_dirs_are_listed_newest_first() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = trace_runs_root(dir.path());
    std::fs::create_dir_all(root.join("run-100")).expect("create old trace");
    std::fs::create_dir_all(root.join("run-300")).expect("create new trace");
    std::fs::create_dir_all(root.join("run-200")).expect("create middle trace");
    std::fs::create_dir_all(root.join("other")).expect("create ignored dir");
    std::fs::write(root.join("run-400"), "{}").expect("create ignored file");

    let runs = list_trace_dirs(&root, 2).expect("list trace dirs");
    let names = runs
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["run-300", "run-200"]);
}

#[test]
fn trace_sort_metrics_read_expected_summary_fields() {
    let summary = json!({
        "max_approx_input_tokens": 42,
        "max_request_duration_ms": 1234,
        "tool_only_turns": {
            "max_consecutive": 8
        },
        "compaction_regrowth": {
            "max_next_request_growth_chars": 65536
        },
        "profile_scenario_call_expectations": {
            "extra_turns_after_satisfied": 6,
            "context_growth_after_satisfied_chars": 101846
        }
    });

    assert_eq!(
        trace_sort_metric(Some(&summary), TraceSort::OverrunContext),
        101_846
    );
    assert_eq!(
        trace_sort_metric(Some(&summary), TraceSort::OverrunTurns),
        6
    );
    assert_eq!(
        trace_sort_metric(Some(&summary), TraceSort::ToolOnlyStreak),
        8
    );
    assert_eq!(
        trace_sort_metric(Some(&summary), TraceSort::CompactionRegrowth),
        65_536
    );
    assert_eq!(trace_sort_metric(Some(&summary), TraceSort::Context), 42);
    assert_eq!(
        trace_sort_metric(Some(&summary), TraceSort::RequestMs),
        1_234
    );
    assert_eq!(trace_sort_metric(None, TraceSort::RequestMs), 0);
    assert_eq!(
        trace_sort_name(TraceSort::OverrunContext),
        "overrun-context"
    );
    assert_eq!(
        trace_sort_name(TraceSort::CompactionRegrowth),
        "compaction-regrowth"
    );
}

#[test]
fn trace_records_sort_by_worst_metric_then_newest_name() {
    let mut records = vec![
        TraceListRecord {
            run: PathBuf::from("run-100"),
            display: PathBuf::from("run-100"),
            summary: Some(json!({
                "tool_only_turns": {"max_consecutive": 2}
            })),
        },
        TraceListRecord {
            run: PathBuf::from("run-300"),
            display: PathBuf::from("run-300"),
            summary: Some(json!({
                "tool_only_turns": {"max_consecutive": 8}
            })),
        },
        TraceListRecord {
            run: PathBuf::from("run-200"),
            display: PathBuf::from("run-200"),
            summary: Some(json!({
                "tool_only_turns": {"max_consecutive": 8}
            })),
        },
    ];

    sort_trace_records(&mut records, TraceSort::ToolOnlyStreak);
    let names = records
        .iter()
        .map(|record| record.run.display().to_string())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["run-300", "run-200", "run-100"]);
}

#[test]
fn latest_trace_dir_uses_highest_run_suffix() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = trace_runs_root(dir.path());
    std::fs::create_dir_all(root.join("run-1")).expect("create old trace");
    std::fs::create_dir_all(root.join("run-2")).expect("create latest trace");

    let latest = latest_trace_dir(&root).expect("latest trace");

    assert_eq!(latest.file_name().unwrap(), "run-2");
}

#[test]
fn token_thresholds_resolve_to_estimated_chars() {
    let chars = resolve_char_threshold(
        "compact-after",
        None,
        Some(32_000),
        DEFAULT_COMPACT_AFTER_CHARS,
    )
    .expect("resolve threshold");

    assert_eq!(chars, 128_000);
}

#[test]
fn char_thresholds_conflict_with_token_thresholds() {
    let error = resolve_char_threshold("max-input", Some(1), Some(1), 10)
        .expect_err("conflicting thresholds");

    assert!(
        error
            .to_string()
            .contains("pass either --max-input-chars or --max-input-tokens")
    );
}

#[test]
fn scenario_repeat_must_be_in_supported_range() {
    validate_scenario_repeat(1).expect("repeat 1");
    validate_scenario_repeat(50).expect("max repeat");

    let zero = validate_scenario_repeat(0).expect_err("zero repeat");
    assert!(zero.to_string().contains("greater than 0"));

    let too_many = validate_scenario_repeat(51).expect_err("too many repeats");
    assert!(too_many.to_string().contains("<= 50"));
}

#[test]
fn trace_diagnostic_filter_requires_all_requested_kinds() {
    let summary = json!({
        "diagnostics": [
            {"kind": "tool_failures"},
            {"kind": "tool_failure_recovered"}
        ]
    });

    assert!(trace_has_all_diagnostics(
        &summary,
        &["tool_failures".to_string()]
    ));
    assert!(trace_has_all_diagnostics(
        &summary,
        &[
            "tool_failures".to_string(),
            "tool_failure_recovered".to_string()
        ]
    ));
    assert!(!trace_has_all_diagnostics(
        &summary,
        &[
            "tool_failures".to_string(),
            "weak_compaction_shrink".to_string()
        ]
    ));
    assert!(!trace_has_all_diagnostics(
        &json!({}),
        &["tool_failures".to_string()]
    ));
}

#[test]
fn trace_filter_label_includes_scenario_and_diagnostics() {
    assert_eq!(
        trace_filter_label(Some("tool-recovery"), &[], None, None, None, None),
        "tool-recovery"
    );
    assert_eq!(
        trace_filter_label(None, &["tool_failures".to_string()], None, None, None, None),
        "all diagnostics=tool_failures"
    );
    assert_eq!(
        trace_filter_label(
            Some("tool-recovery"),
            &[
                "tool_failures".to_string(),
                "tool_failure_recovered".to_string()
            ],
            None,
            None,
            None,
            None,
        ),
        "tool-recovery diagnostics=tool_failures,tool_failure_recovered"
    );
    assert_eq!(
        trace_filter_label(
            Some("skill-use"),
            &["tool_only_turn_streak".to_string()],
            Some(3),
            Some(2),
            Some(10_000),
            Some(64_000),
        ),
        "skill-use diagnostics=tool_only_turn_streak min_tool_only_streak=3 min_overrun_turns=2 min_overrun_context_chars=10000 min_compaction_regrowth_chars=64000"
    );
}

#[test]
fn trace_metric_filters_require_requested_thresholds() {
    let summary = json!({
        "tool_only_turns": {
            "max_consecutive": 8
        },
        "profile_scenario_call_expectations": {
            "extra_turns_after_satisfied": 6,
            "context_growth_after_satisfied_chars": 101846
        },
        "compaction_regrowth": {
            "max_next_request_growth_chars": 64000
        }
    });

    assert!(trace_matches_metric_filters(
        &summary,
        Some(8),
        Some(6),
        Some(101_846),
        Some(64_000)
    ));
    assert!(!trace_matches_metric_filters(
        &summary,
        Some(9),
        Some(6),
        Some(101_846),
        Some(64_000)
    ));
    assert!(!trace_matches_metric_filters(
        &summary,
        Some(8),
        Some(7),
        Some(101_846),
        Some(64_000)
    ));
    assert!(!trace_matches_metric_filters(
        &summary,
        Some(8),
        Some(6),
        Some(101_847),
        Some(64_000)
    ));
    assert!(!trace_matches_metric_filters(
        &summary,
        Some(8),
        Some(6),
        Some(101_846),
        Some(64_001)
    ));
    assert!(!trace_matches_metric_filters(
        &json!({}),
        Some(1),
        None,
        None,
        None
    ));
}

#[test]
fn trace_export_record_wraps_summary_with_paths() {
    let dir = tempfile::tempdir().expect("tempdir");
    let run = dir.path().join(".spark-runs").join("run-42");
    std::fs::create_dir_all(&run).expect("create trace dir");
    let display = PathBuf::from(".spark-runs").join("run-42");
    let summary = json!({
        "requests": 1,
        "tool_calls": 2,
    });

    let record = trace_export_record(dir.path(), &run, &display, Some(&summary));

    assert_eq!(record["type"], "trace");
    assert_eq!(
        record["trace_dir"],
        format!(".spark-runs{}run-42", std::path::MAIN_SEPARATOR)
    );
    assert!(
        record["trace_dir_abs"]
            .as_str()
            .unwrap()
            .ends_with("run-42")
    );
    assert_eq!(record["summary"]["requests"], 1);
    assert_eq!(record["summary"]["tool_calls"], 2);
}

#[test]
fn active_session_matching_handles_same_path() {
    let active = Some("session-a".to_string());

    assert!(is_active_session(&active, "session-a"));
    assert!(!is_active_session(&active, "session-b"));
    assert!(!is_active_session(&None, "session-a"));
}

#[test]
fn timestamp_session_name_is_filename_safe_and_not_workspace_scoped() {
    let name = timestamp_session_name();

    assert!(name.starts_with("chat-"));
    assert!(!name.starts_with("workspace-"));
    assert!(
        name.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
    );
}
