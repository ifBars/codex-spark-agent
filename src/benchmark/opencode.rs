use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use serde_json::{Value, json};
use tokio::process::Command;

use crate::{
    benchmark::{codex_cli::CodexCliBenchmarkRow, expected_scenario_artifacts, workspace},
    cli::{ProfileBenchmarkSuiteKind, ProfileScenarioKind},
    profile::{scenarios, validation},
};

#[derive(Debug, Clone)]
pub(crate) struct OpencodeBenchmarkOptions {
    pub(crate) cwd: PathBuf,
    pub(crate) suite: ProfileBenchmarkSuiteKind,
    pub(crate) model: Option<String>,
    pub(crate) reasoning_effort: String,
    pub(crate) repeat: usize,
    pub(crate) scenarios: Vec<ProfileScenarioKind>,
    pub(crate) timeout_seconds: u64,
    pub(crate) opencode_bin: PathBuf,
    pub(crate) pure: bool,
    pub(crate) output_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct OpencodeBenchmarkOutput {
    pub(crate) json_path: PathBuf,
    pub(crate) rows: usize,
    pub(crate) aggregate: Value,
}

pub(crate) async fn run_opencode_benchmark(
    options: OpencodeBenchmarkOptions,
) -> Result<OpencodeBenchmarkOutput> {
    scenarios::validate_scenario_repeat(options.repeat)?;
    std::fs::create_dir_all(&options.output_dir).map_err(|error| {
        anyhow::anyhow!(
            "failed to create opencode benchmark output directory {}: {error}",
            options.output_dir.display()
        )
    })?;

    let started_at = unix_millis();
    let mut rows = Vec::new();
    let scenarios = selected_scenarios(options.suite, &options.scenarios)?;
    for scenario in scenarios {
        for repeat_index in 1..=options.repeat {
            let scenario_cwd = workspace::create_benchmark_workspace(
                &options.cwd,
                options.suite.name(),
                scenario,
                repeat_index,
            )?;
            println!(
                "opencode_workspace scenario={} path={}",
                scenario.name(),
                scenario_cwd.display()
            );
            scenarios::prepare_profile_scenario(&scenario_cwd, scenario)?;
            let row =
                run_opencode_scenario(&options, &scenario_cwd, scenario, repeat_index, started_at)
                    .await?;
            println!(
                "opencode scenario={} repeat={}/{} score={:.1} success={} duration_ms={} failure_points={}",
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
        "{}-opencode-{started_at}.json",
        options.suite.name()
    ));
    std::fs::write(
        &json_path,
        serde_json::to_string_pretty(&json!({
            "suite": options.suite.name(),
            "runner": "opencode",
            "reasoning_effort": options.reasoning_effort.as_str(),
            "generated_at_unix_ms": started_at,
            "rows": rows,
            "aggregate": aggregate,
        }))?,
    )
    .map_err(|error| anyhow::anyhow!("failed to write {}: {error}", json_path.display()))?;

    Ok(OpencodeBenchmarkOutput {
        json_path,
        rows: rows.len(),
        aggregate,
    })
}

async fn run_opencode_scenario(
    options: &OpencodeBenchmarkOptions,
    scenario_cwd: &Path,
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
            "failed to create opencode run dir {}: {error}",
            run_dir.display()
        )
    })?;
    let prompt = external_benchmark_prompt(&options.cwd, scenario_cwd, scenario);
    let prompt_path = run_dir.join("prompt.txt");
    std::fs::write(&prompt_path, &prompt)
        .map_err(|error| anyhow::anyhow!("failed to write opencode prompt: {error}"))?;

    let mut command = Command::new(&options.opencode_bin);
    command.args(opencode_command_args(options, scenario_cwd, &prompt_path));
    command.kill_on_drop(true);

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
            anyhow::bail!("failed to run opencode for {scenario_name}: {error}");
        }
        Err(_) => (
            String::new(),
            "timed out waiting for opencode".to_string(),
            None,
            true,
        ),
    };

    std::fs::write(run_dir.join("stdout.jsonl"), &stdout)
        .map_err(|error| anyhow::anyhow!("failed to write opencode stdout: {error}"))?;
    std::fs::write(run_dir.join("stderr.txt"), &stderr)
        .map_err(|error| anyhow::anyhow!("failed to write opencode stderr: {error}"))?;

    let metrics = parse_opencode_json_events(&stdout);
    let trimmed_final_message = metrics.final_message.trim();
    let final_message = (!trimmed_final_message.is_empty())
        .then(|| trimmed_final_message.to_string())
        .unwrap_or_else(|| non_json_final_message(&stdout));
    std::fs::write(run_dir.join("last-message.txt"), &final_message)
        .map_err(|error| anyhow::anyhow!("failed to write opencode final message: {error}"))?;

    let stderr_metrics = classify_stderr(&stderr);
    let expected_artifacts = expected_scenario_artifacts(scenario);
    let present_artifacts = expected_artifacts
        .iter()
        .filter(|path| scenario_cwd.join(path).exists())
        .count() as u64;
    let validation =
        validation::run_and_write_scenario_validation(scenario_cwd, &run_dir, scenario).await?;
    let validation_exit_code = validation.as_ref().and_then(|result| result.exit_code);
    let validation_timed_out = validation.as_ref().is_some_and(|result| result.timed_out);
    let browser_validation = validation
        .as_ref()
        .and_then(|result| result.browser.as_ref());
    let browser_validation_present = browser_validation.is_some();
    let browser_validation_exit_code = browser_validation.and_then(|result| result.exit_code);
    let browser_validation_timed_out = browser_validation.is_some_and(|result| result.timed_out);
    let browser_screenshot = browser_validation
        .map(|result| result.screenshot.clone())
        .unwrap_or_default();
    let source_footprint = validation
        .as_ref()
        .and_then(|result| result.source_footprint.as_ref());
    let failure_points = failure_points(
        exit_code,
        timed_out,
        metrics.non_json_stdout_lines,
        stderr_metrics.actionable_lines,
        &final_message,
        expected_artifacts.len() as u64,
        present_artifacts,
        validation_exit_code,
        validation_timed_out,
        browser_validation_present,
        browser_validation_exit_code,
        browser_validation_timed_out,
    );

    let mut row = CodexCliBenchmarkRow {
        runner: "opencode".to_string(),
        suite: options.suite.name().to_string(),
        scenario: scenario_name.to_string(),
        repeat_index,
        model: options
            .model
            .clone()
            .unwrap_or_else(|| "opencode-default".to_string()),
        command_path: String::new(),
        command_version: String::new(),
        reasoning_effort: options.reasoning_effort.clone(),
        score: 0.0,
        success: exit_code == Some(0)
            && !timed_out
            && !final_message.trim().is_empty()
            && present_artifacts == expected_artifacts.len() as u64
            && validation_exit_code.is_none_or(|code| code == 0)
            && !validation_timed_out
            && browser_validation_exit_code.is_none_or(|code| code == 0)
            && !browser_validation_timed_out,
        exit_code,
        timed_out,
        duration_ms,
        json_events: metrics.json_events,
        non_json_stdout_lines: metrics.non_json_stdout_lines,
        stderr_lines: stderr_metrics.non_empty_lines,
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
        validation_exit_code,
        validation_timed_out,
        browser_validation_present,
        browser_validation_exit_code,
        browser_validation_timed_out,
        browser_screenshot,
        source_files: source_footprint
            .map(|footprint| footprint.files)
            .unwrap_or(0),
        source_bytes: source_footprint
            .map(|footprint| footprint.bytes)
            .unwrap_or(0),
        final_message_chars: final_message.chars().count() as u64,
        run_dir: run_dir.display().to_string(),
        provider_retry_hint: String::new(),
        failure_points: failure_points.join(";"),
    };
    row.score = external_agent_score(&row);
    Ok(row)
}

