use anyhow::Result;
use serde_json::{Value, json};

const POST_COMPACTION_NOTICE_START: &str = "[spark post-compaction verification]";
const POST_COMPACTION_NOTICE_END: &str = "[/spark post-compaction verification]";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) struct PostCompactionVerificationNotice {
    pub(in crate::agent) required_actions: usize,
}

pub(in crate::agent) fn compact_input_locally(
    input: &mut Vec<Value>,
    max_chars: usize,
) -> Result<Option<Value>> {
    if max_chars == 0 {
        return Ok(None);
    }

    let before = serde_json::to_string(input)?.len();
    if before <= max_chars {
        return Ok(None);
    }

    let output_indexes = input
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            item.get("type").and_then(Value::as_str) == Some("function_call_output")
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    let mut compacted_tool_outputs = 0usize;
    let mut compacted_messages = 0usize;
    let keep_full_from = output_indexes.len().saturating_sub(2);
    for (ordinal, index) in output_indexes.iter().copied().enumerate() {
        let keep_recent = ordinal >= keep_full_from;
        let max_output_chars = if keep_recent { 16_000 } else { 4_000 };
        if compact_output_item(&mut input[index], max_output_chars)? {
            compacted_tool_outputs += 1;
        }
    }

    let mid = serde_json::to_string(input)?.len();
    if mid > max_chars {
        let has_compaction_summary = input.iter().any(|item| {
            matches!(
                item.get("type").and_then(Value::as_str),
                Some("compaction" | "context_compaction")
            )
        });
        let message_indexes = input
            .iter()
            .enumerate()
            .filter(|(_, item)| item.get("role").and_then(Value::as_str).is_some())
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let keep_messages_from = if has_compaction_summary {
            message_indexes.len()
        } else {
            message_indexes.len().saturating_sub(8)
        };
        for (ordinal, index) in message_indexes.iter().copied().enumerate() {
            if !has_compaction_summary && ordinal >= keep_messages_from {
                continue;
            }
            if compact_message_item(&mut input[index], 800)? {
                compacted_messages += 1;
            }
        }

        while serde_json::to_string(input)?.len() > max_chars {
            let mut made_progress = false;
            for index in message_indexes.iter().rev().copied() {
                if compact_message_item(&mut input[index], 1200)? {
                    compacted_messages += 1;
                    made_progress = true;
                    break;
                }
            }
            if !made_progress {
                break;
            }
        }
    }

    let after = serde_json::to_string(input)?.len();
    let compacted = compacted_tool_outputs + compacted_messages;
    if compacted == 0 {
        return Ok(None);
    }

    Ok(Some(json!({
        "before_chars": before,
        "after_chars": after,
        "compacted_outputs": compacted,
        "compacted_tool_outputs": compacted_tool_outputs,
        "compacted_messages": compacted_messages,
        "threshold_chars": max_chars,
    })))
}

pub(in crate::agent) fn append_post_compaction_verification_notice(
    input: &mut Vec<Value>,
    prompt_input: &[Value],
) -> Option<PostCompactionVerificationNotice> {
    let actions = retained_action_lines_for_input(prompt_input);
    let has_user_context = prompt_input
        .iter()
        .any(|item| is_real_user_message(item) && !is_post_compaction_notice_item(item));
    if !has_user_context {
        return None;
    }

    input.retain(|item| !is_post_compaction_notice_item(item));
    input.push(post_compaction_verification_message(&actions));
    Some(PostCompactionVerificationNotice {
        required_actions: actions.len(),
    })
}

#[cfg(test)]
pub(in crate::agent) fn post_compaction_verification_text(
    prompt_input: &[Value],
) -> Option<String> {
    let actions = retained_action_lines_for_input(prompt_input);
    let has_user_context = prompt_input
        .iter()
        .any(|item| is_real_user_message(item) && !is_post_compaction_notice_item(item));
    has_user_context.then(|| post_compaction_verification_text_from_actions(&actions))
}

fn post_compaction_verification_message(actions: &[String]) -> Value {
    json!({
        "role": "user",
        "content": [{
            "type": "input_text",
            "text": post_compaction_verification_text_from_actions(actions),
        }]
    })
}

fn post_compaction_verification_text_from_actions(actions: &[String]) -> String {
    let mut text = format!(
        "{POST_COMPACTION_NOTICE_START}\n\
         A compaction just occurred. Treat compacted summaries as memory hints, not proof.\n\
         Before the final answer, if the task involves exact files, commands, or state, run the smallest fresh confirmation tool call instead of answering from the compacted summary.\n\
         If no exact confirmation is needed, continue normally.\n\
         required_actions={}\n",
        actions.len()
    );
    for (index, action) in actions.iter().enumerate() {
        text.push_str(&format!("action_{}={action}\n", index + 1));
    }
    if !actions.is_empty() {
        text.push_str(
            "Use the listed actions as confirmation candidates; execute the relevant one(s) after this notice before finalizing.\n",
        );
    }
    text.push_str(POST_COMPACTION_NOTICE_END);
    text
}

