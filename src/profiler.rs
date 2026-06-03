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
    input_chars_by_request: Vec<usize>,
    response_text_chars: usize,
    errors: Vec<Value>,
    compaction_reports: Vec<Value>,
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
        self.input_chars_by_request.push(input_chars);
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
        self.compaction_reports.push(report.clone());
        self.push_signal(json!({
            "kind": "compaction",
            "method": report.get("method").and_then(Value::as_str).unwrap_or("unknown"),
            "before_chars": report.get("before_chars").or_else(|| report.pointer("/fallback/before_chars")).cloned(),
            "after_chars": report.get("after_chars").or_else(|| report.pointer("/fallback/after_chars")).cloned(),
        }));
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

    pub fn record_error(&mut self, turn: usize, stage: &str, error: &str) {
        let value = json!({
            "turn": turn,
            "stage": stage,
            "error": error,
        });
        self.errors.push(value.clone());
        self.push_signal(json!({
            "kind": "error",
            "turn": turn,
            "stage": stage,
            "error": error,
        }));
    }

    pub fn to_json(&self) -> Value {
        let diagnostics = self.diagnostics();
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
            "input_chars_by_request": self.input_chars_by_request,
            "response_text_chars": self.response_text_chars,
            "errors": self.errors,
            "compaction_reports": self.compaction_reports,
            "tool_counts": self.tool_counts,
            "diagnostics": diagnostics,
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

    fn diagnostics(&self) -> Vec<Value> {
        let mut diagnostics = Vec::new();
        if !self.errors.is_empty() {
            diagnostics.push(json!({
                "level": "error",
                "kind": "request_failure",
                "message": "One or more Spark turns failed. Inspect *-error trace files and the input size sequence around the failing turn.",
                "count": self.errors.len(),
            }));
        }

        if self.consecutive_duplicate_tool_calls > 0 {
            diagnostics.push(json!({
                "level": "warning",
                "kind": "consecutive_duplicate_tool_calls",
                "message": "Spark repeated the same tool call back-to-back. Consider tightening tool observations, adding cache hints, or exposing a more targeted tool.",
                "count": self.consecutive_duplicate_tool_calls,
            }));
        }

        if self.repeated_tool_calls > 2 {
            diagnostics.push(json!({
                "level": "warning",
                "kind": "repeated_tool_calls",
                "message": "Spark repeated exact tool-call signatures several times. Compare the repeated arguments in recent_signals before changing prompts.",
                "count": self.repeated_tool_calls,
            }));
        }

        if self.compactions > 0 && self.remote_compactions == 0 {
            diagnostics.push(json!({
                "level": "warning",
                "kind": "no_remote_compaction",
                "message": "Compaction happened without a successful remote Codex compaction. Treat local fallback summaries as profiling data, not the preferred steady state.",
            }));
        }

        for report in &self.compaction_reports {
            let before = report
                .get("before_chars")
                .or_else(|| report.pointer("/fallback/before_chars"))
                .and_then(Value::as_u64);
            let after = report
                .get("after_chars")
                .or_else(|| report.pointer("/fallback/after_chars"))
                .and_then(Value::as_u64);
            if let (Some(before), Some(after)) = (before, after)
                && before > 0
                && after.saturating_mul(2) > before
            {
                diagnostics.push(json!({
                    "level": "warning",
                    "kind": "weak_compaction_shrink",
                    "message": "A compaction reduced history by less than 50%. Inspect retained items before relying on it for long-context runs.",
                    "before_chars": before,
                    "after_chars": after,
                }));
            }
        }

        if self.max_input_chars >= 450_000 {
            diagnostics.push(json!({
                "level": "warning",
                "kind": "near_input_guard",
                "message": "Request input approached the default max-input guard. Long-context profiling should inspect the exact input_chars_by_request sequence and compaction timing.",
                "max_input_chars": self.max_input_chars,
            }));
        } else if self.max_input_chars >= 160_000 && self.compactions == 0 {
            diagnostics.push(json!({
                "level": "info",
                "kind": "large_uncompacted_context",
                "message": "Request input exceeded the default compaction threshold without a recorded compaction. Check whether compaction was disabled or traces are incomplete.",
                "max_input_chars": self.max_input_chars,
            }));
        }

        diagnostics
    }
}

pub fn tool_signature(tool_name: &str, args: &Value) -> String {
    format!("{tool_name}:{}", canonical_json(args))
}