const OPENCODE_PROMPT_FILE_MESSAGE: &str =
    "Run the benchmark task described in the attached prompt file.";

fn opencode_command_args(
    options: &OpencodeBenchmarkOptions,
    scenario_cwd: &Path,
    prompt_path: &Path,
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("run"),
        OsString::from("--format"),
        OsString::from("json"),
        OsString::from("--dir"),
        opencode_cli_path_arg(scenario_cwd),
        OsString::from("--dangerously-skip-permissions"),
    ];
    if options.pure {
        args.push(OsString::from("--pure"));
    }
    if let Some(model) = &options.model {
        args.push(OsString::from("--model"));
        args.push(OsString::from(model));
    }
    args.push(OsString::from("--variant"));
    args.push(OsString::from(opencode_reasoning_variant_arg(
        &options.reasoning_effort,
    )));
    args.push(OsString::from(OPENCODE_PROMPT_FILE_MESSAGE));
    args.push(OsString::from("--file"));
    args.push(opencode_cli_path_arg(prompt_path));
    args
}

fn opencode_cli_path_arg(path: &Path) -> OsString {
    OsString::from(clean_cli_path(path.display().to_string()))
}

#[cfg(windows)]
fn clean_cli_path(path: String) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = path.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    path
}

#[cfg(not(windows))]
fn clean_cli_path(path: String) -> String {
    path
}

