use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::process::Command;

use crate::{
    cli::{ProfileBenchmarkSuiteKind, ProfileScenarioKind},
    profile_scenarios,
};

#[derive(Debug, Clone)]
pub(crate) struct CodexCliBenchmarkOptions {
    pub(crate) cwd: PathBuf,
    pub(crate) suite: ProfileBenchmarkSuiteKind,
    pub(crate) model: String,
    pub(crate) repeat: usize,
    pub(crate) timeout_seconds: u64,
    pub(crate) ignore_user_config: bool,
    pub(crate) isolated_codex_home: bool,
    pub(crate) codex_bin: PathBuf,
    pub(crate) output_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodexCliBenchmarkRow {
    pub(crate) runner: String,
    pub(crate) suite: String,
    pub(crate) scenario: String,
    pub(crate) repeat_index: usize,
    pub(crate) model: String,
    pub(crate) score: f64,
    pub(crate) success: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) duration_ms: u128,
    pub(crate) json_events: u64,
    pub(crate) non_json_stdout_lines: u64,
    pub(crate) stderr_lines: u64,
    #[serde(default)]
    pub(crate) actionable_stderr_lines: u64,
    pub(crate) turns: u64,
    pub(crate) completed_items: u64,
    pub(crate) agent_messages: u64,
    pub(crate) tool_items: u64,
    pub(crate) input_tokens: u64,
    pub(crate) cached_input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) reasoning_output_tokens: u64,
    pub(crate) expected_artifacts: u64,
    pub(crate) present_artifacts: u64,
    pub(crate) validation_exit_code: Option<i32>,
    pub(crate) validation_timed_out: bool,
    pub(crate) final_message_chars: u64,
    pub(crate) run_dir: String,
    pub(crate) failure_points: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CodexCliBenchmarkOutput {
    pub(crate) json_path: PathBuf,
    pub(crate) rows: usize,
    pub(crate) aggregate: Value,
}

pub(crate) async fn run_codex_cli_benchmark(
    options: CodexCliBenchmarkOptions,
) -> Result<CodexCliBenchmarkOutput> {
    profile_scenarios::validate_scenario_repeat(options.repeat)?;
    std::fs::create_dir_all(&options.output_dir).map_err(|error| {
        anyhow::anyhow!(
            "failed to create Codex CLI benchmark output directory {}: {error}",
            options.output_dir.display()
        )
    })?;

    let started_at = unix_millis();
    let mut rows = Vec::new();
    for scenario in options.suite.scenarios() {
        for repeat_index in 1..=options.repeat {
            profile_scenarios::prepare_profile_scenario(&options.cwd, *scenario)?;
            let row = run_codex_cli_scenario(&options, *scenario, repeat_index, started_at).await?;
            println!(
                "codex_cli scenario={} repeat={}/{} score={:.1} success={} duration_ms={} failure_points={}",
                row.scenario,
                repeat_index,
                options.repeat,
                row.score,
                row.success,
                row.duration_ms,
                row.failure_points
            );
            rows.push(row);
        }
    }

    let aggregate = aggregate_rows(options.suite.name(), &rows);
    let json_path = options.output_dir.join(format!(
        "{}-codex-cli-{started_at}.json",
        options.suite.name()
    ));
    std::fs::write(
        &json_path,
        serde_json::to_string_pretty(&json!({
            "suite": options.suite.name(),
            "runner": "codex-cli",
            "generated_at_unix_ms": started_at,
            "rows": rows,
            "aggregate": aggregate,
        }))?,
    )
    .map_err(|error| anyhow::anyhow!("failed to write {}: {error}", json_path.display()))?;

    Ok(CodexCliBenchmarkOutput {
        json_path,
        rows: rows.len(),
        aggregate,
    })
}

