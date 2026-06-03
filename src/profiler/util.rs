use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::SPARK_CONTEXT_WINDOW_TOKENS;

pub fn approx_token_count_from_chars(chars: usize) -> usize {
    chars.div_ceil(4)
}

pub fn context_window_pct(chars: usize) -> f64 {
    let approx_tokens = approx_token_count_from_chars(chars) as f64;
    (approx_tokens / SPARK_CONTEXT_WINDOW_TOKENS as f64) * 100.0
}

pub(super) fn tool_result_is_truncated(data: &Value) -> bool {
    data.get("truncated").and_then(Value::as_bool) == Some(true)
        || data.get("stdout_truncated").and_then(Value::as_bool) == Some(true)
        || data.get("stderr_truncated").and_then(Value::as_bool) == Some(true)
}

pub(super) fn tool_result_timed_out(data: &Value) -> bool {
    data.get("timed_out").and_then(Value::as_bool) == Some(true)
}

pub(super) fn created_parent_dirs(data: &Value) -> Option<Vec<String>> {
    Some(
        data.get("created_parent_dirs")?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
    )
}

pub(super) fn tool_truncation_fields(data: &Value) -> Value {
    let mut fields = Map::new();
    copy_field(data, &mut fields, "truncated");
    copy_field(data, &mut fields, "stdout_truncated");
    copy_field(data, &mut fields, "stderr_truncated");
    copy_field(data, &mut fields, "stdout_chars");
    copy_field(data, &mut fields, "stderr_chars");
    Value::Object(fields)
}

fn copy_field(source: &Value, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key) {
        target.insert(key.to_string(), value.clone());
    }
}

pub fn tool_signature(tool_name: &str, args: &Value) -> String {
    format!("{tool_name}:{}", canonical_json(args))
}

pub(super) fn canonical_json(value: &Value) -> String {
    serde_json::to_string(&canonical_value(value)).unwrap_or_else(|_| "null".to_string())
}

fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonical_value).collect()),
        Value::Object(object) => {
            let sorted = object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_value(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(Map::from_iter(sorted))
        }
        other => other.clone(),
    }
}