pub fn analyze_trace(dir: &Path) -> Result<Value> {
    let mut profiler = AgentProfiler::default();
    let mut embedded_profile_summary = None;
    let mut trace_metadata = None;
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

        if name == "000-trace-metadata.json" {
            trace_metadata = Some(value);
        } else if name.ends_with("-profile-summary.json") {
            embedded_profile_summary = Some(value);
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
        }
    }

    let mut summary = profiler.to_json();
    if let Some(object) = summary.as_object_mut() {
        if let Some(metadata) = trace_metadata {
            object.insert("trace_metadata".to_string(), metadata);
        }
        if let Some(embedded) = embedded_profile_summary {
            object.insert("embedded_profile_summary".to_string(), embedded);
        }
    }
    Ok(summary)
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
        "fs_write" => "fs.write",
        "fs_search" => "fs.search",
        "fs_replace" => "fs.replace",
        "fs_edit" => "fs.edit",
        "cmd_exec" => "cmd.exec",
        other => other,
    }
    .to_string()
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
    fn profiler_records_input_size_sequence_and_errors() {
        let mut profiler = AgentProfiler::default();

        profiler.record_request(120);
        profiler.record_request(240);
        profiler.record_error(2, "response", "stream ended without response.completed");

        let summary = profiler.to_json();
        assert_eq!(summary["requests"], 2);
        assert_eq!(summary["max_input_chars"], 240);
        assert_eq!(summary["input_chars_by_request"], json!([120, 240]));
        assert_eq!(summary["errors"][0]["turn"], 2);
        assert_eq!(summary["recent_signals"][0]["kind"], "error");
        assert_eq!(summary["diagnostics"][0]["kind"], "request_failure");
    }

    #[test]
    fn profiler_diagnoses_duplicate_tool_loops_and_input_pressure() {
        let args = json!({"path": "a.txt"});
        let mut profiler = AgentProfiler::default();

        profiler.record_request(470_000);
        profiler.record_tool_call(1, "fs.read", &args);
        profiler.record_tool_call(2, "fs.read", &args);

        let diagnostics = profiler
            .to_json()
            .get("diagnostics")
            .and_then(Value::as_array)
            .cloned()
            .expect("diagnostics");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic["kind"] == "consecutive_duplicate_tool_calls")
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic["kind"] == "near_input_guard")
        );
    }

    #[test]
    fn profiler_diagnoses_weak_compaction() {
        let mut profiler = AgentProfiler::default();

        profiler.record_compaction(&json!({
            "method": "responses_compact",
            "before_chars": 100_000,
            "after_chars": 75_000
        }));

        let summary = profiler.to_json();
        assert_eq!(summary["remote_compactions"], 1);
        assert!(
            summary["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|diagnostic| diagnostic["kind"] == "weak_compaction_shrink")
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
        assert_eq!(summary["tool_counts"]["fs.read"], 1);
        assert_eq!(summary["response_text_chars"], 4);
    }

    #[test]
    fn analyze_trace_recomputes_even_when_profile_summary_exists() {
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
            dir.path().join("001-profile-summary.json"),
            serde_json::to_vec_pretty(&json!({
                "requests": 999,
                "stale": true
            }))
            .expect("serialize profile"),
        )
        .expect("write profile");

        let summary = analyze_trace(dir.path()).expect("analyze trace");

        assert_eq!(summary["requests"], 1);
        assert_eq!(summary["embedded_profile_summary"]["requests"], 999);
        assert_eq!(summary["embedded_profile_summary"]["stale"], true);
    }

    #[test]
    fn analyze_trace_includes_trace_metadata() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("000-trace-metadata.json"),
            serde_json::to_vec_pretty(&json!({
                "schema_version": 1,
                "model": "gpt-5.3-codex-spark",
                "compact_after_chars": 160000,
                "max_input_chars": 500000
            }))
            .expect("serialize metadata"),
        )
        .expect("write metadata");

        let summary = analyze_trace(dir.path()).expect("analyze trace");

        assert_eq!(summary["trace_metadata"]["schema_version"], 1);
        assert_eq!(summary["trace_metadata"]["model"], "gpt-5.3-codex-spark");
        assert_eq!(summary["trace_metadata"]["compact_after_chars"], 160000);
    }

    #[test]
    fn analyze_trace_reports_response_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("001-request-input.json"),
            serde_json::to_vec_pretty(&json!({
                "input": [{"role": "user", "content": [{"type": "input_text", "text": "large prompt"}]}]
            }))
            .expect("serialize request"),
        )
        .expect("write request");
        std::fs::write(
            dir.path().join("001-response-error.json"),
            serde_json::to_vec_pretty(&json!({
                "stage": "response",
                "error": "Spark stream ended without response.completed"
            }))
            .expect("serialize error"),
        )
        .expect("write error");

        let summary = analyze_trace(dir.path()).expect("analyze trace");

        assert_eq!(summary["requests"], 1);
        assert_eq!(summary["errors"][0]["stage"], "response");
        assert!(
            summary["errors"][0]["error"]
                .as_str()
                .expect("error text")
                .contains("without response.completed")
        );
    }

    #[test]
    fn analyze_trace_reports_generic_terminal_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("002-max_turns-error.json"),
            serde_json::to_vec_pretty(&json!({
                "stage": "max_turns",
                "error": "stopped after 1 turns without completion"
            }))
            .expect("serialize error"),
        )
        .expect("write error");

        let summary = analyze_trace(dir.path()).expect("analyze trace");

        assert_eq!(summary["errors"][0]["turn"], 2);
        assert_eq!(summary["errors"][0]["stage"], "max_turns");
        assert_eq!(summary["diagnostics"][0]["kind"], "request_failure");
    }
}
