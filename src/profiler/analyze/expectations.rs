use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::actions::required_action_from_value;
use super::actions::required_action_matches_call;
use super::{ObservedToolCall, ObservedToolResult};
use crate::profiler::tool_signature;

pub(super) fn scenario_skill_expectation_report(
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

pub(super) fn tool_only_turn_report(timeline: &BTreeMap<usize, Map<String, Value>>) -> Value {
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

pub(super) fn compaction_regrowth_report(timeline: &BTreeMap<usize, Map<String, Value>>) -> Value {
    let mut events = Vec::new();
    let mut max_same_turn_growth = 0u64;
    let mut max_next_request_growth = 0u64;

    for (turn, entry) in timeline {
        let Some(compactions) = entry.get("compactions").and_then(Value::as_array) else {
            continue;
        };
        let request_input_chars = entry.get("request_input_chars").and_then(Value::as_u64);
        let next_request_input_chars = timeline
            .range((turn + 1)..)
            .find_map(|(_, entry)| entry.get("request_input_chars").and_then(Value::as_u64));

        for compaction in compactions {
            let retained_chars = compaction_retained_chars(compaction);
            let same_turn_growth =
                growth_from_retained(retained_chars, request_input_chars).unwrap_or(0);
            let next_request_growth =
                growth_from_retained(retained_chars, next_request_input_chars).unwrap_or(0);
            max_same_turn_growth = max_same_turn_growth.max(same_turn_growth);
            max_next_request_growth = max_next_request_growth.max(next_request_growth);
            events.push(json!({
                "turn": turn,
                "method": compaction.get("method").and_then(Value::as_str).unwrap_or("unknown"),
                "trigger": compaction.get("trigger").cloned().unwrap_or(Value::Null),
                "before_chars": compaction.get("before_chars").cloned().unwrap_or(Value::Null),
                "retained_chars": retained_chars,
                "request_input_chars": request_input_chars,
                "next_request_input_chars": next_request_input_chars,
                "same_turn_growth_chars": same_turn_growth,
                "next_request_growth_chars": next_request_growth,
            }));
        }
    }

    json!({
        "count": events.len(),
        "max_same_turn_growth_chars": max_same_turn_growth,
        "max_next_request_growth_chars": max_next_request_growth,
        "events": events,
    })
}

fn compaction_retained_chars(compaction: &Value) -> Option<u64> {
    compaction
        .pointer("/local_pressure/final_chars")
        .or_else(|| compaction.get("after_chars"))
        .or_else(|| compaction.pointer("/fallback/after_chars"))
        .and_then(Value::as_u64)
}

fn growth_from_retained(retained_chars: Option<u64>, request_chars: Option<u64>) -> Option<u64> {
    Some(request_chars?.saturating_sub(retained_chars?))
}

pub(super) fn scenario_tool_expectation_report(
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

pub(super) fn scenario_tool_call_expectation_report(
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

pub(super) fn tool_failure_recovery_report(results: &[ObservedToolResult]) -> Option<Value> {
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