fn external_benchmark_prompt(
    source_cwd: &Path,
    scenario_cwd: &Path,
    scenario: ProfileScenarioKind,
) -> String {
    let base = scenarios::codex_cli_benchmark_prompt(scenario);
    if same_path(source_cwd, scenario_cwd) {
        return base;
    }
    format!(
        "<benchmark_environment>\n  <cwd>{}</cwd>\n  <read_only_reference_root>{}</read_only_reference_root>\n  <note>Use cwd for scenario files and any writes. For repository source evidence, read from the read-only reference root instead of expecting a copied repo in cwd.</note>\n</benchmark_environment>\n\n{base}",
        scenario_cwd.display(),
        source_cwd.display()
    )
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[derive(Default)]
struct OpencodeEventMetrics {
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
    final_message: String,
}

#[derive(Debug, Default, Clone, Copy)]
struct StderrMetrics {
    non_empty_lines: u64,
    actionable_lines: u64,
}

fn parse_opencode_json_events(stdout: &str) -> OpencodeEventMetrics {
    let mut metrics = OpencodeEventMetrics::default();
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            metrics.non_json_stdout_lines += 1;
            continue;
        };
        metrics.json_events += 1;
        record_usage(&value, &mut metrics);
        let event_type = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if event_type == "step_finish" {
            metrics.turns += 1;
        }
        if looks_like_agent_message(&value) {
            metrics.agent_messages += 1;
            if let Some(text) = best_text_field(&value)
                && text.chars().count() > metrics.final_message.chars().count()
            {
                metrics.final_message = text;
            }
        }
        if event_type.contains("tool") || event_type.contains("bash") || event_type.contains("call")
        {
            metrics.tool_items += 1;
        }
        metrics.completed_items += 1;
    }
    metrics
}

fn record_usage(value: &Value, metrics: &mut OpencodeEventMetrics) {
    match value {
        Value::Object(map) => {
            if let Some(usage) = map.get("usage").or_else(|| map.get("tokens")) {
                metrics.input_tokens += usage_u64(usage, &["input", "input_tokens", "prompt"]);
                metrics.cached_input_tokens += usage_u64(
                    usage,
                    &["cached_input", "cached_input_tokens", "cache_read"],
                ) + usage
                    .get("cache")
                    .map(|cache| usage_u64(cache, &["read"]))
                    .unwrap_or(0);
                metrics.output_tokens +=
                    usage_u64(usage, &["output", "output_tokens", "completion"]);
                metrics.reasoning_output_tokens += usage_u64(
                    usage,
                    &["reasoning", "reasoning_output", "reasoning_output_tokens"],
                );
            }
            for child in map.values() {
                record_usage(child, metrics);
            }
        }
        Value::Array(items) => {
            for item in items {
                record_usage(item, metrics);
            }
        }
        _ => {}
    }
}

fn usage_u64(value: &Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_u64))
        .unwrap_or(0)
}

fn looks_like_agent_message(value: &Value) -> bool {
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let part_type = value
        .pointer("/part/type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let role = value
        .get("role")
        .or_else(|| value.pointer("/message/role"))
        .or_else(|| value.pointer("/part/role"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    event_type.contains("message")
        || event_type == "text"
        || part_type == "text"
        || role == "assistant"
}

fn best_text_field(value: &Value) -> Option<String> {
    let mut best = String::new();
    collect_text_fields(value, &mut best);
    (!best.trim().is_empty()).then(|| best.trim().to_string())
}

fn collect_text_fields(value: &Value, best: &mut String) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if matches!(key.as_str(), "content" | "text" | "message")
                    && let Some(text) = value.as_str()
                    && text.chars().count() > best.chars().count()
                {
                    *best = text.to_string();
                }
                collect_text_fields(value, best);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_text_fields(item, best);
            }
        }
        _ => {}
    }
}

