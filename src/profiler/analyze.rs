use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use serde::Serialize;
use serde_json::{Map, Value, json};

use super::{
    AgentProfiler, approx_token_count_from_chars, context_window_pct, created_parent_dirs,
    tool_result_is_truncated, tool_result_timed_out, tool_signature,
};

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

#[derive(Debug)]
struct RequiredActionReport {
    actions: Vec<RequiredAction>,
    executed: Vec<RequiredAction>,
    missing: Vec<RequiredAction>,
    calls_before_first_required_action: usize,
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

fn timeline_turn(
    timeline: &mut BTreeMap<usize, Map<String, Value>>,
    turn: usize,
) -> &mut Map<String, Value> {
    timeline.entry(turn).or_insert_with(|| {
        let mut entry = Map::new();
        entry.insert("turn".to_string(), json!(turn));
        entry
    })
}

fn push_timeline_array(
    timeline: &mut BTreeMap<usize, Map<String, Value>>,
    turn: usize,
    key: &str,
    value: Value,
) {
    let entry = timeline_turn(timeline, turn);
    match entry.get_mut(key) {
        Some(Value::Array(items)) => items.push(value),
        _ => {
            entry.insert(key.to_string(), Value::Array(vec![value]));
        }
    }
}

fn is_tool_result_trace_file(name: &str) -> bool {
    name.ends_with("-tool-result.json") || name.contains("-tool-result-")
}

pub(super) fn trace_file_sort_key(path: &Path) -> (usize, String, usize, String) {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let stem = name.strip_suffix(".json").unwrap_or(name);
    let (turn, rest) = stem
        .split_once('-')
        .and_then(|(turn, rest)| turn.parse::<usize>().ok().map(|turn| (turn, rest)))
        .unwrap_or((usize::MAX, stem));
    let (kind, sequence) = rest
        .rsplit_once('-')
        .and_then(|(kind, suffix)| {
            suffix
                .parse::<usize>()
                .ok()
                .map(|sequence| (kind, sequence))
        })
        .unwrap_or((rest, 1));
    (turn, kind.to_string(), sequence, name.to_string())
}

pub(super) fn summarize_compaction_report(report: &Value) -> Value {
    let mut summary = Map::new();
    copy_field(report, &mut summary, "method");
    copy_field(report, &mut summary, "forced");
    copy_field(report, &mut summary, "trigger");
    copy_field(report, &mut summary, "duration_ms");
    copy_field(report, &mut summary, "before_chars");
    copy_field(report, &mut summary, "compact_request_chars");
    copy_field(report, &mut summary, "after_chars");
    copy_field(report, &mut summary, "threshold_chars");
    copy_field(report, &mut summary, "compacted_outputs");
    copy_field(report, &mut summary, "compacted_tool_outputs");
    copy_field(report, &mut summary, "compacted_messages");
    copy_field(report, &mut summary, "remote_error");
    copy_field(report, &mut summary, "fallback");
    copy_field(report, &mut summary, "local_pressure");
    add_compaction_retention_metrics(report, &mut summary);

    if let Some(raw) = report.get("raw") {
        let mut raw_summary = Map::new();
        copy_field(raw, &mut raw_summary, "object");
        copy_field(raw, &mut raw_summary, "id");
        copy_field(raw, &mut raw_summary, "created_at");
        copy_field(raw, &mut raw_summary, "usage");
        if let Some(output) = raw.get("output").and_then(Value::as_array) {
            raw_summary.insert("output_items".to_string(), json!(output.len()));
            raw_summary.insert(
                "output_types".to_string(),
                Value::Array(
                    output
                        .iter()
                        .filter_map(|item| item.get("type").and_then(Value::as_str))
                        .map(|kind| Value::String(kind.to_string()))
                        .collect(),
                ),
            );
        }
        if !raw_summary.is_empty() {
            summary.insert("raw_summary".to_string(), Value::Object(raw_summary));
        }
    }

    Value::Object(summary)
}

fn add_compaction_retention_metrics(report: &Value, summary: &mut Map<String, Value>) {
    let before = report.get("before_chars").and_then(Value::as_u64);
    let after = report.get("after_chars").and_then(Value::as_u64);
    if let (Some(before), Some(after)) = (before, after)
        && before > 0
    {
        summary.insert(
            "final_retained_pct".to_string(),
            json!(percent(after, before)),
        );
    }

    let remote_after = report
        .pointer("/local_pressure/remote_after_chars")
        .and_then(Value::as_u64);
    if let Some(remote_after) = remote_after {
        summary.insert("remote_after_chars".to_string(), json!(remote_after));
        if let Some(before) = before
            && before > 0
        {
            summary.insert(
                "remote_retained_pct".to_string(),
                json!(percent(remote_after, before)),
            );
        }
    }

    let final_chars = report
        .pointer("/local_pressure/final_chars")
        .and_then(Value::as_u64);
    if let Some(final_chars) = final_chars {
        summary.insert("local_pressure_final_chars".to_string(), json!(final_chars));
    }

    if let (Some(remote_after), Some(final_chars)) = (remote_after, final_chars)
        && remote_after > 0
    {
        let reduced = remote_after.saturating_sub(final_chars);
        summary.insert(
            "local_pressure_reduction_pct".to_string(),
            json!(percent(reduced, remote_after)),
        );
    }
}

fn percent(part: u64, whole: u64) -> f64 {
    (part as f64 / whole as f64) * 100.0
}

fn sanitize_profile_summary(mut summary: Value) -> Value {
    let Some(object) = summary.as_object_mut() else {
        return summary;
    };
    if let Some(reports) = object
        .get_mut("compaction_reports")
        .and_then(Value::as_array_mut)
    {
        for report in reports {
            *report = summarize_compaction_report(report);
        }
    }
    summary
}

fn copy_field(source: &Value, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key) {
        target.insert(key.to_string(), value.clone());
    }
}