async fn run_codex_cli_scenario(
    options: &CodexCliBenchmarkOptions,
    scenario: ProfileScenarioKind,
    repeat_index: usize,
    batch_stamp: u128,
) -> Result<CodexCliBenchmarkRow> {
    let scenario_name = scenario.name();
    let run_dir = options
        .output_dir
        .join("runs")
        .join(format!("run-{batch_stamp}-{scenario_name}-{repeat_index}"));
    std::fs::create_dir_all(&run_dir).map_err(|error| {
        anyhow::anyhow!(
            "failed to create Codex CLI run dir {}: {error}",
            run_dir.display()
        )
    })?;
    let isolated_codex_home = if options.isolated_codex_home {
        Some(prepare_isolated_codex_home(&options.cwd, &run_dir)?)
    } else {
        None
    };
    let final_message_path = run_dir.join("last-message.txt");
    let prompt = profile_scenarios::codex_cli_benchmark_prompt(scenario);
    std::fs::write(run_dir.join("prompt.txt"), &prompt)
        .map_err(|error| anyhow::anyhow!("failed to write Codex CLI prompt: {error}"))?;

    let mut command = Command::new(&options.codex_bin);
    command
        .arg("exec")
        .arg("--json")
        .arg("--cd")
        .arg(&options.cwd)
        .arg("--sandbox")
        .arg("danger-full-access")
        .arg("--dangerously-bypass-approvals-and-sandbox")
        .arg("--model")
        .arg(&options.model)
        .arg("--output-last-message")
        .arg(&final_message_path);
    if options.ignore_user_config {
        command.arg("--ignore-user-config");
    }
    command.arg("--ignore-rules");
    if let Some(codex_home) = &isolated_codex_home {
        command
            .arg("--disable")
            .arg("plugins")
            .arg("--disable")
            .arg("plugin_sharing")
            .arg("--disable")
            .arg("remote_plugin")
            .arg("--disable")
            .arg("skill_mcp_dependency_install");
        command.env("CODEX_HOME", codex_home);
    }
    command.arg(prompt);

    let started = std::time::Instant::now();
    let output_result = tokio::time::timeout(
        Duration::from_secs(options.timeout_seconds),
        command.output(),
    )
    .await;
    let duration_ms = started.elapsed().as_millis();

    let (stdout, stderr, exit_code, timed_out) = match output_result {
        Ok(Ok(output)) => (
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
            output.status.code(),
            false,
        ),
        Ok(Err(error)) => {
            anyhow::bail!("failed to run Codex CLI for {scenario_name}: {error}");
        }
        Err(_) => (
            String::new(),
            "timed out waiting for Codex CLI".to_string(),
            None,
            true,
        ),
    };

    std::fs::write(run_dir.join("stdout.jsonl"), &stdout)
        .map_err(|error| anyhow::anyhow!("failed to write Codex CLI stdout: {error}"))?;
    std::fs::write(run_dir.join("stderr.txt"), &stderr)
        .map_err(|error| anyhow::anyhow!("failed to write Codex CLI stderr: {error}"))?;

    let final_message = std::fs::read_to_string(&final_message_path).unwrap_or_default();
    let metrics = parse_codex_json_events(&stdout);
    let stderr_metrics = classify_stderr(&stderr);
    let expected_artifacts = expected_artifacts(scenario);
    let present_artifacts = expected_artifacts
        .iter()
        .filter(|path| options.cwd.join(path).exists())
        .count() as u64;
    let validation = run_validation_command(&options.cwd, &run_dir, scenario).await?;
    let failure_points = codex_failure_points(
        exit_code,
        timed_out,
        &metrics,
        &stderr_metrics,
        &final_message,
        expected_artifacts.len() as u64,
        present_artifacts,
        validation.exit_code,
        validation.timed_out,
    );

    let mut row = CodexCliBenchmarkRow {
        runner: "codex-cli".to_string(),
        suite: options.suite.name().to_string(),
        scenario: scenario_name.to_string(),
        repeat_index,
        model: options.model.clone(),
        score: 0.0,
        success: exit_code == Some(0)
            && !timed_out
            && !final_message.trim().is_empty()
            && present_artifacts == expected_artifacts.len() as u64
            && validation.exit_code.is_none_or(|code| code == 0)
            && !validation.timed_out,
        exit_code,
        timed_out,
        duration_ms,
        json_events: metrics.json_events,
        non_json_stdout_lines: metrics.non_json_stdout_lines,
        stderr_lines: stderr
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count() as u64,
        actionable_stderr_lines: stderr_metrics.actionable_lines,
        turns: metrics.turns,
        completed_items: metrics.completed_items,
        agent_messages: metrics.agent_messages,
        tool_items: metrics.tool_items,
        input_tokens: metrics.input_tokens,
        cached_input_tokens: metrics.cached_input_tokens,
        output_tokens: metrics.output_tokens,
        reasoning_output_tokens: metrics.reasoning_output_tokens,
        expected_artifacts: expected_artifacts.len() as u64,
        present_artifacts,
        validation_exit_code: validation.exit_code,
        validation_timed_out: validation.timed_out,
        final_message_chars: final_message.chars().count() as u64,
        run_dir: run_dir.display().to_string(),
        failure_points: failure_points.join(";"),
    };
    row.score = codex_score(&row);
    Ok(row)
}

