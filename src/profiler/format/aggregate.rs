use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::common::diagnostic_kinds;

pub fn format_trace_aggregate_row(label: &str, summaries: &[Value]) -> String {
    let count = summaries.len();
    if count == 0 {
        return format!("{label} aggregate | runs=0");
    }

    let successes = summaries
        .iter()
        .filter(|summary| {
            summary
                .get("errors")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
        })
        .count();
    let failures = count.saturating_sub(successes);
    let max_tokens = summaries
        .iter()
        .filter_map(|summary| {
            summary
                .get("max_approx_input_tokens")
                .and_then(Value::as_u64)
        })
        .max()
        .unwrap_or(0);
    let max_context_pct = summaries
        .iter()
        .filter_map(|summary| {
            summary
                .get("max_context_window_pct")
                .and_then(Value::as_f64)
        })
        .fold(0.0, f64::max);
    let max_request_ms = summaries
        .iter()
        .filter_map(|summary| {
            summary
                .get("max_request_duration_ms")
                .and_then(Value::as_u64)
        })
        .max()
        .unwrap_or(0);
    let total_tools = sum_summary_field(summaries, "tool_calls");
    let total_tool_failures = sum_summary_field(summaries, "tool_failures");
    let total_recovered_tool_failures = sum_recovery_field(summaries, "recovered_failures");
    let total_failed_tool_results = sum_recovery_field(summaries, "failed_tool_results");
    let aggregate_recoveries = if total_failed_tool_results == 0 {
        String::new()
    } else {
        format!(" recoveries={total_recovered_tool_failures}/{total_failed_tool_results}")
    };
    let aggregate_scenario_tools = format_aggregate_expectation_ratio(
        summaries,
        "profile_scenario_tool_expectations",
        "satisfied_groups",
        "total_groups",
        "scenario_tools",
    );
    let aggregate_scenario_calls = format_aggregate_expectation_ratio(
        summaries,
        "profile_scenario_call_expectations",
        "satisfied_calls",
        "total_calls",
        "scenario_calls",
    );
    let aggregate_scenario_skills = format_aggregate_expectation_ratio(
        summaries,
        "profile_scenario_skill_expectations",
        "satisfied_skills",
        "total_skills",
        "scenario_skills",
    );
    let aggregate_scenario_overrun = format_aggregate_scenario_overrun(summaries);
    let total_compactions = sum_summary_field(summaries, "compactions");
    let total_remote_compactions = sum_summary_field(summaries, "remote_compactions");
    let total_fallback_compactions = sum_summary_field(summaries, "fallback_compactions");
    let total_local_pressure_compactions = summaries
        .iter()
        .map(compactions_with_local_pressure)
        .sum::<usize>();
    let max_compaction_regrowth =
        max_compaction_regrowth_field(summaries, "max_next_request_growth_chars");
    let total_tool_only_turns = sum_tool_only_turn_field(summaries, "count");
    let max_tool_only_streak = summaries
        .iter()
        .filter_map(|summary| {
            summary
                .pointer("/tool_only_turns/max_consecutive")
                .and_then(Value::as_u64)
        })
        .max()
        .unwrap_or(0);
    let aggregate_tool_only_turns = if total_tool_only_turns == 0 {
        String::new()
    } else {
        format!(" tool_only={total_tool_only_turns} max_tool_only_streak={max_tool_only_streak}")
    };
    let diagnostics = aggregate_diagnostic_kinds(summaries);
    let diagnostics = if diagnostics.is_empty() {
        "none".to_string()
    } else {
        diagnostics.join(",")
    };

    format!(
        "{label} aggregate | runs={count} success={successes} failure={failures} max_tokens={max_tokens} ({max_context_pct:.1}%) max_request_ms={max_request_ms} tools={total_tools} failures={total_tool_failures}{aggregate_recoveries} compactions={total_compactions} remote={total_remote_compactions} fallback={total_fallback_compactions} local_pressure={total_local_pressure_compactions} max_compaction_regrowth={max_compaction_regrowth}{aggregate_scenario_tools}{aggregate_scenario_calls}{aggregate_scenario_skills}{aggregate_scenario_overrun}{aggregate_tool_only_turns} diagnostics={diagnostics}"
    )
}

