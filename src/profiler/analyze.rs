use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use serde::Serialize;
use serde_json::{Map, Value, json};

use super::{
    AgentProfiler, approx_token_count_from_chars, context_window_pct, created_parent_dirs,
    tool_result_is_truncated, tool_result_timed_out, tool_signature,
};

#[path = "analyze/actions.rs"]
mod actions;
#[path = "analyze/expectations.rs"]
mod expectations;
#[path = "analyze/trace_io.rs"]
mod trace_io;
#[path = "analyze/trace_utils.rs"]
mod trace_utils;

use actions::{
    loaded_skill_contexts_from_request_input, required_action_report,
    required_actions_from_request_input,
};
use expectations::{
    compaction_regrowth_report, scenario_skill_expectation_report,
    scenario_tool_call_expectation_report, scenario_tool_expectation_report,
    tool_failure_recovery_report, tool_only_turn_report,
};
use trace_io::{
    function_calls_from_trace_response, response_text_from_trace_response, tool_result_from_trace,
};
use trace_utils::{
    is_profile_summary_trace_file, is_tool_result_trace_file, profile_summary_rank,
    push_timeline_array, sanitize_profile_summary, timeline_turn,
};
pub(super) use trace_utils::{summarize_compaction_report, trace_file_sort_key};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RequiredAction {
    tool: String,
    path: Option<String>,
    from: Option<String>,
    to: Option<String>,
    recursive: Option<bool>,
}

#[derive(Debug, Clone)]
struct ObservedToolCall {
    turn: usize,
    tool_name: String,
    args: Value,
}

#[derive(Debug, Clone)]
struct ObservedToolResult {
    turn: usize,
    tool_name: String,
    ok: bool,
}