fn non_json_final_message(stdout: &str) -> String {
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .last()
        .unwrap_or_default()
        .trim()
        .to_string()
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
    let benign_fragments = [
        "WARNING: terminal is not fully functional",
        "opencode",
        "server listening",
        "server started",
    ];
    !benign_fragments
        .iter()
        .any(|fragment| line.contains(fragment))
}

fn failure_points(
    exit_code: Option<i32>,
    timed_out: bool,
    non_json_stdout_lines: u64,
    actionable_stderr_lines: u64,
    final_message: &str,
    expected_artifacts: u64,
    present_artifacts: u64,
    validation_exit_code: Option<i32>,
    validation_timed_out: bool,
    browser_validation_present: bool,
    browser_validation_exit_code: Option<i32>,
    browser_validation_timed_out: bool,
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
    if browser_validation_timed_out {
        points.push("browser_validation_timeout".to_string());
    }
    if browser_validation_present && browser_validation_exit_code != Some(0) {
        points.push("browser_validation_failed".to_string());
    }
    if non_json_stdout_lines > 0 {
        points.push("non_json_stdout_noise".to_string());
    }
    if actionable_stderr_lines > 0 {
        points.push("tool_execution_error".to_string());
    }
    points
}

fn external_agent_score(row: &CodexCliBenchmarkRow) -> f64 {
    let mut quality_penalty = 0.0;
    if row.timed_out {
        quality_penalty += 35.0;
    }
    if row.exit_code != Some(0) {
        quality_penalty += 25.0;
    }
    if row.final_message_chars == 0 {
        quality_penalty += 15.0;
    }
    quality_penalty += row.expected_artifacts.saturating_sub(row.present_artifacts) as f64 * 12.0;
    if row.validation_timed_out {
        quality_penalty += 25.0;
    }
    if row.validation_exit_code.is_some_and(|code| code != 0) {
        quality_penalty += 25.0;
    }
    if row.browser_validation_timed_out {
        quality_penalty += 35.0;
    }
    if row.browser_validation_present && row.browser_validation_exit_code != Some(0) {
        quality_penalty += 55.0;
    }
    let quality = (100.0 - quality_penalty).clamp(0.0, 100.0);

    let mut efficiency_penalty = 0.0;
    if row.duration_ms > 180_000 {
        efficiency_penalty += 15.0;
    } else if row.duration_ms > 90_000 {
        efficiency_penalty += 8.0;
    } else if row.duration_ms > 60_000 {
        efficiency_penalty += 3.0;
    }
    if row.source_bytes > 0 {
        efficiency_penalty += row.source_bytes.saturating_sub(12_000) as f64 / 1_000.0;
    }
    let efficiency = (100.0 - efficiency_penalty).clamp(0.0, 100.0);

    let mut score = quality * 0.85 + efficiency * 0.15;
    if row.browser_validation_timed_out
        || (row.browser_validation_present && row.browser_validation_exit_code != Some(0))
    {
        score = score.min(45.0);
    }
    if row.validation_timed_out || row.validation_exit_code.is_some_and(|code| code != 0) {
        score = score.min(55.0);
    }
    if !row.success {
        score = score.min(60.0);
    }
    round1(score.clamp(0.0, 100.0))
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
        "runner": "opencode",
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

fn selected_scenarios(
    suite: ProfileBenchmarkSuiteKind,
    requested: &[ProfileScenarioKind],
) -> Result<Vec<ProfileScenarioKind>> {
    if requested.is_empty() {
        return Ok(suite.scenarios().to_vec());
    }
    let suite_scenarios = suite.scenarios();
    let invalid = requested
        .iter()
        .copied()
        .filter(|scenario| !suite_scenarios.contains(scenario))
        .map(ProfileScenarioKind::name)
        .collect::<Vec<_>>();
    if !invalid.is_empty() {
        anyhow::bail!(
            "scenario filter includes scenario(s) outside suite '{}': {}",
            suite.name(),
            invalid.join(", ")
        );
    }
    Ok(requested.to_vec())
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn opencode_reasoning_variant_arg(reasoning_effort: &str) -> &str {
    reasoning_effort
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_opencode_json_lines_for_messages_and_tools() {
        let metrics = parse_opencode_json_events(
            "{\"type\":\"text\",\"part\":{\"type\":\"text\",\"text\":\"done\"}}\n{\"type\":\"tool_use\",\"part\":{\"type\":\"tool\",\"tool\":\"bash\"}}\n{\"type\":\"step_finish\",\"part\":{\"tokens\":{\"input\":10,\"output\":3,\"reasoning\":1,\"cache\":{\"read\":2}}}}\nnoise\n",
        );

        assert_eq!(metrics.json_events, 3);
        assert_eq!(metrics.non_json_stdout_lines, 1);
        assert_eq!(metrics.agent_messages, 1);
        assert_eq!(metrics.tool_items, 1);
        assert_eq!(metrics.input_tokens, 10);
        assert_eq!(metrics.cached_input_tokens, 2);
        assert_eq!(metrics.output_tokens, 3);
        assert_eq!(metrics.reasoning_output_tokens, 1);
        assert_eq!(metrics.turns, 1);
        assert_eq!(metrics.final_message, "done");
    }

    #[test]
    fn opencode_reasoning_effort_is_passed_as_variant() {
        assert_eq!(opencode_reasoning_variant_arg("high"), "high");
    }

    #[test]
    fn opencode_invocation_uses_prompt_file_instead_of_long_positional_prompt() {
        let options = OpencodeBenchmarkOptions {
            cwd: PathBuf::from("repo"),
            suite: ProfileBenchmarkSuiteKind::RealWorld,
            model: Some("openai/gpt-5.3-codex-spark".to_string()),
            reasoning_effort: "high".to_string(),
            repeat: 1,
            scenarios: Vec::new(),
            timeout_seconds: 60,
            opencode_bin: PathBuf::from("opencode"),
            pure: true,
            output_dir: PathBuf::from(".spark-profile/benchmarks"),
        };
        let long_prompt = "Use cwd for scenario files. ".repeat(200);
        let args = opencode_command_args(
            &options,
            Path::new("workspaces/precise-patch"),
            Path::new(".spark-profile/benchmarks/runs/run-1/prompt.txt"),
        );
        let args = args
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(args.windows(2).any(|pair| pair[0] == "--file"
            && pair[1] == ".spark-profile/benchmarks/runs/run-1/prompt.txt"));
        assert!(args.contains(&OPENCODE_PROMPT_FILE_MESSAGE.to_string()));
        assert!(
            args.iter()
                .position(|arg| arg == OPENCODE_PROMPT_FILE_MESSAGE)
                < args.iter().position(|arg| arg == "--file")
        );
        assert!(!args.contains(&long_prompt));
    }

    #[cfg(windows)]
    #[test]
    fn opencode_invocation_strips_windows_verbatim_paths_for_cli_url_parsing() {
        let options = OpencodeBenchmarkOptions {
            cwd: PathBuf::from("repo"),
            suite: ProfileBenchmarkSuiteKind::RealWorld,
            model: None,
            reasoning_effort: "medium".to_string(),
            repeat: 1,
            scenarios: Vec::new(),
            timeout_seconds: 60,
            opencode_bin: PathBuf::from("opencode"),
            pure: false,
            output_dir: PathBuf::from(".spark-profile/benchmarks"),
        };
        let args = opencode_command_args(
            &options,
            Path::new(r"\\?\C:\repo\.spark-profile\workspace"),
            Path::new(r"\\?\C:\repo\.spark-profile\runs\prompt.txt"),
        )
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

        assert!(
            args.windows(2)
                .any(|pair| pair[0] == "--dir" && pair[1] == r"C:\repo\.spark-profile\workspace")
        );
        assert!(args.windows(2).any(
            |pair| pair[0] == "--file" && pair[1] == r"C:\repo\.spark-profile\runs\prompt.txt"
        ));
    }

    #[cfg(not(windows))]
    #[test]
    fn opencode_invocation_keeps_paths_unchanged_outside_windows() {
        assert_eq!(
            clean_cli_path(r"\\?\C:\repo\.spark-profile\runs\prompt.txt".to_string()),
            r"\\?\C:\repo\.spark-profile\runs\prompt.txt"
        );
    }

    #[test]
    fn merge_conflict_resolution_has_artifact_expectation() {
        assert_eq!(
            expected_scenario_artifacts(ProfileScenarioKind::MergeConflictResolution),
            &[".spark-scenarios/merge-conflict-resolution/src/featureFlags.ts"]
        );
        assert!(expected_scenario_artifacts(ProfileScenarioKind::RepoSurvey).is_empty());
    }
}