pub fn trace_aggregate_json(label: &str, summaries: &[Value]) -> Value {
    let count = summaries.len();
    let successes = summaries
        .iter()
        .filter(|summary| {
            summary
                .get("errors")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
        })
        .count();
    let max_tokens = summaries
        .iter()
        .filter_map(|summary| {
            summary
                .get("max_approx_input_tokens")
                .and_then(Value::as_u64)
        })
        .max()
        .unwrap_or(0);
    let max_context_pct = summaries
        .iter()
        .filter_map(|summary| {
            summary
                .get("max_context_window_pct")
                .and_then(Value::as_f64)
        })
        .fold(0.0, f64::max);
    let max_request_ms = summaries
        .iter()
        .filter_map(|summary| {
            summary
                .get("max_request_duration_ms")
                .and_then(Value::as_u64)
        })
        .max()
        .unwrap_or(0);

    json!({
        "label": label,
        "runs": count,
        "success": successes,
        "failure": count.saturating_sub(successes),
        "max_approx_input_tokens": max_tokens,
        "max_context_window_pct": max_context_pct,
        "max_request_duration_ms": max_request_ms,
        "tool_calls": sum_summary_field(summaries, "tool_calls"),
        "tool_failures": sum_summary_field(summaries, "tool_failures"),
        "recovered_tool_failures": sum_recovery_field(summaries, "recovered_failures"),
        "failed_tool_results": sum_recovery_field(summaries, "failed_tool_results"),
        "compactions": sum_summary_field(summaries, "compactions"),
        "remote_compactions": sum_summary_field(summaries, "remote_compactions"),
        "fallback_compactions": sum_summary_field(summaries, "fallback_compactions"),
        "local_pressure_compactions": summaries.iter().map(compactions_with_local_pressure).sum::<usize>(),
        "max_compaction_regrowth_chars": max_compaction_regrowth_field(summaries, "max_next_request_growth_chars"),
        "tool_only_turns": sum_tool_only_turn_field(summaries, "count"),
        "max_tool_only_turn_streak": summaries
            .iter()
            .filter_map(|summary| summary.pointer("/tool_only_turns/max_consecutive").and_then(Value::as_u64))
            .max()
            .unwrap_or(0),
        "scenario_tools": aggregate_expectation_json(
            summaries,
            "profile_scenario_tool_expectations",
            "satisfied_groups",
            "total_groups",
        ),
        "scenario_calls": aggregate_expectation_json(
            summaries,
            "profile_scenario_call_expectations",
            "satisfied_calls",
            "total_calls",
        ),
        "scenario_skills": aggregate_expectation_json(
            summaries,
            "profile_scenario_skill_expectations",
            "satisfied_skills",
            "total_skills",
        ),
        "scenario_overrun": aggregate_scenario_overrun_json(summaries),
        "diagnostics": aggregate_diagnostic_count_map(summaries),
    })
}

pub(super) fn trace_scenario_name(metadata: &Value) -> Option<&str> {
    metadata
        .pointer("/context/profile_scenario/name")
        .and_then(Value::as_str)
}

fn sum_summary_field(summaries: &[Value], key: &str) -> u64 {
    summaries
        .iter()
        .filter_map(|summary| summary.get(key).and_then(Value::as_u64))
        .sum()
}

fn sum_recovery_field(summaries: &[Value], key: &str) -> u64 {
    summaries
        .iter()
        .filter_map(|summary| {
            summary
                .pointer(&format!("/tool_failure_recovery/{key}"))
                .and_then(Value::as_u64)
        })
        .sum()
}

fn max_compaction_regrowth_field(summaries: &[Value], key: &str) -> u64 {
    summaries
        .iter()
        .filter_map(|summary| {
            summary
                .pointer(&format!("/compaction_regrowth/{key}"))
                .and_then(Value::as_u64)
        })
        .max()
        .unwrap_or(0)
}

fn sum_tool_only_turn_field(summaries: &[Value], key: &str) -> u64 {
    summaries
        .iter()
        .filter_map(|summary| {
            summary
                .pointer(&format!("/tool_only_turns/{key}"))
                .and_then(Value::as_u64)
        })
        .sum()
}

fn sum_scenario_call_field(summaries: &[Value], key: &str) -> u64 {
    summaries
        .iter()
        .filter_map(|summary| {
            summary
                .pointer(&format!("/profile_scenario_call_expectations/{key}"))
                .and_then(Value::as_u64)
        })
        .sum()
}

