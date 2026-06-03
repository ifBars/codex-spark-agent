use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AgentProfiler {
    requests: usize,
    tool_calls: usize,
    repeated_tool_calls: usize,
    consecutive_duplicate_tool_calls: usize,
    compactions: usize,
    remote_compactions: usize,
    fallback_compactions: usize,
    readonly_tool_cache_hits: usize,
    max_input_chars: usize,
    total_input_chars: usize,
    response_text_chars: usize,
    tool_counts: BTreeMap<String, usize>,
    signature_counts: BTreeMap<String, usize>,
    last_signature: Option<String>,
    signals: Vec<Value>,
}

impl AgentProfiler {
    pub fn record_request(&mut self, input_chars: usize) {
        self.requests += 1;
        self.max_input_chars = self.max_input_chars.max(input_chars);
        self.total_input_chars = self.total_input_chars.saturating_add(input_chars);
    }

    pub fn record_response_text(&mut self, text: &str) {
        self.response_text_chars = self.response_text_chars.saturating_add(text.len());
    }

    pub fn record_tool_call(&mut self, turn: usize, tool_name: &str, args: &Value) {
        self.tool_calls += 1;
        *self.tool_counts.entry(tool_name.to_string()).or_default() += 1;

        let signature = tool_signature(tool_name, args);
        let previous_count = *self.signature_counts.get(&signature).unwrap_or(&0);
        if previous_count > 0 {
            self.repeated_tool_calls += 1;
            self.push_signal(json!({
                "kind": "repeated_tool_call",
                "turn": turn,
                "tool": tool_name,
                "seen_before": previous_count,
                "args": args,
            }));
        }

        if self.last_signature.as_deref() == Some(signature.as_str()) {
            self.consecutive_duplicate_tool_calls += 1;
            self.push_signal(json!({
                "kind": "consecutive_duplicate_tool_call",
                "turn": turn,
                "tool": tool_name,
                "args": args,
            }));
        }

        self.signature_counts
            .insert(signature.clone(), previous_count + 1);
        self.last_signature = Some(signature);
    }

    pub fn record_compaction(&mut self, report: &Value) {
        self.compactions += 1;
        match report.get("method").and_then(Value::as_str) {
            Some("responses_compact") => self.remote_compactions += 1,
            Some("local_fallback") => self.fallback_compactions += 1,
            _ => {}
        }
    }

    pub fn record_readonly_tool_cache_hit(&mut self, turn: usize, tool_name: &str, args: &Value) {
        self.readonly_tool_cache_hits += 1;
        self.push_signal(json!({
            "kind": "readonly_tool_cache_hit",
            "turn": turn,
            "tool": tool_name,
            "args": args,
        }));
    }

    pub fn to_json(&self) -> Value {
        json!({
            "requests": self.requests,
            "tool_calls": self.tool_calls,
            "repeated_tool_calls": self.repeated_tool_calls,
            "consecutive_duplicate_tool_calls": self.consecutive_duplicate_tool_calls,
            "compactions": self.compactions,
            "remote_compactions": self.remote_compactions,
            "fallback_compactions": self.fallback_compactions,
            "readonly_tool_cache_hits": self.readonly_tool_cache_hits,
            "max_input_chars": self.max_input_chars,
            "average_input_chars": if self.requests == 0 { 0 } else { self.total_input_chars / self.requests },
            "response_text_chars": self.response_text_chars,
            "tool_counts": self.tool_counts,
            "recent_signals": self.signals,
        })
    }

    pub fn status_line(&self) -> String {
        format!(
            "profile: requests={}, tool_calls={}, repeated_calls={}, consecutive_duplicates={}, readonly_cache_hits={}, compactions={} (remote={}, fallback={}), max_input_chars={}",
            self.requests,
            self.tool_calls,
            self.repeated_tool_calls,
            self.consecutive_duplicate_tool_calls,
            self.readonly_tool_cache_hits,
            self.compactions,
            self.remote_compactions,
            self.fallback_compactions,
            self.max_input_chars
        )
    }

    fn push_signal(&mut self, signal: Value) {
        self.signals.push(signal);
        const MAX_SIGNALS: usize = 20;
        if self.signals.len() > MAX_SIGNALS {
            self.signals.remove(0);
        }
    }
}

pub fn tool_signature(tool_name: &str, args: &Value) -> String {
    format!("{tool_name}:{}", canonical_json(args))
}

