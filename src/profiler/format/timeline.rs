use serde_json::Value;

use super::aggregate::trace_scenario_name;
use super::summary::{
    format_required_actions_summary, format_scenario_call_expectations,
    format_scenario_skill_expectations, format_scenario_tool_expectations,
    format_tool_failure_recovery, format_tool_only_turns,
};

pub fn format_trace_timeline(summary: &Value) -> String {
    let mut lines = Vec::new();
    if let Some(metadata) = summary.get("trace_metadata") {
        let model = metadata
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("unknown-model");
        let max_turns = metadata
            .get("max_turns")
            .map(compact_json_value)
            .unwrap_or_else(|| "null".to_string());
        let compact_after = metadata
            .get("compact_after_chars")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "?".to_string());
        let max_input = metadata
            .get("max_input_chars")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "?".to_string());
        let scenario = trace_scenario_name(metadata)
            .map(|name| format!(" scenario={name}"))
            .unwrap_or_default();
        lines.push(format!(
            "trace model={model}{scenario} max_turns={max_turns} compact_after_chars={compact_after} max_input_chars={max_input}"
        ));
    } else {
        lines.push("trace".to_string());
    }

    if let Some(diagnostics) = summary.get("diagnostics").and_then(Value::as_array)
        && !diagnostics.is_empty()
    {
        let kinds = diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.get("kind").and_then(Value::as_str))
            .collect::<Vec<_>>();
        lines.push(format!("diagnostics: {}", kinds.join(", ")));
    }

    if let Some(required_actions) = format_required_actions_summary(summary) {
        lines.push(required_actions);
    }
    if let Some(scenario_tools) = format_scenario_tool_expectations(summary) {
        lines.push(scenario_tools);
    }
    if let Some(scenario_calls) = format_scenario_call_expectations(summary) {
        lines.push(scenario_calls);
    }
    if let Some(scenario_skills) = format_scenario_skill_expectations(summary) {
        lines.push(scenario_skills);
    }
    if let Some(tool_only_turns) = format_tool_only_turns(summary) {
        lines.push(tool_only_turns);
    }
    if let Some(tool_recovery) = format_tool_failure_recovery(summary) {
        lines.push(tool_recovery);
    }

    let Some(timeline) = summary.get("timeline").and_then(Value::as_array) else {
        lines.push("timeline: none".to_string());
        return format!("{}\n", lines.join("\n"));
    };

    if timeline.is_empty() {
        lines.push("timeline: empty".to_string());
        return format!("{}\n", lines.join("\n"));
    }

    for turn in timeline {
        lines.push(format_timeline_turn(turn));
    }

    format!("{}\n", lines.join("\n"))
}

fn format_timeline_turn(turn: &Value) -> String {
    let turn_number = turn.get("turn").and_then(Value::as_u64).unwrap_or(0);
    let input_chars = turn
        .get("request_input_chars")
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".to_string());
    let approx_tokens = turn
        .get("request_approx_tokens")
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".to_string());
    let context_pct = turn
        .get("context_window_pct")
        .and_then(Value::as_f64)
        .map(|value| format!("{value:.1}%"))
        .unwrap_or_else(|| "?.?%".to_string());
    let request_ms = turn
        .get("request_duration_ms")
        .and_then(Value::as_u64)
        .map(|value| format!("{value}ms"))
        .unwrap_or_else(|| "?ms".to_string());
    let response_chars = turn
        .get("response_text_chars")
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "0".to_string());

    let mut parts = vec![format!(
        "turn {turn_number}: input={input_chars} chars (~{approx_tokens} tok, {context_pct}) request={request_ms} text={response_chars} chars"
    )];

    if let Some(skills) = turn.get("loaded_skills").and_then(Value::as_array)
        && !skills.is_empty()
    {
        parts.push(format!("skills=[{}]", format_string_array(skills)));
    }
    if let Some(tools) = turn.get("tool_calls").and_then(Value::as_array)
        && !tools.is_empty()
    {
        parts.push(format!("calls=[{}]", format_tool_calls(tools)));
    }
    if let Some(results) = turn.get("tool_results").and_then(Value::as_array)
        && !results.is_empty()
    {
        parts.push(format!("results=[{}]", format_tool_results(results)));
    }
    if let Some(compactions) = turn.get("compactions").and_then(Value::as_array)
        && !compactions.is_empty()
    {
        parts.push(format!("compactions=[{}]", format_compactions(compactions)));
    }
    if let Some(errors) = turn.get("errors").and_then(Value::as_array)
        && !errors.is_empty()
    {
        parts.push(format!("errors=[{}]", format_errors(errors)));
    }

    parts.join(" ")
}

fn format_string_array(values: &[Value]) -> String {
    values
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_tool_calls(tools: &[Value]) -> String {
    tools
        .iter()
        .map(|tool| {
            tool.get("tool")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_tool_results(results: &[Value]) -> String {
    results
        .iter()
        .map(|result| {
            let tool = result
                .get("tool")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let ok = result
                .get("ok")
                .and_then(Value::as_bool)
                .map(|ok| if ok { "ok" } else { "fail" })
                .unwrap_or("?");
            let duration_ms = result
                .get("duration_ms")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let output_chars = result
                .get("output_chars")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let mut suffix = Vec::new();
            if result
                .get("cached_observation")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                suffix.push("cached");
            }
            if result
                .get("truncated")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                suffix.push("truncated");
            }
            if result
                .get("timed_out")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                suffix.push("timeout");
            }
            if let Some(error_kind) = result.get("error_kind").and_then(Value::as_str) {
                suffix.push(error_kind);
            }
            let parent_suffix = result
                .get("created_parent_dirs")
                .and_then(Value::as_array)
                .filter(|dirs| !dirs.is_empty())
                .map(|dirs| {
                    let dirs = dirs
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("|");
                    format!(" parents={dirs}")
                })
                .unwrap_or_default();
            let suffix = if suffix.is_empty() {
                String::new()
            } else {
                format!(" {}", suffix.join("+"))
            };
            format!("{tool}:{ok} {duration_ms}ms {output_chars} chars{suffix}{parent_suffix}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_compactions(compactions: &[Value]) -> String {
    compactions
        .iter()
        .map(|compaction| {
            let method = compaction
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let before = compaction
                .get("before_chars")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string());
            let after = compaction
                .get("after_chars")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string());
            let mut parts = vec![format!("{method} {before}->{after}")];
            if let Some(trigger) = compaction.get("trigger").and_then(Value::as_str) {
                parts.push(format!("trigger={trigger}"));
            }
            if let Some(remote_after) = compaction.get("remote_after_chars").and_then(Value::as_u64)
            {
                let remote_pct = compaction
                    .get("remote_retained_pct")
                    .and_then(Value::as_f64)
                    .map(|pct| format!(" {pct:.1}%"))
                    .unwrap_or_default();
                parts.push(format!("remote={remote_after}{remote_pct}"));
            }
            if let (Some(remote_after), Some(final_chars)) = (
                compaction.get("remote_after_chars").and_then(Value::as_u64),
                compaction
                    .get("local_pressure_final_chars")
                    .and_then(Value::as_u64),
            ) {
                parts.push(format!("local_pressure={remote_after}->{final_chars}"));
            }
            parts.join(" ")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_errors(errors: &[Value]) -> String {
    errors
        .iter()
        .map(|error| {
            let stage = error
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let message = error
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            format!("{stage}:{}", truncate_for_line(message, 80))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn compact_json_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

fn truncate_for_line(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut truncated = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}
