use std::collections::BTreeSet;

use serde_json::Value;

use super::{ObservedToolCall, RequiredAction};

pub(super) struct RequiredActionReport {
    pub(super) actions: Vec<RequiredAction>,
    pub(super) executed: Vec<RequiredAction>,
    pub(super) missing: Vec<RequiredAction>,
    pub(super) calls_before_first_required_action: usize,
}

pub(super) fn required_actions_from_request_input(value: &Value) -> Vec<RequiredAction> {
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

pub(super) fn required_action_report(
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

pub(super) fn loaded_skill_contexts_from_request_input(value: &Value) -> Vec<String> {
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

pub(super) fn required_action_from_value(value: &Value) -> Option<RequiredAction> {
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

pub(super) fn required_action_matches_call(
    action: &RequiredAction,
    call: &ObservedToolCall,
) -> bool {
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