fn prepare_isolated_codex_home(cwd: &Path, run_dir: &Path) -> Result<PathBuf> {
    let source_home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .map(|home| home.join(".codex"))
        })
        .unwrap_or_else(|| cwd.join(".codex"));
    let isolated = run_dir.join("codex-home");
    std::fs::create_dir_all(&isolated).map_err(|error| {
        anyhow::anyhow!(
            "failed to create isolated CODEX_HOME {}: {error}",
            isolated.display()
        )
    })?;
    let auth = source_home.join("auth.json");
    if auth.exists() {
        std::fs::copy(&auth, isolated.join("auth.json")).map_err(|error| {
            anyhow::anyhow!(
                "failed to copy Codex auth from {} to {}: {error}",
                auth.display(),
                isolated.display()
            )
        })?;
    }
    Ok(isolated)
}

#[derive(Default)]
struct CodexEventMetrics {
    json_events: u64,
    non_json_stdout_lines: u64,
    turns: u64,
    completed_items: u64,
    agent_messages: u64,
    tool_items: u64,
    input_tokens: u64,
    cached_input_tokens: u64,
    output_tokens: u64,
    reasoning_output_tokens: u64,
}

struct ValidationResult {
    exit_code: Option<i32>,
    timed_out: bool,
}

#[derive(Debug, Default, Clone, Copy)]
struct StderrMetrics {
    non_empty_lines: u64,
    actionable_lines: u64,
}

async fn run_validation_command(
    cwd: &Path,
    run_dir: &Path,
    scenario: ProfileScenarioKind,
) -> Result<ValidationResult> {
    let Some(spec) = profile_scenarios::profile_scenario_validation_command(scenario) else {
        return Ok(ValidationResult {
            exit_code: None,
            timed_out: false,
        });
    };

    let output = tokio::time::timeout(
        Duration::from_secs(180),
        Command::new(spec.program)
            .args(spec.args)
            .current_dir(cwd.join(spec.workdir))
            .output(),
    )
    .await;

    match output {
        Ok(Ok(output)) => {
            std::fs::write(run_dir.join("validation-stdout.txt"), &output.stdout)
                .map_err(|error| anyhow::anyhow!("failed to write validation stdout: {error}"))?;
            std::fs::write(run_dir.join("validation-stderr.txt"), &output.stderr)
                .map_err(|error| anyhow::anyhow!("failed to write validation stderr: {error}"))?;
            Ok(ValidationResult {
                exit_code: output.status.code(),
                timed_out: false,
            })
        }
        Ok(Err(error)) => {
            std::fs::write(
                run_dir.join("validation-stderr.txt"),
                format!("failed to start validation command: {error}"),
            )
            .map_err(|write_error| {
                anyhow::anyhow!("failed to write validation stderr: {write_error}")
            })?;
            Ok(ValidationResult {
                exit_code: None,
                timed_out: false,
            })
        }
        Err(_) => {
            std::fs::write(
                run_dir.join("validation-stderr.txt"),
                "validation timed out",
            )
            .map_err(|error| anyhow::anyhow!("failed to write validation timeout: {error}"))?;
            Ok(ValidationResult {
                exit_code: None,
                timed_out: true,
            })
        }
    }
}