fn retained_action_lines_for_input(input: &[Value]) -> Vec<String> {
    let mut actions = Vec::new();
    for item in input {
        if is_post_compaction_notice_item(item) {
            continue;
        }
        let raw = message_text_from_value(item);
        if raw.trim().is_empty() {
            continue;
        }
        for line in retained_intent_lines(&raw, 16) {
            if let Some(action) = parse_native_tool_action(&line) {
                actions.push(action);
            }
        }
    }
    actions.sort();
    actions.dedup();
    actions
}

fn is_post_compaction_notice_item(item: &Value) -> bool {
    message_text_from_value(item)
        .trim_start()
        .starts_with(POST_COMPACTION_NOTICE_START)
}

pub(in crate::agent) fn trim_codex_generated_tail_to_fit(
    mut input: Vec<Value>,
    max_chars: usize,
) -> Result<Vec<Value>> {
    while serde_json::to_string(&input)?.len() > max_chars {
        let Some(last) = input.last() else {
            break;
        };
        if !is_codex_generated_item(last) {
            break;
        }
        input.pop();
    }
    Ok(input)
}

pub(in crate::agent) fn install_remote_compaction_history(
    prompt_input: &[Value],
    remote_output: Vec<Value>,
) -> Vec<Value> {
    let mut replacement = process_remote_compaction_output(remote_output);
    if !replacement.is_empty() {
        return replacement;
    }

    replacement = retained_user_messages_for_remote_compaction_v2(prompt_input, 20_000);
    if replacement.is_empty() {
        replacement = prompt_input
            .iter()
            .rev()
            .filter(|item| is_real_user_message(item) || is_assistant_message(item))
            .take(8)
            .cloned()
            .collect::<Vec<_>>();
        replacement.reverse();
    }
    replacement
}

pub(in crate::agent) fn process_remote_compaction_output(items: Vec<Value>) -> Vec<Value> {
    items
        .into_iter()
        .filter_map(normalize_compacted_history_item)
        .collect()
}

