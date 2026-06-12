use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Command as StdCommand,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::process::Command;

use crate::{
    benchmark::{
        expected_scenario_artifacts,
        infrastructure::{
            contains_external_infrastructure_failure_signal, external_infrastructure_retry_hint,
            failure_points_contain,
        },
        workspace,
    },
    cli::{ProfileBenchmarkSuiteKind, ProfileScenarioKind},
    profile::{scenarios, validation},
};

#[derive(Debug, Clone)]
pub(crate) struct CodexCliBenchmarkOptions {
    pub(crate) cwd: PathBuf,
    pub(crate) suite: ProfileBenchmarkSuiteKind,
    pub(crate) model: String,
    pub(crate) reasoning_effort: String,
    pub(crate) repeat: usize,
    pub(crate) scenarios: Vec<ProfileScenarioKind>,
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
    #[serde(default)]
    pub(crate) command_path: String,
    #[serde(default)]
    pub(crate) command_version: String,
    #[serde(default = "default_reasoning_effort")]
    pub(crate) reasoning_effort: String,
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
    #[serde(default)]
    pub(crate) browser_validation_present: bool,
    #[serde(default)]
    pub(crate) browser_validation_exit_code: Option<i32>,
    #[serde(default)]
    pub(crate) browser_validation_timed_out: bool,
    #[serde(default)]
    pub(crate) browser_screenshot: String,
    #[serde(default)]
    pub(crate) source_files: u64,
    #[serde(default)]
    pub(crate) source_bytes: u64,
    pub(crate) final_message_chars: u64,
    pub(crate) run_dir: String,
    #[serde(default)]
    pub(crate) provider_retry_hint: String,
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
    scenarios::validate_scenario_repeat(options.repeat)?;
    std::fs::create_dir_all(&options.output_dir).map_err(|error| {
        anyhow::anyhow!(
            "failed to create Codex CLI benchmark output directory {}: {error}",
            options.output_dir.display()
        )
    })?;