fn parse_codex_json_events(stdout: &str) -> CodexEventMetrics {
    let mut metrics = CodexEventMetrics::default();
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            metrics.non_json_stdout_lines += 1;
            continue;
        };
        metrics.json_events += 1;
        if value.get("type").and_then(Value::as_str) == Some("turn.completed") {
            metrics.turns += 1;
            if let Some(usage) = value.get("usage") {
                metrics.input_tokens += usage
                    .get("input_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                metrics.cached_input_tokens += usage
                    .get("cached_input_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                metrics.output_tokens += usage
                    .get("output_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                metrics.reasoning_output_tokens += usage
                    .get("reasoning_output_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
            }
        }
        if value.get("type").and_then(Value::as_str) == Some("item.completed") {
            metrics.completed_items += 1;
            let item_type = value
                .pointer("/item/type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if item_type == "agent_message" {
                metrics.agent_messages += 1;
            }
            if item_type.contains("tool")
                || item_type.contains("call")
                || item_type == "command_execution"
                || item_type == "file_change"
            {
                metrics.tool_items += 1;
            }
        }
    }
    metrics
}

fn classify_stderr(stderr: &str) -> StderrMetrics {
    let mut metrics = StderrMetrics::default();
    for line in stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        metrics.non_empty_lines += 1;
        if actionable_stderr_line(line) {
            metrics.actionable_lines += 1;
        }
    }
    metrics
}

fn actionable_stderr_line(line: &str) -> bool {
    if line == "Reading additional input from stdin..." {
        return false;
    }
    let benign_fragments = [
        " WARN codex_core_plugins::",
        " WARN codex_core_skills::",
        " WARN codex_core::shell_snapshot:",
        " WARN codex_mcp::rmcp_client:",
        " ERROR rmcp::transport::worker:",
        "AuthRequired(",
        "Auth required",
        "plugin MCP server uses an unknown transport type",
        "failed to parse plugin MCP server",
        "failed to load plugin",
        "ignoring interface.",
        "Failed to create shell snapshot for powershell",
        "failed to initialize MCP client during shutdown",
    ];
    !benign_fragments
        .iter()
        .any(|fragment| line.contains(fragment))
}

fn expected_artifacts(scenario: ProfileScenarioKind) -> Vec<&'static str> {
    match scenario {
        ProfileScenarioKind::ReactCalculatorScaffold => vec![
            ".spark-scenarios/react-calculator/package.json",
            ".spark-scenarios/react-calculator/index.html",
            ".spark-scenarios/react-calculator/src/main.tsx",
            ".spark-scenarios/react-calculator/src/App.tsx",
            ".spark-scenarios/react-calculator/src/App.test.tsx",
            ".spark-scenarios/react-calculator/src/styles.css",
        ],
        ProfileScenarioKind::RustLogAnalyzerScaffold => vec![
            ".spark-scenarios/rust-log-analyzer/Cargo.toml",
            ".spark-scenarios/rust-log-analyzer/src/lib.rs",
            ".spark-scenarios/rust-log-analyzer/src/main.rs",
        ],
        _ => Vec::new(),
    }
}

fn codex_failure_points(
    exit_code: Option<i32>,
    timed_out: bool,
    metrics: &CodexEventMetrics,
    stderr_metrics: &StderrMetrics,
    final_message: &str,
    expected_artifacts: u64,
    present_artifacts: u64,
    validation_exit_code: Option<i32>,
    validation_timed_out: bool,
) -> Vec<String> {
    let mut points = Vec::new();
    if timed_out {
        points.push("timeout".to_string());
    }
    if exit_code != Some(0) {
        points.push("nonzero_exit".to_string());
    }
    if final_message.trim().is_empty() {
        points.push("missing_final_message".to_string());
    }
    if present_artifacts < expected_artifacts {
        points.push("missing_expected_artifact".to_string());
    }
    if validation_timed_out {
        points.push("validation_timeout".to_string());
    }
    if validation_exit_code.is_some_and(|code| code != 0) {
        points.push("validation_failed".to_string());
    }
    if metrics.non_json_stdout_lines > 0 {
        points.push("non_json_stdout_noise".to_string());
    }
    if stderr_metrics.actionable_lines > 0 {
        points.push("tool_execution_error".to_string());
    }
    points
}

fn codex_score(row: &CodexCliBenchmarkRow) -> f64 {
    let mut penalty = 0.0;
    if row.timed_out {
        penalty += 45.0;
    }
    if row.exit_code != Some(0) {
        penalty += 35.0;
    }
    if row.final_message_chars == 0 {
        penalty += 15.0;
    }
    penalty += row.expected_artifacts.saturating_sub(row.present_artifacts) as f64 * 12.0;
    if row.validation_timed_out {
        penalty += 25.0;
    }
    if row.validation_exit_code.is_some_and(|code| code != 0) {
        penalty += 25.0;
    }
    penalty += (row.non_json_stdout_lines.min(20) as f64) * 0.5;
    penalty += (row.actionable_stderr_lines.min(20) as f64) * 0.25;
    if row.duration_ms > 180_000 {
        penalty += 10.0;
    } else if row.duration_ms > 90_000 {
        penalty += 5.0;
    }
    round1((100.0 - penalty).clamp(0.0, 100.0))
}