fn is_profile_summary_trace_file(name: &str) -> bool {
    name.ends_with("-profile-summary.json") || name.contains("-profile-summary-")
}

fn profile_summary_rank(name: &str, turn: usize) -> usize {
    let Some(stem) = name.strip_suffix(".json") else {
        return turn.saturating_mul(10_000);
    };
    let duplicate = stem
        .split_once("-profile-summary")
        .and_then(|(_, suffix)| suffix.strip_prefix('-'))
        .and_then(|suffix| suffix.parse::<usize>().ok())
        .unwrap_or(1);
    turn.saturating_mul(10_000).saturating_add(duplicate)
}

struct TraceToolResult {
    tool_name: String,
    args: Value,
    ok: bool,
    data: Value,
    output_chars: usize,
    duration_ms: u64,
    error: Option<String>,
    cached_observation: bool,
}

fn tool_result_from_trace(value: &Value) -> Result<Option<TraceToolResult>> {
    let Some(tool_name) = value.get("tool").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(result) = value.get("result") else {
        return Ok(None);
    };
    let args = value.get("args").cloned().unwrap_or_else(|| json!({}));
    let ok = result.get("ok").and_then(Value::as_bool).unwrap_or(false);
    let data = result.get("data").cloned().unwrap_or_else(|| json!({}));
    let duration_ms = value
        .get("duration_ms")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_chars = serde_json::to_string(result)?.len();
    let error = result
        .get("error")
        .and_then(Value::as_str)
        .map(str::to_string);
    let cached_observation = result
        .pointer("/data/cached_observation")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(Some(TraceToolResult {
        tool_name: tool_name.to_string(),
        args,
        ok,
        data,
        output_chars,
        duration_ms,
        error,
        cached_observation,
    }))
}

fn required_actions_from_request_input(value: &Value) -> Vec<RequiredAction> {
    let mut actions = Vec::new();
    for text in request_input_texts(value) {
        for line in text.lines() {
            let line = line.trim();
            if let Some(action) = line
                .strip_prefix("action_")
                .and_then(|line| line.split_once('='))
                .and_then(|(_, action)| parse_required_action(action))
            {
                actions.push(action);
            }
        }
    }
    actions.sort_by(|left, right| {
        left.tool
            .cmp(&right.tool)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.recursive.cmp(&right.recursive))
    });
    actions.dedup();
    actions
}