fn max_scenario_call_field(summaries: &[Value], key: &str) -> u64 {
    summaries
        .iter()
        .filter_map(|summary| {
            summary
                .pointer(&format!("/profile_scenario_call_expectations/{key}"))
                .and_then(Value::as_u64)
        })
        .max()
        .unwrap_or(0)
}

fn format_aggregate_scenario_overrun(summaries: &[Value]) -> String {
    let extra_calls = sum_scenario_call_field(summaries, "extra_calls_after_satisfied");
    let extra_turns = sum_scenario_call_field(summaries, "extra_turns_after_satisfied");
    let context_growth = sum_scenario_call_field(summaries, "context_growth_after_satisfied_chars");
    if extra_calls == 0 && extra_turns == 0 && context_growth == 0 {
        return String::new();
    }
    let max_extra_turns = max_scenario_call_field(summaries, "extra_turns_after_satisfied");
    format!(
        " scenario_overrun_calls={extra_calls} scenario_overrun_turns={extra_turns} max_overrun_turns={max_extra_turns} scenario_overrun_context={context_growth}"
    )
}

fn aggregate_scenario_overrun_json(summaries: &[Value]) -> Value {
    json!({
        "extra_calls_after_satisfied": sum_scenario_call_field(summaries, "extra_calls_after_satisfied"),
        "extra_turns_after_satisfied": sum_scenario_call_field(summaries, "extra_turns_after_satisfied"),
        "max_extra_turns_after_satisfied": max_scenario_call_field(summaries, "extra_turns_after_satisfied"),
        "context_growth_after_satisfied_chars": sum_scenario_call_field(summaries, "context_growth_after_satisfied_chars"),
        "max_context_growth_after_satisfied_chars": max_scenario_call_field(summaries, "context_growth_after_satisfied_chars"),
    })
}

fn format_aggregate_expectation_ratio(
    summaries: &[Value],
    report_key: &str,
    satisfied_key: &str,
    total_key: &str,
    label: &str,
) -> String {
    let (satisfied, total) = summaries
        .iter()
        .filter_map(|summary| summary.get(report_key))
        .fold((0_u64, 0_u64), |(satisfied_sum, total_sum), report| {
            (
                satisfied_sum
                    + report
                        .get(satisfied_key)
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                total_sum + report.get(total_key).and_then(Value::as_u64).unwrap_or(0),
            )
        });
    if total == 0 {
        String::new()
    } else {
        format!(" {label}={satisfied}/{total}")
    }
}

fn aggregate_expectation_json(
    summaries: &[Value],
    report_key: &str,
    satisfied_key: &str,
    total_key: &str,
) -> Value {
    let (satisfied, total) = summaries
        .iter()
        .filter_map(|summary| summary.get(report_key))
        .fold((0, 0), |(satisfied, total), report| {
            (
                satisfied
                    + report
                        .get(satisfied_key)
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                total + report.get(total_key).and_then(Value::as_u64).unwrap_or(0),
            )
        });

    json!({
        "satisfied": satisfied,
        "total": total,
    })
}

pub(super) fn compactions_with_local_pressure(summary: &Value) -> usize {
    summary
        .get("compaction_reports")
        .and_then(Value::as_array)
        .map(|reports| {
            reports
                .iter()
                .filter(|report| {
                    report
                        .get("local_pressure")
                        .is_some_and(|value| !value.is_null())
                })
                .count()
        })
        .unwrap_or(0)
}

fn aggregate_diagnostic_kinds(summaries: &[Value]) -> Vec<String> {
    aggregate_diagnostic_counts(summaries)
        .into_iter()
        .map(|(kind, count)| format!("{kind}:{count}"))
        .collect()
}

fn aggregate_diagnostic_count_map(summaries: &[Value]) -> Value {
    let counts = aggregate_diagnostic_counts(summaries);
    Value::Object(
        counts
            .into_iter()
            .map(|(kind, count)| (kind, json!(count)))
            .collect(),
    )
}

fn aggregate_diagnostic_counts(summaries: &[Value]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::<String, usize>::new();
    for summary in summaries {
        for kind in diagnostic_kinds(summary) {
            *counts.entry(kind).or_default() += 1;
        }
    }
    counts
}