fn aggregate_rows(suite: &str, rows: &[CodexCliBenchmarkRow]) -> Value {
    let average_score = if rows.is_empty() {
        0.0
    } else {
        rows.iter().map(|row| row.score).sum::<f64>() / rows.len() as f64
    };
    let failure_points = rows
        .iter()
        .flat_map(|row| {
            row.failure_points
                .split(';')
                .filter(|item| !item.is_empty())
        })
        .fold(BTreeMap::<String, u64>::new(), |mut counts, item| {
            *counts.entry(item.to_string()).or_default() += 1;
            counts
        });
    json!({
        "suite": suite,
        "runner": "codex-cli",
        "runs": rows.len(),
        "successful_runs": rows.iter().filter(|row| row.success).count(),
        "average_score": round1(average_score),
        "min_score": rows.iter().map(|row| row.score).fold(100.0, f64::min),
        "max_score": rows.iter().map(|row| row.score).fold(0.0, f64::max),
        "total_duration_ms": rows.iter().map(|row| row.duration_ms).sum::<u128>(),
        "total_input_tokens": rows.iter().map(|row| row.input_tokens).sum::<u64>(),
        "total_output_tokens": rows.iter().map(|row| row.output_tokens).sum::<u64>(),
        "total_json_events": rows.iter().map(|row| row.json_events).sum::<u64>(),
        "total_non_json_stdout_lines": rows.iter().map(|row| row.non_json_stdout_lines).sum::<u64>(),
        "total_stderr_lines": rows.iter().map(|row| row.stderr_lines).sum::<u64>(),
        "total_actionable_stderr_lines": rows.iter().map(|row| row.actionable_stderr_lines).sum::<u64>(),
        "failure_points": failure_points,
    })
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codex_json_events_and_ignores_warning_lines() {
        let metrics = parse_codex_json_events(
            "{\"type\":\"turn.started\"}\nwarn noise\n{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\"}}\n{\"type\":\"item.completed\",\"item\":{\"type\":\"command_execution\"}}\n{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":10,\"cached_input_tokens\":2,\"output_tokens\":3,\"reasoning_output_tokens\":1}}\n",
        );

        assert_eq!(metrics.json_events, 4);
        assert_eq!(metrics.non_json_stdout_lines, 1);
        assert_eq!(metrics.turns, 1);
        assert_eq!(metrics.agent_messages, 1);
        assert_eq!(metrics.tool_items, 1);
        assert_eq!(metrics.input_tokens, 10);
        assert_eq!(metrics.cached_input_tokens, 2);
        assert_eq!(metrics.output_tokens, 3);
        assert_eq!(metrics.reasoning_output_tokens, 1);
    }

    #[test]
    fn codex_score_penalizes_missing_artifacts_and_noise() {
        let row = CodexCliBenchmarkRow {
            runner: "codex-cli".to_string(),
            suite: "real-world".to_string(),
            scenario: "react-calculator-scaffold".to_string(),
            repeat_index: 1,
            model: "gpt-5.3-codex-spark".to_string(),
            score: 0.0,
            success: false,
            exit_code: Some(0),
            timed_out: false,
            duration_ms: 10_000,
            json_events: 5,
            non_json_stdout_lines: 4,
            stderr_lines: 2,
            actionable_stderr_lines: 2,
            turns: 1,
            completed_items: 1,
            agent_messages: 1,
            tool_items: 0,
            input_tokens: 1,
            cached_input_tokens: 0,
            output_tokens: 1,
            reasoning_output_tokens: 0,
            expected_artifacts: 4,
            present_artifacts: 2,
            validation_exit_code: None,
            validation_timed_out: false,
            final_message_chars: 20,
            run_dir: "run".to_string(),
            failure_points: "missing_expected_artifact".to_string(),
        };

        assert_eq!(codex_score(&row), 73.5);
    }

    #[test]
    fn codex_stderr_classifier_ignores_startup_plugin_noise() {
        let stderr = r#"Reading additional input from stdin...
2026-06-04T06:55:39.703102Z  WARN codex_core_plugins::loader: plugin MCP server uses an unknown transport type plugin=C:\Users\ghost\.codex\plugins\cache\bars-local\codex-memory\2.1.0 transport="local"
2026-06-04T06:56:11.358255Z  WARN codex_core_skills::loader: ignoring interface.icon_small: icon path with '..' must resolve under plugin assets/
2026-06-04T06:55:41.120472Z ERROR rmcp::transport::worker: worker quit with fatal: Transport channel closed, when AuthRequired(AuthRequiredError { error="invalid_token" })
"#;

        let metrics = classify_stderr(stderr);

        assert_eq!(metrics.non_empty_lines, 4);
        assert_eq!(metrics.actionable_lines, 0);
    }

    #[test]
    fn codex_stderr_classifier_keeps_unrecognized_errors_actionable() {
        let metrics = classify_stderr("ERROR failed to run command\n");

        assert_eq!(metrics.non_empty_lines, 1);
        assert_eq!(metrics.actionable_lines, 1);
    }

    #[test]
    fn scaffold_scenarios_have_artifact_expectations() {
        assert_eq!(
            expected_artifacts(ProfileScenarioKind::RustLogAnalyzerScaffold).len(),
            3
        );
        assert!(expected_artifacts(ProfileScenarioKind::RepoSurvey).is_empty());
    }
}