pub fn analyze_trace(dir: &Path) -> Result<Value> {
    let mut profiler = AgentProfiler::default();
    let mut embedded_profile_summary = None;
    let mut embedded_profile_summary_rank = 0usize;
    let mut trace_metadata = None;
    let mut timeline = BTreeMap::<usize, Map<String, Value>>::new();
    let mut retained_required_actions = Vec::<RequiredAction>::new();
    let mut observed_tool_calls = Vec::<ObservedToolCall>::new();
    let mut observed_tool_results = Vec::<ObservedToolResult>::new();
    let mut loaded_skill_contexts = BTreeSet::<String>::new();
    let mut files = std::fs::read_dir(dir)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    files.sort_by(|left, right| trace_file_sort_key(left).cmp(&trace_file_sort_key(right)));

    for path in files {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let raw = std::fs::read_to_string(&path)?;
        let value = serde_json::from_str::<Value>(&raw)?;
        let turn = name
            .split_once('-')
            .and_then(|(prefix, _)| prefix.parse::<usize>().ok())
            .unwrap_or(0);

        if name == "000-trace-metadata.json" {
            trace_metadata = Some(value);
        } else if is_profile_summary_trace_file(name) {
            let rank = profile_summary_rank(name, turn);
            if rank >= embedded_profile_summary_rank {
                embedded_profile_summary_rank = rank;
                embedded_profile_summary = Some(value);
            }
        } else if name.ends_with("-request-input.json") {
            let input_chars = value
                .get("input")
                .map(serde_json::to_string)
                .transpose()?
                .map(|input| input.len())
                .unwrap_or(raw.len());
            profiler.record_request(input_chars);
            let turn_entry = timeline_turn(&mut timeline, turn);
            turn_entry.insert("request_input_chars".to_string(), json!(input_chars));
            turn_entry.insert(
                "request_approx_tokens".to_string(),
                json!(approx_token_count_from_chars(input_chars)),
            );
            turn_entry.insert(
                "context_window_pct".to_string(),
                json!(context_window_pct(input_chars)),
            );
            retained_required_actions.extend(required_actions_from_request_input(&value));
            let turn_loaded_skills = loaded_skill_contexts_from_request_input(&value);
            if !turn_loaded_skills.is_empty() {
                loaded_skill_contexts.extend(turn_loaded_skills.iter().cloned());
                turn_entry.insert("loaded_skills".to_string(), json!(turn_loaded_skills));
            }
        } else if name.ends_with("-response.json") {
            if let Some(duration_ms) = value.get("duration_ms").and_then(Value::as_u64) {
                profiler.record_request_duration(turn, duration_ms);
                timeline_turn(&mut timeline, turn)
                    .insert("request_duration_ms".to_string(), json!(duration_ms));
            }
            for (tool_name, args) in function_calls_from_trace_response(&value) {
                profiler.record_tool_call(turn, &tool_name, &args);
                observed_tool_calls.push(ObservedToolCall {
                    turn,
                    tool_name: tool_name.clone(),
                    args: args.clone(),
                });
                push_timeline_array(
                    &mut timeline,
                    turn,
                    "tool_calls",
                    json!({
                        "tool": tool_name,
                        "signature": tool_signature(&tool_name, &args),
                    }),
                );
            }
            if let Some(text) = response_text_from_trace_response(&value) {
                profiler.record_response_text(&text);
                timeline_turn(&mut timeline, turn)
                    .insert("response_text_chars".to_string(), json!(text.len()));
            }
        } else if is_tool_result_trace_file(name) {
            if let Some(result) = tool_result_from_trace(&value)? {
                observed_tool_results.push(ObservedToolResult {
                    turn,
                    tool_name: result.tool_name.clone(),
                    ok: result.ok,
                });
                profiler.record_tool_result(
                    turn,
                    &result.tool_name,
                    result.ok,
                    &result.data,
                    result.output_chars,
                    result.duration_ms,
                    result.error.as_deref(),
                );
                if result.cached_observation {
                    profiler.record_readonly_tool_cache_hit(turn, &result.tool_name, &result.args);
                }
                push_timeline_array(
                    &mut timeline,
                    turn,
                    "tool_results",
                    json!({
                        "tool": result.tool_name,
                        "ok": result.ok,
                        "duration_ms": result.duration_ms,
                        "output_chars": result.output_chars,
                        "error": result.error,
                        "error_kind": result.data.get("error_kind").cloned().unwrap_or(Value::Null),
                        "cached_observation": result.cached_observation,
                        "truncated": tool_result_is_truncated(&result.data),
                        "timed_out": tool_result_timed_out(&result.data),
                        "created_parent_dirs": created_parent_dirs(&result.data).unwrap_or_default(),
                    }),
                );
            }
        } else if name.ends_with("-compaction.json") {
            profiler.record_compaction(&value);
            push_timeline_array(
                &mut timeline,
                turn,
                "compactions",
                summarize_compaction_report(&value),
            );
        } else if name.ends_with("-error.json") {
            let stage = value
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let error = value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            profiler.record_error(turn, stage, error);
            push_timeline_array(
                &mut timeline,
                turn,
                "errors",
                json!({
                    "stage": stage,
                    "error": error,
                }),
            );
        }
    }

    retained_required_actions.sort_by(|left, right| {
        left.tool
            .cmp(&right.tool)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.from.cmp(&right.from))
            .then_with(|| left.to.cmp(&right.to))
            .then_with(|| left.recursive.cmp(&right.recursive))
    });
    retained_required_actions.dedup();

    let mut summary = profiler.to_json();
    let required_action_report =
        required_action_report(&retained_required_actions, &observed_tool_calls);
    let scenario_tool_expectation_report =
        scenario_tool_expectation_report(trace_metadata.as_ref(), &observed_tool_calls);
    let scenario_call_expectation_report = scenario_tool_call_expectation_report(
        trace_metadata.as_ref(),
        &observed_tool_calls,
        &timeline,
    );
    let scenario_skill_expectation_report =
        scenario_skill_expectation_report(trace_metadata.as_ref(), &loaded_skill_contexts);
    let tool_only_turn_report = tool_only_turn_report(&timeline);
    let compaction_regrowth_report = compaction_regrowth_report(&timeline);
    let tool_failure_recovery_report = tool_failure_recovery_report(&observed_tool_results);
    if let Some(object) = summary.as_object_mut() {
        object.insert(
            "timeline".to_string(),
            Value::Array(timeline.into_values().map(Value::Object).collect()),
        );
        if let Some(metadata) = trace_metadata {
            object.insert("trace_metadata".to_string(), metadata);
        }
        if let Some(embedded) = embedded_profile_summary {
            object.insert(
                "embedded_profile_summary".to_string(),
                sanitize_profile_summary(embedded),
            );
        }
        object.insert(
            "retained_required_actions".to_string(),
            json!(&required_action_report.actions),
        );
        object.insert(
            "retained_required_actions_executed".to_string(),
            json!(&required_action_report.executed),
        );
        object.insert(
            "retained_required_actions_missing".to_string(),
            json!(&required_action_report.missing),
        );
        object.insert(
            "tool_calls_before_first_required_action".to_string(),
            json!(required_action_report.calls_before_first_required_action),
        );
        object.insert(
            "loaded_skill_contexts".to_string(),
            json!(loaded_skill_contexts.iter().collect::<Vec<_>>()),
        );
        object.insert("tool_only_turns".to_string(), tool_only_turn_report.clone());
        if compaction_regrowth_report
            .get("count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0)
        {
            object.insert(
                "compaction_regrowth".to_string(),
                compaction_regrowth_report.clone(),
            );
        }
        if let Some(report) = &scenario_tool_expectation_report {
            object.insert(
                "profile_scenario_tool_expectations".to_string(),
                report.clone(),
            );
        }
        if let Some(report) = &scenario_call_expectation_report {
            object.insert(
                "profile_scenario_call_expectations".to_string(),
                report.clone(),
            );
        }
        if let Some(report) = &scenario_skill_expectation_report {
            object.insert(
                "profile_scenario_skill_expectations".to_string(),
                report.clone(),
            );
        }
        if let Some(report) = &tool_failure_recovery_report {
            object.insert("tool_failure_recovery".to_string(), report.clone());
        }
        let response_text_chars = object
            .get("response_text_chars")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let compactions = object
            .get("compactions")
            .cloned()
            .unwrap_or_else(|| json!(0));
        let remote_compactions = object
            .get("remote_compactions")
            .cloned()
            .unwrap_or_else(|| json!(0));
        let fallback_compactions = object
            .get("fallback_compactions")
            .cloned()
            .unwrap_or_else(|| json!(0));
        if let Some(diagnostics) = object.get_mut("diagnostics").and_then(Value::as_array_mut) {
            if !required_action_report.missing.is_empty() {
                diagnostics.push(json!({
                    "level": "warning",
                    "kind": "retained_required_actions_missing",
                    "message": "One or more required actions retained by local compaction were not observed in the trace tool calls.",
                    "missing": &required_action_report.missing,
                }));
            }
            if required_action_report.calls_before_first_required_action > 0 {
                diagnostics.push(json!({
                    "level": "info",
                    "kind": "retained_required_action_detour",
                    "message": "Spark made tool calls before executing the first required action retained by local compaction.",
                    "calls_before_first_required_action": required_action_report.calls_before_first_required_action,
                }));
            }
            if let Some(report) = &scenario_tool_expectation_report
                && report
                    .get("missing_groups")
                    .and_then(Value::as_array)
                    .is_some_and(|missing| !missing.is_empty())
            {
                diagnostics.push(json!({
                    "level": "warning",
                    "kind": "profile_scenario_expected_tools_missing",
                    "message": "The trace did not include all native tool groups expected for this profiling scenario.",
                    "missing_groups": report.get("missing_groups").cloned().unwrap_or_else(|| json!([])),
                }));
            }
            if let Some(report) = &scenario_call_expectation_report
                && report
                    .get("missing_calls")
                    .and_then(Value::as_array)
                    .is_some_and(|missing| !missing.is_empty())
            {
                diagnostics.push(json!({
                    "level": "warning",
                    "kind": "profile_scenario_expected_calls_missing",
                    "message": "The trace did not include all exact native tool calls expected for this profiling scenario.",
                    "missing_calls": report.get("missing_calls").cloned().unwrap_or_else(|| json!([])),
                }));
            }
            if let Some(report) = &scenario_call_expectation_report
                && report
                    .get("extra_calls_after_satisfied")
                    .and_then(Value::as_u64)
                    .is_some_and(|count| count > 0)
            {
                diagnostics.push(json!({
                    "level": "info",
                    "kind": "profile_scenario_extra_calls_after_expected",
                    "message": "Spark satisfied all exact native tool calls expected for this profiling scenario, then made additional tool calls before completing.",
                    "extra_calls_after_satisfied": report.get("extra_calls_after_satisfied").cloned().unwrap_or_else(|| json!(0)),
                    "extra_turns_after_satisfied": report.get("extra_turns_after_satisfied").cloned().unwrap_or_else(|| json!(0)),
                    "context_growth_after_satisfied_chars": report.get("context_growth_after_satisfied_chars").cloned().unwrap_or_else(|| json!(0)),
                    "first_satisfied_call_index": report.get("first_satisfied_call_index").cloned().unwrap_or(Value::Null),
                    "first_satisfied_turn": report.get("first_satisfied_turn").cloned().unwrap_or(Value::Null),
                }));
            }
            if let Some(report) = &scenario_skill_expectation_report
                && report
                    .get("missing_skills")
                    .and_then(Value::as_array)
                    .is_some_and(|missing| !missing.is_empty())
            {
                diagnostics.push(json!({
                    "level": "warning",
                    "kind": "profile_scenario_expected_skills_missing",
                    "message": "The trace did not include all loaded skill contexts expected for this profiling scenario.",
                    "missing_skills": report.get("missing_skills").cloned().unwrap_or_else(|| json!([])),
                }));
            }
            if compaction_regrowth_report
                .get("max_next_request_growth_chars")
                .and_then(Value::as_u64)
                .is_some_and(|chars| chars >= 100_000)
            {
                diagnostics.push(json!({
                    "level": "info",
                    "kind": "post_compaction_context_regrowth",
                    "message": "Request input grew substantially after a compaction boundary. Compare the compaction_regrowth report with subsequent tool calls before tuning thresholds.",
                    "max_same_turn_growth_chars": compaction_regrowth_report.get("max_same_turn_growth_chars").cloned().unwrap_or_else(|| json!(0)),
                    "max_next_request_growth_chars": compaction_regrowth_report.get("max_next_request_growth_chars").cloned().unwrap_or_else(|| json!(0)),
                }));
            }
            if tool_only_turn_report
                .get("max_consecutive")
                .and_then(Value::as_u64)
                .is_some_and(|count| count >= 3)
            {
                diagnostics.push(json!({
                    "level": "info",
                    "kind": "tool_only_turn_streak",
                    "message": "Spark spent several consecutive turns calling tools without producing user-facing text. Compare this with scenario completion and context growth before changing harness defaults.",
                    "count": tool_only_turn_report.get("count").cloned().unwrap_or_else(|| json!(0)),
                    "max_consecutive": tool_only_turn_report.get("max_consecutive").cloned().unwrap_or_else(|| json!(0)),
                    "turns": tool_only_turn_report.get("turns").cloned().unwrap_or_else(|| json!([])),
                }));
            }
            if tool_only_turn_report
                .get("max_consecutive")
                .and_then(Value::as_u64)
                .is_some_and(|count| count >= 8)
                && response_text_chars == 0
                && !diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic["kind"] == "completion_starvation")
            {
                diagnostics.push(json!({
                    "level": "warning",
                    "kind": "completion_starvation",
                    "message": "Spark kept calling tools across many turns without emitting any user-facing response text. Profile tool-call sequence, compaction timing, and context growth before adding stop conditions or changing defaults.",
                    "tool_only_turns": tool_only_turn_report.get("count").cloned().unwrap_or_else(|| json!(0)),
                    "max_consecutive": tool_only_turn_report.get("max_consecutive").cloned().unwrap_or_else(|| json!(0)),
                    "compactions": compactions,
                    "remote_compactions": remote_compactions,
                    "fallback_compactions": fallback_compactions,
                }));
            }
            if let Some(report) = &tool_failure_recovery_report {
                if report
                    .get("recovered_failures")
                    .and_then(Value::as_u64)
                    .is_some_and(|count| count > 0)
                {
                    diagnostics.push(json!({
                        "level": "info",
                        "kind": "tool_failure_recovered",
                        "message": "Spark recovered from one or more failed native tool observations later in the trace.",
                        "recovered_failures": report.get("recovered_failures").cloned().unwrap_or_else(|| json!(0)),
                        "failed_tool_results": report.get("failed_tool_results").cloned().unwrap_or_else(|| json!(0)),
                    }));
                }
                if report
                    .get("unrecovered_failures")
                    .and_then(Value::as_u64)
                    .is_some_and(|count| count > 0)
                {
                    diagnostics.push(json!({
                        "level": "warning",
                        "kind": "tool_failure_unrecovered",
                        "message": "One or more failed native tool observations had no later successful observation from the same tool.",
                        "unrecovered_failures": report.get("unrecovered_failures").cloned().unwrap_or_else(|| json!(0)),
                        "failed_tool_results": report.get("failed_tool_results").cloned().unwrap_or_else(|| json!(0)),
                    }));
                }
            }
        }
    }
    Ok(summary)
}
