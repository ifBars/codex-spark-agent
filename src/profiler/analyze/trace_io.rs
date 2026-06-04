use anyhow::Result;
use serde_json::{Value, json};

pub(super) struct TraceToolResult {
    pub(super) tool_name: String,
    pub(super) args: Value,
    pub(super) ok: bool,
    pub(super) data: Value,
    pub(super) output_chars: usize,
    pub(super) duration_ms: u64,
    pub(super) error: Option<String>,
    pub(super) cached_observation: bool,
}

pub(super) fn tool_result_from_trace(value: &Value) -> Result<Option<TraceToolResult>> {
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

pub(super) fn function_calls_from_trace_response(value: &Value) -> Vec<(String, Value)> {
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

pub(super) fn response_text_from_trace_response(value: &Value) -> Option<String> {
    let text = output_items_from_trace_response(value)
        .into_iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter_map(|item| item.get("content").and_then(Value::as_array).cloned())
        .flatten()
        .filter_map(|part| part.get("text").and_then(Value::as_str).map(str::to_string))
        .collect::<String>();
    (!text.is_empty()).then_some(text)
}
