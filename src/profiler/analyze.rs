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
#[path = "analyze/diagnostics.rs"]
mod diagnostics;
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
use diagnostics::{AnalysisReports, insert_analysis_reports};
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
    push_timeline_array, timeline_turn,
};
pub(super) use trace_utils::{summarize_compaction_report, trace_file_sort_key};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RequiredAction {
    tool: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    alternate_tools: Vec<String>,
    path: Option<String>,
    from: Option<String>,
    to: Option<String>,
    recursive: Option<bool>,
    command: Option<String>,
    ok: Option<bool>,
}

#[derive(Debug, Clone)]
struct ObservedToolCall {
    turn: usize,
    call_id: Option<String>,
    tool_name: String,
    args: Value,
}

#[derive(Debug, Clone)]
struct ObservedToolResult {
    turn: usize,
    call_id: Option<String>,
    tool_name: String,
    args: Value,
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
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("json")
        {
            files.push(path);
        }
    }
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
            for call in function_calls_from_trace_response(&value) {
                let tool_name = call.tool_name;
                let args = call.args;
                profiler.record_tool_call(turn, &tool_name, &args);
                observed_tool_calls.push(ObservedToolCall {
                    turn,
                    call_id: call.call_id,
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
                    call_id: result.call_id,
                    tool_name: result.tool_name.clone(),
                    args: result.args.clone(),
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
            .then_with(|| left.command.cmp(&right.command))
            .then_with(|| left.ok.cmp(&right.ok))
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
        &observed_tool_results,
        &timeline,
    );
    let scenario_skill_expectation_report =
        scenario_skill_expectation_report(trace_metadata.as_ref(), &loaded_skill_contexts);
    let tool_only_turn_report = tool_only_turn_report(&timeline);
    let compaction_regrowth_report = compaction_regrowth_report(&timeline);
    let tool_failure_recovery_report = tool_failure_recovery_report(&observed_tool_results);
    if let Some(object) = summary.as_object_mut() {
        insert_analysis_reports(
            object,
            AnalysisReports {
                timeline,
                trace_metadata,
                embedded_profile_summary,
                required_action_report: &required_action_report,
                loaded_skill_contexts: &loaded_skill_contexts,
                tool_only_turn_report: &tool_only_turn_report,
                compaction_regrowth_report: &compaction_regrowth_report,
                scenario_tool_expectation_report: &scenario_tool_expectation_report,
                scenario_call_expectation_report: &scenario_call_expectation_report,
                scenario_skill_expectation_report: &scenario_skill_expectation_report,
                tool_failure_recovery_report: &tool_failure_recovery_report,
            },
        );
    }
    Ok(summary)
}
