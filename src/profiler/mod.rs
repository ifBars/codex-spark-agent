use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use analyze::summarize_compaction_report;

pub const SPARK_CONTEXT_WINDOW_TOKENS: usize = 128_000;
const SLOW_SPARK_REQUEST_MS: u64 = 30_000;
const SLOW_TOOL_RESULT_MS: u64 = 10_000;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AgentProfiler {
    requests: usize,
    tool_calls: usize,
    tool_results: usize,
    tool_failures: usize,
    truncated_tool_results: usize,
    total_tool_duration_ms: u64,
    max_tool_duration_ms: u64,
    repeated_tool_calls: usize,
    consecutive_duplicate_tool_calls: usize,
    #[serde(default)]
    tool_only_turns: usize,
    #[serde(default)]
    max_consecutive_tool_only_turns: usize,
    #[serde(default)]
    current_consecutive_tool_only_turns: usize,
    compactions: usize,
    remote_compactions: usize,
    fallback_compactions: usize,
    readonly_tool_cache_hits: usize,
    #[serde(default)]
    mutation_parent_dir_creations: usize,
    max_input_chars: usize,
    total_input_chars: usize,
    input_chars_by_request: Vec<usize>,
    total_request_duration_ms: u64,
    max_request_duration_ms: u64,
    request_duration_ms_by_request: Vec<u64>,
    #[serde(default)]
    tool_only_turn_numbers: Vec<usize>,
    response_text_chars: usize,
    errors: Vec<Value>,
    compaction_reports: Vec<Value>,
    tool_counts: BTreeMap<String, usize>,
    tool_failure_counts: BTreeMap<String, usize>,
    tool_truncation_counts: BTreeMap<String, usize>,
    #[serde(default)]
    mutation_parent_dir_creation_counts: BTreeMap<String, usize>,
    tool_duration_ms_by_tool: BTreeMap<String, u64>,
    max_tool_duration_ms_by_tool: BTreeMap<String, u64>,
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

    pub fn record_request_duration(&mut self, turn: usize, duration_ms: u64) {
        self.total_request_duration_ms = self.total_request_duration_ms.saturating_add(duration_ms);
        self.max_request_duration_ms = self.max_request_duration_ms.max(duration_ms);
        self.request_duration_ms_by_request.push(duration_ms);
        if duration_ms >= SLOW_SPARK_REQUEST_MS {
            self.push_signal(json!({
                "kind": "slow_spark_request",
                "turn": turn,
                "duration_ms": duration_ms,
            }));
        }
    }

    pub fn record_response_text(&mut self, text: &str) {
        self.response_text_chars = self.response_text_chars.saturating_add(text.len());
    }

    pub fn current_tool_only_turn_streak(&self) -> usize {
        self.current_consecutive_tool_only_turns
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

    pub fn record_turn_activity(
        &mut self,
        turn: usize,
        has_tool_calls: bool,
        response_text_chars: usize,
    ) {
        if has_tool_calls && response_text_chars == 0 {
            self.tool_only_turns += 1;
            self.current_consecutive_tool_only_turns += 1;
            self.max_consecutive_tool_only_turns = self
                .max_consecutive_tool_only_turns
                .max(self.current_consecutive_tool_only_turns);
            self.tool_only_turn_numbers.push(turn);
            self.push_signal(json!({
                "kind": "tool_only_turn",
                "turn": turn,
                "consecutive": self.current_consecutive_tool_only_turns,
            }));
        } else {
            self.current_consecutive_tool_only_turns = 0;
        }
    }

    pub fn record_tool_result(
        &mut self,
        turn: usize,
        tool_name: &str,
        ok: bool,
        data: &Value,
        output_chars: usize,
        duration_ms: u64,
        error: Option<&str>,
    ) {
        self.tool_results += 1;
        self.total_tool_duration_ms = self.total_tool_duration_ms.saturating_add(duration_ms);
        self.max_tool_duration_ms = self.max_tool_duration_ms.max(duration_ms);
        *self
            .tool_duration_ms_by_tool
            .entry(tool_name.to_string())
            .or_default() += duration_ms;
        let max_by_tool = self
            .max_tool_duration_ms_by_tool
            .entry(tool_name.to_string())
            .or_default();
        *max_by_tool = (*max_by_tool).max(duration_ms);
        if duration_ms >= SLOW_TOOL_RESULT_MS {
            self.push_signal(json!({
                "kind": "slow_tool_result",
                "turn": turn,
                "tool": tool_name,
                "duration_ms": duration_ms,
            }));
        }
        if tool_result_is_truncated(data) {
            self.truncated_tool_results += 1;
            *self
                .tool_truncation_counts
                .entry(tool_name.to_string())
                .or_default() += 1;
            self.push_signal(json!({
                "kind": "tool_result_truncated",
                "turn": turn,
                "tool": tool_name,
                "output_chars": output_chars,
                "truncation": tool_truncation_fields(data),
            }));
        }
        if let Some(created_parent_dirs) = created_parent_dirs(data)
            && !created_parent_dirs.is_empty()
        {
            self.mutation_parent_dir_creations += created_parent_dirs.len();
            *self
                .mutation_parent_dir_creation_counts
                .entry(tool_name.to_string())
                .or_default() += created_parent_dirs.len();
            self.push_signal(json!({
                "kind": "mutation_created_parent_dirs",
                "turn": turn,
                "tool": tool_name,
                "created_parent_dirs": created_parent_dirs,
            }));
        }
        if !ok {
            self.tool_failures += 1;
            *self
                .tool_failure_counts
                .entry(tool_name.to_string())
                .or_default() += 1;
            self.push_signal(json!({
                "kind": "tool_failure",
                "turn": turn,
                "tool": tool_name,
                "output_chars": output_chars,
                "error": error,
            }));
        }
    }

    pub fn record_compaction(&mut self, report: &Value) {
        self.compactions += 1;
        match report.get("method").and_then(Value::as_str) {
            Some("responses_compact") => self.remote_compactions += 1,
            Some("local_fallback") => self.fallback_compactions += 1,
            _ => {}
        }
        self.compaction_reports
            .push(summarize_compaction_report(report));
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
            "tool_results": self.tool_results,
            "tool_failures": self.tool_failures,
            "truncated_tool_results": self.truncated_tool_results,
            "total_tool_duration_ms": self.total_tool_duration_ms,
            "max_tool_duration_ms": self.max_tool_duration_ms,
            "average_tool_duration_ms": if self.tool_results == 0 { 0 } else { self.total_tool_duration_ms / self.tool_results as u64 },
            "repeated_tool_calls": self.repeated_tool_calls,
            "consecutive_duplicate_tool_calls": self.consecutive_duplicate_tool_calls,
            "tool_only_turn_count": self.tool_only_turns,
            "tool_only_turns": {
                "count": self.tool_only_turns,
                "max_consecutive": self.max_consecutive_tool_only_turns,
                "turns": self.tool_only_turn_numbers,
            },
            "compactions": self.compactions,
            "remote_compactions": self.remote_compactions,
            "fallback_compactions": self.fallback_compactions,
            "readonly_tool_cache_hits": self.readonly_tool_cache_hits,
            "mutation_parent_dir_creations": self.mutation_parent_dir_creations,
            "max_input_chars": self.max_input_chars,
            "approx_context_window_tokens": SPARK_CONTEXT_WINDOW_TOKENS,
            "max_approx_input_tokens": approx_token_count_from_chars(self.max_input_chars),
            "max_context_window_pct": context_window_pct(self.max_input_chars),
            "average_input_chars": if self.requests == 0 { 0 } else { self.total_input_chars / self.requests },
            "input_chars_by_request": self.input_chars_by_request,
            "approx_input_tokens_by_request": self.input_chars_by_request.iter().copied().map(approx_token_count_from_chars).collect::<Vec<_>>(),
            "total_request_duration_ms": self.total_request_duration_ms,
            "max_request_duration_ms": self.max_request_duration_ms,
            "average_request_duration_ms": if self.request_duration_ms_by_request.is_empty() { 0 } else { self.total_request_duration_ms / self.request_duration_ms_by_request.len() as u64 },
            "request_duration_ms_by_request": self.request_duration_ms_by_request,
            "response_text_chars": self.response_text_chars,
            "errors": self.errors,
            "compaction_reports": self.compaction_reports,
            "tool_counts": self.tool_counts,
            "tool_failure_counts": self.tool_failure_counts,
            "tool_truncation_counts": self.tool_truncation_counts,
            "mutation_parent_dir_creation_counts": self.mutation_parent_dir_creation_counts,
            "tool_duration_ms_by_tool": self.tool_duration_ms_by_tool,
            "max_tool_duration_ms_by_tool": self.max_tool_duration_ms_by_tool,
            "diagnostics": diagnostics,
            "recent_signals": self.signals,
        })
    }

    pub fn status_line(&self) -> String {
        format!(
            "profile: requests={}, max_request_ms={}, tool_calls={}, tool_failures={}, repeated_calls={}, consecutive_duplicates={}, tool_only_turns={}, max_tool_only_streak={}, readonly_cache_hits={}, compactions={} (remote={}, fallback={}), max_input_chars={}, max_approx_input_tokens={} ({:.1}% of 128k)",
            self.requests,
            self.max_request_duration_ms,
            self.tool_calls,
            self.tool_failures,
            self.repeated_tool_calls,
            self.consecutive_duplicate_tool_calls,
            self.tool_only_turns,
            self.max_consecutive_tool_only_turns,
            self.readonly_tool_cache_hits,
            self.compactions,
            self.remote_compactions,
            self.fallback_compactions,
            self.max_input_chars,
            approx_token_count_from_chars(self.max_input_chars),
            context_window_pct(self.max_input_chars)
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

        if self.tool_failures > 0 {
            diagnostics.push(json!({
                "level": "warning",
                "kind": "tool_failures",
                "message": "One or more native tools returned a failure observation. Inspect recent_signals and tool_failure_counts before changing prompts or model settings.",
                "count": self.tool_failures,
                "tool_failure_counts": self.tool_failure_counts,
            }));
        }

        if self.truncated_tool_results > 0 {
            diagnostics.push(json!({
                "level": "info",
                "kind": "tool_result_truncation",
                "message": "One or more tool observations were truncated before being returned to Spark. Inspect recent_signals and rerun narrower commands or searches when exact output matters.",
                "count": self.truncated_tool_results,
                "tool_truncation_counts": self.tool_truncation_counts,
            }));
        }

        if self.mutation_parent_dir_creations > 0 {
            diagnostics.push(json!({
                "level": "info",
                "kind": "mutation_created_parent_dirs",
                "message": "One or more native file mutations created parent directories. Inspect recent_signals when unexpected path segments may indicate a typo.",
                "count": self.mutation_parent_dir_creations,
                "tool_counts": self.mutation_parent_dir_creation_counts,
            }));
        }

        if self.max_tool_duration_ms >= SLOW_TOOL_RESULT_MS {
            diagnostics.push(json!({
                "level": "info",
                "kind": "slow_tool_results",
                "message": "One or more native tools took at least 10 seconds. Inspect tool_duration_ms_by_tool and recent_signals to decide whether the harness needs a narrower tool or timeout tuning.",
                "max_tool_duration_ms": self.max_tool_duration_ms,
                "max_tool_duration_ms_by_tool": self.max_tool_duration_ms_by_tool,
            }));
        }

        if self.max_request_duration_ms >= SLOW_SPARK_REQUEST_MS {
            diagnostics.push(json!({
                "level": "info",
                "kind": "slow_spark_requests",
                "message": "One or more Spark response requests took at least 30 seconds. Compare request_duration_ms_by_request with input size and compaction timing.",
                "max_request_duration_ms": self.max_request_duration_ms,
                "request_duration_ms_by_request": self.request_duration_ms_by_request,
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

        if self.max_consecutive_tool_only_turns >= 3 {
            diagnostics.push(json!({
                "level": "info",
                "kind": "tool_only_turn_streak",
                "message": "Spark spent several consecutive turns calling tools without producing user-facing text. Compare this with task completion and context growth before changing harness defaults.",
                "count": self.tool_only_turns,
                "max_consecutive": self.max_consecutive_tool_only_turns,
                "turns": self.tool_only_turn_numbers,
            }));
        }

        if self.response_text_chars == 0 && self.max_consecutive_tool_only_turns >= 8 {
            diagnostics.push(json!({
                "level": "warning",
                "kind": "completion_starvation",
                "message": "Spark kept calling tools across many turns without emitting any user-facing response text. Profile tool-call sequence, compaction timing, and context growth before adding stop conditions or changing defaults.",
                "tool_only_turns": self.tool_only_turns,
                "max_consecutive": self.max_consecutive_tool_only_turns,
                "compactions": self.compactions,
                "remote_compactions": self.remote_compactions,
                "fallback_compactions": self.fallback_compactions,
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
            if let Some(duration_ms) = report.get("duration_ms").and_then(Value::as_u64)
                && duration_ms >= SLOW_SPARK_REQUEST_MS
            {
                diagnostics.push(json!({
                    "level": "info",
                    "kind": "slow_compaction",
                    "message": "A compaction request took at least 30 seconds. Compare duration_ms with before/after size and whether the compaction was forced.",
                    "duration_ms": duration_ms,
                    "forced": report.get("forced").and_then(Value::as_bool).unwrap_or(false),
                    "method": report.get("method").and_then(Value::as_str).unwrap_or("unknown"),
                }));
            }

            let before = report
                .get("before_chars")
                .or_else(|| report.pointer("/fallback/before_chars"))
                .and_then(Value::as_u64);
            let after = report
                .get("after_chars")
                .or_else(|| report.pointer("/fallback/after_chars"))
                .and_then(Value::as_u64);
            if let (Some(before), Some(after)) = (before, after)
                && after > before
            {
                diagnostics.push(json!({
                    "level": "warning",
                    "kind": "compaction_expanded_context",
                    "message": "A compaction made retained history larger. This can happen on tiny forced compactions due to encrypted summary overhead; avoid relying on it as a shrink signal.",
                    "before_chars": before,
                    "after_chars": after,
                    "forced": report.get("forced").and_then(Value::as_bool).unwrap_or(false),
                    "method": report.get("method").and_then(Value::as_str).unwrap_or("unknown"),
                }));
            }
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

            if report
                .get("local_pressure")
                .is_some_and(|pressure| !pressure.is_null())
            {
                let made_progress = report
                    .pointer("/local_pressure/made_progress")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                diagnostics.push(json!({
                    "level": if made_progress { "info" } else { "warning" },
                    "kind": "remote_compaction_local_pressure",
                    "message": if made_progress {
                        "Remote Codex compaction returned history above the harness threshold, so a local pressure pass reduced retained context before the next Spark request."
                    } else {
                        "Remote Codex compaction returned history above the harness threshold, and the local pressure pass could not reduce retained context."
                    },
                    "remote_after_chars": report.pointer("/local_pressure/remote_after_chars").cloned(),
                    "final_chars": report.pointer("/local_pressure/final_chars").cloned(),
                    "made_progress": made_progress,
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
        } else if approx_token_count_from_chars(self.max_input_chars) >= 100_000 {
            diagnostics.push(json!({
                "level": "warning",
                "kind": "near_context_window",
                "message": "Estimated request tokens approached Spark's 128k context window. Inspect approx_input_tokens_by_request and compaction timing.",
                "max_approx_input_tokens": approx_token_count_from_chars(self.max_input_chars),
                "context_window_tokens": SPARK_CONTEXT_WINDOW_TOKENS,
                "max_context_window_pct": context_window_pct(self.max_input_chars),
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

fn tool_truncation_fields(data: &Value) -> Value {
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

mod analyze;
mod format;

pub use analyze::analyze_trace;
#[cfg(test)]
use analyze::*;

pub use format::{
    format_trace_aggregate_row, format_trace_summary_row, format_trace_timeline,
    trace_aggregate_json, trace_profile_scenario_name,
};

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

#[cfg(test)]
mod tests;
