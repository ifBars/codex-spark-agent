use serde_json::Value;

use super::aggregate::{compactions_with_local_pressure, trace_scenario_name};
use super::common::{diagnostic_kinds, format_required_action, format_tool_group, number_field};

pub fn format_trace_summary_row(label: &str, summary: &Value) -> String {
    let model = summary
        .pointer("/trace_metadata/model")
        .and_then(Value::as_str)
        .unwrap_or("unknown-model");
    let scenario = summary
        .get("trace_metadata")
        .and_then(trace_scenario_name)
        .map(|name| format!(" scenario={name}"))
        .unwrap_or_default();
    let requests = number_field(summary, "requests");
    let max_tokens = number_field(summary, "max_approx_input_tokens");
    let context_pct = summary
        .get("max_context_window_pct")
        .and_then(Value::as_f64)
        .map(|value| format!("{value:.1}%"))
        .unwrap_or_else(|| "?.?%".to_string());
    let max_request_ms = number_field(summary, "max_request_duration_ms");
    let tool_calls = number_field(summary, "tool_calls");
    let tool_failures = number_field(summary, "tool_failures");
    let compactions = number_field(summary, "compactions");
    let remote_compactions = number_field(summary, "remote_compactions");
    let fallback_compactions = number_field(summary, "fallback_compactions");
    let local_pressure_compactions = compactions_with_local_pressure(summary);
    let scenario_tools = format_scenario_tools_for_summary_row(summary);
    let scenario_calls = format_scenario_calls_for_summary_row(summary);
    let scenario_skills = format_scenario_skills_for_summary_row(summary);
    let tool_only_turns = format_tool_only_turns_for_summary_row(summary);
    let recoveries = format_tool_failure_recovery_for_summary_row(summary);
    let diagnostics = diagnostic_kinds(summary);
    let diagnostics = if diagnostics.is_empty() {
        "none".to_string()
    } else {
        diagnostics.join(",")
    };

    format!(
        "{label} | model={model}{scenario} requests={requests} max_tokens={max_tokens} ({context_pct}) max_request_ms={max_request_ms} tools={tool_calls} failures={tool_failures}{recoveries} compactions={compactions} remote={remote_compactions} fallback={fallback_compactions} local_pressure={local_pressure_compactions}{scenario_tools}{scenario_calls}{scenario_skills}{tool_only_turns} diagnostics={diagnostics}"
    )
}

pub fn trace_profile_scenario_name(summary: &Value) -> Option<&str> {
    summary.get("trace_metadata").and_then(trace_scenario_name)
}