fn request_input_texts(value: &Value) -> Vec<String> {
    value
        .get("input")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

fn parse_required_action(raw: &str) -> Option<RequiredAction> {
    let mut tool = None;
    let mut path = None;
    let mut from = None;
    let mut to = None;
    let mut recursive = None;
    for part in raw.split_whitespace() {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key {
            "tool" => tool = Some(value.to_string()),
            "path" => path = Some(value.trim_matches('`').to_string()),
            "from" => from = Some(value.trim_matches('`').to_string()),
            "to" => to = Some(value.trim_matches('`').to_string()),
            "recursive" => match value {
                "true" => recursive = Some(true),
                "false" => recursive = Some(false),
                _ => {}
            },
            _ => {}
        }
    }
    Some(RequiredAction {
        tool: tool?,
        path,
        from,
        to,
        recursive,
    })
}

fn required_action_report(
    actions: &[RequiredAction],
    calls: &[ObservedToolCall],
) -> RequiredActionReport {
    let mut executed = Vec::new();
    let mut missing = Vec::new();
    let mut first_required_call_index = None;
    for action in actions {
        if let Some(index) = calls
            .iter()
            .position(|call| required_action_matches_call(action, call))
        {
            executed.push(action.clone());
            first_required_call_index =
                Some(first_required_call_index.map_or(index, |current: usize| current.min(index)));
        } else {
            missing.push(action.clone());
        }
    }
    RequiredActionReport {
        actions: actions.to_vec(),
        executed,
        missing,
        calls_before_first_required_action: first_required_call_index.unwrap_or(0),
    }
}

fn loaded_skill_contexts_from_request_input(value: &Value) -> Vec<String> {
    let mut skills = BTreeSet::<String>::new();
    collect_loaded_skill_contexts(value, &mut skills);
    skills.into_iter().collect()
}

fn collect_loaded_skill_contexts(value: &Value, skills: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => {
            let mut rest = text.as_str();
            while let Some((_, after_prefix)) = rest.split_once("[spark skill loaded: ") {
                let Some((name, after_name)) = after_prefix.split_once(']') else {
                    break;
                };
                let name = name.trim();
                if !name.is_empty() {
                    skills.insert(name.to_string());
                }
                rest = after_name;
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_loaded_skill_contexts(item, skills);
            }
        }
        Value::Object(object) => {
            for item in object.values() {
                collect_loaded_skill_contexts(item, skills);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn scenario_skill_expectation_report(
    metadata: Option<&Value>,
    loaded_skills: &BTreeSet<String>,
) -> Option<Value> {
    let expected_skills = metadata?
        .pointer("/context/profile_scenario/expected_skills")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if expected_skills.is_empty() {
        return None;
    }

    let mut satisfied = Vec::new();
    let mut missing = Vec::new();
    for skill in &expected_skills {
        if loaded_skills.contains(skill) {
            satisfied.push(skill.clone());
        } else {
            missing.push(skill.clone());
        }
    }

    Some(json!({
        "expected_skills": expected_skills,
        "loaded_skills": loaded_skills.iter().collect::<Vec<_>>(),
        "total_skills": satisfied.len() + missing.len(),
        "satisfied_skills": satisfied.len(),
        "missing_skills": missing,
        "satisfied_skill_contexts": satisfied,
    }))
}

fn tool_only_turn_report(timeline: &BTreeMap<usize, Map<String, Value>>) -> Value {
    let mut turns = Vec::new();
    let mut max_consecutive = 0usize;
    let mut current_consecutive = 0usize;

    for (turn, entry) in timeline {
        let has_tool_calls = entry
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| !calls.is_empty());
        let response_text_chars = entry
            .get("response_text_chars")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if has_tool_calls && response_text_chars == 0 {
            turns.push(*turn);
            current_consecutive += 1;
            max_consecutive = max_consecutive.max(current_consecutive);
        } else {
            current_consecutive = 0;
        }
    }

    json!({
        "count": turns.len(),
        "max_consecutive": max_consecutive,
        "turns": turns,
    })
}

fn scenario_tool_expectation_report(
    metadata: Option<&Value>,
    calls: &[ObservedToolCall],
) -> Option<Value> {
    let groups = metadata?
        .pointer("/context/profile_scenario/expected_tool_groups")?
        .as_array()?
        .iter()
        .filter_map(|group| {
            let tools = group
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>();
            (!tools.is_empty()).then_some(tools)
        })
        .collect::<Vec<_>>();
    if groups.is_empty() {
        return None;
    }

    let mut satisfied = Vec::new();
    let mut missing = Vec::new();
    for group in &groups {
        if calls
            .iter()
            .any(|call| group.iter().any(|tool| tool == &call.tool_name))
        {
            satisfied.push(group.clone());
        } else {
            missing.push(group.clone());
        }
    }

    Some(json!({
        "expected_tool_groups": groups,
        "total_groups": satisfied.len() + missing.len(),
        "satisfied_groups": satisfied.len(),
        "missing_groups": missing,
        "satisfied_tool_groups": satisfied,
    }))
}

fn scenario_tool_call_expectation_report(
    metadata: Option<&Value>,
    calls: &[ObservedToolCall],
    timeline: &BTreeMap<usize, Map<String, Value>>,
) -> Option<Value> {
    let expected_calls = metadata?
        .pointer("/context/profile_scenario/expected_tool_calls")?
        .as_array()?
        .iter()
        .filter_map(required_action_from_value)
        .collect::<Vec<_>>();
    if expected_calls.is_empty() {
        return None;
    }

    let mut satisfied = Vec::new();
    let mut missing = Vec::new();
    for expected in &expected_calls {
        if calls
            .iter()
            .any(|call| required_action_matches_call(expected, call))
        {
            satisfied.push(expected.clone());
        } else {
            missing.push(expected.clone());
        }
    }
    let first_satisfied_call_index = if missing.is_empty() {
        (0..calls.len()).find(|index| {
            expected_calls.iter().all(|expected| {
                calls
                    .iter()
                    .take(index + 1)
                    .any(|call| required_action_matches_call(expected, call))
            })
        })
    } else {
        None
    };
    let extra_calls_after_satisfied = first_satisfied_call_index
        .map(|index| calls.len().saturating_sub(index + 1))
        .unwrap_or(0);
    let first_satisfied_turn = first_satisfied_call_index.map(|index| calls[index].turn);
    let final_tool_call_turn = calls.last().map(|call| call.turn);
    let extra_turns_after_satisfied = match (first_satisfied_turn, final_tool_call_turn) {
        (Some(first), Some(final_turn)) => final_turn.saturating_sub(first),
        _ => 0,
    };
    let input_chars_at_satisfaction =
        first_satisfied_turn.and_then(|turn| request_input_chars_for_turn(timeline, turn));
    let final_request_input_chars = latest_request_input_chars(timeline);
    let context_growth_after_satisfied_chars =
        match (input_chars_at_satisfaction, final_request_input_chars) {
            (Some(first), Some(final_chars)) => final_chars.saturating_sub(first),
            _ => 0,
        };
    let extra_calls = first_satisfied_call_index
        .map(|index| {
            calls
                .iter()
                .skip(index + 1)
                .map(|call| {
                    json!({
                        "turn": call.turn,
                        "tool": &call.tool_name,
                        "signature": tool_signature(&call.tool_name, &call.args),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(json!({
        "expected_calls": expected_calls,
        "total_calls": satisfied.len() + missing.len(),
        "satisfied_calls": satisfied.len(),
        "missing_calls": missing,
        "satisfied_tool_calls": satisfied,
        "first_satisfied_call_index": first_satisfied_call_index,
        "first_satisfied_turn": first_satisfied_turn,
        "final_tool_call_turn": final_tool_call_turn,
        "extra_calls_after_satisfied": extra_calls_after_satisfied,
        "extra_turns_after_satisfied": extra_turns_after_satisfied,
        "input_chars_at_satisfaction": input_chars_at_satisfaction,
        "final_request_input_chars": final_request_input_chars,
        "context_growth_after_satisfied_chars": context_growth_after_satisfied_chars,
        "extra_tool_calls": extra_calls,
    }))
}

fn request_input_chars_for_turn(
    timeline: &BTreeMap<usize, Map<String, Value>>,
    turn: usize,
) -> Option<u64> {
    timeline
        .get(&turn)?
        .get("request_input_chars")
        .and_then(Value::as_u64)
}

fn latest_request_input_chars(timeline: &BTreeMap<usize, Map<String, Value>>) -> Option<u64> {
    timeline
        .values()
        .rev()
        .find_map(|entry| entry.get("request_input_chars").and_then(Value::as_u64))
}

fn tool_failure_recovery_report(results: &[ObservedToolResult]) -> Option<Value> {
    let failures = results
        .iter()
        .enumerate()
        .filter(|(_, result)| !result.ok)
        .collect::<Vec<_>>();
    if failures.is_empty() {
        return None;
    }

    let mut recovered = Vec::new();
    let mut unrecovered = Vec::new();
    let mut by_tool = BTreeMap::<String, (usize, usize)>::new();
    for (index, failure) in failures {
        let entry = by_tool.entry(failure.tool_name.clone()).or_default();
        entry.1 += 1;
        let recovery = results
            .iter()
            .skip(index + 1)
            .find(|candidate| candidate.ok && candidate.tool_name == failure.tool_name);
        let record = json!({
            "turn": failure.turn,
            "tool": failure.tool_name,
        });
        if let Some(recovery) = recovery {
            entry.0 += 1;
            recovered.push(json!({
                "turn": failure.turn,
                "tool": failure.tool_name,
                "recovered_at_turn": recovery.turn,
            }));
        } else {
            unrecovered.push(record);
        }
    }

    let by_tool = by_tool
        .into_iter()
        .map(|(tool, (recovered, failed))| {
            (
                tool,
                json!({
                    "recovered": recovered,
                    "failed": failed,
                    "unrecovered": failed.saturating_sub(recovered),
                }),
            )
        })
        .collect::<Map<_, _>>();

    Some(json!({
        "failed_tool_results": recovered.len() + unrecovered.len(),
        "recovered_failures": recovered.len(),
        "unrecovered_failures": unrecovered.len(),
        "recovered": recovered,
        "unrecovered": unrecovered,
        "by_tool": by_tool,
    }))
}

fn required_action_from_value(value: &Value) -> Option<RequiredAction> {
    let tool = value.get("tool").and_then(Value::as_str)?.to_string();
    Some(RequiredAction {
        tool,
        path: value
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_string),
        from: value
            .get("from")
            .and_then(Value::as_str)
            .map(str::to_string),
        to: value.get("to").and_then(Value::as_str).map(str::to_string),
        recursive: value.get("recursive").and_then(Value::as_bool),
    })
}

fn required_action_matches_call(action: &RequiredAction, call: &ObservedToolCall) -> bool {
    if action.tool != call.tool_name {
        return false;
    }
    if let Some(path) = &action.path
        && call.args.get("path").and_then(Value::as_str) != Some(path.as_str())
    {
        return false;
    }
    if let Some(from) = &action.from
        && call.args.get("from").and_then(Value::as_str) != Some(from.as_str())
    {
        return false;
    }
    if let Some(to) = &action.to
        && call.args.get("to").and_then(Value::as_str) != Some(to.as_str())
    {
        return false;
    }
    if let Some(recursive) = action.recursive
        && call.args.get("recursive").and_then(Value::as_bool) != Some(recursive)
    {
        return false;
    }
    true
}

fn function_calls_from_trace_response(value: &Value) -> Vec<(String, Value)> {
    output_items_from_trace_response(value)
        .into_iter()
        .filter_map(|item| {
            if item.get("type").and_then(Value::as_str) != Some("function_call") {
                return None;
            }
            let name = wire_tool_name_to_local(item.get("name").and_then(Value::as_str)?);
            let args = match item.get("arguments") {
                Some(Value::String(raw)) => {
                    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.clone()))
                }
                Some(value) => value.clone(),
                None => Value::Object(Default::default()),
            };
            Some((name, args))
        })
        .collect()
}

fn wire_tool_name_to_local(name: &str) -> String {
    match name {
        "fs_read" => "fs.read",
        "fs_list" => "fs.list",
        "fs_stat" => "fs.stat",
        "fs_write" => "fs.write",
        "fs_search" => "fs.search",
        "fs_replace" => "fs.replace",
        "fs_edit" => "fs.edit",
        "fs_rename" => "fs.rename",
        "cmd_exec" => "cmd.exec",
        other => other,
    }
    .to_string()
}

fn output_items_from_trace_response(value: &Value) -> Vec<Value> {
    let response_value = value.get("raw").unwrap_or(value);
    if let Some(items) = response_value
        .get("response")
        .and_then(|response| response.get("output"))
        .and_then(Value::as_array)
        && !items.is_empty()
    {
        return items.clone();
    }

    let mut indexed = response_value
        .get("events")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|event| {
            event.get("type").and_then(Value::as_str) == Some("response.output_item.done")
        })
        .filter_map(|event| {
            let index = event
                .get("output_index")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            Some((index, event.get("item")?.clone()))
        })
        .collect::<Vec<_>>();
    indexed.sort_by_key(|(index, _)| *index);
    indexed.into_iter().map(|(_, item)| item).collect()
}

fn response_text_from_trace_response(value: &Value) -> Option<String> {
    let text = output_items_from_trace_response(value)
        .into_iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter_map(|item| item.get("content").and_then(Value::as_array).cloned())
        .flatten()
        .filter_map(|part| part.get("text").and_then(Value::as_str).map(str::to_string))
        .collect::<String>();
    (!text.is_empty()).then_some(text)
}