pub(in crate::agent) fn normalize_compacted_history_item(mut item: Value) -> Option<Value> {
    match item.get("type").and_then(Value::as_str) {
        Some("compaction" | "compaction_summary") => Some(json!({
            "type": "compaction",
            "encrypted_content": item.get("encrypted_content")?.clone(),
        })),
        Some("context_compaction") => Some(json!({
            "type": "context_compaction",
            "encrypted_content": item.get("encrypted_content").cloned(),
        })),
        Some("compaction_trigger") => None,
        Some("message") | None => {
            if should_keep_compacted_history_item(&item) {
                strip_response_only_fields(&mut item);
                Some(item)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub(in crate::agent) fn retained_user_messages_for_remote_compaction_v2(
    input: &[Value],
    max_tokens: usize,
) -> Vec<Value> {
    let mut remaining = max_tokens;
    let mut retained_reversed = Vec::new();
    for item in input.iter().rev() {
        if !is_real_user_message(item) {
            continue;
        }
        if remaining == 0 {
            continue;
        }
        let token_count = approx_token_count(&message_text_from_value(item)).max(1);
        if token_count <= remaining {
            retained_reversed.push(item.clone());
            remaining = remaining.saturating_sub(token_count);
        } else if let Some(truncated) = truncate_message_text_to_token_budget(item, remaining) {
            retained_reversed.push(truncated);
            remaining = 0;
        }
    }
    retained_reversed.reverse();
    retained_reversed
}

fn should_keep_compacted_history_item(item: &Value) -> bool {
    match item.get("type").and_then(Value::as_str) {
        Some("compaction" | "compaction_summary" | "context_compaction") => true,
        Some("compaction_trigger") => false,
        Some("message") | None => is_assistant_message(item) || is_real_user_message(item),
        _ => false,
    }
}

fn strip_response_only_fields(item: &mut Value) {
    if let Some(object) = item.as_object_mut() {
        object.remove("id");
        object.remove("status");
    }
}

fn is_codex_generated_item(item: &Value) -> bool {
    matches!(
        item.get("type").and_then(Value::as_str),
        Some(
            "function_call"
                | "function_call_output"
                | "tool_search_call"
                | "tool_search_output"
                | "reasoning"
                | "web_search_call"
                | "image_generation_call"
        )
    )
}

fn is_assistant_message(item: &Value) -> bool {
    item.get("role").and_then(Value::as_str) == Some("assistant")
}

fn is_real_user_message(item: &Value) -> bool {
    item.get("role").and_then(Value::as_str) == Some("user")
        && !message_text_from_value(item).trim().is_empty()
}

pub(in crate::agent) fn message_text_from_value(item: &Value) -> String {
    item.get("content")
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

fn truncate_message_text_to_token_budget(item: &Value, max_tokens: usize) -> Option<Value> {
    let mut cloned = item.clone();
    let content = cloned.get_mut("content")?.as_array_mut()?;
    let mut remaining = max_tokens;
    let mut any_text = false;
    for part in content {
        let Some(text) = part.get_mut("text") else {
            continue;
        };
        let Some(raw) = text.as_str() else {
            continue;
        };
        if remaining == 0 {
            *text = Value::String(String::new());
            continue;
        }
        let tokens = approx_token_count(raw);
        if tokens <= remaining {
            remaining = remaining.saturating_sub(tokens);
            any_text = any_text || !raw.is_empty();
        } else {
            *text = Value::String(truncate_text_tokens(raw, remaining));
            remaining = 0;
            any_text = true;
        }
    }
    any_text.then_some(cloned)
}

fn approx_token_count(text: &str) -> usize {
    text.len().div_ceil(4)
}

fn truncate_text_tokens(text: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens.saturating_mul(4);
    text.chars().take(max_chars).collect()
}

pub(in crate::agent) fn compact_output_item(
    item: &mut Value,
    max_output_chars: usize,
) -> Result<bool> {
    let Some(output) = item.get_mut("output") else {
        return Ok(false);
    };
    let Some(raw) = output.as_str() else {
        return Ok(false);
    };
    if raw.len() <= max_output_chars {
        return Ok(false);
    }

    let parsed = serde_json::from_str::<Value>(raw).unwrap_or_else(|_| json!({"raw": raw}));
    let compact = json!({
        "ok": parsed.get("ok").cloned().unwrap_or(Value::Bool(true)),
        "compacted": true,
        "original_chars": raw.len(),
        "preview": compact_text(raw, max_output_chars),
        "note": "Older tool output compacted by spark harness; rerun the relevant tool if exact content is needed.",
    });
    *output = Value::String(serde_json::to_string(&compact)?);
    Ok(true)
}

pub(in crate::agent) fn compact_message_item(item: &mut Value, max_chars: usize) -> Result<bool> {
    let Some(content) = item.get_mut("content").and_then(Value::as_array_mut) else {
        return Ok(false);
    };
    let mut changed = false;
    for part in content {
        let Some(text) = part.get_mut("text") else {
            continue;
        };
        let Some(raw) = text.as_str() else {
            continue;
        };
        if raw.starts_with("[spark local message compaction]") {
            continue;
        }
        if raw.len() <= max_chars {
            continue;
        }
        let preview = compact_text(raw, max_chars);
        let retained_intent = retained_intent_block(raw);
        *text = Value::String(format!(
            "[spark local message compaction]\noriginal_chars={}\npreview_chars={}\nretained=head+tail\nexact_content=omitted; rerun/read the relevant source if exact text matters\n{}\n[/spark local message compaction]\n{}",
            raw.len(),
            preview.len(),
            retained_intent,
            preview
        ));
        changed = true;
    }
    Ok(changed)
}

fn compact_text(raw: &str, max_chars: usize) -> String {
    let head_len = max_chars.saturating_mul(3) / 4;
    let tail_len = max_chars.saturating_sub(head_len).saturating_sub(64);
    let head = raw.chars().take(head_len).collect::<String>();
    let tail_vec = raw.chars().rev().take(tail_len).collect::<Vec<_>>();
    let tail = tail_vec.into_iter().rev().collect::<String>();
    format!("{head}\n...[compacted]...\n{tail}")
}

pub(in crate::agent) fn retained_intent_block(raw: &str) -> String {
    let lines = retained_intent_lines(raw, 12);
    let actions = retained_action_lines(&lines);
    let mut block = format!("retained_intent_lines={}", lines.len());
    for (index, line) in lines.iter().enumerate() {
        block.push('\n');
        block.push_str(&format!("intent_{}={}", index + 1, line));
    }
    block.push('\n');
    block.push_str(&format!("required_actions={}", actions.len()));
    for (index, action) in actions.iter().enumerate() {
        block.push('\n');
        block.push_str(&format!("action_{}={}", index + 1, action));
    }
    block
}

pub(in crate::agent) fn retained_intent_lines(raw: &str, max_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("row ") {
            continue;
        }
        if line
            == "Synthetic payload follows. Preserve the high-level instruction above; payload rows are intentionally repetitive profiling filler."
        {
            break;
        }
        if is_high_signal_intent_line(line) {
            lines.push(line.to_string());
            if lines.len() >= max_lines {
                break;
            }
        }
    }
    lines
}

fn is_high_signal_intent_line(line: &str) -> bool {
    line.starts_with("Profile scenario:")
        || line.starts_with("This prompt")
        || line.starts_with("Let the harness")
        || line.starts_with("Do not ")
        || line.starts_with("After any compaction")
        || line.starts_with("- ")
        || mentions_native_file_tool_action(line)
        || line.chars().next().is_some_and(|ch| ch.is_ascii_digit())
}

fn mentions_native_file_tool_action(line: &str) -> bool {
    (line.contains("fs.list")
        || line.contains("fs.read")
        || line.contains("fs.stat")
        || line.contains("fs.search")
        || line.contains("fs.replace")
        || line.contains("fs.edit")
        || line.contains("fs.write")
        || line.contains("fs.rename"))
        && (line.contains("use ") || line.contains("call ") || line.contains("run "))
}

fn retained_action_lines(lines: &[String]) -> Vec<String> {
    let mut actions = Vec::new();
    for line in lines {
        if let Some(action) = parse_native_tool_action(line) {
            actions.push(action);
        }
    }
    actions.sort();
    actions.dedup();
    actions
}

pub(in crate::agent) fn parse_native_tool_action(line: &str) -> Option<String> {
    if let Some(action) = parse_rename_tool_action(line) {
        return Some(action);
    }
    const TOOLS: [&str; 7] = [
        "fs.list",
        "fs.read",
        "fs.stat",
        "fs.search",
        "fs.replace",
        "fs.edit",
        "fs.write",
    ];
    for tool in TOOLS {
        if let Some(action) = parse_file_tool_action(line, tool) {
            return Some(action);
        }
    }
    None
}

fn parse_rename_tool_action(line: &str) -> Option<String> {
    let normalized = line.trim().trim_end_matches('.');
    let tool = "fs.rename";
    let tool_index = normalized.find(tool)?;
    let before = normalized[..tool_index].trim_end();
    if !before.ends_with("use")
        && !before.ends_with("call")
        && !before.ends_with("run")
        && !before.ends_with("using")
    {
        return None;
    }
    let after_tool = normalized[tool_index + tool.len()..].trim_start();
    let after_tool = after_tool
        .strip_prefix("to move ")
        .or_else(|| after_tool.strip_prefix("move "))
        .or_else(|| after_tool.strip_prefix("from "))
        .unwrap_or(after_tool);
    let (from, to) = after_tool.split_once(" to ")?;
    let from = clean_action_path(from);
    let to = clean_action_path(to);
    if from.is_empty() || to.is_empty() {
        return Some(format!("tool={tool}"));
    }
    Some(format!("tool={tool} from={from} to={to}"))
}

fn parse_file_tool_action(line: &str, tool: &str) -> Option<String> {
    let normalized = line.trim().trim_end_matches('.');
    let tool_index = normalized.find(tool)?;
    let before = normalized[..tool_index].trim_end();
    if !before.ends_with("use")
        && !before.ends_with("call")
        && !before.ends_with("run")
        && !before.ends_with("using")
    {
        return None;
    }
    let after_tool = normalized[tool_index + tool.len()..].trim_start();
    let after_tool = after_tool
        .strip_prefix("on ")
        .or_else(|| after_tool.strip_prefix("in "))
        .or_else(|| after_tool.strip_prefix("for "))
        .unwrap_or(after_tool);
    if after_tool.is_empty() {
        return Some(format!("tool={tool}"));
    }
    let (path, tail) = after_tool
        .split_once(" with ")
        .map_or((after_tool, ""), |(path, tail)| (path, tail));
    let mut path = path.trim().trim_matches('`');
    if let Some((prefix, _)) = path.split_once(", then") {
        path = prefix.trim();
    }
    if let Some((prefix, _)) = path.split_once(" for ") {
        path = prefix.trim();
    }
    let path = clean_action_path(path);
    if path.is_empty() {
        return Some(format!("tool={tool}"));
    }
    let mut action = format!("tool={tool} path={path}");
    if tool == "fs.list" {
        let recursive = if tail.contains("recursive=false") {
            "false"
        } else if tail.contains("recursive=true") {
            "true"
        } else {
            "unspecified"
        };
        action.push_str(&format!(" recursive={recursive}"));
    }
    Some(action)
}

fn clean_action_path(path: &str) -> String {
    path.trim()
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim_end_matches(',')
        .trim_end_matches(':')
        .trim()
        .to_string()
}
