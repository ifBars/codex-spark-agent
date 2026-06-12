use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde_json::{Value, json};

use crate::benchmark::{
    codex_cli::CodexCliBenchmarkRow,
    infrastructure::{
        contains_external_infrastructure_failure_signal, external_infrastructure_retry_hint,
        failure_points_contain,
    },
};

use super::{BenchmarkRunRow, resolve_manifest_trace_dir};

#[derive(Debug)]
pub(super) struct ExternalAgentReportRows {
    pub(super) rows: Vec<CodexCliBenchmarkRow>,
    pub(super) skipped_infrastructure_failures: usize,
    pub(super) skipped_infrastructure_scenarios: BTreeMap<String, usize>,
    pub(super) skipped_infrastructure_retry_hints: BTreeMap<String, String>,
}

#[derive(Debug)]
pub(super) struct HarnessComparisonRows {
    pub(super) rows: Vec<BenchmarkRunRow>,
    pub(super) skipped_request_failures: usize,
    pub(super) skipped_request_failure_scenarios: BTreeMap<String, usize>,
    pub(super) skipped_request_failure_retry_hints: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
pub(super) struct ComparisonDiagnostics {
    pub(super) skipped_spark_infrastructure_failures: usize,
    pub(super) skipped_spark_infrastructure_scenarios: BTreeMap<String, usize>,
    pub(super) skipped_spark_infrastructure_retry_hints: BTreeMap<String, String>,
    pub(super) skipped_codex_infrastructure_failures: usize,
    pub(super) skipped_codex_infrastructure_scenarios: BTreeMap<String, usize>,
    pub(super) skipped_codex_infrastructure_retry_hints: BTreeMap<String, String>,
    pub(super) skipped_opencode_infrastructure_failures: usize,
    pub(super) skipped_opencode_infrastructure_scenarios: BTreeMap<String, usize>,
    pub(super) skipped_opencode_infrastructure_retry_hints: BTreeMap<String, String>,
}

pub(super) fn filter_harness_request_failure_rows(
    cwd: &Path,
    rows: Vec<BenchmarkRunRow>,
) -> HarnessComparisonRows {
    let mut kept = Vec::new();
    let mut skipped_request_failures = 0usize;
    let mut skipped_request_failure_scenarios = BTreeMap::<String, usize>::new();
    let mut skipped_request_failure_retry_hints = BTreeMap::<String, String>::new();
    for row in rows {
        if let Some(retry_hint) = harness_row_infrastructure_retry_hint(cwd, &row) {
            skipped_request_failures += 1;
            *skipped_request_failure_scenarios
                .entry(row.scenario.clone())
                .or_default() += 1;
            if !retry_hint.trim().is_empty() {
                skipped_request_failure_retry_hints
                    .entry(row.scenario.clone())
                    .or_insert(retry_hint);
            }
            continue;
        }
        kept.push(row);
    }
    if skipped_request_failures > 0 {
        eprintln!(
            "{}",
            skipped_infrastructure_rows_message(
                skipped_request_failures,
                "Spark harness provider/API failure",
                &skipped_request_failure_scenarios,
            )
        );
    }
    HarnessComparisonRows {
        rows: kept,
        skipped_request_failures,
        skipped_request_failure_scenarios,
        skipped_request_failure_retry_hints,
    }
}

fn harness_row_infrastructure_retry_hint(cwd: &Path, row: &BenchmarkRunRow) -> Option<String> {
    if !failure_points_contain(&row.diagnostics, "request_failure") {
        return None;
    }

    let trace_dir = resolve_manifest_trace_dir(cwd, &row.trace_dir);
    let mut evidence = format!("{}\n{}", row.diagnostics, row.failure_points);
    if let Ok(entries) = std::fs::read_dir(trace_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
                continue;
            };
            if !matches!(extension, "json" | "jsonl" | "txt" | "log") {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(path) {
                evidence.push('\n');
                evidence.push_str(&text);
            }
        }
    }
    if contains_external_infrastructure_failure_signal(&evidence) {
        return Some(external_infrastructure_retry_hint(&evidence).unwrap_or_default());
    }
    None
}

