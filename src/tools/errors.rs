use serde_json::{Value, json};

pub(super) fn structured_tool_error(tool_name: &str, args: &Value, message: &str) -> Value {
    let kind = tool_error_kind(message);
    json!({
        "error_kind": kind,
        "message": message,
        "tool": tool_name,
        "args_shape": args_shape(args),
        "hint": tool_error_hint(kind),
    })
}

fn args_shape(args: &Value) -> Value {
    match args {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), Value::String(value_kind(value).to_string())))
                .collect(),
        ),
        other => json!({"_root": value_kind(other)}),
    }
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn tool_error_kind(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("unknown tool") {
        "unknown_tool"
    } else if lower.contains(" is required")
        || lower.contains("must not be empty")
        || lower.contains("expected")
    {
        "invalid_arguments"
    } else if lower.contains("escapes workspace") {
        "workspace_escape"
    } else if lower.contains("failed to read")
        || lower.contains("failed to list")
        || lower.contains("old text not found")
        || lower.contains("no such file")
        || lower.contains("cannot find the file")
    {
        "not_found_or_unavailable"
    } else {
        "tool_error"
    }
}

fn tool_error_hint(kind: &str) -> &'static str {
    match kind {
        "unknown_tool" => "Use one of the advertised native tool names exactly.",
        "invalid_arguments" => {
            "Retry the same tool with the required schema fields and valid values."
        }
        "workspace_escape" => "Use a path inside the configured workspace.",
        "not_found_or_unavailable" => {
            "List or search the workspace to find the correct path, then retry with a narrower tool call."
        }
        _ => {
            "Inspect the message, adjust the tool arguments, and retry if the task still requires this tool."
        }
    }
}