pub(super) fn format_required_actions_summary(summary: &Value) -> Option<String> {
    let total = summary
        .get("retained_required_actions")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if total == 0 {
        return None;
    }
    let executed = summary
        .get("retained_required_actions_executed")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let missing = summary
        .get("retained_required_actions_missing")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let detours = summary
        .get("tool_calls_before_first_required_action")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let actions = summary
        .get("retained_required_actions")
        .and_then(Value::as_array)
        .map(|actions| {
            actions
                .iter()
                .map(format_required_action)
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    Some(format!(
        "required-actions: total={total} executed={executed} missing={missing} detours_before_first={detours} actions=[{actions}]"
    ))
}

pub(super) fn format_scenario_tool_expectations(summary: &Value) -> Option<String> {
    let report = summary.get("profile_scenario_tool_expectations")?;
    let total = report
        .get("total_groups")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if total == 0 {
        return None;
    }
    let satisfied = report
        .get("satisfied_groups")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let missing = report
        .get("missing_groups")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let groups = report
        .get("expected_tool_groups")
        .and_then(Value::as_array)
        .map(|groups| {
            groups
                .iter()
                .filter_map(format_tool_group)
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    Some(format!(
        "scenario-tools: satisfied={satisfied}/{total} missing={missing} groups=[{groups}]"
    ))
}

pub(super) fn format_scenario_call_expectations(summary: &Value) -> Option<String> {
    let report = summary.get("profile_scenario_call_expectations")?;
    let total = report
        .get("total_calls")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if total == 0 {
        return None;
    }
    let satisfied = report
        .get("satisfied_calls")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let missing = report
        .get("missing_calls")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let calls = report
        .get("expected_calls")
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .map(format_required_action)
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    let extra = report
        .get("extra_calls_after_satisfied")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0)
        .map(|count| format!(" extra_after={count}"))
        .unwrap_or_default();
    let extra_turns = report
        .get("extra_turns_after_satisfied")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0)
        .map(|count| format!(" extra_turns={count}"))
        .unwrap_or_default();
    let growth = report
        .get("context_growth_after_satisfied_chars")
        .and_then(Value::as_u64)
        .filter(|chars| *chars > 0)
        .map(|chars| format!(" post_satisfied_chars={chars}"))
        .unwrap_or_default();
    Some(format!(
        "scenario-calls: satisfied={satisfied}/{total} missing={missing}{extra}{extra_turns}{growth} calls=[{calls}]",
    ))
}

pub(super) fn format_scenario_skill_expectations(summary: &Value) -> Option<String> {
    let report = summary.get("profile_scenario_skill_expectations")?;
    let total = report
        .get("total_skills")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if total == 0 {
        return None;
    }
    let satisfied = report
        .get("satisfied_skills")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let missing = report
        .get("missing_skills")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let skills = report
        .get("expected_skills")
        .and_then(Value::as_array)
        .map(|skills| {
            skills
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    Some(format!(
        "scenario-skills: satisfied={satisfied}/{total} missing={missing} skills=[{skills}]"
    ))
}

fn format_scenario_tools_for_summary_row(summary: &Value) -> String {
    let Some(report) = summary.get("profile_scenario_tool_expectations") else {
        return String::new();
    };
    let total = report
        .get("total_groups")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if total == 0 {
        return String::new();
    }
    let satisfied = report
        .get("satisfied_groups")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    format!(" scenario_tools={satisfied}/{total}")
}

fn format_scenario_calls_for_summary_row(summary: &Value) -> String {
    let Some(report) = summary.get("profile_scenario_call_expectations") else {
        return String::new();
    };
    let total = report
        .get("total_calls")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if total == 0 {
        return String::new();
    }
    let satisfied = report
        .get("satisfied_calls")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let extra = report
        .get("extra_calls_after_satisfied")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0)
        .map(|count| format!(" extra_calls={count}"))
        .unwrap_or_default();
    let extra_turns = report
        .get("extra_turns_after_satisfied")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0)
        .map(|count| format!(" extra_turns={count}"))
        .unwrap_or_default();
    let growth = report
        .get("context_growth_after_satisfied_chars")
        .and_then(Value::as_u64)
        .filter(|chars| *chars > 0)
        .map(|chars| format!(" context_growth={chars}"))
        .unwrap_or_default();
    format!(" scenario_calls={satisfied}/{total}{extra}{extra_turns}{growth}")
}

fn format_scenario_skills_for_summary_row(summary: &Value) -> String {
    let Some(report) = summary.get("profile_scenario_skill_expectations") else {
        return String::new();
    };
    let total = report
        .get("total_skills")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if total == 0 {
        return String::new();
    }
    let satisfied = report
        .get("satisfied_skills")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    format!(" scenario_skills={satisfied}/{total}")
}

pub(super) fn format_tool_only_turns(summary: &Value) -> Option<String> {
    let report = summary.get("tool_only_turns")?;
    let count = report.get("count").and_then(Value::as_u64).unwrap_or(0);
    if count == 0 {
        return None;
    }
    let max_consecutive = report
        .get("max_consecutive")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let turns = report
        .get("turns")
        .and_then(Value::as_array)
        .map(|turns| {
            turns
                .iter()
                .filter_map(Value::as_u64)
                .map(|turn| turn.to_string())
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    Some(format!(
        "tool-only-turns: count={count} max_consecutive={max_consecutive} turns=[{turns}]"
    ))
}

pub(super) fn format_tool_only_turns_for_summary_row(summary: &Value) -> String {
    let Some(report) = summary.get("tool_only_turns") else {
        return String::new();
    };
    let count = report.get("count").and_then(Value::as_u64).unwrap_or(0);
    if count == 0 {
        return String::new();
    }
    let max_consecutive = report
        .get("max_consecutive")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    format!(" tool_only={count} max_tool_only_streak={max_consecutive}")
}

pub(super) fn format_tool_failure_recovery(summary: &Value) -> Option<String> {
    let report = summary.get("tool_failure_recovery")?;
    let failed = report
        .get("failed_tool_results")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if failed == 0 {
        return None;
    }
    let recovered = report
        .get("recovered_failures")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let unrecovered = report
        .get("unrecovered_failures")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let by_tool = report
        .get("by_tool")
        .and_then(Value::as_object)
        .map(|tools| {
            tools
                .iter()
                .map(|(tool, counts)| {
                    let recovered = counts.get("recovered").and_then(Value::as_u64).unwrap_or(0);
                    let failed = counts.get("failed").and_then(Value::as_u64).unwrap_or(0);
                    format!("{tool}:{recovered}/{failed}")
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    Some(format!(
        "tool-recovery: recovered={recovered}/{failed} unrecovered={unrecovered} by_tool=[{by_tool}]"
    ))
}

pub(super) fn format_tool_failure_recovery_for_summary_row(summary: &Value) -> String {
    let Some(report) = summary.get("tool_failure_recovery") else {
        return String::new();
    };
    let failed = report
        .get("failed_tool_results")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if failed == 0 {
        return String::new();
    }
    let recovered = report
        .get("recovered_failures")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    format!(" recoveries={recovered}/{failed}")
}
