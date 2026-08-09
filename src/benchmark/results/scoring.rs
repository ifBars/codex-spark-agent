use std::collections::BTreeMap;

use serde_json::Value;

use crate::benchmark::codex_cli::CodexCliBenchmarkRow;

use super::{BenchmarkRunRow, ComparisonRow, round1, value_u64};

pub(super) fn benchmark_score(row: &BenchmarkRunRow) -> f64 {
    let mut score = row.completion_score * 0.6
        + row.quality_score * 0.2
        + row.process_score * 0.1
        + row.efficiency_score * 0.1;
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

pub(super) fn completion_score(row: &BenchmarkRunRow) -> f64 {
    let mut penalty = 0.0;
    if !row.success {
        penalty += 35.0;
    }
    penalty += row
        .expected_tool_groups
        .saturating_sub(row.satisfied_tool_groups) as f64
        * 10.0;
    penalty += row
        .expected_tool_calls
        .saturating_sub(row.satisfied_tool_calls) as f64
        * 8.0;
    if row.validation_timed_out {
        penalty += 30.0;
    }
    if row.validation_present && row.validation_exit_code != Some(0) {
        penalty += 35.0;
    }
    if row.browser_validation_timed_out {
        penalty += 40.0;
    }
    if row.browser_validation_present && row.browser_validation_exit_code != Some(0) {
        penalty += 60.0;
    }
    let score = 100.0 - penalty;
    if !row.success {
        return round1(score.min(60.0).clamp(0.0, 100.0));
    }
    round1(score.clamp(0.0, 100.0))
}

pub(super) fn quality_score(row: &BenchmarkRunRow) -> f64 {
    let mut penalty = 100.0 - row.completion_score;
    penalty += row
        .expected_tool_groups
        .saturating_sub(row.satisfied_tool_groups) as f64
        * 4.0;
    penalty += row
        .expected_tool_calls
        .saturating_sub(row.satisfied_tool_calls) as f64
        * 3.0;
    if row.browser_validation_present && row.browser_screenshot.is_empty() {
        penalty += 8.0;
    }
    if row.source_bytes > 0 {
        penalty += source_quality_penalty(row.source_files, row.source_bytes);
    }
    round1((100.0 - penalty).clamp(0.0, 100.0))
}

pub(super) fn quality_score_with_validation(
    row: &BenchmarkRunRow,
    validation_score: Option<f64>,
) -> f64 {
    validated_outcome_quality(validation_score, row.validation_present, row.success)
        .unwrap_or_else(|| quality_score(row))
}

fn source_quality_penalty(source_files: u64, source_bytes: u64) -> f64 {
    let size_penalty = source_bytes.saturating_sub(8_000) as f64 / 1_500.0;
    let file_penalty = source_files.saturating_sub(8) as f64 * 1.5;
    (size_penalty + file_penalty).min(18.0)
}

pub(super) fn efficiency_score(duration_ms: u128, source_bytes: u64) -> f64 {
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

pub(super) fn process_score(row: &BenchmarkRunRow) -> f64 {
    let mut penalty = 0.0;
    let exact_completion = exact_completion_pressure_scenario(&row.scenario);
    penalty += row.unrecovered_tool_failures as f64 * 15.0;
    penalty += row.recovered_tool_failures as f64 * 3.0;
    penalty += row.truncated_tool_results as f64 * 5.0;
    penalty += row.repeated_tool_calls as f64 * 3.0;
    if exact_completion {
        penalty += row.extra_calls_after_satisfied as f64 * 2.5;
        penalty += row.extra_turns_after_satisfied as f64 * 3.0;
    }
    penalty += row.max_tool_only_streak.saturating_sub(4) as f64 * 2.0;
    penalty += row.compactions as f64 * 5.0;
    penalty += (row.max_context_window_pct - 20.0).max(0.0) * 0.5;
    round1((100.0 - penalty).clamp(0.0, 100.0))
}

pub(super) fn unexpected_repeated_tool_calls(summary: &Value) -> u64 {
    value_u64(summary, "/repeated_tool_calls").saturating_sub(expected_repeated_tool_calls(summary))
}

pub(super) fn expected_repeated_tool_calls(summary: &Value) -> u64 {
    let Some(expected_calls) = summary
        .pointer("/profile_scenario_call_expectations/expected_calls")
        .and_then(Value::as_array)
    else {
        return 0;
    };
    let mut counts = BTreeMap::<String, u64>::new();
    for call in expected_calls {
        let key = expected_call_repeat_key(call);
        *counts.entry(key).or_default() += 1;
    }
    counts.values().map(|count| count.saturating_sub(1)).sum()
}

fn expected_call_repeat_key(call: &Value) -> String {
    let tool = call
        .get("tool")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            call.get("tools")
                .map(|tools| serde_json::to_string(tools).unwrap_or_else(|_| tools.to_string()))
        })
        .unwrap_or_default();
    let path = call.get("path").and_then(Value::as_str).unwrap_or_default();
    let command = call
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let from = call.get("from").and_then(Value::as_str).unwrap_or_default();
    let to = call.get("to").and_then(Value::as_str).unwrap_or_default();
    format!("tool={tool};path={path};command={command};from={from};to={to}")
}