    let started_at = unix_millis();
    let mut rows = Vec::new();
    let scenarios = selected_scenarios(options.suite, &options.scenarios)?;
    let codex_command_path = resolve_executable_path(&options.codex_bin);
    let codex_command_version = command_version(&options.codex_bin);
    for scenario in scenarios {
        for repeat_index in 1..=options.repeat {
            let scenario_cwd = workspace::create_benchmark_workspace(
                &options.cwd,
                options.suite.name(),
                scenario,
                repeat_index,
            )?;
            println!(
                "codex_cli_workspace scenario={} path={}",
                scenario.name(),
                scenario_cwd.display()
            );
            scenarios::prepare_profile_scenario(&scenario_cwd, scenario)?;
            let row = run_codex_cli_scenario(
                &options,
                &scenario_cwd,
                scenario,
                repeat_index,
                started_at,
                &codex_command_path,
                &codex_command_version,
            )
            .await?;
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
            "reasoning_effort": options.reasoning_effort.as_str(),
            "codex_bin": options.codex_bin.display().to_string(),
            "codex_command_path": codex_command_path,
            "codex_command_version": codex_command_version,
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
    scenario_cwd: &Path,
    scenario: ProfileScenarioKind,
    repeat_index: usize,
    batch_stamp: u128,
    codex_command_path: &str,
    codex_command_version: &str,
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
    let prompt = external_benchmark_prompt(&options.cwd, scenario_cwd, scenario);
    std::fs::write(run_dir.join("prompt.txt"), &prompt)
        .map_err(|error| anyhow::anyhow!("failed to write Codex CLI prompt: {error}"))?;

    let mut command = Command::new(&options.codex_bin);
    command
        .arg("exec")
        .arg("--json")
        .arg("--cd")
        .arg(scenario_cwd)
        .arg("--sandbox")
        .arg("danger-full-access")
        .arg("--dangerously-bypass-approvals-and-sandbox")
        .arg("--model")
        .arg(&options.model)
        .arg("--config")
        .arg(codex_cli_reasoning_config_arg(&options.reasoning_effort))
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
    let request_failure =
        contains_external_infrastructure_failure_signal(&format!("{stdout}\n{stderr}"));
    let provider_retry_hint =
        external_infrastructure_retry_hint(&format!("{stdout}\n{stderr}")).unwrap_or_default();
    let expected_artifacts = expected_scenario_artifacts(scenario);
    let present_artifacts = expected_artifacts
        .iter()
        .filter(|path| scenario_cwd.join(path).exists())
        .count() as u64;
    let validation = if request_failure {
        None
    } else {
        validation::run_and_write_scenario_validation(scenario_cwd, &run_dir, scenario).await?
    };
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
    let failure_points = codex_failure_points(
        exit_code,
        timed_out,
        &metrics,
        &stderr_metrics,
        &final_message,
        expected_artifacts.len() as u64,
        present_artifacts,
        validation_exit_code,
        validation_timed_out,
        browser_validation_present,
        browser_validation_exit_code,
        browser_validation_timed_out,
        request_failure,
    );

    let mut row = CodexCliBenchmarkRow {
        runner: "codex-cli".to_string(),
        suite: options.suite.name().to_string(),
        scenario: scenario_name.to_string(),
        repeat_index,
        model: options.model.clone(),
        command_path: codex_command_path.to_string(),
        command_version: codex_command_version.to_string(),
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
        provider_retry_hint,
        failure_points: failure_points.join(";"),
    };
    row.score = codex_score(&row);
    Ok(row)
}

fn default_reasoning_effort() -> String {
    "unknown".to_string()
}

fn resolve_executable_path(command: &Path) -> String {
    if command_has_directory_component(command) {
        return canonical_display(command).unwrap_or_else(|| command.display().to_string());
    }

    let Some(file_name) = command.file_name() else {
        return String::new();
    };
    let candidates = executable_candidate_names(file_name);
    let Some(path) = std::env::var_os("PATH") else {
        return String::new();
    };
    for dir in std::env::split_paths(&path) {
        for candidate in &candidates {
            let executable = dir.join(candidate);
            if executable.is_file() {
                return canonical_display(&executable)
                    .unwrap_or_else(|| executable.display().to_string());
            }
        }
    }
    String::new()
}

fn command_has_directory_component(command: &Path) -> bool {
    command.is_absolute() || command.components().count() > 1
}

fn canonical_display(path: &Path) -> Option<String> {
    std::fs::canonicalize(path)
        .ok()
        .map(|path| display_path(&path))
}

fn display_path(path: &Path) -> String {
    clean_display_path(path.display().to_string())
}

#[cfg(windows)]
fn clean_display_path(path: String) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = path.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    path
}

#[cfg(not(windows))]
fn clean_display_path(path: String) -> String {
    path
}

#[cfg(windows)]
fn executable_candidate_names(file_name: &OsStr) -> Vec<OsString> {
    let mut names = vec![file_name.to_os_string()];
    if Path::new(file_name).extension().is_some() {
        return names;
    }

    let pathext =
        std::env::var_os("PATHEXT").unwrap_or_else(|| OsString::from(".COM;.EXE;.BAT;.CMD"));
    for extension in pathext.to_string_lossy().split(';') {
        let extension = extension.trim();
        if extension.is_empty() {
            continue;
        }
        let mut name = file_name.to_os_string();
        name.push(extension);
        names.push(name);
    }
    names
}

#[cfg(not(windows))]
fn executable_candidate_names(file_name: &OsStr) -> Vec<OsString> {
    vec![file_name.to_os_string()]
}

fn command_version(command: &Path) -> String {
    let Ok(output) = StdCommand::new(command).arg("--version").output() else {
        return String::new();
    };
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(normalized_version_line)
        .unwrap_or_default()
}

fn normalized_version_line(line: &str) -> String {
    let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized.chars().take(200).collect()
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

fn codex_cli_reasoning_config_arg(reasoning_effort: &str) -> String {
    format!("model_reasoning_effort=\"{reasoning_effort}\"")
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

#[derive(Debug, Default, Clone, Copy)]
struct StderrMetrics {
    non_empty_lines: u64,
    actionable_lines: u64,
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
        " WARN codex_rmcp_client::rmcp_client:",
        " WARN rmcp::transport::auth:",
        " ERROR rmcp::transport::worker:",
        "AuthRequired(",
        "Auth required",
        "Auth(AuthorizationRequired)",
        "AuthorizationRequired",
        "OAuth authorization required",
        "plugin MCP server uses an unknown transport type",
        "failed to parse plugin MCP server",
        "failed to load plugin",
        "ignoring interface.defaultPrompt",
        "ignoring interface.",
        "Failed to create shell snapshot for powershell",
        "failed to initialize MCP client during shutdown",
    ];
    !benign_fragments
        .iter()
        .any(|fragment| line.contains(fragment))
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
    browser_validation_present: bool,
    browser_validation_exit_code: Option<i32>,
    browser_validation_timed_out: bool,
    request_failure: bool,
) -> Vec<String> {
    let mut points = Vec::new();
    if request_failure {
        points.push("request_failure".to_string());
        if timed_out {
            points.push("timeout".to_string());
        }
        if exit_code != Some(0) {
            points.push("nonzero_exit".to_string());
        }
        if metrics.non_json_stdout_lines > 0 {
            points.push("non_json_stdout_noise".to_string());
        }
        return points;
    }
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
    if metrics.non_json_stdout_lines > 0 {
        points.push("non_json_stdout_noise".to_string());
    }
    let unrecovered_failure = timed_out
        || exit_code != Some(0)
        || final_message.trim().is_empty()
        || present_artifacts < expected_artifacts
        || validation_timed_out
        || validation_exit_code.is_some_and(|code| code != 0)
        || browser_validation_timed_out
        || (browser_validation_present && browser_validation_exit_code != Some(0))
        || request_failure;
    if stderr_metrics.actionable_lines > 0 && unrecovered_failure {
        points.push("tool_execution_error".to_string());
    }
    points
}

fn codex_score(row: &CodexCliBenchmarkRow) -> f64 {
    if failure_points_contain(&row.failure_points, "request_failure") {
        return 0.0;
    }
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
    let comparable_rows = rows
        .iter()
        .filter(|row| !failure_points_contain(&row.failure_points, "request_failure"))
        .collect::<Vec<_>>();
    let comparable_average_score = if comparable_rows.is_empty() {
        None
    } else {
        Some(round1(
            comparable_rows.iter().map(|row| row.score).sum::<f64>() / comparable_rows.len() as f64,
        ))
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
    let request_failure_scenarios = rows
        .iter()
        .filter(|row| failure_points_contain(&row.failure_points, "request_failure"))
        .fold(BTreeMap::<String, usize>::new(), |mut counts, row| {
            *counts.entry(row.scenario.clone()).or_default() += 1;
            counts
        });
    let request_failure_retry_hints = rows
        .iter()
        .filter(|row| failure_points_contain(&row.failure_points, "request_failure"))
        .filter(|row| !row.provider_retry_hint.trim().is_empty())
        .fold(BTreeMap::<String, String>::new(), |mut hints, row| {
            hints
                .entry(row.scenario.clone())
                .or_insert_with(|| row.provider_retry_hint.clone());
            hints
        });
    let diagnostics = json!({
        "request_failure": rows
            .iter()
            .filter(|row| failure_points_contain(&row.failure_points, "request_failure"))
            .count(),
        "request_failure_scenarios": request_failure_scenarios,
        "request_failure_retry_hints": request_failure_retry_hints,
    });
    json!({
        "suite": suite,
        "runner": "codex-cli",
        "runs": rows.len(),
        "successful_runs": rows.iter().filter(|row| row.success).count(),
        "comparable_runs": comparable_rows.len(),
        "successful_comparable_runs": comparable_rows.iter().filter(|row| row.success).count(),
        "average_score": round1(average_score),
        "comparable_average_score": comparable_average_score,
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
        "diagnostics": diagnostics,
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
    fn codex_reasoning_effort_is_passed_as_config_override() {
        assert_eq!(
            codex_cli_reasoning_config_arg("high"),
            "model_reasoning_effort=\"high\""
        );
    }

    #[test]
    fn version_line_is_normalized_and_bounded() {
        let long_version = format!(" codex-cli   {}\t{}", "0.139.0", "x".repeat(240));

        let normalized = normalized_version_line(&long_version);

        assert!(normalized.starts_with("codex-cli 0.139.0 "));
        assert_eq!(normalized.chars().count(), 200);
    }

    #[test]
    fn display_path_keeps_non_verbatim_paths() {
        assert_eq!(
            clean_display_path(r"C:\Users\ghost\.bun\bin\codex.exe".to_string()),
            r"C:\Users\ghost\.bun\bin\codex.exe"
        );
    }

    #[cfg(windows)]
    #[test]
    fn display_path_strips_windows_verbatim_prefixes() {
        assert_eq!(
            clean_display_path(r"\\?\C:\Users\ghost\.bun\bin\codex.exe".to_string()),
            r"C:\Users\ghost\.bun\bin\codex.exe"
        );
        assert_eq!(
            clean_display_path(r"\\?\UNC\server\share\codex.exe".to_string()),
            r"\\server\share\codex.exe"
        );
    }

    #[test]
    fn missing_command_provenance_deserializes_as_empty_strings() {
        let row: CodexCliBenchmarkRow = serde_json::from_value(json!({
            "runner": "codex-cli",
            "suite": "real-world",
            "scenario": "config-migration",
            "repeat_index": 1,
            "model": "gpt-test",
            "score": 100.0,
            "success": true,
            "exit_code": 0,
            "timed_out": false,
            "duration_ms": 10000,
            "json_events": 1,
            "non_json_stdout_lines": 0,
            "stderr_lines": 0,
            "turns": 1,
            "completed_items": 1,
            "agent_messages": 1,
            "tool_items": 1,
            "input_tokens": 1000,
            "cached_input_tokens": 0,
            "output_tokens": 100,
            "reasoning_output_tokens": 0,
            "expected_artifacts": 0,
            "present_artifacts": 0,
            "validation_exit_code": 0,
            "validation_timed_out": false,
            "final_message_chars": 100,
            "run_dir": "run",
            "failure_points": ""
        }))
        .expect("legacy row should deserialize");

        assert_eq!(row.command_path, "");
        assert_eq!(row.command_version, "");
    }

    #[test]
    fn codex_score_penalizes_missing_artifacts_and_noise() {
        let row = CodexCliBenchmarkRow {
            runner: "codex-cli".to_string(),
            suite: "real-world".to_string(),
            scenario: "react-calculator-scaffold".to_string(),
            repeat_index: 1,
            model: "gpt-5.3-codex-spark".to_string(),
            command_path: String::new(),
            command_version: String::new(),
            reasoning_effort: "medium".to_string(),
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
            browser_validation_present: false,
            browser_validation_exit_code: None,
            browser_validation_timed_out: false,
            browser_screenshot: String::new(),
            source_files: 0,
            source_bytes: 0,
            final_message_chars: 20,
            run_dir: "run".to_string(),
            provider_retry_hint: String::new(),
            failure_points: "missing_expected_artifact".to_string(),
        };

        assert_eq!(codex_score(&row), 60.0);
    }

    #[test]
    fn codex_stderr_classifier_ignores_startup_plugin_noise() {
        let stderr = r#"Reading additional input from stdin...
2026-06-04T06:55:39.703102Z  WARN codex_core_plugins::loader: plugin MCP server uses an unknown transport type plugin=C:\Users\ghost\.codex\plugins\cache\bars-local\codex-memory\2.1.0 transport="local"
2026-06-04T06:56:11.358255Z  WARN codex_core_skills::loader: ignoring interface.icon_small: icon path with '..' must resolve under plugin assets/
2026-06-04T06:55:41.120472Z ERROR rmcp::transport::worker: worker quit with fatal: Transport channel closed, when AuthRequired(AuthRequiredError { error="invalid_token" })
2026-06-09T22:33:59.864877Z  WARN codex_rmcp_client::rmcp_client: failed to refresh OAuth tokens: failed to refresh OAuth tokens for server context7
2026-06-09T22:34:30.221724Z  WARN codex_core_plugins::manifest: ignoring interface.defaultPrompt[0]: prompt must be at most 128 characters path=C:\Users\ghost\.codex\.tmp\plugins\plugins\ngs-analysis\.codex-plugin/plugin.json
2026-06-09T22:45:18.521013Z  WARN rmcp::transport::auth: Token refresh not possible, re-authorization required. error=OAuth token refresh failed: Server returned error response: invalid_grant: Invalid refresh token
2026-06-09T22:45:18.825115Z ERROR rmcp::transport::worker: worker quit with fatal: Transport channel closed, when Auth(AuthorizationRequired)
2026-06-09T22:46:15.630294Z  WARN codex_mcp::rmcp_client: failed to initialize MCP client during shutdown: MCP startup failed: handshaking with MCP server failed: Auth error: OAuth authorization required, when send initialize request
"#;

        let metrics = classify_stderr(stderr);

        assert_eq!(metrics.non_empty_lines, 9);
        assert_eq!(metrics.actionable_lines, 0);
    }

    #[test]
    fn codex_stderr_classifier_keeps_unrecognized_errors_actionable() {
        let metrics = classify_stderr("ERROR failed to run command\n");

        assert_eq!(metrics.non_empty_lines, 1);
        assert_eq!(metrics.actionable_lines, 1);
    }

    #[test]
    fn recovered_codex_stderr_does_not_become_tool_failure_point() {
        let points = codex_failure_points(
            Some(0),
            false,
            &CodexEventMetrics::default(),
            &StderrMetrics {
                non_empty_lines: 1,
                actionable_lines: 1,
            },
            "done",
            3,
            3,
            Some(0),
            false,
            false,
            None,
            false,
            false,
        );

        assert!(!points.contains(&"tool_execution_error".to_string()));
    }

    #[test]
    fn unrecovered_codex_stderr_still_becomes_tool_failure_point() {
        let points = codex_failure_points(
            Some(0),
            false,
            &CodexEventMetrics::default(),
            &StderrMetrics {
                non_empty_lines: 1,
                actionable_lines: 1,
            },
            "done",
            3,
            3,
            Some(1),
            false,
            false,
            None,
            false,
            false,
        );

        assert!(points.contains(&"validation_failed".to_string()));
        assert!(points.contains(&"tool_execution_error".to_string()));
    }

    #[test]
    fn codex_usage_limits_are_request_failures() {
        let text = r#"{"type":"error","message":"You've hit your usage limit for GPT-5.3-Codex-Spark. Switch to another model now, or try again at 5:38 PM."}"#;

        assert!(contains_external_infrastructure_failure_signal(text));

        let points = codex_failure_points(
            Some(1),
            false,
            &CodexEventMetrics::default(),
            &StderrMetrics::default(),
            "",
            3,
            3,
            Some(1),
            false,
            false,
            None,
            false,
            true,
        );

        assert!(points.contains(&"request_failure".to_string()));
        assert!(points.contains(&"nonzero_exit".to_string()));
        assert!(!points.contains(&"missing_final_message".to_string()));
        assert!(!points.contains(&"validation_failed".to_string()));
    }

    #[test]
    fn codex_aggregate_separates_infrastructure_runs_from_comparable_runs() {
        let mut row = CodexCliBenchmarkRow {
            runner: "codex-cli".to_string(),
            suite: "real-world".to_string(),
            scenario: "config-migration".to_string(),
            repeat_index: 1,
            model: "gpt-test".to_string(),
            command_path: String::new(),
            command_version: String::new(),
            reasoning_effort: "medium".to_string(),
            score: 0.0,
            success: false,
            exit_code: Some(1),
            timed_out: false,
            duration_ms: 10_000,
            json_events: 4,
            non_json_stdout_lines: 0,
            stderr_lines: 0,
            actionable_stderr_lines: 0,
            turns: 0,
            completed_items: 0,
            agent_messages: 0,
            tool_items: 0,
            input_tokens: 0,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            expected_artifacts: 3,
            present_artifacts: 3,
            validation_exit_code: None,
            validation_timed_out: false,
            browser_validation_present: false,
            browser_validation_exit_code: None,
            browser_validation_timed_out: false,
            browser_screenshot: String::new(),
            source_files: 2,
            source_bytes: 376,
            final_message_chars: 0,
            run_dir: "run".to_string(),
            provider_retry_hint: "try again at 5:38 PM".to_string(),
            failure_points: "request_failure;nonzero_exit".to_string(),
        };

        let infrastructure_only = aggregate_rows("real-world", &[row.clone()]);

        assert_eq!(infrastructure_only["runs"], 1);
        assert_eq!(infrastructure_only["comparable_runs"], 0);
        assert_eq!(infrastructure_only["successful_comparable_runs"], 0);
        assert_eq!(infrastructure_only["diagnostics"]["request_failure"], 1);
        assert_eq!(
            infrastructure_only["diagnostics"]["request_failure_scenarios"]["config-migration"],
            1
        );
        assert_eq!(
            infrastructure_only["diagnostics"]["request_failure_retry_hints"]["config-migration"],
            "try again at 5:38 PM"
        );
        assert_eq!(infrastructure_only["average_score"], 0.0);
        assert!(infrastructure_only["comparable_average_score"].is_null());
        assert_eq!(codex_score(&row), 0.0);

        row.success = true;
        row.score = 100.0;
        row.validation_exit_code = Some(0);
        row.provider_retry_hint = String::new();
        row.failure_points = String::new();
        let mixed = aggregate_rows("real-world", &[row]);

        assert_eq!(mixed["runs"], 1);
        assert_eq!(mixed["comparable_runs"], 1);
        assert_eq!(mixed["successful_comparable_runs"], 1);
        assert_eq!(mixed["comparable_average_score"], 100.0);
    }

    #[test]
    fn scaffold_scenarios_have_artifact_expectations() {
        assert_eq!(
            expected_scenario_artifacts(ProfileScenarioKind::RustLogAnalyzerScaffold).len(),
            3
        );
        assert_eq!(
            expected_scenario_artifacts(ProfileScenarioKind::MergeConflictResolution),
            &[".spark-scenarios/merge-conflict-resolution/src/featureFlags.ts"]
        );
        assert!(expected_scenario_artifacts(ProfileScenarioKind::RepoSurvey).is_empty());
    }

    #[test]
    fn selected_scenarios_must_belong_to_suite() {
        let selected = selected_scenarios(
            ProfileBenchmarkSuiteKind::RealWorld,
            &[
                ProfileScenarioKind::ShellRecovery,
                ProfileScenarioKind::PrecisePatch,
                ProfileScenarioKind::MultiFilePatch,
            ],
        )
        .unwrap();

        assert_eq!(selected.len(), 3);
        assert!(
            selected_scenarios(
                ProfileBenchmarkSuiteKind::Editing,
                &[ProfileScenarioKind::ShellRecovery]
            )
            .is_err()
        );
    }
}
