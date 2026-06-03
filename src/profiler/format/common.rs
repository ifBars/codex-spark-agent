use serde_json::Value;

pub(super) fn format_tool_group(group: &Value) -> Option<String> {
    let tools = group.as_array()?;
    if tools.is_empty() {
        return None;
    }
    Some(
        tools
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("|"),
    )
}

pub(super) fn format_required_action(action: &Value) -> String {
    let tool = action
        .get("tool")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mut parts = vec![format!("tool={tool}")];
    if let Some(path) = action.get("path").and_then(Value::as_str) {
        parts.push(format!("path={path}"));
    }
    if let Some(from) = action.get("from").and_then(Value::as_str) {
        parts.push(format!("from={from}"));
    }
    if let Some(to) = action.get("to").and_then(Value::as_str) {
        parts.push(format!("to={to}"));
    }
    if let Some(recursive) = action.get("recursive").and_then(Value::as_bool) {
        parts.push(format!("recursive={recursive}"));
    }
    parts.join(" ")
}

pub(super) fn number_field(summary: &Value, key: &str) -> String {
    summary
        .get(key)
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".to_string())
}

pub(super) fn diagnostic_kinds(summary: &Value) -> Vec<String> {
    summary
        .get("diagnostics")
        .and_then(Value::as_array)
        .map(|diagnostics| {
            diagnostics
                .iter()
                .filter_map(|diagnostic| diagnostic.get("kind").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