pub(super) fn exact_completion_pressure_scenario(scenario: &str) -> bool {
    matches!(
        scenario,
        "file-edit"
            | "file-ops"
            | "tool-recovery"
            | "shell-recovery"
            | "precise-patch"
            | "multi-file-patch"
            | "github-issue-bugfix"
            | "rust-failing-test-bugfix"
            | "typescript-reducer-bugfix"
            | "merge-conflict-resolution"
            | "github-issue-triage"
            | "ci-failure-triage"
            | "pull-request-review"
            | "dependency-upgrade-triage"
            | "technical-essay"
            | "config-migration"
            | "ops-report"
            | "multi-module-bugfix"
            | "terminal-repair"
            | "multi-hop-analysis"
            | "policy-support-agent"
            | "react-calculator-scaffold"
            | "rust-log-analyzer-scaffold"
            | "rust-notes-tui-scaffold"
            | "natural-compaction"
            | "compaction-pressure"
    )
}

pub(super) fn comparison_score_from_components(row: &ComparisonRow) -> f64 {
    let mut score = row.completion_score * 0.6
        + row.quality_score * 0.2
        + row.process_score * 0.1
        + row.efficiency_score * 0.1;
    if row.validation_timed_out || row.validation_exit_code.is_some_and(|code| code != 0) {
        score = score.min(55.0);
    }
    if !row.success || row.completion_score < 100.0 {
        score = score.min(row.completion_score.min(60.0));
    }
    round1(score.clamp(0.0, 100.0))
}

pub(super) fn codex_score_from_components(
    row: &CodexCliBenchmarkRow,
    completion_score: f64,
    quality_score: f64,
    process_score: f64,
    efficiency_score: f64,
) -> f64 {
    let mut score =
        completion_score * 0.6 + quality_score * 0.2 + process_score * 0.1 + efficiency_score * 0.1;
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

pub(super) fn codex_completion_score(row: &CodexCliBenchmarkRow) -> f64 {
    let mut penalty = 0.0;
    if row.timed_out {
        penalty += 40.0;
    }
    if row.exit_code != Some(0) {
        penalty += 35.0;
    }
    if row.final_message_chars == 0 {
        penalty += 20.0;
    }
    penalty += row.expected_artifacts.saturating_sub(row.present_artifacts) as f64 * 10.0;
    if row.validation_timed_out {
        penalty += 30.0;
    }
    if row.validation_exit_code.is_some_and(|code| code != 0) {
        penalty += 35.0;
    }
    if row.browser_validation_timed_out {
        penalty += 40.0;
    }
    if row.browser_validation_present && row.browser_validation_exit_code != Some(0) {
        penalty += 60.0;
    }
    let score = 100.0 - penalty;
    if !row.success || row.exit_code != Some(0) {
        return round1(score.min(60.0).clamp(0.0, 100.0));
    }
    round1(score.clamp(0.0, 100.0))
}

pub(super) fn codex_quality_score(row: &CodexCliBenchmarkRow, completion_score: f64) -> f64 {
    let mut penalty = 100.0 - completion_score;
    penalty += row.expected_artifacts.saturating_sub(row.present_artifacts) as f64 * 4.0;
    if row.browser_validation_present && row.browser_screenshot.is_empty() {
        penalty += 8.0;
    }
    penalty += row.actionable_stderr_lines.min(10) as f64 * 1.5;
    if row.success && row.input_tokens == 0 {
        penalty += 4.0;
    }
    if row.success && row.final_message_chars < 120 {
        penalty += 4.0;
    }
    if row.source_bytes > 0 {
        penalty += source_quality_penalty(row.source_files, row.source_bytes);
    }
    round1((100.0 - penalty).clamp(0.0, 100.0))
}

pub(super) fn codex_quality_score_with_validation(
    row: &CodexCliBenchmarkRow,
    completion_score: f64,
) -> f64 {
    let validation_present = row.validation_exit_code.is_some() || row.validation_timed_out;
    validated_outcome_quality(row.validation_score, validation_present, row.success)
        .unwrap_or_else(|| codex_quality_score(row, completion_score))
}

fn validated_outcome_quality(
    validation_score: Option<f64>,
    validation_present: bool,
    success: bool,
) -> Option<f64> {
    validation_score
        .map(|score| round1(score.clamp(0.0, 100.0)))
        .or_else(|| validation_present.then_some(if success { 100.0 } else { 0.0 }))
}

pub(super) fn codex_efficiency_score(row: &CodexCliBenchmarkRow) -> f64 {
    efficiency_score(row.duration_ms, row.source_bytes)
}

pub(super) fn codex_process_score(row: &CodexCliBenchmarkRow) -> f64 {
    let mut penalty = 0.0;
    if row.timed_out {
        penalty += 25.0;
    }
    penalty += (row.non_json_stdout_lines.min(20) as f64) * 0.5;
    penalty += (row.actionable_stderr_lines.min(20) as f64) * 0.25;
    if row.failure_points.contains("tool_execution_error") {
        penalty += 4.0;
    }
    if row.duration_ms > 180_000 {
        penalty += 10.0;
    } else if row.duration_ms > 90_000 {
        penalty += 5.0;
    }
    round1((100.0 - penalty).clamp(0.0, 100.0))
}

pub(super) fn quality_gate(row: &ComparisonRow) -> f64 {
    if row.llm_solution_score.is_some() {
        row.completion_score * 0.3 + row.quality_score * 0.6 + row.process_score * 0.1
    } else {
        row.completion_score * 0.6 + row.quality_score * 0.3 + row.process_score * 0.1
    }
}
