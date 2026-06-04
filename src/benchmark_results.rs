use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    cli::ProfileBenchmarkSuiteKind, codex_cli_benchmark::CodexCliBenchmarkRow, profiler,
    scenario_validation, trace_commands,
};

#[derive(Debug, Clone)]
pub(crate) struct BenchmarkReportOptions {
    pub(crate) cwd: PathBuf,
    pub(crate) suite: ProfileBenchmarkSuiteKind,
    pub(crate) limit: usize,
    pub(crate) all_runs: bool,
    pub(crate) output_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct BenchmarkRunRow {
    run_id: String,
    trace_dir: String,
    suite: String,
    scenario: String,
    model: String,
    score: f64,
    task_quality_score: f64,
    efficiency_score: f64,
    harness_pressure_score: f64,
    success: bool,
    validation_present: bool,
    validation_exit_code: Option<i32>,
    validation_timed_out: bool,
    browser_validation_present: bool,
    browser_validation_exit_code: Option<i32>,
    browser_validation_timed_out: bool,
    browser_screenshot: String,
    requests: u64,
    tool_calls: u64,
    max_approx_input_tokens: u64,
    max_context_window_pct: f64,
    max_request_duration_ms: u64,
    total_duration_ms: u64,
    source_files: u64,
    source_bytes: u64,
    compactions: u64,
    tool_failures: u64,
    recovered_tool_failures: u64,
    unrecovered_tool_failures: u64,
    truncated_tool_results: u64,
    repeated_tool_calls: u64,
    tool_only_turns: u64,
    max_tool_only_streak: u64,
    expected_tool_groups: u64,
    satisfied_tool_groups: u64,
    expected_tool_calls: u64,
    satisfied_tool_calls: u64,
    extra_calls_after_satisfied: u64,
    extra_turns_after_satisfied: u64,
    context_growth_after_satisfied_chars: u64,
    diagnostics: String,
    failure_points: String,
}

#[derive(Debug, Clone)]
pub(crate) struct BenchmarkReportOutput {
    pub(crate) json_path: PathBuf,
    pub(crate) csv_path: PathBuf,
    pub(crate) html_path: PathBuf,
    pub(crate) rows: usize,
    pub(crate) aggregate: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct BenchmarkComparisonOptions {
    pub(crate) cwd: PathBuf,
    pub(crate) suite: ProfileBenchmarkSuiteKind,
    pub(crate) limit: usize,
    pub(crate) all_runs: bool,
    pub(crate) codex_cli_report: PathBuf,
    pub(crate) output_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct BenchmarkComparisonOutput {
    pub(crate) json_path: PathBuf,
    pub(crate) csv_path: PathBuf,
    pub(crate) html_path: PathBuf,
    pub(crate) rows: usize,
    pub(crate) aggregate: Value,
}

#[derive(Debug, Clone, Serialize)]
struct ComparisonRow {
    runner: String,
    suite: String,
    scenario: String,
    model: String,
    score: f64,
    task_quality_score: f64,
    efficiency_score: f64,
    harness_pressure_score: f64,
    success: bool,
    validation_exit_code: Option<i32>,
    validation_timed_out: bool,
    duration_ms: u128,
    requests_or_turns: u64,
    tool_or_item_calls: u64,
    input_tokens: u64,
    output_tokens: u64,
    source_files: u64,
    source_bytes: u64,
    failure_points: String,
    source: String,
}

pub(crate) fn write_benchmark_report(
    options: BenchmarkReportOptions,
) -> Result<BenchmarkReportOutput> {
    let rows = collect_benchmark_rows(&options)?;
    if rows.is_empty() {
        anyhow::bail!(
            "no traces found for benchmark suite '{}' under {}",
            options.suite.name(),
            trace_commands::trace_runs_root(&options.cwd).display()
        );
    }

    std::fs::create_dir_all(&options.output_dir).map_err(|error| {
        anyhow::anyhow!(
            "failed to create benchmark report directory {}: {error}",
            options.output_dir.display()
        )
    })?;

    let stamp = unix_millis();
    let stem = format!("{}-{stamp}", options.suite.name());
    let json_path = options.output_dir.join(format!("{stem}.json"));
    let csv_path = options.output_dir.join(format!("{stem}.csv"));
    let html_path = options.output_dir.join(format!("{stem}.html"));
    let aggregate = aggregate_rows(options.suite.name(), &rows);

    std::fs::write(
        &json_path,
        serde_json::to_string_pretty(&json!({
            "suite": options.suite.name(),
            "generated_at_unix_ms": stamp,
            "latest_per_scenario": !options.all_runs,
            "rows": rows,
            "aggregate": aggregate,
        }))?,
    )
    .map_err(|error| anyhow::anyhow!("failed to write {}: {error}", json_path.display()))?;
    std::fs::write(&csv_path, rows_to_csv(&rows))
        .map_err(|error| anyhow::anyhow!("failed to write {}: {error}", csv_path.display()))?;
    std::fs::write(
        &html_path,
        rows_to_html(options.suite.name(), &rows, &aggregate),
    )
    .map_err(|error| anyhow::anyhow!("failed to write {}: {error}", html_path.display()))?;

    Ok(BenchmarkReportOutput {
        json_path,
        csv_path,
        html_path,
        rows: rows.len(),
        aggregate,
    })
}

pub(crate) fn write_benchmark_comparison(
    options: BenchmarkComparisonOptions,
) -> Result<BenchmarkComparisonOutput> {
    let harness_rows = collect_benchmark_rows(&BenchmarkReportOptions {
        cwd: options.cwd.clone(),
        suite: options.suite,
        limit: options.limit,
        all_runs: options.all_runs,
        output_dir: options.output_dir.clone(),
    })?;
    if harness_rows.is_empty() {
        anyhow::bail!(
            "no harness traces found for benchmark suite '{}' under {}",
            options.suite.name(),
            trace_commands::trace_runs_root(&options.cwd).display()
        );
    }
    let codex_rows = read_codex_cli_rows(&options.codex_cli_report)?;
    if codex_rows.is_empty() {
        anyhow::bail!(
            "no Codex CLI rows found in {}",
            options.codex_cli_report.display()
        );
    }

    std::fs::create_dir_all(&options.output_dir).map_err(|error| {
        anyhow::anyhow!(
            "failed to create benchmark comparison directory {}: {error}",
            options.output_dir.display()
        )
    })?;

    let mut rows = Vec::new();
    rows.extend(harness_rows.iter().map(comparison_row_from_harness));
    rows.extend(
        codex_rows
            .iter()
            .filter(|row| row.suite == options.suite.name())
            .map(comparison_row_from_codex_cli),
    );
    rows.sort_by(|left, right| {
        scenario_order(options.suite, &left.scenario)
            .cmp(&scenario_order(options.suite, &right.scenario))
            .then_with(|| left.runner.cmp(&right.runner))
    });

    let aggregate = aggregate_comparison(options.suite.name(), &rows);
    let stamp = unix_millis();
    let stem = format!("{}-comparison-{stamp}", options.suite.name());
    let json_path = options.output_dir.join(format!("{stem}.json"));
    let csv_path = options.output_dir.join(format!("{stem}.csv"));
    let html_path = options.output_dir.join(format!("{stem}.html"));

    std::fs::write(
        &json_path,
        serde_json::to_string_pretty(&json!({
            "suite": options.suite.name(),
            "generated_at_unix_ms": stamp,
            "rows": rows,
            "aggregate": aggregate,
        }))?,
    )
    .map_err(|error| anyhow::anyhow!("failed to write {}: {error}", json_path.display()))?;
    std::fs::write(&csv_path, comparison_rows_to_csv(&rows))
        .map_err(|error| anyhow::anyhow!("failed to write {}: {error}", csv_path.display()))?;
    std::fs::write(
        &html_path,
        comparison_rows_to_html(options.suite.name(), &rows, &aggregate),
    )
    .map_err(|error| anyhow::anyhow!("failed to write {}: {error}", html_path.display()))?;

    Ok(BenchmarkComparisonOutput {
        json_path,
        csv_path,
        html_path,
        rows: rows.len(),
        aggregate,
    })
}

fn collect_benchmark_rows(options: &BenchmarkReportOptions) -> Result<Vec<BenchmarkRunRow>> {
    let mut rows = Vec::new();
    let mut seen_scenarios = BTreeSet::new();
    for run in trace_commands::list_trace_dirs(
        &trace_commands::trace_runs_root(&options.cwd),
        options.limit,
    )? {
        let summary = profiler::analyze_trace(&run)?;
        if benchmark_suite(&summary) != Some(options.suite.name()) {
            continue;
        }
        let Some(scenario) = scenario_name(&summary) else {
            continue;
        };
        if !options.all_runs && !seen_scenarios.insert(scenario.to_string()) {
            continue;
        }
        rows.push(row_from_summary(&options.cwd, &run, &summary));
    }
    rows.sort_by(|left, right| {
        scenario_order(options.suite, &left.scenario)
            .cmp(&scenario_order(options.suite, &right.scenario))
    });
    Ok(rows)
}

fn row_from_summary(cwd: &Path, run: &Path, summary: &Value) -> BenchmarkRunRow {
    let trace_dir = trace_commands::display_trace_dir(cwd, run)
        .display()
        .to_string();
    let run_id = run
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| trace_dir.clone());
    let expected_groups = value_u64(summary, "/profile_scenario_tool_expectations/total_groups");
    let satisfied_groups = value_u64(
        summary,
        "/profile_scenario_tool_expectations/satisfied_groups",
    );
    let expected_calls = value_u64(summary, "/profile_scenario_call_expectations/total_calls");
    let satisfied_calls = value_u64(
        summary,
        "/profile_scenario_call_expectations/satisfied_calls",
    );
    let recovered_failures = value_u64(summary, "/tool_failure_recovery/recovered_failures");
    let unrecovered_failures = value_u64(summary, "/tool_failure_recovery/unrecovered_failures");
    let diagnostics = diagnostic_kinds(summary);
    let validation = scenario_validation::read_scenario_validation(run);
    let validation_present = validation.is_some();
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
    let trace_success = summary
        .get("errors")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty);
    let validation_success = validation
        .as_ref()
        .is_none_or(|result| !result.timed_out && result.exit_code == Some(0))
        && browser_validation.is_none_or(|result| !result.timed_out && result.exit_code == Some(0));
    let failure_points = failure_points(
        scenario_name(summary).unwrap_or("unknown"),
        summary,
        expected_groups,
        satisfied_groups,
        expected_calls,
        satisfied_calls,
        unrecovered_failures,
        validation_present,
        validation_exit_code,
        validation_timed_out,
        browser_validation_present,
        browser_validation_exit_code,
        browser_validation_timed_out,
    );