pub fn analyze_trace(dir: &Path) -> Result<Value> {
    let mut profiler = AgentProfiler::default();
    let mut latest_profile_summary = None;
    let mut files = std::fs::read_dir(dir)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    files.sort();

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

        if name.ends_with("-profile-summary.json") {
            latest_profile_summary = Some(value);
        } else if name.ends_with("-request-input.json") {
            let input_chars = value
                .get("input")
                .map(serde_json::to_string)
                .transpose()?
                .map(|input| input.len())
                .unwrap_or(raw.len());
            profiler.record_request(input_chars);
        } else if name.ends_with("-response.json") {
            for (tool_name, args) in function_calls_from_trace_response(&value) {
                profiler.record_tool_call(turn, &tool_name, &args);
            }
            if let Some(text) = response_text_from_trace_response(&value) {
                profiler.record_response_text(&text);
            }
        } else if name.ends_with("-compaction.json") {
            profiler.record_compaction(&value);
        }
    }

    Ok(latest_profile_summary.unwrap_or_else(|| profiler.to_json()))
}

fn function_calls_from_trace_response(value: &Value) -> Vec<(String, Value)> {
    output_items_from_trace_response(value)
        .into_iter()
        .filter_map(|item| {
            if item.get("type").and_then(Value::as_str) != Some("function_call") {
                return None;
            }
            let name = item.get("name").and_then(Value::as_str)?.to_string();
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

fn output_items_from_trace_response(value: &Value) -> Vec<Value> {
    if let Some(items) = value
        .get("response")
        .and_then(|response| response.get("output"))
        .and_then(Value::as_array)
        && !items.is_empty()
    {
        return items.clone();
    }

    let mut indexed = value
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

fn canonical_json(value: &Value) -> String {
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn tool_signature_is_stable_for_object_key_order() {
        let left = json!({"path": "a.txt", "offset": 1, "limit": 5});
        let right = json!({"limit": 5, "offset": 1, "path": "a.txt"});

        assert_eq!(
            tool_signature("fs.read", &left),
            tool_signature("fs.read", &right)
        );
    }

    #[test]
    fn profiler_records_repeated_and_consecutive_tool_calls() {
        let args = json!({"path": "a.txt", "offset": 1, "limit": 5});
        let mut profiler = AgentProfiler::default();

        profiler.record_tool_call(1, "fs.read", &args);
        profiler.record_tool_call(2, "fs.read", &args);

        let summary = profiler.to_json();
        assert_eq!(summary["tool_calls"], 2);
        assert_eq!(summary["repeated_tool_calls"], 1);
        assert_eq!(summary["consecutive_duplicate_tool_calls"], 1);
        assert_eq!(summary["tool_counts"]["fs.read"], 2);
        assert_eq!(summary["recent_signals"][0]["kind"], "repeated_tool_call");
        assert_eq!(
            summary["recent_signals"][1]["kind"],
            "consecutive_duplicate_tool_call"
        );
    }

    #[test]
    fn profiler_records_readonly_cache_hits() {
        let args = json!({"path": "a.txt"});
        let mut profiler = AgentProfiler::default();

        profiler.record_readonly_tool_cache_hit(3, "fs.read", &args);

        let summary = profiler.to_json();
        assert_eq!(summary["readonly_tool_cache_hits"], 1);
        assert_eq!(
            summary["recent_signals"][0]["kind"],
            "readonly_tool_cache_hit"
        );
    }

    #[test]
    fn analyze_trace_reconstructs_tool_calls_from_stream_events() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("001-request-input.json"),
            serde_json::to_vec_pretty(&json!({
                "input": [{"role": "user", "content": [{"type": "input_text", "text": "read"}]}]
            }))
            .expect("serialize request"),
        )
        .expect("write request");
        std::fs::write(
            dir.path().join("001-response.json"),
            serde_json::to_vec_pretty(&json!({
                "response": {"output": []},
                "events": [
                    {
                        "type": "response.output_item.done",
                        "output_index": 0,
                        "item": {
                            "type": "function_call",
                            "name": "fs_read",
                            "arguments": "{\"path\":\"a.txt\",\"limit\":5}"
                        }
                    },
                    {
                        "type": "response.output_item.done",
                        "output_index": 1,
                        "item": {
                            "type": "message",
                            "content": [{"type": "output_text", "text": "done"}]
                        }
                    }
                ]
            }))
            .expect("serialize response"),
        )
        .expect("write response");

        let summary = analyze_trace(dir.path()).expect("analyze trace");

        assert_eq!(summary["requests"], 1);
        assert_eq!(summary["tool_calls"], 1);
        assert_eq!(summary["tool_counts"]["fs_read"], 1);
        assert_eq!(summary["response_text_chars"], 4);
    }
}