pub(super) fn read_external_agent_report_rows_with_skips(
    cwd: &Path,
    paths: &[PathBuf],
    label: &str,
) -> Result<ExternalAgentReportRows> {
    let mut rows = Vec::new();
    let mut skipped_infrastructure_failures = 0usize;
    let mut skipped_infrastructure_scenarios = BTreeMap::<String, usize>::new();
    let mut skipped_infrastructure_retry_hints = BTreeMap::<String, String>::new();
    for path in paths {
        for row in read_external_agent_rows(path, label)? {
            if let Some(retry_hint) = external_agent_row_infrastructure_retry_hint(cwd, &row) {
                skipped_infrastructure_failures += 1;
                *skipped_infrastructure_scenarios
                    .entry(row.scenario.clone())
                    .or_default() += 1;
                if !retry_hint.trim().is_empty() {
                    skipped_infrastructure_retry_hints
                        .entry(row.scenario.clone())
                        .or_insert(retry_hint);
                }
                continue;
            }
            rows.push(row);
        }
    }
    if skipped_infrastructure_failures > 0 {
        eprintln!(
            "{}",
            skipped_infrastructure_rows_message(
                skipped_infrastructure_failures,
                &format!("{label} infrastructure/API failure"),
                &skipped_infrastructure_scenarios,
            )
        );
    }
    Ok(ExternalAgentReportRows {
        rows,
        skipped_infrastructure_failures,
        skipped_infrastructure_scenarios,
        skipped_infrastructure_retry_hints,
    })
}

fn read_external_agent_rows(path: &Path, label: &str) -> Result<Vec<CodexCliBenchmarkRow>> {
    let value: Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(|error| {
        anyhow::anyhow!("failed to read {label} report {}: {error}", path.display())
    })?)
    .map_err(|error| {
        anyhow::anyhow!(
            "failed to parse {label} report JSON {}: {error}",
            path.display()
        )
    })?;
    serde_json::from_value(value.get("rows").cloned().unwrap_or_else(|| json!([]))).map_err(
        |error| {
            anyhow::anyhow!(
                "failed to parse {label} report rows from {}: {error}",
                path.display()
            )
        },
    )
}

fn external_agent_row_infrastructure_retry_hint(
    cwd: &Path,
    row: &CodexCliBenchmarkRow,
) -> Option<String> {
    let run_dir = resolve_external_run_dir(cwd, &row.run_dir);
    let mut evidence = String::new();
    evidence.push_str(&row.failure_points);
    if !row.provider_retry_hint.trim().is_empty() {
        evidence.push('\n');
        evidence.push_str(&row.provider_retry_hint);
    }
    for file_name in ["last-message.txt", "stdout.jsonl", "stderr.txt"] {
        let path = run_dir.join(file_name);
        if let Ok(text) = std::fs::read_to_string(path) {
            evidence.push('\n');
            evidence.push_str(&text);
        }
    }

    let request_failure = failure_points_contain(&row.failure_points, "request_failure");
    if !request_failure && !contains_external_infrastructure_failure_signal(&evidence) {
        return None;
    }
    if !row.provider_retry_hint.trim().is_empty() {
        return Some(row.provider_retry_hint.clone());
    }
    Some(external_infrastructure_retry_hint(&evidence).unwrap_or_default())
}

pub(super) fn skipped_infrastructure_scenarios_text(scenarios: &BTreeMap<String, usize>) -> String {
    if scenarios.is_empty() {
        return String::new();
    }
    let parts = scenarios
        .iter()
        .filter(|(_, count)| **count > 0)
        .map(|(scenario, count)| {
            if *count == 1 {
                scenario.clone()
            } else {
                format!("{scenario} x{count}")
            }
        })
        .collect::<Vec<_>>();
    if parts.is_empty() {
        String::new()
    } else {
        format!(": {}", parts.join(", "))
    }
}

pub(super) fn skipped_infrastructure_rows_message(
    count: usize,
    label: &str,
    scenarios: &BTreeMap<String, usize>,
) -> String {
    format!(
        "benchmark_compare skipped {count} {label} row(s){}",
        skipped_infrastructure_scenarios_text(scenarios)
    )
}

fn resolve_external_run_dir(cwd: &Path, run_dir: &str) -> PathBuf {
    let path = PathBuf::from(run_dir);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}