    let mut row = BenchmarkRunRow {
        run_id,
        trace_dir,
        suite: benchmark_suite(summary).unwrap_or("unknown").to_string(),
        scenario: scenario_name(summary).unwrap_or("unknown").to_string(),
        model: string_at(summary, "/trace_metadata/model")
            .unwrap_or("unknown")
            .to_string(),
        score: 0.0,
        task_quality_score: 0.0,
        efficiency_score: 0.0,
        harness_pressure_score: 0.0,
        success: trace_success && validation_success,
        validation_present,
        validation_exit_code,
        validation_timed_out,
        browser_validation_present,
        browser_validation_exit_code,
        browser_validation_timed_out,
        browser_screenshot,
        requests: value_u64(summary, "/requests"),
        tool_calls: value_u64(summary, "/tool_calls"),
        max_approx_input_tokens: value_u64(summary, "/max_approx_input_tokens"),
        max_context_window_pct: value_f64(summary, "/max_context_window_pct"),
        max_request_duration_ms: value_u64(summary, "/max_request_duration_ms"),
        total_duration_ms: value_u64(summary, "/total_request_duration_ms")
            .saturating_add(value_u64(summary, "/total_tool_duration_ms")),
        source_files: source_footprint
            .map(|footprint| footprint.files)
            .unwrap_or(0),
        source_bytes: source_footprint
            .map(|footprint| footprint.bytes)
            .unwrap_or(0),
        compactions: value_u64(summary, "/compactions"),
        tool_failures: value_u64(summary, "/tool_failures"),
        recovered_tool_failures: recovered_failures,
        unrecovered_tool_failures: unrecovered_failures,
        truncated_tool_results: value_u64(summary, "/truncated_tool_results"),
        repeated_tool_calls: value_u64(summary, "/repeated_tool_calls"),
        tool_only_turns: value_u64(summary, "/tool_only_turns/count"),
        max_tool_only_streak: value_u64(summary, "/tool_only_turns/max_consecutive"),
        expected_tool_groups: expected_groups,
        satisfied_tool_groups: satisfied_groups,
        expected_tool_calls: expected_calls,
        satisfied_tool_calls: satisfied_calls,
        extra_calls_after_satisfied: value_u64(
            summary,
            "/profile_scenario_call_expectations/extra_calls_after_satisfied",
        ),
        extra_turns_after_satisfied: value_u64(
            summary,
            "/profile_scenario_call_expectations/extra_turns_after_satisfied",
        ),
        context_growth_after_satisfied_chars: value_u64(
            summary,
            "/profile_scenario_call_expectations/context_growth_after_satisfied_chars",
        ),
        diagnostics: diagnostics.join(";"),
        failure_points: failure_points.join(";"),
    };
    row.task_quality_score = task_quality_score(&row);
    row.efficiency_score = efficiency_score(row.total_duration_ms as u128, row.source_bytes);
    row.harness_pressure_score = harness_pressure_score(&row);
    row.score = benchmark_score(&row);
    row
}

fn aggregate_rows(suite: &str, rows: &[BenchmarkRunRow]) -> Value {
    let average_score = rows.iter().map(|row| row.score).sum::<f64>() / rows.len() as f64;
    let average_task_quality =
        rows.iter().map(|row| row.task_quality_score).sum::<f64>() / rows.len() as f64;
    let average_efficiency =
        rows.iter().map(|row| row.efficiency_score).sum::<f64>() / rows.len() as f64;
    let average_harness_pressure = rows
        .iter()
        .map(|row| row.harness_pressure_score)
        .sum::<f64>()
        / rows.len() as f64;
    let diagnostics = rows
        .iter()
        .flat_map(|row| row.diagnostics.split(';').filter(|item| !item.is_empty()))
        .fold(BTreeMap::<String, u64>::new(), |mut counts, item| {
            *counts.entry(item.to_string()).or_default() += 1;
            counts
        });

    json!({
        "suite": suite,
        "runs": rows.len(),
        "average_score": round1(average_score),
        "average_task_quality_score": round1(average_task_quality),
        "average_efficiency_score": round1(average_efficiency),
        "average_harness_pressure_score": round1(average_harness_pressure),
        "min_score": rows.iter().map(|row| row.score).fold(100.0, f64::min),
        "max_score": rows.iter().map(|row| row.score).fold(0.0, f64::max),
        "successful_runs": rows.iter().filter(|row| row.success).count(),
        "total_tools": rows.iter().map(|row| row.tool_calls).sum::<u64>(),
        "total_source_files": rows.iter().map(|row| row.source_files).sum::<u64>(),
        "total_source_bytes": rows.iter().map(|row| row.source_bytes).sum::<u64>(),
        "total_tool_failures": rows.iter().map(|row| row.tool_failures).sum::<u64>(),
        "total_unrecovered_failures": rows.iter().map(|row| row.unrecovered_tool_failures).sum::<u64>(),
        "total_truncations": rows.iter().map(|row| row.truncated_tool_results).sum::<u64>(),
        "total_extra_calls_after_expected": rows.iter().map(|row| row.extra_calls_after_satisfied).sum::<u64>(),
        "max_tool_only_streak": rows.iter().map(|row| row.max_tool_only_streak).max().unwrap_or(0),
        "max_context_window_pct": rows.iter().map(|row| row.max_context_window_pct).fold(0.0, f64::max),
        "diagnostics": diagnostics,
    })
}

fn benchmark_score(row: &BenchmarkRunRow) -> f64 {
    let mut score = row.task_quality_score * 0.85 + row.efficiency_score * 0.15;
    if row.browser_validation_timed_out
        || (row.browser_validation_present && row.browser_validation_exit_code != Some(0))
    {
        score = score.min(45.0);
    }
    if row.validation_timed_out || (row.validation_present && row.validation_exit_code != Some(0)) {
        score = score.min(55.0);
    }
    if !row.success {
        score = score.min(60.0);
    }
    round1(score.clamp(0.0, 100.0))
}

fn task_quality_score(row: &BenchmarkRunRow) -> f64 {
    let mut penalty = 0.0;
    if !row.success {
        penalty += 25.0;
    }
    penalty += row
        .expected_tool_groups
        .saturating_sub(row.satisfied_tool_groups) as f64
        * 12.0;
    penalty += row
        .expected_tool_calls
        .saturating_sub(row.satisfied_tool_calls) as f64
        * 10.0;
    if row.validation_timed_out {
        penalty += 25.0;
    }
    if row.validation_present && row.validation_exit_code != Some(0) {
        penalty += 25.0;
    }
    if row.browser_validation_timed_out {
        penalty += 35.0;
    }
    if row.browser_validation_present && row.browser_validation_exit_code != Some(0) {
        penalty += 55.0;
    }
    round1((100.0 - penalty).clamp(0.0, 100.0))
}

fn efficiency_score(duration_ms: u128, source_bytes: u64) -> f64 {
    let mut penalty = 0.0;
    if duration_ms > 180_000 {
        penalty += 15.0;
    } else if duration_ms > 90_000 {
        penalty += 8.0;
    } else if duration_ms > 60_000 {
        penalty += 3.0;
    }
    if source_bytes > 0 {
        penalty += source_bytes.saturating_sub(12_000) as f64 / 1_000.0;
    }
    round1((100.0 - penalty).clamp(0.0, 100.0))
}

fn harness_pressure_score(row: &BenchmarkRunRow) -> f64 {
    let mut penalty = 0.0;
    let exact_completion = exact_completion_pressure_scenario(&row.scenario);
    penalty += row.unrecovered_tool_failures as f64 * 12.0;
    penalty += row.truncated_tool_results as f64 * 4.0;
    penalty += row.repeated_tool_calls as f64 * 2.0;
    if exact_completion {
        penalty += row.extra_calls_after_satisfied as f64 * 2.0;
        penalty += row.extra_turns_after_satisfied as f64 * 3.0;
    }
    penalty += row.max_tool_only_streak.saturating_sub(4) as f64 * 2.0;
    penalty += row.compactions as f64 * 1.5;
    penalty += (row.max_context_window_pct - 20.0).max(0.0) * 0.5;
    round1((100.0 - penalty).clamp(0.0, 100.0))
}

fn failure_points(
    scenario: &str,
    summary: &Value,
    expected_groups: u64,
    satisfied_groups: u64,
    expected_calls: u64,
    satisfied_calls: u64,
    unrecovered_failures: u64,
    validation_present: bool,
    validation_exit_code: Option<i32>,
    validation_timed_out: bool,
    browser_validation_present: bool,
    browser_validation_exit_code: Option<i32>,
    browser_validation_timed_out: bool,
) -> Vec<String> {
    let mut points = Vec::new();
    if !summary
        .get("errors")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
    {
        points.push("terminal_error".to_string());
    }
    if satisfied_groups < expected_groups {
        points.push("missing_expected_tool_group".to_string());
    }
    if satisfied_calls < expected_calls {
        points.push("missing_expected_tool_call".to_string());
    }
    if unrecovered_failures > 0 {
        points.push("unrecovered_tool_failure".to_string());
    }
    if value_u64(summary, "/truncated_tool_results") > 0 {
        points.push("truncated_tool_result".to_string());
    }
    if value_u64(summary, "/repeated_tool_calls") > 0 {
        points.push("repeated_tool_call".to_string());
    }
    if exact_completion_pressure_scenario(scenario)
        && value_u64(
            summary,
            "/profile_scenario_call_expectations/extra_calls_after_satisfied",
        ) > 0
    {
        points.push("extra_calls_after_expected".to_string());
    }
    if value_u64(summary, "/tool_only_turns/max_consecutive") >= 8 {
        points.push("long_tool_only_streak".to_string());
    }
    if value_f64(summary, "/max_context_window_pct") >= 20.0 {
        points.push("high_context_pressure".to_string());
    }
    if value_u64(summary, "/compactions") > 0 {
        points.push("compaction_needed".to_string());
    }
    if validation_timed_out {
        points.push("validation_timeout".to_string());
    }
    if validation_present && validation_exit_code != Some(0) {
        points.push("validation_failed".to_string());
    }
    if browser_validation_timed_out {
        points.push("browser_validation_timeout".to_string());
    }
    if browser_validation_present && browser_validation_exit_code != Some(0) {
        points.push("browser_validation_failed".to_string());
    }
    points
}

fn exact_completion_pressure_scenario(scenario: &str) -> bool {
    matches!(
        scenario,
        "file-edit"
            | "file-ops"
            | "tool-recovery"
            | "react-calculator-scaffold"
            | "rust-log-analyzer-scaffold"
            | "natural-compaction"
            | "compaction-pressure"
    )
}

fn rows_to_csv(rows: &[BenchmarkRunRow]) -> String {
    let mut csv = String::from(
        "run_id,trace_dir,suite,scenario,model,score,task_quality_score,efficiency_score,harness_pressure_score,success,validation_present,validation_exit_code,validation_timed_out,browser_validation_present,browser_validation_exit_code,browser_validation_timed_out,browser_screenshot,requests,tool_calls,max_approx_input_tokens,max_context_window_pct,max_request_duration_ms,total_duration_ms,source_files,source_bytes,compactions,tool_failures,recovered_tool_failures,unrecovered_tool_failures,truncated_tool_results,repeated_tool_calls,tool_only_turns,max_tool_only_streak,expected_tool_groups,satisfied_tool_groups,expected_tool_calls,satisfied_tool_calls,extra_calls_after_satisfied,extra_turns_after_satisfied,context_growth_after_satisfied_chars,diagnostics,failure_points\n",
    );
    for row in rows {
        let values = [
            row.run_id.clone(),
            row.trace_dir.clone(),
            row.suite.clone(),
            row.scenario.clone(),
            row.model.clone(),
            row.score.to_string(),
            row.task_quality_score.to_string(),
            row.efficiency_score.to_string(),
            row.harness_pressure_score.to_string(),
            row.success.to_string(),
            row.validation_present.to_string(),
            row.validation_exit_code
                .map(|code| code.to_string())
                .unwrap_or_default(),
            row.validation_timed_out.to_string(),
            row.browser_validation_present.to_string(),
            row.browser_validation_exit_code
                .map(|code| code.to_string())
                .unwrap_or_default(),
            row.browser_validation_timed_out.to_string(),
            row.browser_screenshot.clone(),
            row.requests.to_string(),
            row.tool_calls.to_string(),
            row.max_approx_input_tokens.to_string(),
            round1(row.max_context_window_pct).to_string(),
            row.max_request_duration_ms.to_string(),
            row.total_duration_ms.to_string(),
            row.source_files.to_string(),
            row.source_bytes.to_string(),
            row.compactions.to_string(),
            row.tool_failures.to_string(),
            row.recovered_tool_failures.to_string(),
            row.unrecovered_tool_failures.to_string(),
            row.truncated_tool_results.to_string(),
            row.repeated_tool_calls.to_string(),
            row.tool_only_turns.to_string(),
            row.max_tool_only_streak.to_string(),
            row.expected_tool_groups.to_string(),
            row.satisfied_tool_groups.to_string(),
            row.expected_tool_calls.to_string(),
            row.satisfied_tool_calls.to_string(),
            row.extra_calls_after_satisfied.to_string(),
            row.extra_turns_after_satisfied.to_string(),
            row.context_growth_after_satisfied_chars.to_string(),
            row.diagnostics.clone(),
            row.failure_points.clone(),
        ];
        csv.push_str(
            &values
                .iter()
                .map(|value| csv_escape(value))
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push('\n');
    }
    csv
}

fn rows_to_html(suite: &str, rows: &[BenchmarkRunRow], aggregate: &Value) -> String {
    let mut html = String::new();
    let avg = aggregate
        .get("average_score")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let avg_quality = aggregate
        .get("average_task_quality_score")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let avg_pressure = aggregate
        .get("average_harness_pressure_score")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let avg_efficiency = aggregate
        .get("average_efficiency_score")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let max_streak = aggregate
        .get("max_tool_only_streak")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let truncations = aggregate
        .get("total_truncations")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let _ = write!(
        html,
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Spark Benchmark Report - {suite}</title>
<style>
body {{ margin: 0; font-family: Segoe UI, Arial, sans-serif; color: #172033; background: #f7f8fb; }}
main {{ max-width: 1180px; margin: 0 auto; padding: 32px 24px 48px; }}
h1 {{ margin: 0 0 8px; font-size: 28px; }}
h2 {{ margin: 32px 0 12px; font-size: 18px; }}
p {{ line-height: 1.5; color: #4d5a70; }}
.kpis {{ display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; margin: 20px 0 28px; }}
.kpi {{ background: #fff; border: 1px solid #dce2ec; border-radius: 8px; padding: 14px; }}
.kpi strong {{ display: block; font-size: 24px; color: #111827; }}
.kpi span {{ display: block; margin-top: 4px; color: #607089; font-size: 13px; }}
.panel {{ background: #fff; border: 1px solid #dce2ec; border-radius: 8px; padding: 18px; margin-bottom: 18px; overflow-x: auto; }}
svg {{ width: 100%; height: auto; display: block; }}
table {{ width: 100%; border-collapse: collapse; font-size: 13px; }}
th, td {{ padding: 9px 10px; border-bottom: 1px solid #e5eaf2; text-align: left; vertical-align: top; }}
th {{ color: #3a475d; font-weight: 650; background: #f3f6fa; }}
td.num {{ text-align: right; font-variant-numeric: tabular-nums; }}
.muted {{ color: #68778f; }}
@media (max-width: 760px) {{ .kpis {{ grid-template-columns: repeat(2, minmax(0, 1fr)); }} }}
</style>
</head>
<body>
<main>
<h1>Spark Benchmark Report: {suite}</h1>
<p>Generated from saved benchmark traces. Scores keep failure modes visible alongside pass/fail results.</p>
<section class="kpis">
<div class="kpi"><strong>{avg:.1}</strong><span>Average score</span></div>
<div class="kpi"><strong>{avg_quality:.1}</strong><span>Task quality</span></div>
<div class="kpi"><strong>{avg_efficiency:.1}</strong><span>Efficiency</span></div>
<div class="kpi"><strong>{avg_pressure:.1}</strong><span>Harness pressure</span></div>
</section>
"#,
        suite = html_escape(suite),
        avg = avg,
        avg_quality = avg_quality,
        avg_efficiency = avg_efficiency,
        avg_pressure = avg_pressure
    );
    html.push_str("<h2>Score by Scenario</h2><div class=\"panel\">");
    html.push_str(&score_svg(rows));
    html.push_str("</div><h2>Failure Pressure by Scenario</h2><div class=\"panel\">");
    html.push_str(&pressure_svg(rows));
    html.push_str("</div>");
    let _ = write!(
        html,
        "<p class=\"muted\">Total truncations: {truncations}. Final score is production outcome plus efficiency. Harness pressure is diagnostic and does not reduce production score directly. Max tool-only streak: {max_streak}.</p>",
        max_streak = max_streak
    );
    html.push_str("<h2>Run Details</h2><div class=\"panel\"><table><thead><tr><th>Scenario</th><th>Score</th><th>Task quality</th><th>Efficiency</th><th>Harness pressure</th><th>Validation</th><th>Browser</th><th>Duration</th><th>Source</th><th>Requests</th><th>Tools</th><th>Max context</th><th>Extra calls</th><th>Tool-only</th><th>Diagnostics</th><th>Failure points</th></tr></thead><tbody>");
    for row in rows {
        let validation = if row.validation_present {
            row.validation_exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "no exit".to_string())
        } else {
            "n/a".to_string()
        };
        let browser = if row.browser_validation_present {
            row.browser_validation_exit_code
                .map(|code| format!("exit {code}"))
                .unwrap_or_else(|| "no exit".to_string())
        } else {
            "n/a".to_string()
        };
        let _ = write!(
            html,
            "<tr><td>{}</td><td class=\"num\">{:.1}</td><td class=\"num\">{:.1}</td><td class=\"num\">{:.1}</td><td class=\"num\">{:.1}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{} / {}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{:.1}%</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td><td>{}</td></tr>",
            html_escape(&row.scenario),
            row.score,
            row.task_quality_score,
            row.efficiency_score,
            row.harness_pressure_score,
            html_escape(&validation),
            html_escape(&browser),
            row.total_duration_ms,
            row.source_files,
            row.source_bytes,
            row.requests,
            row.tool_calls,
            row.max_context_window_pct,
            row.extra_calls_after_satisfied,
            row.max_tool_only_streak,
            html_escape(&row.diagnostics),
            html_escape(&row.failure_points)
        );
    }
    html.push_str("</tbody></table></div></main></body></html>");
    html
}

fn score_svg(rows: &[BenchmarkRunRow]) -> String {
    let row_height = 34usize;
    let label_width = 230usize;
    let chart_width = 760usize;
    let height = 32 + rows.len() * row_height;
    let mut svg = format!(
        r#"<svg viewBox="0 0 {} {}" role="img" aria-label="Benchmark score by scenario">"#,
        label_width + chart_width + 80,
        height
    );
    for (index, row) in rows.iter().enumerate() {
        let y = 28 + index * row_height;
        let width = (row.score / 100.0 * chart_width as f64).round() as usize;
        let color = if row.score >= 80.0 {
            "#2f7da1"
        } else if row.score >= 65.0 {
            "#b8832f"
        } else {
            "#b85c5c"
        };
        let _ = write!(
            svg,
            r##"<text x="0" y="{}" font-size="13" fill="#263244">{}</text><rect x="{}" y="{}" width="{}" height="18" rx="3" fill="{}"/><text x="{}" y="{}" font-size="12" fill="#263244">{:.1}</text>"##,
            y + 14,
            html_escape(&row.scenario),
            label_width,
            y,
            width,
            color,
            label_width + width + 8,
            y + 14,
            row.score
        );
    }
    svg.push_str("</svg>");
    svg
}

fn pressure_svg(rows: &[BenchmarkRunRow]) -> String {
    let row_height = 34usize;
    let label_width = 230usize;
    let chart_width = 760usize;
    let max_pressure = rows
        .iter()
        .map(|row| {
            row.max_tool_only_streak + row.extra_calls_after_satisfied + row.truncated_tool_results
        })
        .max()
        .unwrap_or(1)
        .max(1);
    let height = 32 + rows.len() * row_height;
    let mut svg = format!(
        r#"<svg viewBox="0 0 {} {}" role="img" aria-label="Failure pressure by scenario">"#,
        label_width + chart_width + 120,
        height
    );
    for (index, row) in rows.iter().enumerate() {
        let y = 28 + index * row_height;
        let tool_only = (row.max_tool_only_streak as f64 / max_pressure as f64 * chart_width as f64)
            .round() as usize;
        let extra = (row.extra_calls_after_satisfied as f64 / max_pressure as f64
            * chart_width as f64)
            .round() as usize;
        let trunc = (row.truncated_tool_results as f64 / max_pressure as f64 * chart_width as f64)
            .round() as usize;
        let _ = write!(
            svg,
            r##"<text x="0" y="{}" font-size="13" fill="#263244">{}</text><rect x="{}" y="{}" width="{}" height="18" rx="3" fill="#2f7da1"/><rect x="{}" y="{}" width="{}" height="18" fill="#d39b35"/><rect x="{}" y="{}" width="{}" height="18" fill="#b85c5c"/><text x="{}" y="{}" font-size="12" fill="#263244">tool-only {} / extra {} / trunc {}</text>"##,
            y + 14,
            html_escape(&row.scenario),
            label_width,
            y,
            tool_only,
            label_width + tool_only,
            y,
            extra,
            label_width + tool_only + extra,
            y,
            trunc,
            label_width + tool_only + extra + trunc + 8,
            y + 14,
            row.max_tool_only_streak,
            row.extra_calls_after_satisfied,
            row.truncated_tool_results
        );
    }
    svg.push_str("</svg>");
    svg
}

fn read_codex_cli_rows(path: &Path) -> Result<Vec<CodexCliBenchmarkRow>> {
    let value: Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(|error| {
        anyhow::anyhow!(
            "failed to read Codex CLI report {}: {error}",
            path.display()
        )
    })?)
    .map_err(|error| {
        anyhow::anyhow!(
            "failed to parse Codex CLI report JSON {}: {error}",
            path.display()
        )
    })?;
    serde_json::from_value(value.get("rows").cloned().unwrap_or_else(|| json!([]))).map_err(
        |error| {
            anyhow::anyhow!(
                "failed to parse Codex CLI report rows from {}: {error}",
                path.display()
            )
        },
    )
}

fn comparison_row_from_harness(row: &BenchmarkRunRow) -> ComparisonRow {
    ComparisonRow {
        runner: "spark-harness".to_string(),
        suite: row.suite.clone(),
        scenario: row.scenario.clone(),
        model: row.model.clone(),
        score: row.score,
        task_quality_score: row.task_quality_score,
        efficiency_score: row.efficiency_score,
        harness_pressure_score: row.harness_pressure_score,
        success: row.success,
        validation_exit_code: row.validation_exit_code,
        validation_timed_out: row.validation_timed_out,
        duration_ms: row.total_duration_ms as u128,
        requests_or_turns: row.requests,
        tool_or_item_calls: row.tool_calls,
        input_tokens: row.max_approx_input_tokens,
        output_tokens: 0,
        source_files: row.source_files,
        source_bytes: row.source_bytes,
        failure_points: row.failure_points.clone(),
        source: row.trace_dir.clone(),
    }
}

fn comparison_row_from_codex_cli(row: &CodexCliBenchmarkRow) -> ComparisonRow {
    let task_quality_score = codex_task_quality_score(row);
    let efficiency_score = codex_efficiency_score(row);
    ComparisonRow {
        runner: "codex-cli".to_string(),
        suite: row.suite.clone(),
        scenario: row.scenario.clone(),
        model: row.model.clone(),
        score: codex_score_from_components(row, task_quality_score, efficiency_score),
        task_quality_score,
        efficiency_score,
        harness_pressure_score: codex_pressure_score(row),
        success: row.success,
        validation_exit_code: row.validation_exit_code,
        validation_timed_out: row.validation_timed_out,
        duration_ms: row.duration_ms,
        requests_or_turns: row.turns,
        tool_or_item_calls: row.completed_items,
        input_tokens: row.input_tokens,
        output_tokens: row.output_tokens,
        source_files: row.source_files,
        source_bytes: row.source_bytes,
        failure_points: row.failure_points.clone(),
        source: row.run_dir.clone(),
    }
}

fn codex_score_from_components(
    row: &CodexCliBenchmarkRow,
    task_quality_score: f64,
    efficiency_score: f64,
) -> f64 {
    let mut score = task_quality_score * 0.85 + efficiency_score * 0.15;
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

fn codex_task_quality_score(row: &CodexCliBenchmarkRow) -> f64 {
    let mut penalty = 0.0;
    if row.timed_out {
        penalty += 35.0;
    }
    if row.exit_code != Some(0) {
        penalty += 25.0;
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
    if row.browser_validation_timed_out {
        penalty += 35.0;
    }
    if row.browser_validation_present && row.browser_validation_exit_code != Some(0) {
        penalty += 55.0;
    }
    round1((100.0 - penalty).clamp(0.0, 100.0))
}

fn codex_efficiency_score(row: &CodexCliBenchmarkRow) -> f64 {
    efficiency_score(row.duration_ms, row.source_bytes)
}

fn codex_pressure_score(row: &CodexCliBenchmarkRow) -> f64 {
    let mut penalty = 0.0;
    penalty += (row.non_json_stdout_lines.min(20) as f64) * 0.5;
    penalty += (row.actionable_stderr_lines.min(20) as f64) * 0.25;
    if row.duration_ms > 180_000 {
        penalty += 10.0;
    } else if row.duration_ms > 90_000 {
        penalty += 5.0;
    }
    round1((100.0 - penalty).clamp(0.0, 100.0))
}

fn aggregate_comparison(suite: &str, rows: &[ComparisonRow]) -> Value {
    let mut runner_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_quality_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_efficiency_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_pressure_scores = BTreeMap::<String, Vec<f64>>::new();
    for row in rows {
        runner_scores
            .entry(row.runner.clone())
            .or_default()
            .push(row.score);
        runner_quality_scores
            .entry(row.runner.clone())
            .or_default()
            .push(row.task_quality_score);
        runner_efficiency_scores
            .entry(row.runner.clone())
            .or_default()
            .push(row.efficiency_score);
        runner_pressure_scores
            .entry(row.runner.clone())
            .or_default()
            .push(row.harness_pressure_score);
    }
    let runner_averages = runner_scores
        .into_iter()
        .map(|(runner, scores)| {
            let average = scores.iter().sum::<f64>() / scores.len() as f64;
            (runner, round1(average))
        })
        .collect::<BTreeMap<_, _>>();
    let runner_task_quality_averages = average_map(runner_quality_scores);
    let runner_efficiency_averages = average_map(runner_efficiency_scores);
    let runner_harness_pressure_averages = average_map(runner_pressure_scores);
    let winner = runner_averages
        .iter()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(runner, score)| json!({"runner": runner, "average_score": score}));
    let scenario_winners = comparison_scenario_winners(rows);

    json!({
        "suite": suite,
        "rows": rows.len(),
        "runner_averages": runner_averages,
        "runner_task_quality_averages": runner_task_quality_averages,
        "runner_efficiency_averages": runner_efficiency_averages,
        "runner_harness_pressure_averages": runner_harness_pressure_averages,
        "winner": winner,
        "scenario_winners": scenario_winners,
    })
}

fn average_map(scores_by_key: BTreeMap<String, Vec<f64>>) -> BTreeMap<String, f64> {
    scores_by_key
        .into_iter()
        .map(|(key, scores)| {
            let average = scores.iter().sum::<f64>() / scores.len() as f64;
            (key, round1(average))
        })
        .collect()
}

fn comparison_scenario_winners(rows: &[ComparisonRow]) -> Vec<Value> {
    let mut by_scenario = BTreeMap::<String, Vec<&ComparisonRow>>::new();
    for row in rows {
        by_scenario
            .entry(row.scenario.clone())
            .or_default()
            .push(row);
    }
    by_scenario
        .into_iter()
        .map(|(scenario, rows)| {
            let winner = rows
                .iter()
                .max_by(|left, right| left.score.total_cmp(&right.score));
            json!({
                "scenario": scenario,
                "winner": winner.map(|row| row.runner.clone()).unwrap_or_default(),
                "winning_score": winner.map(|row| row.score).unwrap_or_default(),
                "scores": rows.iter().map(|row| json!({
                    "runner": row.runner,
                    "score": row.score,
                    "task_quality_score": row.task_quality_score,
                    "efficiency_score": row.efficiency_score,
                    "harness_pressure_score": row.harness_pressure_score,
                    "success": row.success,
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn comparison_rows_to_csv(rows: &[ComparisonRow]) -> String {
    let mut csv = String::from(
        "runner,suite,scenario,model,score,task_quality_score,efficiency_score,harness_pressure_score,success,validation_exit_code,validation_timed_out,duration_ms,requests_or_turns,tool_or_item_calls,input_tokens,output_tokens,source_files,source_bytes,failure_points,source\n",
    );
    for row in rows {
        let values = [
            row.runner.clone(),
            row.suite.clone(),
            row.scenario.clone(),
            row.model.clone(),
            row.score.to_string(),
            row.task_quality_score.to_string(),
            row.efficiency_score.to_string(),
            row.harness_pressure_score.to_string(),
            row.success.to_string(),
            row.validation_exit_code
                .map(|code| code.to_string())
                .unwrap_or_default(),
            row.validation_timed_out.to_string(),
            row.duration_ms.to_string(),
            row.requests_or_turns.to_string(),
            row.tool_or_item_calls.to_string(),
            row.input_tokens.to_string(),
            row.output_tokens.to_string(),
            row.source_files.to_string(),
            row.source_bytes.to_string(),
            row.failure_points.clone(),
            row.source.clone(),
        ];
        csv.push_str(
            &values
                .iter()
                .map(|value| csv_escape(value))
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push('\n');
    }
    csv
}

fn comparison_rows_to_html(suite: &str, rows: &[ComparisonRow], aggregate: &Value) -> String {
    let winner = aggregate
        .pointer("/winner/runner")
        .and_then(Value::as_str)
        .unwrap_or("undetermined");
    let mut html = format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Spark Harness vs Codex CLI - {}</title>
<style>
body {{ margin: 0; font-family: Segoe UI, Arial, sans-serif; color: #172033; background: #f7f8fb; }}
main {{ max-width: 1180px; margin: 0 auto; padding: 32px 24px 48px; }}
h1 {{ margin: 0 0 8px; font-size: 28px; }}
h2 {{ margin: 32px 0 12px; font-size: 18px; }}
p {{ line-height: 1.5; color: #4d5a70; }}
.panel {{ background: #fff; border: 1px solid #dce2ec; border-radius: 8px; padding: 18px; margin-bottom: 18px; overflow-x: auto; }}
table {{ width: 100%; border-collapse: collapse; font-size: 13px; }}
th, td {{ padding: 9px 10px; border-bottom: 1px solid #e5eaf2; text-align: left; vertical-align: top; }}
th {{ color: #3a475d; font-weight: 650; background: #f3f6fa; }}
td.num {{ text-align: right; font-variant-numeric: tabular-nums; }}
svg {{ width: 100%; height: auto; display: block; }}
</style>
</head>
<body><main>
<h1>Spark Harness vs Codex CLI: {}</h1>
<p>Winner by average score: <strong>{}</strong>. Scores are runner-specific and should be read with the failure points and source artifacts.</p>
"#,
        html_escape(suite),
        html_escape(suite),
        html_escape(winner)
    );
    html.push_str("<h2>Score Comparison</h2><div class=\"panel\">");
    html.push_str(&comparison_score_svg(rows));
    html.push_str("</div><h2>Rows</h2><div class=\"panel\"><table><thead><tr><th>Runner</th><th>Scenario</th><th>Score</th><th>Task quality</th><th>Efficiency</th><th>Harness pressure</th><th>Validation</th><th>Success</th><th>Duration</th><th>Source</th><th>Turns/Requests</th><th>Items/Tools</th><th>Failure points</th></tr></thead><tbody>");
    for row in rows {
        let validation = row
            .validation_exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| {
                if row.validation_timed_out {
                    "timeout".to_string()
                } else {
                    "n/a".to_string()
                }
            });
        let _ = write!(
            html,
            "<tr><td>{}</td><td>{}</td><td class=\"num\">{:.1}</td><td class=\"num\">{:.1}</td><td class=\"num\">{:.1}</td><td class=\"num\">{:.1}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{} / {}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td></tr>",
            html_escape(&row.runner),
            html_escape(&row.scenario),
            row.score,
            row.task_quality_score,
            row.efficiency_score,
            row.harness_pressure_score,
            html_escape(&validation),
            row.success,
            row.duration_ms,
            row.source_files,
            row.source_bytes,
            row.requests_or_turns,
            row.tool_or_item_calls,
            html_escape(&row.failure_points)
        );
    }
    html.push_str("</tbody></table></div></main></body></html>");
    html
}

fn comparison_score_svg(rows: &[ComparisonRow]) -> String {
    let row_height = 30usize;
    let label_width = 290usize;
    let chart_width = 690usize;
    let height = 30 + rows.len() * row_height;
    let mut svg = format!(
        r#"<svg viewBox="0 0 {} {}" role="img" aria-label="Benchmark score comparison">"#,
        label_width + chart_width + 70,
        height
    );
    for (index, row) in rows.iter().enumerate() {
        let y = 24 + index * row_height;
        let width = (row.score / 100.0 * chart_width as f64).round() as usize;
        let color = if row.runner == "spark-harness" {
            "#2f7da1"
        } else {
            "#7b6bb7"
        };
        let label = format!("{} / {}", row.scenario, row.runner);
        let _ = write!(
            svg,
            r##"<text x="0" y="{}" font-size="12" fill="#263244">{}</text><rect x="{}" y="{}" width="{}" height="18" rx="3" fill="{}"/><text x="{}" y="{}" font-size="12" fill="#263244">{:.1}</text>"##,
            y + 13,
            html_escape(&label),
            label_width,
            y,
            width,
            color,
            label_width + width + 8,
            y + 13,
            row.score
        );
    }
    svg.push_str("</svg>");
    svg
}

fn scenario_order(suite: ProfileBenchmarkSuiteKind, scenario: &str) -> usize {
    suite
        .scenarios()
        .iter()
        .position(|kind| kind.name() == scenario)
        .unwrap_or(usize::MAX)
}

fn benchmark_suite(summary: &Value) -> Option<&str> {
    string_at(
        summary,
        "/trace_metadata/context/profile_scenario/benchmark_suite",
    )
}

fn scenario_name(summary: &Value) -> Option<&str> {
    string_at(summary, "/trace_metadata/context/profile_scenario/name")
}

fn diagnostic_kinds(summary: &Value) -> Vec<String> {
    summary
        .get("diagnostics")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|diagnostic| diagnostic.get("kind").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

fn value_u64(summary: &Value, pointer: &str) -> u64 {
    summary
        .pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn value_f64(summary: &Value, pointer: &str) -> f64 {
    summary
        .pointer(pointer)
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

fn string_at<'a>(summary: &'a Value, pointer: &str) -> Option<&'a str> {
    summary.pointer(pointer).and_then(Value::as_str)
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn unix_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn benchmark_score_penalizes_overrun_and_tool_loops() {
        let clean = BenchmarkRunRow {
            run_id: "run-1".to_string(),
            trace_dir: ".spark-runs/run-1".to_string(),
            suite: "real-world".to_string(),
            scenario: "react-calculator-scaffold".to_string(),
            model: "spark".to_string(),
            score: 0.0,
            task_quality_score: 0.0,
            efficiency_score: 0.0,
            harness_pressure_score: 0.0,
            success: true,
            validation_present: false,
            validation_exit_code: None,
            validation_timed_out: false,
            browser_validation_present: false,
            browser_validation_exit_code: None,
            browser_validation_timed_out: false,
            browser_screenshot: String::new(),
            requests: 4,
            tool_calls: 4,
            max_approx_input_tokens: 1000,
            max_context_window_pct: 1.0,
            max_request_duration_ms: 1000,
            total_duration_ms: 1500,
            source_files: 0,
            source_bytes: 0,
            compactions: 0,
            tool_failures: 0,
            recovered_tool_failures: 0,
            unrecovered_tool_failures: 0,
            truncated_tool_results: 0,
            repeated_tool_calls: 0,
            tool_only_turns: 3,
            max_tool_only_streak: 3,
            expected_tool_groups: 2,
            satisfied_tool_groups: 2,
            expected_tool_calls: 2,
            satisfied_tool_calls: 2,
            extra_calls_after_satisfied: 0,
            extra_turns_after_satisfied: 0,
            context_growth_after_satisfied_chars: 0,
            diagnostics: String::new(),
            failure_points: String::new(),
        };
        let mut noisy = clean.clone();
        noisy.max_tool_only_streak = 12;
        noisy.extra_calls_after_satisfied = 4;
        noisy.extra_turns_after_satisfied = 4;
        noisy.truncated_tool_results = 1;

        let mut clean = clean;
        clean.task_quality_score = task_quality_score(&clean);
        clean.efficiency_score = efficiency_score(clean.total_duration_ms as u128, 0);
        clean.harness_pressure_score = harness_pressure_score(&clean);
        let mut noisy = noisy;
        noisy.task_quality_score = task_quality_score(&noisy);
        noisy.efficiency_score = efficiency_score(noisy.total_duration_ms as u128, 0);
        noisy.harness_pressure_score = harness_pressure_score(&noisy);

        assert_eq!(benchmark_score(&clean), 100.0);
        assert!(benchmark_score(&noisy) > 90.0);
        assert!(noisy.harness_pressure_score < 70.0);
    }

    #[test]
    fn failure_points_include_benchmark_specific_signals() {
        let summary = json!({
            "errors": [],
            "truncated_tool_results": 1,
            "repeated_tool_calls": 2,
            "tool_only_turns": {"max_consecutive": 12},
            "max_context_window_pct": 24.0,
            "compactions": 1,
            "profile_scenario_call_expectations": {
                "extra_calls_after_satisfied": 3
            }
        });

        let points = failure_points(
            "react-calculator-scaffold",
            &summary,
            2,
            2,
            4,
            4,
            0,
            false,
            None,
            false,
            false,
            None,
            false,
        );

        assert!(points.contains(&"truncated_tool_result".to_string()));
        assert!(points.contains(&"repeated_tool_call".to_string()));
        assert!(points.contains(&"extra_calls_after_expected".to_string()));
        assert!(points.contains(&"long_tool_only_streak".to_string()));
        assert!(points.contains(&"high_context_pressure".to_string()));
        assert!(points.contains(&"compaction_needed".to_string()));
    }

    #[test]
    fn survey_extra_calls_are_not_exact_completion_pressure() {
        let summary = json!({
            "errors": [],
            "truncated_tool_results": 0,
            "repeated_tool_calls": 0,
            "tool_only_turns": {"max_consecutive": 4},
            "max_context_window_pct": 1.0,
            "compactions": 0,
            "profile_scenario_call_expectations": {
                "extra_calls_after_satisfied": 6
            }
        });

        let points = failure_points(
            "benchmark-design-survey",
            &summary,
            2,
            2,
            4,
            4,
            0,
            false,
            None,
            false,
            false,
            None,
            false,
        );

        assert!(!points.contains(&"extra_calls_after_expected".to_string()));
    }

    #[test]
    fn comparison_aggregate_picks_highest_average_runner() {
        let rows = vec![
            ComparisonRow {
                runner: "spark-harness".to_string(),
                suite: "real-world".to_string(),
                scenario: "one".to_string(),
                model: "spark".to_string(),
                score: 80.0,
                task_quality_score: 90.0,
                efficiency_score: 85.0,
                harness_pressure_score: 70.0,
                success: true,
                validation_exit_code: None,
                validation_timed_out: false,
                duration_ms: 100,
                requests_or_turns: 2,
                tool_or_item_calls: 3,
                input_tokens: 100,
                output_tokens: 0,
                source_files: 0,
                source_bytes: 0,
                failure_points: String::new(),
                source: "trace".to_string(),
            },
            ComparisonRow {
                runner: "codex-cli".to_string(),
                suite: "real-world".to_string(),
                scenario: "one".to_string(),
                model: "spark".to_string(),
                score: 70.0,
                task_quality_score: 80.0,
                efficiency_score: 75.0,
                harness_pressure_score: 60.0,
                success: true,
                validation_exit_code: None,
                validation_timed_out: false,
                duration_ms: 100,
                requests_or_turns: 1,
                tool_or_item_calls: 1,
                input_tokens: 100,
                output_tokens: 10,
                source_files: 0,
                source_bytes: 0,
                failure_points: String::new(),
                source: "run".to_string(),
            },
        ];

        let aggregate = aggregate_comparison("real-world", &rows);

        assert_eq!(aggregate["winner"]["runner"], "spark-harness");
        assert_eq!(aggregate["runner_averages"]["spark-harness"], 80.0);
        assert_eq!(
            aggregate["runner_task_quality_averages"]["spark-harness"],
            90.0
        );
    }

    #[test]
    fn task_quality_score_penalizes_validation_failure() {
        let row = BenchmarkRunRow {
            run_id: "run-1".to_string(),
            trace_dir: ".spark-runs/run-1".to_string(),
            suite: "real-world".to_string(),
            scenario: "react-calculator-scaffold".to_string(),
            model: "spark".to_string(),
            score: 0.0,
            task_quality_score: 0.0,
            efficiency_score: 0.0,
            harness_pressure_score: 0.0,
            success: false,
            validation_present: true,
            validation_exit_code: Some(1),
            validation_timed_out: false,
            browser_validation_present: false,
            browser_validation_exit_code: None,
            browser_validation_timed_out: false,
            browser_screenshot: String::new(),
            requests: 4,
            tool_calls: 4,
            max_approx_input_tokens: 1000,
            max_context_window_pct: 1.0,
            max_request_duration_ms: 1000,
            total_duration_ms: 1500,
            source_files: 0,
            source_bytes: 0,
            compactions: 0,
            tool_failures: 0,
            recovered_tool_failures: 0,
            unrecovered_tool_failures: 0,
            truncated_tool_results: 0,
            repeated_tool_calls: 0,
            tool_only_turns: 3,
            max_tool_only_streak: 3,
            expected_tool_groups: 2,
            satisfied_tool_groups: 2,
            expected_tool_calls: 2,
            satisfied_tool_calls: 2,
            extra_calls_after_satisfied: 0,
            extra_turns_after_satisfied: 0,
            context_growth_after_satisfied_chars: 0,
            diagnostics: String::new(),
            failure_points: "validation_failed".to_string(),
        };

        assert_eq!(task_quality_score(&row), 50.0);
    }

    #[test]
    fn task_quality_score_penalizes_browser_validation_failure() {
        let row = BenchmarkRunRow {
            run_id: "run-1".to_string(),
            trace_dir: ".spark-runs/run-1".to_string(),
            suite: "scaffolding".to_string(),
            scenario: "react-calculator-scaffold".to_string(),
            model: "spark".to_string(),
            score: 0.0,
            task_quality_score: 0.0,
            efficiency_score: 0.0,
            harness_pressure_score: 0.0,
            success: false,
            validation_present: true,
            validation_exit_code: Some(0),
            validation_timed_out: false,
            browser_validation_present: true,
            browser_validation_exit_code: Some(1),
            browser_validation_timed_out: false,
            browser_screenshot: "react-calculator-browser.png".to_string(),
            requests: 4,
            tool_calls: 4,
            max_approx_input_tokens: 1000,
            max_context_window_pct: 1.0,
            max_request_duration_ms: 1000,
            total_duration_ms: 1500,
            source_files: 0,
            source_bytes: 0,
            compactions: 0,
            tool_failures: 0,
            recovered_tool_failures: 0,
            unrecovered_tool_failures: 0,
            truncated_tool_results: 0,
            repeated_tool_calls: 0,
            tool_only_turns: 3,
            max_tool_only_streak: 3,
            expected_tool_groups: 2,
            satisfied_tool_groups: 2,
            expected_tool_calls: 2,
            satisfied_tool_calls: 2,
            extra_calls_after_satisfied: 0,
            extra_turns_after_satisfied: 0,
            context_growth_after_satisfied_chars: 0,
            diagnostics: String::new(),
            failure_points: "browser_validation_failed".to_string(),
        };

        assert_eq!(task_quality_score(&row), 20.0);
    }
}
