use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    benchmark::{
        codex_cli::CodexCliBenchmarkRow, infrastructure::failure_points_contain,
        judge::BenchmarkJudgeReport,
    },
    cli::{ProfileBenchmarkSuiteKind, ProfileScenarioKind},
    profile::validation,
    profiler,
    trace::commands,
};

mod html;
mod infrastructure;
mod scenarios;
mod scoring;

#[cfg(test)]
use html::{comparison_input_table, comparison_score_svg};
use html::{comparison_rows_to_html, format_duration_ms};
use infrastructure::{
    ComparisonDiagnostics, filter_harness_request_failure_rows,
    read_external_agent_report_rows_with_skips, skipped_infrastructure_scenarios_text,
};
#[cfg(test)]
use scoring::expected_repeated_tool_calls;
use scoring::{
    benchmark_score, codex_completion_score, codex_efficiency_score, codex_process_score,
    codex_quality_score_with_validation, codex_score_from_components,
    comparison_score_from_components, completion_score, efficiency_score,
    exact_completion_pressure_scenario, process_score, quality_gate, quality_score_with_validation,
    unexpected_repeated_tool_calls,
};
#[cfg(test)]
use scoring::{codex_quality_score, quality_score};

#[derive(Debug, Clone)]
pub(crate) struct BenchmarkReportOptions {
    pub(crate) cwd: PathBuf,
    pub(crate) suite: ProfileBenchmarkSuiteKind,
    pub(crate) limit: usize,
    pub(crate) all_runs: bool,
    pub(crate) run_manifests: Vec<PathBuf>,
    pub(crate) output_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct BenchmarkRunRow {
    run_id: String,
    trace_dir: String,
    suite: String,
    scenario: String,
    model: String,
    reasoning_effort: String,
    completion_score: f64,
    quality_score: f64,
    process_score: f64,
    efficiency_index: Option<f64>,
    benchmark_index: Option<f64>,
    score: f64,
    task_quality_score: f64,
    efficiency_score: f64,
    harness_pressure_score: f64,
    success: bool,
    validation_present: bool,
    validation_exit_code: Option<i32>,
    #[serde(default)]
    validation_score: Option<f64>,
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
    pub(crate) harness_reports: Vec<PathBuf>,
    pub(crate) codex_cli_reports: Vec<PathBuf>,
    pub(crate) opencode_reports: Vec<PathBuf>,
    pub(crate) llm_judge_report: Option<PathBuf>,
    pub(crate) group_by_reasoning: bool,
    pub(crate) group_by_model: bool,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct BenchmarkRunManifestTrace {
    pub(crate) scenario: String,
    pub(crate) repeat_index: usize,
    pub(crate) workspace: String,
    pub(crate) trace_dir: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct BenchmarkRunManifest {
    suite: String,
    generated_at_unix_ms: u128,
    expected_scenarios: Vec<String>,
    traces: Vec<BenchmarkRunManifestTrace>,
    missing_scenarios: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ComparisonRow {
    runner: String,
    suite: String,
    scenario: String,
    attempts: usize,
    successful_attempts: usize,
    model: String,
    reasoning_effort: String,
    command_path: String,
    command_version: String,
    completion_score: f64,
    quality_score: f64,
    process_score: f64,
    llm_solution_score: Option<f64>,
    llm_process_score: Option<f64>,
    llm_confidence: Option<f64>,
    llm_notes: String,
    efficiency_index: Option<f64>,
    benchmark_index: Option<f64>,
    score: f64,
    task_quality_score: f64,
    efficiency_score: f64,
    harness_pressure_score: f64,
    success: bool,
    validation_exit_code: Option<i32>,
    validation_timed_out: bool,
    duration_ms: u128,
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
    let rows = if options.run_manifests.is_empty() {
        collect_benchmark_rows(&options)?
    } else {
        collect_benchmark_rows_from_manifest_paths(
            &options.cwd,
            options.suite,
            &options.run_manifests,
        )?
    };
    if rows.is_empty() {
        anyhow::bail!(
            "no traces found for benchmark suite '{}' under {}",
            options.suite.name(),
            commands::trace_runs_root(&options.cwd).display()
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
            "latest_per_scenario": options.run_manifests.is_empty() && !options.all_runs,
            "run_manifests": options.run_manifests,
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
    let harness_rows = if options.harness_reports.is_empty() {
        collect_benchmark_rows(&BenchmarkReportOptions {
            cwd: options.cwd.clone(),
            suite: options.suite,
            limit: options.limit,
            all_runs: options.all_runs,
            run_manifests: Vec::new(),
            output_dir: options.output_dir.clone(),
        })?
    } else {
        collect_benchmark_rows_from_harness_input_paths(
            &options.cwd,
            options.suite,
            &options.harness_reports,
        )?
    };
    let harness_rows = filter_harness_request_failure_rows(&options.cwd, harness_rows);
    if harness_rows.rows.is_empty() {
        if harness_rows.skipped_request_failures > 0 {
            let skipped_scenarios = skipped_infrastructure_scenarios_text(
                &harness_rows.skipped_request_failure_scenarios,
            );
            anyhow::bail!(
                "no Spark harness rows found after skipping {} provider/API failure row(s){}",
                harness_rows.skipped_request_failures,
                skipped_scenarios
            );
        }
        anyhow::bail!(
            "no harness traces found for benchmark suite '{}' under {}",
            options.suite.name(),
            commands::trace_runs_root(&options.cwd).display()
        );
    }
    let codex_external_rows = read_external_agent_report_rows_with_skips(
        &options.cwd,
        &options.codex_cli_reports,
        "Codex CLI",
    )?;
    let codex_rows = codex_external_rows.rows;
    if codex_rows.is_empty() {
        if codex_external_rows.skipped_infrastructure_failures > 0 {
            let skipped_scenarios = skipped_infrastructure_scenarios_text(
                &codex_external_rows.skipped_infrastructure_scenarios,
            );
            anyhow::bail!(
                "no Codex CLI rows found in provided reports after skipping {} infrastructure/API failure row(s){}",
                codex_external_rows.skipped_infrastructure_failures,
                skipped_scenarios
            );
        }
        anyhow::bail!("no Codex CLI rows found in provided reports");
    }
    let opencode_external_rows = read_external_agent_report_rows_with_skips(
        &options.cwd,
        &options.opencode_reports,
        "opencode",
    )?;
    let opencode_rows = opencode_external_rows.rows;

    std::fs::create_dir_all(&options.output_dir).map_err(|error| {
        anyhow::anyhow!(
            "failed to create benchmark comparison directory {}: {error}",
            options.output_dir.display()
        )
    })?;

    let mut rows = Vec::new();
    rows.extend(harness_rows.rows.iter().map(comparison_row_from_harness));
    rows.extend(
        codex_rows
            .iter()
            .filter(|row| row.suite == options.suite.name())
            .map(comparison_row_from_external_agent),
    );
    rows.extend(
        opencode_rows
            .iter()
            .filter(|row| row.suite == options.suite.name())
            .map(comparison_row_from_external_agent),
    );
    if options.group_by_model || options.group_by_reasoning {
        label_rows_by_model_and_reasoning(
            &mut rows,
            options.group_by_model,
            options.group_by_reasoning,
        );
    }
    if let Some(path) = &options.llm_judge_report {
        let judge_report = read_llm_judge_report(path)?;
        apply_llm_judge_scores(&mut rows, &judge_report);
    }
    let mut rows = average_comparison_attempts(rows);
    let baseline_runner = comparison_baseline_runner(&rows, "codex-cli");
    apply_comparison_indices(&mut rows, &baseline_runner);
    rows.sort_by(|left, right| {
        scenario_order(options.suite, &left.scenario)
            .cmp(&scenario_order(options.suite, &right.scenario))
            .then_with(|| left.runner.cmp(&right.runner))
    });

    let mut aggregate = aggregate_comparison_with_diagnostics(
        options.suite.name(),
        &rows,
        ComparisonDiagnostics {
            skipped_spark_infrastructure_failures: harness_rows.skipped_request_failures,
            skipped_spark_infrastructure_scenarios: harness_rows.skipped_request_failure_scenarios,
            skipped_spark_infrastructure_retry_hints: harness_rows
                .skipped_request_failure_retry_hints,
            skipped_codex_infrastructure_failures: codex_external_rows
                .skipped_infrastructure_failures,
            skipped_codex_infrastructure_scenarios: codex_external_rows
                .skipped_infrastructure_scenarios,
            skipped_codex_infrastructure_retry_hints: codex_external_rows
                .skipped_infrastructure_retry_hints,
            skipped_opencode_infrastructure_failures: opencode_external_rows
                .skipped_infrastructure_failures,
            skipped_opencode_infrastructure_scenarios: opencode_external_rows
                .skipped_infrastructure_scenarios,
            skipped_opencode_infrastructure_retry_hints: opencode_external_rows
                .skipped_infrastructure_retry_hints,
        },
    );
    let stamp = unix_millis();
    let stem = format!("{}-comparison-{stamp}", options.suite.name());
    let json_path = options.output_dir.join(format!("{stem}.json"));
    let csv_path = options.output_dir.join(format!("{stem}.csv"));
    let html_path = options.output_dir.join(format!("{stem}.html"));
    let inputs = comparison_input_metadata(&options);
    annotate_comparison_validity(&mut aggregate, &inputs);

    std::fs::write(
        &json_path,
        serde_json::to_string_pretty(&json!({
            "suite": options.suite.name(),
            "generated_at_unix_ms": stamp,
            "inputs": inputs,
            "rows": rows,
            "aggregate": aggregate,
        }))?,
    )
    .map_err(|error| anyhow::anyhow!("failed to write {}: {error}", json_path.display()))?;
    std::fs::write(&csv_path, comparison_rows_to_csv(&rows))
        .map_err(|error| anyhow::anyhow!("failed to write {}: {error}", csv_path.display()))?;
    std::fs::write(
        &html_path,
        comparison_rows_to_html(options.suite.name(), &rows, &aggregate, &inputs),
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

fn comparison_input_metadata(options: &BenchmarkComparisonOptions) -> Value {
    let mut inputs = json!({
        "harness_reports": input_report_metadata_list(&options.cwd, &options.harness_reports),
        "codex_cli_reports": input_report_metadata_list(&options.cwd, &options.codex_cli_reports),
        "opencode_reports": input_report_metadata_list(&options.cwd, &options.opencode_reports),
        "llm_judge_report": options
            .llm_judge_report
            .as_ref()
            .map(|path| input_report_metadata(&options.cwd, path))
            .unwrap_or(Value::Null),
        "harness_source": if options.harness_reports.is_empty() {
            "latest-trace-scan"
        } else {
            "explicit-report-inputs"
        },
        "group_by_model": options.group_by_model,
        "group_by_reasoning": options.group_by_reasoning,
    });
    let freshness = input_freshness_summary(&inputs);
    if let Some(object) = inputs.as_object_mut() {
        object.insert("freshness".to_string(), freshness);
    }
    inputs
}

fn annotate_comparison_validity(aggregate: &mut Value, inputs: &Value) {
    let freshness = inputs.get("freshness").unwrap_or(&Value::Null);
    let mixed_input_warning = freshness
        .get("mixed_input_warning")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let input_modified_span_label = freshness
        .get("modified_span_label")
        .and_then(Value::as_str)
        .unwrap_or("n/a");
    let input_modified_span_ms = freshness
        .get("modified_span_ms")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let excluded_provider_api_rows = aggregate
        .pointer("/diagnostics/total_skipped_infrastructure_failures")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let directional_until_fresh_paired_run = mixed_input_warning || excluded_provider_api_rows > 0;
    let mut caveats = Vec::<String>::new();
    if mixed_input_warning {
        caveats.push(format!(
            "selected input reports span {input_modified_span_label}"
        ));
    }
    if excluded_provider_api_rows > 0 {
        caveats.push(format!(
            "{excluded_provider_api_rows} provider/API failure row(s) were excluded"
        ));
    }
    let validity = json!({
        "mixed_input_warning": mixed_input_warning,
        "input_modified_span_ms": input_modified_span_ms,
        "input_modified_span_label": input_modified_span_label,
        "excluded_provider_api_rows": excluded_provider_api_rows,
        "directional_until_fresh_paired_run": directional_until_fresh_paired_run,
        "caveats": caveats,
    });
    if let Some(diagnostics) = aggregate
        .get_mut("diagnostics")
        .and_then(Value::as_object_mut)
    {
        diagnostics.insert("comparison_validity".to_string(), validity);
    }
}

pub(crate) fn comparison_directional_failure_message(aggregate: &Value) -> Option<String> {
    let validity = aggregate.pointer("/diagnostics/comparison_validity")?;
    let directional = validity
        .get("directional_until_fresh_paired_run")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !directional {
        return None;
    }
    let caveats = validity
        .get("caveats")
        .and_then(Value::as_array)
        .map(|caveats| {
            caveats
                .iter()
                .filter_map(Value::as_str)
                .filter(|caveat| !caveat.trim().is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let caveat_text = if caveats.is_empty() {
        "no specific caveats recorded".to_string()
    } else {
        caveats.join("; ")
    };
    Some(format!(
        "benchmark comparison is directional until a fresh paired run completes: {caveat_text}"
    ))
}

fn input_report_metadata_list(cwd: &Path, paths: &[PathBuf]) -> Vec<Value> {
    paths
        .iter()
        .map(|path| input_report_metadata(cwd, path))
        .collect()
}

fn input_report_metadata(cwd: &Path, path: &Path) -> Value {
    let resolved = resolve_input_path(cwd, path);
    let mut metadata = json!({
        "path": path.display().to_string(),
        "resolved_path": resolved.display().to_string(),
    });
    let Some(object) = metadata.as_object_mut() else {
        return metadata;
    };

    match std::fs::metadata(&resolved) {
        Ok(file_metadata) => {
            object.insert("bytes".to_string(), json!(file_metadata.len()));
            object.insert(
                "modified_unix_ms".to_string(),
                json!(
                    file_metadata
                        .modified()
                        .ok()
                        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                        .map(|duration| duration.as_millis())
                ),
            );
        }
        Err(error) => {
            object.insert("error".to_string(), json!(format!("metadata: {error}")));
            return metadata;
        }
    }

    let contents = match std::fs::read_to_string(&resolved) {
        Ok(contents) => contents,
        Err(error) => {
            object.insert("error".to_string(), json!(format!("read: {error}")));
            return metadata;
        }
    };
    let value = match serde_json::from_str::<Value>(&contents) {
        Ok(value) => value,
        Err(error) => {
            object.insert("error".to_string(), json!(format!("parse: {error}")));
            return metadata;
        }
    };

    object.insert(
        "suite".to_string(),
        json!(value.get("suite").and_then(Value::as_str).unwrap_or("")),
    );
    object.insert(
        "runner".to_string(),
        json!(value.get("runner").and_then(Value::as_str).unwrap_or("")),
    );
    object.insert(
        "generated_at_unix_ms".to_string(),
        value
            .get("generated_at_unix_ms")
            .cloned()
            .unwrap_or(Value::Null),
    );
    object.insert("row_count".to_string(), json!(input_row_count(&value)));
    object.insert(
        "scenarios".to_string(),
        json!(input_report_scenarios(&value)),
    );
    object.insert(
        "missing_scenarios".to_string(),
        value
            .get("missing_scenarios")
            .cloned()
            .unwrap_or_else(|| json!([])),
    );
    metadata
}

fn input_row_count(value: &Value) -> usize {
    value
        .get("rows")
        .and_then(Value::as_array)
        .map(Vec::len)
        .or_else(|| value.get("traces").and_then(Value::as_array).map(Vec::len))
        .unwrap_or(0)
}

fn input_report_scenarios(value: &Value) -> Vec<String> {
    let mut scenarios = value
        .get("rows")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row.get("scenario").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .or_else(|| {
            value.get("traces").and_then(Value::as_array).map(|traces| {
                traces
                    .iter()
                    .filter_map(|trace| trace.get("scenario").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default();
    scenarios.sort();
    scenarios.dedup();
    scenarios
}

fn input_freshness_summary(inputs: &Value) -> Value {
    let values = input_report_modified_values(inputs);
    if values.is_empty() {
        return json!({
            "input_count": 0,
            "modified_span_ms": 0u64,
            "modified_span_label": "n/a",
            "mixed_input_warning": false,
        });
    }
    let oldest = *values.iter().min().unwrap_or(&0);
    let latest = *values.iter().max().unwrap_or(&0);
    let span = latest.saturating_sub(oldest);
    json!({
        "input_count": values.len(),
        "oldest_modified_unix_ms": oldest,
        "latest_modified_unix_ms": latest,
        "modified_span_ms": span,
        "modified_span_label": format_duration_ms(span),
        "mixed_input_warning": span > 60 * 60 * 1000,
    })
}

fn input_report_modified_values(inputs: &Value) -> Vec<u64> {
    [
        "/harness_reports",
        "/codex_cli_reports",
        "/opencode_reports",
    ]
    .into_iter()
    .filter_map(|pointer| inputs.pointer(pointer).and_then(Value::as_array))
    .flat_map(|reports| {
        reports
            .iter()
            .filter_map(|report| report.get("modified_unix_ms").and_then(Value::as_u64))
    })
    .collect()
}

fn collect_benchmark_rows_from_manifest_paths(
    cwd: &Path,
    suite: ProfileBenchmarkSuiteKind,
    paths: &[PathBuf],
) -> Result<Vec<BenchmarkRunRow>> {
    let mut rows = Vec::new();
    for path in paths {
        let manifest = read_benchmark_run_manifest(cwd, path)?;
        if manifest.suite != suite.name() {
            anyhow::bail!(
                "benchmark manifest {} belongs to suite '{}', expected '{}'",
                path.display(),
                manifest.suite,
                suite.name()
            );
        }
        rows.extend(collect_benchmark_rows_from_manifest(
            &BenchmarkReportOptions {
                cwd: cwd.to_path_buf(),
                suite,
                limit: 0,
                all_runs: true,
                run_manifests: Vec::new(),
                output_dir: PathBuf::new(),
            },
            &manifest,
        )?);
    }
    rows.sort_by(|left, right| {
        scenario_order(suite, &left.scenario)
            .cmp(&scenario_order(suite, &right.scenario))
            .then_with(|| left.reasoning_effort.cmp(&right.reasoning_effort))
            .then_with(|| left.run_id.cmp(&right.run_id))
    });
    Ok(rows)
}

fn collect_benchmark_rows_from_harness_input_paths(
    cwd: &Path,
    suite: ProfileBenchmarkSuiteKind,
    paths: &[PathBuf],
) -> Result<Vec<BenchmarkRunRow>> {
    let mut rows = Vec::new();
    for path in paths {
        let resolved = resolve_input_path(cwd, path);
        let contents = std::fs::read_to_string(&resolved)
            .map_err(|error| anyhow::anyhow!("failed to read {}: {error}", resolved.display()))?;
        let value = serde_json::from_str::<Value>(&contents)
            .map_err(|error| anyhow::anyhow!("failed to parse {}: {error}", resolved.display()))?;
        if value.get("rows").is_some() {
            rows.extend(read_benchmark_rows_from_report_value(
                &resolved, suite, value,
            )?);
            continue;
        }
        let manifest = serde_json::from_value::<BenchmarkRunManifest>(value).map_err(|error| {
            anyhow::anyhow!(
                "failed to parse {} as benchmark manifest: {error}",
                resolved.display()
            )
        })?;
        if manifest.suite != suite.name() {
            anyhow::bail!(
                "benchmark manifest {} belongs to suite '{}', expected '{}'",
                resolved.display(),
                manifest.suite,
                suite.name()
            );
        }
        rows.extend(collect_benchmark_rows_from_manifest(
            &BenchmarkReportOptions {
                cwd: cwd.to_path_buf(),
                suite,
                limit: 0,
                all_runs: true,
                run_manifests: Vec::new(),
                output_dir: PathBuf::new(),
            },
            &manifest,
        )?);
    }
    sort_benchmark_rows(suite, &mut rows);
    Ok(rows)
}

fn read_benchmark_rows_from_report_value(
    path: &Path,
    suite: ProfileBenchmarkSuiteKind,
    value: Value,
) -> Result<Vec<BenchmarkRunRow>> {
    let report_suite = value.get("suite").and_then(Value::as_str).unwrap_or("");
    if report_suite != suite.name() {
        anyhow::bail!(
            "benchmark report {} belongs to suite '{}', expected '{}'",
            path.display(),
            report_suite,
            suite.name()
        );
    }
    let rows_value = value
        .get("rows")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("benchmark report {} is missing rows", path.display()))?;
    let rows = serde_json::from_value::<Vec<BenchmarkRunRow>>(rows_value).map_err(|error| {
        anyhow::anyhow!(
            "failed to parse benchmark rows from {}: {error}",
            path.display()
        )
    })?;
    for row in &rows {
        if row.suite != suite.name() {
            anyhow::bail!(
                "benchmark report {} contains row for suite '{}', expected '{}'",
                path.display(),
                row.suite,
                suite.name()
            );
        }
    }
    Ok(rows)
}

fn sort_benchmark_rows(suite: ProfileBenchmarkSuiteKind, rows: &mut [BenchmarkRunRow]) {
    rows.sort_by(|left, right| {
        scenario_order(suite, &left.scenario)
            .cmp(&scenario_order(suite, &right.scenario))
            .then_with(|| left.reasoning_effort.cmp(&right.reasoning_effort))
            .then_with(|| left.run_id.cmp(&right.run_id))
    });
}

pub(crate) fn write_benchmark_run_manifest(
    cwd: &Path,
    suite: &str,
    scenarios: &[ProfileScenarioKind],
    traces: &[BenchmarkRunManifestTrace],
) -> Result<PathBuf> {
    let output_dir = cwd.join(".spark-profile").join("benchmarks");
    std::fs::create_dir_all(&output_dir).map_err(|error| {
        anyhow::anyhow!(
            "failed to create benchmark manifest directory {}: {error}",
            output_dir.display()
        )
    })?;

    let expected_scenarios = scenarios
        .iter()
        .map(|scenario| scenario.name().to_string())
        .collect::<Vec<_>>();
    let seen = traces
        .iter()
        .map(|trace| trace.scenario.as_str())
        .collect::<BTreeSet<_>>();
    let missing_scenarios = expected_scenarios
        .iter()
        .filter(|scenario| !seen.contains(scenario.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let stamp = unix_millis();
    let manifest = BenchmarkRunManifest {
        suite: suite.to_string(),
        generated_at_unix_ms: stamp,
        expected_scenarios,
        traces: traces.to_vec(),
        missing_scenarios,
    };
    let path = output_dir.join(format!("{suite}-run-{stamp}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(&manifest)?)
        .map_err(|error| anyhow::anyhow!("failed to write {}: {error}", path.display()))?;
    Ok(path)
}

fn collect_benchmark_rows(options: &BenchmarkReportOptions) -> Result<Vec<BenchmarkRunRow>> {
    if !options.all_runs
        && let Some(manifest) =
            latest_benchmark_run_manifest(&options.output_dir, options.suite.name())?
    {
        return collect_benchmark_rows_from_manifest(options, &manifest);
    }

    let mut rows = Vec::new();
    let mut seen_scenarios = BTreeSet::new();
    for run in commands::list_trace_dirs(&commands::trace_runs_root(&options.cwd), options.limit)? {
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

fn collect_benchmark_rows_from_manifest(
    options: &BenchmarkReportOptions,
    manifest: &BenchmarkRunManifest,
) -> Result<Vec<BenchmarkRunRow>> {
    let mut rows = Vec::new();
    for trace in &manifest.traces {
        let run = resolve_manifest_trace_dir(&options.cwd, &trace.trace_dir);
        if !run.exists() {
            anyhow::bail!(
                "benchmark manifest for suite '{}' points to missing trace {}",
                manifest.suite,
                run.display()
            );
        }
        let summary = profiler::analyze_trace(&run)?;
        if benchmark_suite(&summary) != Some(options.suite.name()) {
            anyhow::bail!(
                "benchmark manifest trace {} belongs to suite {:?}, expected '{}'",
                run.display(),
                benchmark_suite(&summary),
                options.suite.name()
            );
        }
        rows.push(row_from_summary(&options.cwd, &run, &summary));
    }
    rows.sort_by(|left, right| {
        scenario_order(options.suite, &left.scenario)
            .cmp(&scenario_order(options.suite, &right.scenario))
            .then_with(|| left.run_id.cmp(&right.run_id))
    });
    Ok(rows)
}

fn latest_benchmark_run_manifest(
    output_dir: &Path,
    suite: &str,
) -> Result<Option<BenchmarkRunManifest>> {
    let prefix = format!("{suite}-run-");
    let mut candidates = Vec::new();
    if !output_dir.exists() {
        return Ok(None);
    }
    for entry in std::fs::read_dir(output_dir)
        .map_err(|error| anyhow::anyhow!("failed to read {}: {error}", output_dir.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(&prefix) || !name.ends_with(".json") {
            continue;
        }
        candidates.push(entry.path());
    }
    candidates.sort();
    let Some(path) = candidates.pop() else {
        return Ok(None);
    };
    let manifest = read_benchmark_run_manifest(Path::new("."), &path)?;
    if manifest.suite == suite {
        Ok(Some(manifest))
    } else {
        Ok(None)
    }
}

fn read_benchmark_run_manifest(cwd: &Path, path: &Path) -> Result<BenchmarkRunManifest> {
    let path = resolve_input_path(cwd, path);
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| anyhow::anyhow!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str::<BenchmarkRunManifest>(&contents)
        .map_err(|error| anyhow::anyhow!("failed to parse {}: {error}", path.display()))
}

fn resolve_input_path(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn resolve_manifest_trace_dir(cwd: &Path, trace_dir: &str) -> PathBuf {
    let path = PathBuf::from(trace_dir);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn row_from_summary(cwd: &Path, run: &Path, summary: &Value) -> BenchmarkRunRow {
    let trace_dir = commands::display_trace_dir(cwd, run).display().to_string();
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
    let validation = validation::read_scenario_validation(run);
    let validation_present = validation.is_some();
    let validation_exit_code = validation.as_ref().and_then(|result| result.exit_code);
    let validation_score = validation
        .as_ref()
        .and_then(|result| result.granular_score());
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
    let unexpected_repeated_tool_calls = unexpected_repeated_tool_calls(summary);
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
        reasoning_effort: reasoning_effort(summary).unwrap_or("unknown").to_string(),
        completion_score: 0.0,
        quality_score: 0.0,
        process_score: 0.0,
        efficiency_index: None,
        benchmark_index: None,
        score: 0.0,
        task_quality_score: 0.0,
        efficiency_score: 0.0,
        harness_pressure_score: 0.0,
        success: trace_success && validation_success,
        validation_present,
        validation_exit_code,
        validation_score,
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
        repeated_tool_calls: unexpected_repeated_tool_calls,
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
    row.completion_score = completion_score(&row);
    row.quality_score = quality_score_with_validation(&row, validation_score);
    row.process_score = process_score(&row);
    row.task_quality_score = row.quality_score;
    row.efficiency_score = efficiency_score(row.total_duration_ms as u128, row.source_bytes);
    row.harness_pressure_score = row.process_score;
    row.score = benchmark_score(&row);
    row
}

fn aggregate_rows(suite: &str, rows: &[BenchmarkRunRow]) -> Value {
    let average_score = rows.iter().map(|row| row.score).sum::<f64>() / rows.len() as f64;
    let average_completion =
        rows.iter().map(|row| row.completion_score).sum::<f64>() / rows.len() as f64;
    let average_quality = rows.iter().map(|row| row.quality_score).sum::<f64>() / rows.len() as f64;
    let average_process = rows.iter().map(|row| row.process_score).sum::<f64>() / rows.len() as f64;
    let average_task_quality =
        rows.iter().map(|row| row.task_quality_score).sum::<f64>() / rows.len() as f64;
    let average_efficiency =
        rows.iter().map(|row| row.efficiency_score).sum::<f64>() / rows.len() as f64;
    let average_harness_pressure = rows
        .iter()
        .map(|row| row.harness_pressure_score)
        .sum::<f64>()
        / rows.len() as f64;
    let diagnostic_counts = rows
        .iter()
        .flat_map(|row| row.diagnostics.split(';').filter(|item| !item.is_empty()))
        .fold(BTreeMap::<String, u64>::new(), |mut counts, item| {
            *counts.entry(item.to_string()).or_default() += 1;
            counts
        });
    let request_failure_scenarios = rows
        .iter()
        .filter(|row| failure_points_contain(&row.diagnostics, "request_failure"))
        .fold(BTreeMap::<String, usize>::new(), |mut counts, row| {
            *counts.entry(row.scenario.clone()).or_default() += 1;
            counts
        });
    let mut diagnostics = json!(diagnostic_counts);
    if !request_failure_scenarios.is_empty()
        && let Some(object) = diagnostics.as_object_mut()
    {
        object.insert(
            "request_failure_scenarios".to_string(),
            json!(request_failure_scenarios),
        );
    }

    json!({
        "suite": suite,
        "runs": rows.len(),
        "average_score": round1(average_score),
        "average_completion_score": round1(average_completion),
        "average_quality_score": round1(average_quality),
        "average_process_score": round1(average_process),
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
    if diagnostic_kinds(summary)
        .iter()
        .any(|kind| kind == "request_failure")
    {
        points.push("request_failure".to_string());
    }
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
    if unexpected_repeated_tool_calls(summary) > 0 {
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

fn rows_to_csv(rows: &[BenchmarkRunRow]) -> String {
    let mut csv = String::from(
        "run_id,trace_dir,suite,scenario,model,reasoning_effort,score,task_quality_score,efficiency_score,harness_pressure_score,success,validation_present,validation_exit_code,validation_timed_out,browser_validation_present,browser_validation_exit_code,browser_validation_timed_out,browser_screenshot,requests,tool_calls,max_approx_input_tokens,max_context_window_pct,max_request_duration_ms,total_duration_ms,source_files,source_bytes,compactions,tool_failures,recovered_tool_failures,unrecovered_tool_failures,truncated_tool_results,repeated_tool_calls,tool_only_turns,max_tool_only_streak,expected_tool_groups,satisfied_tool_groups,expected_tool_calls,satisfied_tool_calls,extra_calls_after_satisfied,extra_turns_after_satisfied,context_growth_after_satisfied_chars,diagnostics,failure_points,completion_score,quality_score,process_score,efficiency_index,benchmark_index,validation_score\n",
    );
    for row in rows {
        let values = [
            row.run_id.clone(),
            row.trace_dir.clone(),
            row.suite.clone(),
            row.scenario.clone(),
            row.model.clone(),
            row.reasoning_effort.clone(),
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
            row.completion_score.to_string(),
            row.quality_score.to_string(),
            row.process_score.to_string(),
            row.efficiency_index
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.benchmark_index
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.validation_score
                .map(|value| value.to_string())
                .unwrap_or_default(),
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
    let avg_completion = aggregate
        .get("average_completion_score")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let avg_quality = aggregate
        .get("average_quality_score")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let avg_pressure = aggregate
        .get("average_process_score")
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
    let request_failure_note = request_failure_scenario_note(aggregate);
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
.note {{ background: #fff7ed; border: 1px solid #fed7aa; border-radius: 8px; padding: 12px 14px; color: #7c2d12; }}
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
<p>Generated from saved benchmark traces. This report shows bounded component scores only; Benchmark Index is available in Codex-baselined comparison reports.</p>
{request_failure_note}
<section class="kpis">
<div class="kpi"><strong>{avg_completion:.1}</strong><span>Completion</span></div>
<div class="kpi"><strong>{avg_quality:.1}</strong><span>Quality</span></div>
<div class="kpi"><strong>{avg_efficiency:.1}</strong><span>Efficiency</span></div>
<div class="kpi"><strong>{avg_pressure:.1}</strong><span>Process</span></div>
</section>
"#,
        suite = html_escape(suite),
        request_failure_note = request_failure_note,
        avg_completion = avg_completion,
        avg_quality = avg_quality,
        avg_efficiency = avg_efficiency,
        avg_pressure = avg_pressure
    );
    html.push_str("<h2>Completion by Scenario</h2><div class=\"panel\">");
    html.push_str(&score_svg(rows));
    html.push_str("</div><h2>Failure Pressure by Scenario</h2><div class=\"panel\">");
    html.push_str(&pressure_svg(rows));
    html.push_str("</div>");
    let _ = write!(
        html,
        "<p class=\"muted\">Total truncations: {truncations}. Benchmark Index is omitted here because no Codex baseline is present. Max tool-only streak: {max_streak}.</p>",
        max_streak = max_streak
    );
    html.push_str("<h2>Run Details</h2><div class=\"panel\"><table><thead><tr><th>Scenario</th><th>Completion</th><th>Quality</th><th>Efficiency</th><th>Process</th><th>Legacy score</th><th>Validation</th><th>Browser</th><th>Duration</th><th>Source</th><th>Requests</th><th>Tools</th><th>Max context</th><th>Extra calls</th><th>Tool-only</th><th>Diagnostics</th><th>Failure points</th></tr></thead><tbody>");
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
            "<tr><td>{}</td><td class=\"num\">{:.1}</td><td class=\"num\">{:.1}</td><td class=\"num\">{:.1}</td><td class=\"num\">{:.1}</td><td class=\"num\">{:.1}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{} / {}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{:.1}%</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td><td>{}</td></tr>",
            html_escape(&row.scenario),
            row.completion_score,
            row.quality_score,
            row.efficiency_score,
            row.process_score,
            row.score,
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

fn request_failure_scenario_note(aggregate: &Value) -> String {
    let Some(scenarios) = aggregate
        .pointer("/diagnostics/request_failure_scenarios")
        .and_then(Value::as_object)
    else {
        return String::new();
    };
    if scenarios.is_empty() {
        return String::new();
    }
    let mut parts = scenarios
        .iter()
        .filter_map(|(scenario, count)| {
            let count = count.as_u64()?;
            (count > 0).then(|| {
                if count == 1 {
                    html_escape(scenario)
                } else {
                    format!("{} x{}", html_escape(scenario), count)
                }
            })
        })
        .collect::<Vec<_>>();
    parts.sort();
    if parts.is_empty() {
        return String::new();
    }
    format!(
        "<p class=\"note\">Request failure scenarios: {}. Provider/API failures are tracked separately from task performance; rerun these scenarios after the provider recovers.</p>",
        parts.join(", ")
    )
}

fn score_svg(rows: &[BenchmarkRunRow]) -> String {
    let row_height = 34usize;
    let label_width = 230usize;
    let chart_width = 760usize;
    let height = 32 + rows.len() * row_height;
    let mut svg = format!(
        r#"<svg viewBox="0 0 {} {}" role="img" aria-label="Completion by scenario">"#,
        label_width + chart_width + 80,
        height
    );
    for (index, row) in rows.iter().enumerate() {
        let y = 28 + index * row_height;
        let width = (row.completion_score / 100.0 * chart_width as f64).round() as usize;
        let color = if row.completion_score >= 80.0 {
            "#2f7da1"
        } else if row.completion_score >= 65.0 {
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
            row.completion_score
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

fn read_llm_judge_report(path: &Path) -> Result<BenchmarkJudgeReport> {
    serde_json::from_str(&std::fs::read_to_string(path).map_err(|error| {
        anyhow::anyhow!(
            "failed to read LLM judge report {}: {error}",
            path.display()
        )
    })?)
    .map_err(|error| {
        anyhow::anyhow!(
            "failed to parse LLM judge report JSON {}: {error}",
            path.display()
        )
    })
}

fn apply_llm_judge_scores(rows: &mut [ComparisonRow], report: &BenchmarkJudgeReport) {
    let mut scores = BTreeMap::<(String, String), _>::new();
    for scenario in &report.rows {
        for score in &scenario.scores {
            scores.insert((scenario.scenario.clone(), score.runner.clone()), score);
        }
    }
    for row in rows {
        let Some(score) = scores.get(&(row.scenario.clone(), row.runner.clone())) else {
            continue;
        };
        row.llm_solution_score = Some(round1(score.solution_score));
        row.llm_process_score = Some(round1(score.process_score));
        row.llm_confidence = Some(round1(score.confidence));
        row.llm_notes = score.notes.clone();
        row.quality_score = round1(score.solution_score.clamp(0.0, 100.0));
        row.process_score = round1(score.process_score.clamp(0.0, 100.0));
        row.task_quality_score = row.quality_score;
        row.harness_pressure_score = row.process_score;
        row.score = comparison_score_from_components(row);
    }
}

fn comparison_row_from_harness(row: &BenchmarkRunRow) -> ComparisonRow {
    ComparisonRow {
        runner: "spark-harness".to_string(),
        suite: row.suite.clone(),
        scenario: row.scenario.clone(),
        attempts: 1,
        successful_attempts: usize::from(row.success),
        model: row.model.clone(),
        reasoning_effort: row.reasoning_effort.clone(),
        command_path: String::new(),
        command_version: String::new(),
        completion_score: row.completion_score,
        quality_score: row.quality_score,
        process_score: row.process_score,
        llm_solution_score: None,
        llm_process_score: None,
        llm_confidence: None,
        llm_notes: String::new(),
        efficiency_index: None,
        benchmark_index: None,
        score: row.score,
        task_quality_score: row.task_quality_score,
        efficiency_score: row.efficiency_score,
        harness_pressure_score: row.harness_pressure_score,
        success: row.success,
        validation_exit_code: row.validation_exit_code,
        validation_timed_out: row.validation_timed_out,
        duration_ms: row.total_duration_ms as u128,
        tool_or_item_calls: row.tool_calls,
        input_tokens: row.max_approx_input_tokens,
        output_tokens: 0,
        source_files: row.source_files,
        source_bytes: row.source_bytes,
        failure_points: row.failure_points.clone(),
        source: row.trace_dir.clone(),
    }
}

fn comparison_row_from_external_agent(row: &CodexCliBenchmarkRow) -> ComparisonRow {
    let completion_score = codex_completion_score(row);
    let quality_score = codex_quality_score_with_validation(row, completion_score);
    let process_score = codex_process_score(row);
    let efficiency_score = codex_efficiency_score(row);
    ComparisonRow {
        runner: row.runner.clone(),
        suite: row.suite.clone(),
        scenario: row.scenario.clone(),
        attempts: 1,
        successful_attempts: usize::from(row.success),
        model: row.model.clone(),
        reasoning_effort: row.reasoning_effort.clone(),
        command_path: row.command_path.clone(),
        command_version: row.command_version.clone(),
        completion_score,
        quality_score,
        process_score,
        llm_solution_score: None,
        llm_process_score: None,
        llm_confidence: None,
        llm_notes: String::new(),
        efficiency_index: None,
        benchmark_index: None,
        score: codex_score_from_components(
            row,
            completion_score,
            quality_score,
            process_score,
            efficiency_score,
        ),
        task_quality_score: quality_score,
        efficiency_score,
        harness_pressure_score: process_score,
        success: row.success,
        validation_exit_code: row.validation_exit_code,
        validation_timed_out: row.validation_timed_out,
        duration_ms: row.duration_ms,
        tool_or_item_calls: row.completed_items,
        input_tokens: row.input_tokens,
        output_tokens: row.output_tokens,
        source_files: row.source_files,
        source_bytes: row.source_bytes,
        failure_points: row.failure_points.clone(),
        source: row.run_dir.clone(),
    }
}

fn average_comparison_attempts(rows: Vec<ComparisonRow>) -> Vec<ComparisonRow> {
    let mut groups = BTreeMap::<(String, String), Vec<ComparisonRow>>::new();
    for row in rows {
        groups
            .entry((row.runner.clone(), row.scenario.clone()))
            .or_default()
            .push(row);
    }

    groups.into_values().map(average_comparison_group).collect()
}

fn average_comparison_group(rows: Vec<ComparisonRow>) -> ComparisonRow {
    let attempts = rows.iter().map(|row| row.attempts).sum::<usize>();
    let successful_attempts = rows
        .iter()
        .map(|row| row.successful_attempts)
        .sum::<usize>();
    let mut row = rows
        .first()
        .expect("comparison group should contain at least one row")
        .clone();
    if rows.len() == 1 && row.attempts == 1 {
        return row;
    }

    row.attempts = attempts;
    row.successful_attempts = successful_attempts;
    row.model = join_unique(rows.iter().map(|row| row.model.as_str()));
    row.reasoning_effort = join_unique(rows.iter().map(|row| row.reasoning_effort.as_str()));
    row.command_path = join_unique(rows.iter().map(|row| row.command_path.as_str()));
    row.command_version = join_unique(rows.iter().map(|row| row.command_version.as_str()));
    row.completion_score = average_f64(rows.iter().map(|row| row.completion_score));
    row.quality_score = average_f64(rows.iter().map(|row| row.quality_score));
    row.process_score = average_f64(rows.iter().map(|row| row.process_score));
    row.llm_solution_score =
        average_option_f64(rows.iter().filter_map(|row| row.llm_solution_score));
    row.llm_process_score = average_option_f64(rows.iter().filter_map(|row| row.llm_process_score));
    row.llm_confidence = average_option_f64(rows.iter().filter_map(|row| row.llm_confidence));
    row.llm_notes = join_unique(
        rows.iter()
            .map(|row| row.llm_notes.as_str())
            .filter(|note| !note.is_empty()),
    );
    row.efficiency_index = None;
    row.benchmark_index = None;
    row.score = average_f64(rows.iter().map(|row| row.score));
    row.task_quality_score = average_f64(rows.iter().map(|row| row.task_quality_score));
    row.efficiency_score = average_f64(rows.iter().map(|row| row.efficiency_score));
    row.harness_pressure_score = average_f64(rows.iter().map(|row| row.harness_pressure_score));
    row.success = successful_attempts == attempts;
    row.validation_exit_code = combined_exit_code(rows.iter().map(|row| row.validation_exit_code));
    row.validation_timed_out = rows.iter().any(|row| row.validation_timed_out);
    row.duration_ms = average_u128(rows.iter().map(|row| row.duration_ms));
    row.tool_or_item_calls = average_u64(rows.iter().map(|row| row.tool_or_item_calls));
    row.input_tokens = average_u64(rows.iter().map(|row| row.input_tokens));
    row.output_tokens = average_u64(rows.iter().map(|row| row.output_tokens));
    row.source_files = average_u64(rows.iter().map(|row| row.source_files));
    row.source_bytes = average_u64(rows.iter().map(|row| row.source_bytes));
    row.failure_points = joined_failure_points(&rows, successful_attempts, attempts);
    row.source = format!(
        "averaged {} attempts: {}",
        attempts,
        join_unique(rows.iter().map(|row| row.source.as_str()))
    );
    row
}

fn average_f64(values: impl Iterator<Item = f64>) -> f64 {
    round1(values.pipe_average().unwrap_or(0.0))
}

fn average_option_f64(values: impl Iterator<Item = f64>) -> Option<f64> {
    values.pipe_average().map(round1)
}

fn average_u64(values: impl Iterator<Item = u64>) -> u64 {
    let mut count = 0u128;
    let mut total = 0u128;
    for value in values {
        count += 1;
        total += value as u128;
    }
    if count == 0 {
        0
    } else {
        ((total as f64 / count as f64).round()) as u64
    }
}

fn average_u128(values: impl Iterator<Item = u128>) -> u128 {
    let mut count = 0u128;
    let mut total = 0u128;
    for value in values {
        count += 1;
        total += value;
    }
    if count == 0 {
        0
    } else {
        ((total as f64 / count as f64).round()) as u128
    }
}

fn combined_exit_code(values: impl Iterator<Item = Option<i32>>) -> Option<i32> {
    let mut saw_zero = false;
    for value in values {
        match value {
            Some(0) => saw_zero = true,
            Some(code) => return Some(code),
            None => {}
        }
    }
    saw_zero.then_some(0)
}

fn join_unique<'a>(values: impl Iterator<Item = &'a str>) -> String {
    values
        .flat_map(|value| value.split(';'))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(";")
}

fn joined_failure_points(
    rows: &[ComparisonRow],
    successful_attempts: usize,
    attempts: usize,
) -> String {
    let mut points = rows
        .iter()
        .flat_map(|row| row.failure_points.split(';'))
        .map(str::trim)
        .filter(|point| !point.is_empty())
        .collect::<BTreeSet<_>>();
    if successful_attempts < attempts {
        points.insert("partial_or_failed_attempts");
    }
    points.into_iter().collect::<Vec<_>>().join(";")
}

#[derive(Debug, Clone, Copy)]
struct EfficiencyComponents {
    duration_ms: f64,
    input_tokens: f64,
    tool_or_item_calls: f64,
}

#[derive(Debug, Clone, Copy)]
struct ScenarioBaseline {
    efficiency: EfficiencyComponents,
}

fn label_rows_by_model_and_reasoning(
    rows: &mut [ComparisonRow],
    group_by_model: bool,
    group_by_reasoning: bool,
) {
    for row in rows {
        let mut parts = Vec::new();
        if group_by_model {
            let model = row.model.trim();
            if !model.is_empty() && model != "unknown" {
                parts.push(runner_variant_slug(model));
            }
        }
        if group_by_reasoning {
            let reasoning = row.reasoning_effort.trim();
            if !reasoning.is_empty() && reasoning != "unknown" && reasoning != "None" {
                parts.push(runner_variant_slug(reasoning));
            }
        }
        if parts.is_empty() {
            continue;
        }
        row.runner = format!("{}/{}", row.runner, parts.join("+"));
    }
}

fn runner_variant_slug(value: &str) -> String {
    value
        .trim()
        .replace("openai/", "")
        .replace(['/', '\\', ' '], "-")
}

fn comparison_baseline_runner(rows: &[ComparisonRow], preferred_runner: &str) -> String {
    if rows.iter().any(|row| row.runner == preferred_runner) {
        return preferred_runner.to_string();
    }
    let prefix = format!("{preferred_runner}/");
    let candidates = rows
        .iter()
        .map(|row| row.runner.as_str())
        .filter(|runner| runner.starts_with(&prefix))
        .collect::<Vec<_>>();
    candidates
        .iter()
        .copied()
        .find(|runner| runner.contains("gpt-5.5"))
        .or_else(|| candidates.iter().copied().min())
        .unwrap_or(preferred_runner)
        .to_string()
}

fn apply_comparison_indices(rows: &mut [ComparisonRow], baseline_runner: &str) {
    let matched_scenarios = scenarios_with_both_runners(rows);
    let baselines = scenario_baselines(rows, baseline_runner, &matched_scenarios);
    let raw = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let baseline = baselines.get(&row.scenario)?;
            let raw_efficiency = raw_efficiency_index(row, baseline);
            let raw_benchmark = raw_benchmark_index(row, raw_efficiency);
            Some((index, raw_efficiency, raw_benchmark))
        })
        .collect::<Vec<_>>();
    let baseline_efficiency_average = raw
        .iter()
        .filter(|(index, _, _)| rows[*index].runner == baseline_runner)
        .map(|(_, efficiency, _)| *efficiency)
        .pipe_average()
        .unwrap_or(100.0)
        .max(1.0);
    let baseline_benchmark_average = raw
        .iter()
        .filter(|(index, _, _)| rows[*index].runner == baseline_runner)
        .map(|(_, _, benchmark)| *benchmark)
        .pipe_average()
        .unwrap_or(100.0)
        .max(1.0);

    for (index, raw_efficiency, raw_benchmark) in raw {
        let row = &mut rows[index];
        row.efficiency_index = Some(round1(raw_efficiency / baseline_efficiency_average * 100.0));
        let mut benchmark_index = raw_benchmark / baseline_benchmark_average * 100.0;
        if !row.success || row.completion_score < 100.0 {
            benchmark_index = benchmark_index.min(row.completion_score.min(60.0));
        }
        row.benchmark_index = Some(round1(benchmark_index.max(0.0)));
    }
}

fn scenario_baselines(
    rows: &[ComparisonRow],
    baseline_runner: &str,
    matched_scenarios: &BTreeSet<String>,
) -> BTreeMap<String, ScenarioBaseline> {
    rows.iter()
        .filter(|row| row.runner == baseline_runner && matched_scenarios.contains(&row.scenario))
        .map(|row| {
            (
                row.scenario.clone(),
                ScenarioBaseline {
                    efficiency: efficiency_components(row),
                },
            )
        })
        .collect()
}

fn raw_benchmark_index(row: &ComparisonRow, raw_efficiency: f64) -> f64 {
    let resource_multiplier = raw_efficiency / 100.0;
    (quality_gate(row) * resource_multiplier).max(0.0)
}

fn raw_efficiency_index(row: &ComparisonRow, baseline: &ScenarioBaseline) -> f64 {
    let row_efficiency = efficiency_components(row);
    resource_efficiency_multiplier(baseline.efficiency, row_efficiency) * 100.0
}

fn resource_efficiency_multiplier(
    baseline: EfficiencyComponents,
    value: EfficiencyComponents,
) -> f64 {
    const DURATION_WEIGHT: f64 = 0.7;
    const INPUT_TOKEN_WEIGHT: f64 = 0.2;
    const TOOL_CALL_WEIGHT: f64 = 0.1;
    const OUTLIER_DAMPING: f64 = 0.5;

    let resource_ratio = positive_ratio(baseline.duration_ms, value.duration_ms)
        .powf(DURATION_WEIGHT)
        * positive_ratio(baseline.input_tokens, value.input_tokens).powf(INPUT_TOKEN_WEIGHT)
        * positive_ratio(baseline.tool_or_item_calls, value.tool_or_item_calls)
            .powf(TOOL_CALL_WEIGHT);
    resource_ratio.powf(OUTLIER_DAMPING)
}

fn efficiency_components(row: &ComparisonRow) -> EfficiencyComponents {
    EfficiencyComponents {
        duration_ms: row.duration_ms.max(1) as f64,
        input_tokens: row.input_tokens.max(1) as f64,
        tool_or_item_calls: row.tool_or_item_calls.max(1) as f64,
    }
}

fn positive_ratio(baseline: f64, value: f64) -> f64 {
    baseline.max(1.0) / value.max(1.0)
}

trait AverageIterator: Iterator<Item = f64> + Sized {
    fn pipe_average(self) -> Option<f64> {
        let mut count = 0usize;
        let mut total = 0.0;
        for value in self {
            count += 1;
            total += value;
        }
        (count > 0).then_some(total / count as f64)
    }
}

impl<T> AverageIterator for T where T: Iterator<Item = f64> {}

fn aggregate_comparison_with_diagnostics(
    suite: &str,
    rows: &[ComparisonRow],
    diagnostics: ComparisonDiagnostics,
) -> Value {
    let mut runner_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_completion_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_quality_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_process_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_efficiency_indexes = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_benchmark_indexes = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_task_quality_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_efficiency_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_pressure_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_duration_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_item_call_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_input_token_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_source_file_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut runner_source_byte_scores = BTreeMap::<String, Vec<f64>>::new();
    let mut rows_by_runner = BTreeMap::<String, Vec<&ComparisonRow>>::new();
    let matched_rows = matched_comparison_rows(rows);
    for row in rows {
        rows_by_runner
            .entry(row.runner.clone())
            .or_default()
            .push(row);
        runner_scores
            .entry(row.runner.clone())
            .or_default()
            .push(row.score);
        runner_completion_scores
            .entry(row.runner.clone())
            .or_default()
            .push(row.completion_score);
        runner_quality_scores
            .entry(row.runner.clone())
            .or_default()
            .push(row.quality_score);
        runner_process_scores
            .entry(row.runner.clone())
            .or_default()
            .push(row.process_score);
        if let Some(index) = row.efficiency_index {
            runner_efficiency_indexes
                .entry(row.runner.clone())
                .or_default()
                .push(index);
        }
        if let Some(index) = row.benchmark_index {
            runner_benchmark_indexes
                .entry(row.runner.clone())
                .or_default()
                .push(index);
        }
        runner_task_quality_scores
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
        runner_duration_scores
            .entry(row.runner.clone())
            .or_default()
            .push(row.duration_ms as f64);
        runner_item_call_scores
            .entry(row.runner.clone())
            .or_default()
            .push(row.tool_or_item_calls as f64);
        runner_input_token_scores
            .entry(row.runner.clone())
            .or_default()
            .push(row.input_tokens as f64);
        runner_source_file_scores
            .entry(row.runner.clone())
            .or_default()
            .push(row.source_files as f64);
        runner_source_byte_scores
            .entry(row.runner.clone())
            .or_default()
            .push(row.source_bytes as f64);
    }
    let runner_averages = runner_scores
        .into_iter()
        .map(|(runner, scores)| {
            let average = scores.iter().sum::<f64>() / scores.len() as f64;
            (runner, round1(average))
        })
        .collect::<BTreeMap<_, _>>();
    let runner_completion_averages = average_map(runner_completion_scores);
    let runner_quality_averages = average_map(runner_quality_scores);
    let runner_process_averages = average_map(runner_process_scores);
    let runner_efficiency_index_averages = average_map(runner_efficiency_indexes);
    let runner_benchmark_index_averages = average_map(runner_benchmark_indexes);
    let runner_task_quality_averages = average_map(runner_task_quality_scores);
    let runner_efficiency_averages = average_map(runner_efficiency_scores);
    let runner_harness_pressure_averages = average_map(runner_pressure_scores);
    let runner_duration_ms_averages = average_map(runner_duration_scores);
    let runner_tool_or_item_call_averages = average_map(runner_item_call_scores);
    let runner_input_token_averages = average_map(runner_input_token_scores);
    let runner_source_file_averages = average_map(runner_source_file_scores);
    let runner_source_byte_averages = average_map(runner_source_byte_scores);
    let matched_rows_by_runner = comparison_rows_by_runner(&matched_rows);
    let matched_runner_benchmark_index_averages =
        comparison_optional_average_map(&matched_rows_by_runner, |row| row.benchmark_index);
    let winner_pool = if matched_rows_by_runner.is_empty() {
        rows_by_runner.clone()
    } else {
        matched_rows_by_runner.clone()
    };
    let winner_entry = winner_pool
        .iter()
        .max_by(|left, right| compare_runner_rows(left.1, right.1));
    let baseline_runner = comparison_baseline_runner(rows, "codex-cli");
    let headline = winner_entry.map(|(winner_runner, winner_rows)| {
        let winner_benchmark_index =
            average_optional_comparison_field(winner_rows, |row| row.benchmark_index);
        let baseline_benchmark_index = matched_runner_benchmark_index_averages
            .get(&baseline_runner)
            .copied()
            .or_else(|| runner_benchmark_index_averages.get(&baseline_runner).copied());
        let benchmark_index_margin_vs_baseline = winner_benchmark_index
            .zip(baseline_benchmark_index)
            .map(|(winner, baseline)| round1(winner - baseline));
        json!({
            "winner": winner_runner,
            "baseline_runner": baseline_runner,
            "winner_benchmark_index": winner_benchmark_index,
            "baseline_benchmark_index": baseline_benchmark_index,
            "benchmark_index_margin_vs_baseline": benchmark_index_margin_vs_baseline,
            "winner_beats_baseline": benchmark_index_margin_vs_baseline.is_some_and(|margin| margin > 0.0),
        })
    });
    let winner = winner_entry.map(|(runner, rows)| {
            json!({
                "runner": runner,
                "average_benchmark_index": average_optional_comparison_field(rows, |row| row.benchmark_index),
                "average_efficiency_index": average_optional_comparison_field(rows, |row| row.efficiency_index),
                "average_completion_score": average_comparison_field(rows, |row| row.completion_score),
                "average_quality_score": average_comparison_field(rows, |row| row.quality_score),
                "average_process_score": average_comparison_field(rows, |row| row.process_score),
                "average_score": average_comparison_field(rows, |row| row.score),
                "average_task_quality_score": average_comparison_field(rows, |row| row.task_quality_score),
                "average_efficiency_score": average_comparison_field(rows, |row| row.efficiency_score),
                "average_harness_pressure_score": average_comparison_field(rows, |row| row.harness_pressure_score),
                "average_duration_ms": average_comparison_field(rows, |row| row.duration_ms as f64),
                "average_tool_or_item_calls": average_comparison_field(rows, |row| row.tool_or_item_calls as f64),
                "average_input_tokens": average_comparison_field(rows, |row| row.input_tokens as f64),
                "average_source_files": average_comparison_field(rows, |row| row.source_files as f64),
                "average_source_bytes": average_comparison_field(rows, |row| row.source_bytes as f64),
            })
        });
    let scenario_winners = comparison_scenario_winners(rows);
    let unmatched_scenarios = unmatched_comparison_scenarios(rows);
    let total_skipped_infrastructure_failures = diagnostics.skipped_spark_infrastructure_failures
        + diagnostics.skipped_codex_infrastructure_failures
        + diagnostics.skipped_opencode_infrastructure_failures;
    let comparison_diagnostics = json!({
        "skipped_infrastructure_failures": {
            "spark-harness": diagnostics.skipped_spark_infrastructure_failures,
            "codex-cli": diagnostics.skipped_codex_infrastructure_failures,
            "opencode": diagnostics.skipped_opencode_infrastructure_failures,
        },
        "skipped_infrastructure_scenarios": {
            "spark-harness": diagnostics.skipped_spark_infrastructure_scenarios,
            "codex-cli": diagnostics.skipped_codex_infrastructure_scenarios,
            "opencode": diagnostics.skipped_opencode_infrastructure_scenarios,
        },
        "skipped_infrastructure_retry_hints": {
            "spark-harness": diagnostics.skipped_spark_infrastructure_retry_hints,
            "codex-cli": diagnostics.skipped_codex_infrastructure_retry_hints,
            "opencode": diagnostics.skipped_opencode_infrastructure_retry_hints,
        },
        "total_skipped_infrastructure_failures": total_skipped_infrastructure_failures,
    });

    json!({
        "suite": suite,
        "rows": rows.len(),
        "matched_rows": matched_rows.len(),
        "runner_averages": runner_averages,
        "runner_completion_score_averages": runner_completion_averages,
        "runner_quality_score_averages": runner_quality_averages,
        "runner_process_score_averages": runner_process_averages,
        "runner_efficiency_index_averages": runner_efficiency_index_averages,
        "runner_benchmark_index_averages": runner_benchmark_index_averages,
        "runner_task_quality_averages": runner_task_quality_averages,
        "runner_efficiency_averages": runner_efficiency_averages,
        "runner_harness_pressure_averages": runner_harness_pressure_averages,
        "runner_duration_ms_averages": runner_duration_ms_averages,
        "runner_tool_or_item_call_averages": runner_tool_or_item_call_averages,
        "runner_input_token_averages": runner_input_token_averages,
        "runner_source_file_averages": runner_source_file_averages,
        "runner_source_byte_averages": runner_source_byte_averages,
        "matched_runner_averages": comparison_average_map(&matched_rows_by_runner, |row| row.score),
        "matched_runner_completion_score_averages": comparison_average_map(&matched_rows_by_runner, |row| row.completion_score),
        "matched_runner_quality_score_averages": comparison_average_map(&matched_rows_by_runner, |row| row.quality_score),
        "matched_runner_process_score_averages": comparison_average_map(&matched_rows_by_runner, |row| row.process_score),
        "matched_runner_efficiency_index_averages": comparison_optional_average_map(&matched_rows_by_runner, |row| row.efficiency_index),
        "matched_runner_benchmark_index_averages": matched_runner_benchmark_index_averages,
        "matched_runner_task_quality_averages": comparison_average_map(&matched_rows_by_runner, |row| row.task_quality_score),
        "matched_runner_efficiency_averages": comparison_average_map(&matched_rows_by_runner, |row| row.efficiency_score),
        "matched_runner_harness_pressure_averages": comparison_average_map(&matched_rows_by_runner, |row| row.harness_pressure_score),
        "matched_runner_duration_ms_averages": comparison_average_map(&matched_rows_by_runner, |row| row.duration_ms as f64),
        "matched_runner_tool_or_item_call_averages": comparison_average_map(&matched_rows_by_runner, |row| row.tool_or_item_calls as f64),
        "matched_runner_input_token_averages": comparison_average_map(&matched_rows_by_runner, |row| row.input_tokens as f64),
        "matched_runner_source_file_averages": comparison_average_map(&matched_rows_by_runner, |row| row.source_files as f64),
        "matched_runner_source_byte_averages": comparison_average_map(&matched_rows_by_runner, |row| row.source_bytes as f64),
        "headline": headline,
        "winner": winner,
        "scenario_winners": scenario_winners,
        "unmatched_scenarios": unmatched_scenarios,
        "diagnostics": comparison_diagnostics,
    })
}

fn matched_comparison_rows(rows: &[ComparisonRow]) -> Vec<&ComparisonRow> {
    let matched_scenarios = scenarios_with_both_runners(rows);
    rows.iter()
        .filter(|row| matched_scenarios.contains(&row.scenario))
        .collect()
}

fn scenarios_with_both_runners(rows: &[ComparisonRow]) -> BTreeSet<String> {
    let mut runners_by_scenario = BTreeMap::<String, BTreeSet<String>>::new();
    for row in rows {
        runners_by_scenario
            .entry(row.scenario.clone())
            .or_default()
            .insert(row.runner.clone());
    }
    runners_by_scenario
        .into_iter()
        .filter_map(|(scenario, runners)| (runners.len() >= 2).then_some(scenario))
        .collect()
}

fn comparison_rows_by_runner<'a>(
    rows: &[&'a ComparisonRow],
) -> BTreeMap<String, Vec<&'a ComparisonRow>> {
    let mut rows_by_runner = BTreeMap::<String, Vec<&ComparisonRow>>::new();
    for row in rows {
        rows_by_runner
            .entry(row.runner.clone())
            .or_default()
            .push(*row);
    }
    rows_by_runner
}

fn comparison_average_map(
    rows_by_runner: &BTreeMap<String, Vec<&ComparisonRow>>,
    value: impl Fn(&ComparisonRow) -> f64,
) -> BTreeMap<String, f64> {
    rows_by_runner
        .iter()
        .map(|(runner, rows)| (runner.clone(), average_comparison_field(rows, &value)))
        .collect()
}

fn comparison_optional_average_map(
    rows_by_runner: &BTreeMap<String, Vec<&ComparisonRow>>,
    value: impl Fn(&ComparisonRow) -> Option<f64>,
) -> BTreeMap<String, f64> {
    rows_by_runner
        .iter()
        .filter_map(|(runner, rows)| {
            average_optional_comparison_field(rows, &value).map(|average| (runner.clone(), average))
        })
        .collect()
}

fn unmatched_comparison_scenarios(rows: &[ComparisonRow]) -> Vec<Value> {
    let matched = scenarios_with_both_runners(rows);
    let mut by_scenario = BTreeMap::<String, Vec<&ComparisonRow>>::new();
    for row in rows {
        if matched.contains(&row.scenario) {
            continue;
        }
        by_scenario
            .entry(row.scenario.clone())
            .or_default()
            .push(row);
    }
    by_scenario
        .into_iter()
        .map(|(scenario, rows)| {
            json!({
                "scenario": scenario,
                "runners": rows.iter().map(|row| row.runner.clone()).collect::<Vec<_>>(),
            })
        })
        .collect()
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

fn compare_runner_rows(left: &[&ComparisonRow], right: &[&ComparisonRow]) -> Ordering {
    average_optional_comparison_field(left, |row| row.benchmark_index)
        .unwrap_or_else(|| average_comparison_field(left, |row| row.score))
        .total_cmp(
            &average_optional_comparison_field(right, |row| row.benchmark_index)
                .unwrap_or_else(|| average_comparison_field(right, |row| row.score)),
        )
        .then_with(|| {
            average_comparison_field(left, |row| row.completion_score)
                .total_cmp(&average_comparison_field(right, |row| row.completion_score))
        })
        .then_with(|| {
            average_comparison_field(left, |row| row.quality_score)
                .total_cmp(&average_comparison_field(right, |row| row.quality_score))
        })
        .then_with(|| {
            average_comparison_field(left, |row| row.task_quality_score).total_cmp(
                &average_comparison_field(right, |row| row.task_quality_score),
            )
        })
        .then_with(|| {
            average_comparison_field(left, |row| row.efficiency_score)
                .total_cmp(&average_comparison_field(right, |row| row.efficiency_score))
        })
        .then_with(|| {
            average_comparison_field(right, |row| row.duration_ms as f64).total_cmp(
                &average_comparison_field(left, |row| row.duration_ms as f64),
            )
        })
        .then_with(|| {
            average_comparison_field(right, |row| row.tool_or_item_calls as f64).total_cmp(
                &average_comparison_field(left, |row| row.tool_or_item_calls as f64),
            )
        })
        .then_with(|| {
            average_comparison_field(right, |row| row.input_tokens as f64).total_cmp(
                &average_comparison_field(left, |row| row.input_tokens as f64),
            )
        })
        .then_with(|| {
            average_comparison_field(right, |row| row.source_files as f64).total_cmp(
                &average_comparison_field(left, |row| row.source_files as f64),
            )
        })
        .then_with(|| {
            average_comparison_field(right, |row| row.source_bytes as f64).total_cmp(
                &average_comparison_field(left, |row| row.source_bytes as f64),
            )
        })
        .then_with(|| {
            average_comparison_field(left, |row| row.harness_pressure_score).total_cmp(
                &average_comparison_field(right, |row| row.harness_pressure_score),
            )
        })
}

fn average_comparison_field(rows: &[&ComparisonRow], value: impl Fn(&ComparisonRow) -> f64) -> f64 {
    if rows.is_empty() {
        return 0.0;
    }
    round1(rows.iter().map(|row| value(row)).sum::<f64>() / rows.len() as f64)
}

fn average_optional_comparison_field(
    rows: &[&ComparisonRow],
    value: impl Fn(&ComparisonRow) -> Option<f64>,
) -> Option<f64> {
    let values = rows.iter().filter_map(|row| value(row)).collect::<Vec<_>>();
    (!values.is_empty()).then(|| round1(values.iter().sum::<f64>() / values.len() as f64))
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
                .max_by(|left, right| compare_comparison_rows(left, right));
            json!({
                "scenario": scenario,
                "winner": winner.map(|row| row.runner.clone()).unwrap_or_default(),
                "winning_benchmark_index": winner.and_then(|row| row.benchmark_index),
                "winning_score": winner.map(|row| row.score).unwrap_or_default(),
                "spark_vs_codex": scenario_delta(&rows),
                "scores": rows.iter().map(|row| json!({
                    "runner": row.runner,
                    "benchmark_index": row.benchmark_index,
                    "efficiency_index": row.efficiency_index,
                    "completion_score": row.completion_score,
                    "quality_score": row.quality_score,
                    "process_score": row.process_score,
                    "llm_solution_score": row.llm_solution_score,
                    "llm_process_score": row.llm_process_score,
                    "llm_confidence": row.llm_confidence,
                    "llm_notes": row.llm_notes,
                    "score": row.score,
                    "task_quality_score": row.task_quality_score,
                    "efficiency_score": row.efficiency_score,
                    "harness_pressure_score": row.harness_pressure_score,
                    "duration_ms": row.duration_ms,
                    "tool_or_item_calls": row.tool_or_item_calls,
                    "input_tokens": row.input_tokens,
                    "source_files": row.source_files,
                    "source_bytes": row.source_bytes,
                    "success": row.success,
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn scenario_delta(rows: &[&ComparisonRow]) -> Option<Value> {
    let spark = rows.iter().find(|row| row.runner == "spark-harness")?;
    let codex = rows.iter().find(|row| row.runner == "codex-cli")?;
    Some(json!({
        "duration_ms_delta": spark.duration_ms as i128 - codex.duration_ms as i128,
        "token_ratio": ratio_or_zero(spark.input_tokens as f64, codex.input_tokens as f64),
        "tool_ratio": ratio_or_zero(spark.tool_or_item_calls as f64, codex.tool_or_item_calls as f64),
        "benchmark_index_delta": spark.benchmark_index.unwrap_or(0.0) - codex.benchmark_index.unwrap_or(0.0),
    }))
}

fn ratio_or_zero(denominator: f64, numerator: f64) -> f64 {
    if denominator <= 0.0 {
        0.0
    } else {
        round1(numerator / denominator)
    }
}

fn compare_comparison_rows(left: &ComparisonRow, right: &ComparisonRow) -> Ordering {
    left.benchmark_index
        .unwrap_or(left.score)
        .total_cmp(&right.benchmark_index.unwrap_or(right.score))
        .then_with(|| left.completion_score.total_cmp(&right.completion_score))
        .then_with(|| left.quality_score.total_cmp(&right.quality_score))
        .then_with(|| left.task_quality_score.total_cmp(&right.task_quality_score))
        .then_with(|| left.efficiency_score.total_cmp(&right.efficiency_score))
        .then_with(|| right.duration_ms.cmp(&left.duration_ms))
        .then_with(|| right.tool_or_item_calls.cmp(&left.tool_or_item_calls))
        .then_with(|| right.input_tokens.cmp(&left.input_tokens))
        .then_with(|| right.source_files.cmp(&left.source_files))
        .then_with(|| right.source_bytes.cmp(&left.source_bytes))
        .then_with(|| {
            left.harness_pressure_score
                .total_cmp(&right.harness_pressure_score)
        })
}

fn comparison_rows_to_csv(rows: &[ComparisonRow]) -> String {
    let mut csv = String::from(
        "runner,suite,scenario,model,reasoning_effort,command_path,command_version,score,task_quality_score,efficiency_score,harness_pressure_score,success,validation_exit_code,validation_timed_out,duration_ms,tool_or_item_calls,input_tokens,output_tokens,source_files,source_bytes,failure_points,source,completion_score,quality_score,process_score,llm_solution_score,llm_process_score,llm_confidence,llm_notes,efficiency_index,benchmark_index\n",
    );
    for row in rows {
        let values = [
            row.runner.clone(),
            row.suite.clone(),
            row.scenario.clone(),
            row.model.clone(),
            row.reasoning_effort.clone(),
            row.command_path.clone(),
            row.command_version.clone(),
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
            row.tool_or_item_calls.to_string(),
            row.input_tokens.to_string(),
            row.output_tokens.to_string(),
            row.source_files.to_string(),
            row.source_bytes.to_string(),
            row.failure_points.clone(),
            row.source.clone(),
            row.completion_score.to_string(),
            row.quality_score.to_string(),
            row.process_score.to_string(),
            row.llm_solution_score
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.llm_process_score
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.llm_confidence
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.llm_notes.clone(),
            row.efficiency_index
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.benchmark_index
                .map(|value| value.to_string())
                .unwrap_or_default(),
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

fn reasoning_effort(summary: &Value) -> Option<&str> {
    string_at(summary, "/trace_metadata/reasoning_effort").or_else(|| {
        string_at(
            summary,
            "/trace_metadata/context/profile_scenario/reasoning_effort",
        )
    })
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
mod tests;
