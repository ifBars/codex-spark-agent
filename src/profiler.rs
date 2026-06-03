use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

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
            "profile: requests={}, max_request_ms={}, tool_calls={}, tool_failures={}, repeated_calls={}, consecutive_duplicates={}, readonly_cache_hits={}, compactions={} (remote={}, fallback={}), max_input_chars={}, max_approx_input_tokens={} ({:.1}% of 128k)",
            self.requests,
            self.max_request_duration_ms,
            self.tool_calls,
            self.tool_failures,
            self.repeated_tool_calls,
            self.consecutive_duplicate_tool_calls,
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

fn tool_result_is_truncated(data: &Value) -> bool {
    data.get("truncated").and_then(Value::as_bool) == Some(true)
        || data.get("stdout_truncated").and_then(Value::as_bool) == Some(true)
        || data.get("stderr_truncated").and_then(Value::as_bool) == Some(true)
}

fn tool_result_timed_out(data: &Value) -> bool {
    data.get("timed_out").and_then(Value::as_bool) == Some(true)
}

fn created_parent_dirs(data: &Value) -> Option<Vec<String>> {
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

pub fn tool_signature(tool_name: &str, args: &Value) -> String {
    format!("{tool_name}:{}", canonical_json(args))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RequiredAction {
    tool: String,
    path: Option<String>,
    from: Option<String>,
    to: Option<String>,
    recursive: Option<bool>,
}

#[derive(Debug, Clone)]
struct ObservedToolCall {
    tool_name: String,
    args: Value,
}

#[derive(Debug)]
struct RequiredActionReport {
    actions: Vec<RequiredAction>,
    executed: Vec<RequiredAction>,
    missing: Vec<RequiredAction>,
    calls_before_first_required_action: usize,
}

pub fn analyze_trace(dir: &Path) -> Result<Value> {
    let mut profiler = AgentProfiler::default();
    let mut embedded_profile_summary = None;
    let mut embedded_profile_summary_rank = 0usize;
    let mut trace_metadata = None;
    let mut timeline = BTreeMap::<usize, Map<String, Value>>::new();
    let mut retained_required_actions = Vec::<RequiredAction>::new();
    let mut observed_tool_calls = Vec::<ObservedToolCall>::new();
    let mut files = std::fs::read_dir(dir)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    files.sort_by(|left, right| trace_file_sort_key(left).cmp(&trace_file_sort_key(right)));

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
        } else if is_profile_summary_trace_file(name) {
            let rank = profile_summary_rank(name, turn);
            if rank >= embedded_profile_summary_rank {
                embedded_profile_summary_rank = rank;
                embedded_profile_summary = Some(value);
            }
        } else if name.ends_with("-request-input.json") {
            let input_chars = value
                .get("input")
                .map(serde_json::to_string)
                .transpose()?
                .map(|input| input.len())
                .unwrap_or(raw.len());
            profiler.record_request(input_chars);
            let turn_entry = timeline_turn(&mut timeline, turn);
            turn_entry.insert("request_input_chars".to_string(), json!(input_chars));
            turn_entry.insert(
                "request_approx_tokens".to_string(),
                json!(approx_token_count_from_chars(input_chars)),
            );
            turn_entry.insert(
                "context_window_pct".to_string(),
                json!(context_window_pct(input_chars)),
            );
            retained_required_actions.extend(required_actions_from_request_input(&value));
        } else if name.ends_with("-response.json") {
            if let Some(duration_ms) = value.get("duration_ms").and_then(Value::as_u64) {
                profiler.record_request_duration(turn, duration_ms);
                timeline_turn(&mut timeline, turn)
                    .insert("request_duration_ms".to_string(), json!(duration_ms));
            }
            for (tool_name, args) in function_calls_from_trace_response(&value) {
                profiler.record_tool_call(turn, &tool_name, &args);
                observed_tool_calls.push(ObservedToolCall {
                    tool_name: tool_name.clone(),
                    args: args.clone(),
                });
                push_timeline_array(
                    &mut timeline,
                    turn,
                    "tool_calls",
                    json!({
                        "tool": tool_name,
                        "signature": tool_signature(&tool_name, &args),
                    }),
                );
            }
            if let Some(text) = response_text_from_trace_response(&value) {
                profiler.record_response_text(&text);
                timeline_turn(&mut timeline, turn)
                    .insert("response_text_chars".to_string(), json!(text.len()));
            }
        } else if is_tool_result_trace_file(name) {
            if let Some(result) = tool_result_from_trace(&value)? {
                profiler.record_tool_result(
                    turn,
                    &result.tool_name,
                    result.ok,
                    &result.data,
                    result.output_chars,
                    result.duration_ms,
                    result.error.as_deref(),
                );
                if result.cached_observation {
                    profiler.record_readonly_tool_cache_hit(turn, &result.tool_name, &result.args);
                }
                push_timeline_array(
                    &mut timeline,
                    turn,
                    "tool_results",
                    json!({
                        "tool": result.tool_name,
                        "ok": result.ok,
                        "duration_ms": result.duration_ms,
                        "output_chars": result.output_chars,
                        "error": result.error,
                        "cached_observation": result.cached_observation,
                        "truncated": tool_result_is_truncated(&result.data),
                        "timed_out": tool_result_timed_out(&result.data),
                        "created_parent_dirs": created_parent_dirs(&result.data).unwrap_or_default(),
                    }),
                );
            }
        } else if name.ends_with("-compaction.json") {
            profiler.record_compaction(&value);
            push_timeline_array(
                &mut timeline,
                turn,
                "compactions",
                summarize_compaction_report(&value),
            );
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
            push_timeline_array(
                &mut timeline,
                turn,
                "errors",
                json!({
                    "stage": stage,
                    "error": error,
                }),
            );
        }
    }

    retained_required_actions.sort_by(|left, right| {
        left.tool
            .cmp(&right.tool)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.from.cmp(&right.from))
            .then_with(|| left.to.cmp(&right.to))
            .then_with(|| left.recursive.cmp(&right.recursive))
    });
    retained_required_actions.dedup();

    let mut summary = profiler.to_json();
    let required_action_report =
        required_action_report(&retained_required_actions, &observed_tool_calls);
    let scenario_tool_expectation_report =
        scenario_tool_expectation_report(trace_metadata.as_ref(), &observed_tool_calls);
    let scenario_call_expectation_report =
        scenario_tool_call_expectation_report(trace_metadata.as_ref(), &observed_tool_calls);
    if let Some(object) = summary.as_object_mut() {
        object.insert(
            "timeline".to_string(),
            Value::Array(timeline.into_values().map(Value::Object).collect()),
        );
        if let Some(metadata) = trace_metadata {
            object.insert("trace_metadata".to_string(), metadata);
        }
        if let Some(embedded) = embedded_profile_summary {
            object.insert(
                "embedded_profile_summary".to_string(),
                sanitize_profile_summary(embedded),
            );
        }
        object.insert(
            "retained_required_actions".to_string(),
            json!(&required_action_report.actions),
        );
        object.insert(
            "retained_required_actions_executed".to_string(),
            json!(&required_action_report.executed),
        );
        object.insert(
            "retained_required_actions_missing".to_string(),
            json!(&required_action_report.missing),
        );
        object.insert(
            "tool_calls_before_first_required_action".to_string(),
            json!(required_action_report.calls_before_first_required_action),
        );
        if let Some(report) = &scenario_tool_expectation_report {
            object.insert(
                "profile_scenario_tool_expectations".to_string(),
                report.clone(),
            );
        }
        if let Some(report) = &scenario_call_expectation_report {
            object.insert(
                "profile_scenario_call_expectations".to_string(),
                report.clone(),
            );
        }
        if let Some(diagnostics) = object.get_mut("diagnostics").and_then(Value::as_array_mut) {
            if !required_action_report.missing.is_empty() {
                diagnostics.push(json!({
                    "level": "warning",
                    "kind": "retained_required_actions_missing",
                    "message": "One or more required actions retained by local compaction were not observed in the trace tool calls.",
                    "missing": &required_action_report.missing,
                }));
            }
            if required_action_report.calls_before_first_required_action > 0 {
                diagnostics.push(json!({
                    "level": "info",
                    "kind": "retained_required_action_detour",
                    "message": "Spark made tool calls before executing the first required action retained by local compaction.",
                    "calls_before_first_required_action": required_action_report.calls_before_first_required_action,
                }));
            }
            if let Some(report) = &scenario_tool_expectation_report
                && report
                    .get("missing_groups")
                    .and_then(Value::as_array)
                    .is_some_and(|missing| !missing.is_empty())
            {
                diagnostics.push(json!({
                    "level": "warning",
                    "kind": "profile_scenario_expected_tools_missing",
                    "message": "The trace did not include all native tool groups expected for this profiling scenario.",
                    "missing_groups": report.get("missing_groups").cloned().unwrap_or_else(|| json!([])),
                }));
            }
            if let Some(report) = &scenario_call_expectation_report
                && report
                    .get("missing_calls")
                    .and_then(Value::as_array)
                    .is_some_and(|missing| !missing.is_empty())
            {
                diagnostics.push(json!({
                    "level": "warning",
                    "kind": "profile_scenario_expected_calls_missing",
                    "message": "The trace did not include all exact native tool calls expected for this profiling scenario.",
                    "missing_calls": report.get("missing_calls").cloned().unwrap_or_else(|| json!([])),
                }));
            }
        }
    }
    Ok(summary)
}

pub fn format_trace_timeline(summary: &Value) -> String {
    let mut lines = Vec::new();
    if let Some(metadata) = summary.get("trace_metadata") {
        let model = metadata
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("unknown-model");
        let max_turns = metadata
            .get("max_turns")
            .map(compact_json_value)
            .unwrap_or_else(|| "null".to_string());
        let compact_after = metadata
            .get("compact_after_chars")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "?".to_string());
        let max_input = metadata
            .get("max_input_chars")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "?".to_string());
        let scenario = trace_scenario_name(metadata)
            .map(|name| format!(" scenario={name}"))
            .unwrap_or_default();
        lines.push(format!(
            "trace model={model}{scenario} max_turns={max_turns} compact_after_chars={compact_after} max_input_chars={max_input}"
        ));
    } else {
        lines.push("trace".to_string());
    }

    if let Some(diagnostics) = summary.get("diagnostics").and_then(Value::as_array)
        && !diagnostics.is_empty()
    {
        let kinds = diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.get("kind").and_then(Value::as_str))
            .collect::<Vec<_>>();
        lines.push(format!("diagnostics: {}", kinds.join(", ")));
    }

    if let Some(required_actions) = format_required_actions_summary(summary) {
        lines.push(required_actions);
    }
    if let Some(scenario_tools) = format_scenario_tool_expectations(summary) {
        lines.push(scenario_tools);
    }
    if let Some(scenario_calls) = format_scenario_call_expectations(summary) {
        lines.push(scenario_calls);
    }

    let Some(timeline) = summary.get("timeline").and_then(Value::as_array) else {
        lines.push("timeline: none".to_string());
        return format!("{}\n", lines.join("\n"));
    };

    if timeline.is_empty() {
        lines.push("timeline: empty".to_string());
        return format!("{}\n", lines.join("\n"));
    }

    for turn in timeline {
        lines.push(format_timeline_turn(turn));
    }

    format!("{}\n", lines.join("\n"))
}

pub fn format_trace_summary_row(label: &str, summary: &Value) -> String {
    let model = summary
        .pointer("/trace_metadata/model")
        .and_then(Value::as_str)
        .unwrap_or("unknown-model");
    let scenario = summary
        .get("trace_metadata")
        .and_then(trace_scenario_name)
        .map(|name| format!(" scenario={name}"))
        .unwrap_or_default();
    let requests = number_field(summary, "requests");
    let max_tokens = number_field(summary, "max_approx_input_tokens");
    let context_pct = summary
        .get("max_context_window_pct")
        .and_then(Value::as_f64)
        .map(|value| format!("{value:.1}%"))
        .unwrap_or_else(|| "?.?%".to_string());
    let max_request_ms = number_field(summary, "max_request_duration_ms");
    let tool_calls = number_field(summary, "tool_calls");
    let tool_failures = number_field(summary, "tool_failures");
    let compactions = number_field(summary, "compactions");
    let remote_compactions = number_field(summary, "remote_compactions");
    let fallback_compactions = number_field(summary, "fallback_compactions");
    let local_pressure_compactions = compactions_with_local_pressure(summary);
    let scenario_tools = format_scenario_tools_for_summary_row(summary);
    let scenario_calls = format_scenario_calls_for_summary_row(summary);
    let diagnostics = diagnostic_kinds(summary);
    let diagnostics = if diagnostics.is_empty() {
        "none".to_string()
    } else {
        diagnostics.join(",")
    };

    format!(
        "{label} | model={model}{scenario} requests={requests} max_tokens={max_tokens} ({context_pct}) max_request_ms={max_request_ms} tools={tool_calls} failures={tool_failures} compactions={compactions} remote={remote_compactions} fallback={fallback_compactions} local_pressure={local_pressure_compactions}{scenario_tools}{scenario_calls} diagnostics={diagnostics}"
    )
}

pub fn trace_profile_scenario_name(summary: &Value) -> Option<&str> {
    summary.get("trace_metadata").and_then(trace_scenario_name)
}

pub fn format_trace_aggregate_row(label: &str, summaries: &[Value]) -> String {
    let count = summaries.len();
    if count == 0 {
        return format!("{label} aggregate | runs=0");
    }

    let successes = summaries
        .iter()
        .filter(|summary| {
            summary
                .get("errors")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
        })
        .count();
    let failures = count.saturating_sub(successes);
    let max_tokens = summaries
        .iter()
        .filter_map(|summary| {
            summary
                .get("max_approx_input_tokens")
                .and_then(Value::as_u64)
        })
        .max()
        .unwrap_or(0);
    let max_context_pct = summaries
        .iter()
        .filter_map(|summary| {
            summary
                .get("max_context_window_pct")
                .and_then(Value::as_f64)
        })
        .fold(0.0, f64::max);
    let max_request_ms = summaries
        .iter()
        .filter_map(|summary| {
            summary
                .get("max_request_duration_ms")
                .and_then(Value::as_u64)
        })
        .max()
        .unwrap_or(0);
    let total_tools = sum_summary_field(summaries, "tool_calls");
    let total_tool_failures = sum_summary_field(summaries, "tool_failures");
    let total_compactions = sum_summary_field(summaries, "compactions");
    let total_remote_compactions = sum_summary_field(summaries, "remote_compactions");
    let total_fallback_compactions = sum_summary_field(summaries, "fallback_compactions");
    let total_local_pressure_compactions = summaries
        .iter()
        .map(compactions_with_local_pressure)
        .sum::<usize>();
    let diagnostics = aggregate_diagnostic_kinds(summaries);
    let diagnostics = if diagnostics.is_empty() {
        "none".to_string()
    } else {
        diagnostics.join(",")
    };

    format!(
        "{label} aggregate | runs={count} success={successes} failure={failures} max_tokens={max_tokens} ({max_context_pct:.1}%) max_request_ms={max_request_ms} tools={total_tools} failures={total_tool_failures} compactions={total_compactions} remote={total_remote_compactions} fallback={total_fallback_compactions} local_pressure={total_local_pressure_compactions} diagnostics={diagnostics}"
    )
}

fn trace_scenario_name(metadata: &Value) -> Option<&str> {
    metadata
        .pointer("/context/profile_scenario/name")
        .and_then(Value::as_str)
}

fn sum_summary_field(summaries: &[Value], key: &str) -> u64 {
    summaries
        .iter()
        .filter_map(|summary| summary.get(key).and_then(Value::as_u64))
        .sum()
}

fn compactions_with_local_pressure(summary: &Value) -> usize {
    summary
        .get("compaction_reports")
        .and_then(Value::as_array)
        .map(|reports| {
            reports
                .iter()
                .filter(|report| {
                    report
                        .get("local_pressure")
                        .is_some_and(|value| !value.is_null())
                })
                .count()
        })
        .unwrap_or(0)
}

fn aggregate_diagnostic_kinds(summaries: &[Value]) -> Vec<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for summary in summaries {
        for kind in diagnostic_kinds(summary) {
            *counts.entry(kind).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .map(|(kind, count)| format!("{kind}:{count}"))
        .collect()
}

fn format_required_actions_summary(summary: &Value) -> Option<String> {
    let total = summary
        .get("retained_required_actions")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if total == 0 {
        return None;
    }
    let executed = summary
        .get("retained_required_actions_executed")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let missing = summary
        .get("retained_required_actions_missing")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let detours = summary
        .get("tool_calls_before_first_required_action")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let actions = summary
        .get("retained_required_actions")
        .and_then(Value::as_array)
        .map(|actions| {
            actions
                .iter()
                .map(format_required_action)
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    Some(format!(
        "required-actions: total={total} executed={executed} missing={missing} detours_before_first={detours} actions=[{actions}]"
    ))
}

fn format_scenario_tool_expectations(summary: &Value) -> Option<String> {
    let report = summary.get("profile_scenario_tool_expectations")?;
    let total = report
        .get("total_groups")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if total == 0 {
        return None;
    }
    let satisfied = report
        .get("satisfied_groups")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let missing = report
        .get("missing_groups")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let groups = report
        .get("expected_tool_groups")
        .and_then(Value::as_array)
        .map(|groups| {
            groups
                .iter()
                .filter_map(format_tool_group)
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    Some(format!(
        "scenario-tools: satisfied={satisfied}/{total} missing={missing} groups=[{groups}]"
    ))
}

fn format_scenario_call_expectations(summary: &Value) -> Option<String> {
    let report = summary.get("profile_scenario_call_expectations")?;
    let total = report
        .get("total_calls")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if total == 0 {
        return None;
    }
    let satisfied = report
        .get("satisfied_calls")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let missing = report
        .get("missing_calls")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let calls = report
        .get("expected_calls")
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .map(format_required_action)
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    Some(format!(
        "scenario-calls: satisfied={satisfied}/{total} missing={missing} calls=[{calls}]"
    ))
}

fn format_scenario_tools_for_summary_row(summary: &Value) -> String {
    let Some(report) = summary.get("profile_scenario_tool_expectations") else {
        return String::new();
    };
    let total = report
        .get("total_groups")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if total == 0 {
        return String::new();
    }
    let satisfied = report
        .get("satisfied_groups")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    format!(" scenario_tools={satisfied}/{total}")
}

fn format_scenario_calls_for_summary_row(summary: &Value) -> String {
    let Some(report) = summary.get("profile_scenario_call_expectations") else {
        return String::new();
    };
    let total = report
        .get("total_calls")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if total == 0 {
        return String::new();
    }
    let satisfied = report
        .get("satisfied_calls")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    format!(" scenario_calls={satisfied}/{total}")
}

fn format_tool_group(group: &Value) -> Option<String> {
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

fn format_required_action(action: &Value) -> String {
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

fn number_field(summary: &Value, key: &str) -> String {
    summary
        .get(key)
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".to_string())
}

fn diagnostic_kinds(summary: &Value) -> Vec<String> {
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

fn format_timeline_turn(turn: &Value) -> String {
    let turn_number = turn.get("turn").and_then(Value::as_u64).unwrap_or(0);
    let input_chars = turn
        .get("request_input_chars")
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".to_string());
    let approx_tokens = turn
        .get("request_approx_tokens")
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".to_string());
    let context_pct = turn
        .get("context_window_pct")
        .and_then(Value::as_f64)
        .map(|value| format!("{value:.1}%"))
        .unwrap_or_else(|| "?.?%".to_string());
    let request_ms = turn
        .get("request_duration_ms")
        .and_then(Value::as_u64)
        .map(|value| format!("{value}ms"))
        .unwrap_or_else(|| "?ms".to_string());
    let response_chars = turn
        .get("response_text_chars")
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "0".to_string());

    let mut parts = vec![format!(
        "turn {turn_number}: input={input_chars} chars (~{approx_tokens} tok, {context_pct}) request={request_ms} text={response_chars} chars"
    )];

    if let Some(tools) = turn.get("tool_calls").and_then(Value::as_array)
        && !tools.is_empty()
    {
        parts.push(format!("calls=[{}]", format_tool_calls(tools)));
    }
    if let Some(results) = turn.get("tool_results").and_then(Value::as_array)
        && !results.is_empty()
    {
        parts.push(format!("results=[{}]", format_tool_results(results)));
    }
    if let Some(compactions) = turn.get("compactions").and_then(Value::as_array)
        && !compactions.is_empty()
    {
        parts.push(format!("compactions=[{}]", format_compactions(compactions)));
    }
    if let Some(errors) = turn.get("errors").and_then(Value::as_array)
        && !errors.is_empty()
    {
        parts.push(format!("errors=[{}]", format_errors(errors)));
    }

    parts.join(" ")
}

fn format_tool_calls(tools: &[Value]) -> String {
    tools
        .iter()
        .map(|tool| {
            tool.get("tool")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_tool_results(results: &[Value]) -> String {
    results
        .iter()
        .map(|result| {
            let tool = result
                .get("tool")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let ok = result
                .get("ok")
                .and_then(Value::as_bool)
                .map(|ok| if ok { "ok" } else { "fail" })
                .unwrap_or("?");
            let duration_ms = result
                .get("duration_ms")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let output_chars = result
                .get("output_chars")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let mut suffix = Vec::new();
            if result
                .get("cached_observation")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                suffix.push("cached");
            }
            if result
                .get("truncated")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                suffix.push("truncated");
            }
            if result
                .get("timed_out")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                suffix.push("timeout");
            }
            let parent_suffix = result
                .get("created_parent_dirs")
                .and_then(Value::as_array)
                .filter(|dirs| !dirs.is_empty())
                .map(|dirs| {
                    let dirs = dirs
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("|");
                    format!(" parents={dirs}")
                })
                .unwrap_or_default();
            let suffix = if suffix.is_empty() {
                String::new()
            } else {
                format!(" {}", suffix.join("+"))
            };
            format!("{tool}:{ok} {duration_ms}ms {output_chars} chars{suffix}{parent_suffix}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_compactions(compactions: &[Value]) -> String {
    compactions
        .iter()
        .map(|compaction| {
            let method = compaction
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let before = compaction
                .get("before_chars")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string());
            let after = compaction
                .get("after_chars")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string());
            let mut parts = vec![format!("{method} {before}->{after}")];
            if let Some(remote_after) = compaction.get("remote_after_chars").and_then(Value::as_u64)
            {
                let remote_pct = compaction
                    .get("remote_retained_pct")
                    .and_then(Value::as_f64)
                    .map(|pct| format!(" {pct:.1}%"))
                    .unwrap_or_default();
                parts.push(format!("remote={remote_after}{remote_pct}"));
            }
            if let (Some(remote_after), Some(final_chars)) = (
                compaction.get("remote_after_chars").and_then(Value::as_u64),
                compaction
                    .get("local_pressure_final_chars")
                    .and_then(Value::as_u64),
            ) {
                parts.push(format!("local_pressure={remote_after}->{final_chars}"));
            }
            parts.join(" ")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_errors(errors: &[Value]) -> String {
    errors
        .iter()
        .map(|error| {
            let stage = error
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let message = error
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            format!("{stage}:{}", truncate_for_line(message, 80))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn compact_json_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

fn truncate_for_line(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut truncated = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

fn timeline_turn(
    timeline: &mut BTreeMap<usize, Map<String, Value>>,
    turn: usize,
) -> &mut Map<String, Value> {
    timeline.entry(turn).or_insert_with(|| {
        let mut entry = Map::new();
        entry.insert("turn".to_string(), json!(turn));
        entry
    })
}

fn push_timeline_array(
    timeline: &mut BTreeMap<usize, Map<String, Value>>,
    turn: usize,
    key: &str,
    value: Value,
) {
    let entry = timeline_turn(timeline, turn);
    match entry.get_mut(key) {
        Some(Value::Array(items)) => items.push(value),
        _ => {
            entry.insert(key.to_string(), Value::Array(vec![value]));
        }
    }
}

fn is_tool_result_trace_file(name: &str) -> bool {
    name.ends_with("-tool-result.json") || name.contains("-tool-result-")
}

fn trace_file_sort_key(path: &Path) -> (usize, String, usize, String) {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let stem = name.strip_suffix(".json").unwrap_or(name);
    let (turn, rest) = stem
        .split_once('-')
        .and_then(|(turn, rest)| turn.parse::<usize>().ok().map(|turn| (turn, rest)))
        .unwrap_or((usize::MAX, stem));
    let (kind, sequence) = rest
        .rsplit_once('-')
        .and_then(|(kind, suffix)| {
            suffix
                .parse::<usize>()
                .ok()
                .map(|sequence| (kind, sequence))
        })
        .unwrap_or((rest, 1));
    (turn, kind.to_string(), sequence, name.to_string())
}

fn summarize_compaction_report(report: &Value) -> Value {
    let mut summary = Map::new();
    copy_field(report, &mut summary, "method");
    copy_field(report, &mut summary, "forced");
    copy_field(report, &mut summary, "duration_ms");
    copy_field(report, &mut summary, "before_chars");
    copy_field(report, &mut summary, "compact_request_chars");
    copy_field(report, &mut summary, "after_chars");
    copy_field(report, &mut summary, "threshold_chars");
    copy_field(report, &mut summary, "compacted_outputs");
    copy_field(report, &mut summary, "compacted_tool_outputs");
    copy_field(report, &mut summary, "compacted_messages");
    copy_field(report, &mut summary, "remote_error");
    copy_field(report, &mut summary, "fallback");
    copy_field(report, &mut summary, "local_pressure");
    add_compaction_retention_metrics(report, &mut summary);

    if let Some(raw) = report.get("raw") {
        let mut raw_summary = Map::new();
        copy_field(raw, &mut raw_summary, "object");
        copy_field(raw, &mut raw_summary, "id");
        copy_field(raw, &mut raw_summary, "created_at");
        copy_field(raw, &mut raw_summary, "usage");
        if let Some(output) = raw.get("output").and_then(Value::as_array) {
            raw_summary.insert("output_items".to_string(), json!(output.len()));
            raw_summary.insert(
                "output_types".to_string(),
                Value::Array(
                    output
                        .iter()
                        .filter_map(|item| item.get("type").and_then(Value::as_str))
                        .map(|kind| Value::String(kind.to_string()))
                        .collect(),
                ),
            );
        }
        if !raw_summary.is_empty() {
            summary.insert("raw_summary".to_string(), Value::Object(raw_summary));
        }
    }

    Value::Object(summary)
}

fn add_compaction_retention_metrics(report: &Value, summary: &mut Map<String, Value>) {
    let before = report.get("before_chars").and_then(Value::as_u64);
    let after = report.get("after_chars").and_then(Value::as_u64);
    if let (Some(before), Some(after)) = (before, after)
        && before > 0
    {
        summary.insert(
            "final_retained_pct".to_string(),
            json!(percent(after, before)),
        );
    }

    let remote_after = report
        .pointer("/local_pressure/remote_after_chars")
        .and_then(Value::as_u64);
    if let Some(remote_after) = remote_after {
        summary.insert("remote_after_chars".to_string(), json!(remote_after));
        if let Some(before) = before
            && before > 0
        {
            summary.insert(
                "remote_retained_pct".to_string(),
                json!(percent(remote_after, before)),
            );
        }
    }

    let final_chars = report
        .pointer("/local_pressure/final_chars")
        .and_then(Value::as_u64);
    if let Some(final_chars) = final_chars {
        summary.insert("local_pressure_final_chars".to_string(), json!(final_chars));
    }

    if let (Some(remote_after), Some(final_chars)) = (remote_after, final_chars)
        && remote_after > 0
    {
        let reduced = remote_after.saturating_sub(final_chars);
        summary.insert(
            "local_pressure_reduction_pct".to_string(),
            json!(percent(reduced, remote_after)),
        );
    }
}

fn percent(part: u64, whole: u64) -> f64 {
    (part as f64 / whole as f64) * 100.0
}

fn sanitize_profile_summary(mut summary: Value) -> Value {
    let Some(object) = summary.as_object_mut() else {
        return summary;
    };
    if let Some(reports) = object
        .get_mut("compaction_reports")
        .and_then(Value::as_array_mut)
    {
        for report in reports {
            *report = summarize_compaction_report(report);
        }
    }
    summary
}

fn copy_field(source: &Value, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key) {
        target.insert(key.to_string(), value.clone());
    }
}

fn is_profile_summary_trace_file(name: &str) -> bool {
    name.ends_with("-profile-summary.json") || name.contains("-profile-summary-")
}

fn profile_summary_rank(name: &str, turn: usize) -> usize {
    let Some(stem) = name.strip_suffix(".json") else {
        return turn.saturating_mul(10_000);
    };
    let duplicate = stem
        .split_once("-profile-summary")
        .and_then(|(_, suffix)| suffix.strip_prefix('-'))
        .and_then(|suffix| suffix.parse::<usize>().ok())
        .unwrap_or(1);
    turn.saturating_mul(10_000).saturating_add(duplicate)
}

struct TraceToolResult {
    tool_name: String,
    args: Value,
    ok: bool,
    data: Value,
    output_chars: usize,
    duration_ms: u64,
    error: Option<String>,
    cached_observation: bool,
}

fn tool_result_from_trace(value: &Value) -> Result<Option<TraceToolResult>> {
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

fn required_actions_from_request_input(value: &Value) -> Vec<RequiredAction> {
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

fn required_action_report(
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

fn scenario_tool_expectation_report(
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

fn scenario_tool_call_expectation_report(
    metadata: Option<&Value>,
    calls: &[ObservedToolCall],
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

    Some(json!({
        "expected_calls": expected_calls,
        "total_calls": satisfied.len() + missing.len(),
        "satisfied_calls": satisfied.len(),
        "missing_calls": missing,
        "satisfied_tool_calls": satisfied,
    }))
}

fn required_action_from_value(value: &Value) -> Option<RequiredAction> {
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

fn required_action_matches_call(action: &RequiredAction, call: &ObservedToolCall) -> bool {
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
    use std::path::Path;

    use serde_json::json;

    use super::*;

    fn write_request_with_required_action(dir: &Path, action: &str) {
        write_turn_request_with_required_action(dir, 1, action);
    }

    fn write_turn_request_with_required_action(dir: &Path, turn: usize, action: &str) {
        std::fs::write(
            dir.join(format!("{turn:03}-request-input.json")),
            serde_json::to_vec_pretty(&json!({
                "input": [{
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": format!(
                            "[spark local message compaction]\nrequired_actions=1\n{action}\n[/spark local message compaction]"
                        )
                    }]
                }]
            }))
            .expect("serialize request"),
        )
        .expect("write request");
    }

    fn write_trace_metadata_with_expected_tools(dir: &Path, groups: Value) {
        std::fs::write(
            dir.join("000-trace-metadata.json"),
            serde_json::to_vec_pretty(&json!({
                "model": "gpt-5.3-codex-spark",
                "context": {
                    "profile_scenario": {
                        "name": "file-ops",
                        "expected_tool_groups": groups,
                    }
                }
            }))
            .expect("serialize metadata"),
        )
        .expect("write metadata");
    }

    fn write_trace_metadata_with_expected_tool_calls(dir: &Path, calls: Value) {
        std::fs::write(
            dir.join("000-trace-metadata.json"),
            serde_json::to_vec_pretty(&json!({
                "model": "gpt-5.3-codex-spark",
                "context": {
                    "profile_scenario": {
                        "name": "file-ops",
                        "expected_tool_calls": calls,
                    }
                }
            }))
            .expect("serialize metadata"),
        )
        .expect("write metadata");
    }

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
    fn profiler_records_tool_failures() {
        let mut profiler = AgentProfiler::default();

        profiler.record_tool_result(
            1,
            "cmd.exec",
            false,
            &json!({"code": 1}),
            128,
            250,
            Some("command exited with code 1"),
        );

        let summary = profiler.to_json();

        assert_eq!(summary["tool_results"], 1);
        assert_eq!(summary["tool_failures"], 1);
        assert_eq!(summary["tool_failure_counts"]["cmd.exec"], 1);
        assert_eq!(summary["recent_signals"][0]["kind"], "tool_failure");
        assert!(
            summary["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|diagnostic| diagnostic["kind"] == "tool_failures")
        );
    }

    #[test]
    fn profiler_records_tool_result_truncation() {
        let mut profiler = AgentProfiler::default();

        profiler.record_tool_result(
            1,
            "cmd.exec",
            true,
            &json!({
                "stdout_truncated": true,
                "stderr_truncated": false,
                "stdout_chars": 40_000,
                "stderr_chars": 0
            }),
            24_512,
            400,
            None,
        );

        let summary = profiler.to_json();

        assert_eq!(summary["truncated_tool_results"], 1);
        assert_eq!(summary["tool_truncation_counts"]["cmd.exec"], 1);
        assert_eq!(
            summary["recent_signals"][0]["kind"],
            "tool_result_truncated"
        );
        assert_eq!(
            summary["recent_signals"][0]["truncation"]["stdout_truncated"],
            true
        );
        assert!(
            summary["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|diagnostic| diagnostic["kind"] == "tool_result_truncation")
        );
    }

    #[test]
    fn profiler_records_parent_dirs_created_by_mutation_tools() {
        let mut profiler = AgentProfiler::default();

        profiler.record_tool_result(
            3,
            "fs.rename",
            true,
            &json!({"created_parent_dirs": ["nested", "nested/final"]}),
            128,
            1,
            None,
        );
        let summary = profiler.to_json();

        assert_eq!(summary["mutation_parent_dir_creations"], 2);
        assert_eq!(
            summary["mutation_parent_dir_creation_counts"]["fs.rename"],
            2
        );
        assert_eq!(
            summary["recent_signals"][0]["kind"],
            "mutation_created_parent_dirs"
        );
        assert_eq!(
            summary["recent_signals"][0]["created_parent_dirs"],
            json!(["nested", "nested/final"])
        );
        assert!(
            summary["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|diagnostic| diagnostic["kind"] == "mutation_created_parent_dirs")
        );
    }

    #[test]
    fn profiler_records_tool_duration_and_slow_diagnostics() {
        let mut profiler = AgentProfiler::default();

        profiler.record_tool_result(1, "cmd.exec", true, &json!({}), 64, 12_345, None);

        let summary = profiler.to_json();

        assert_eq!(summary["total_tool_duration_ms"], 12_345);
        assert_eq!(summary["max_tool_duration_ms"], 12_345);
        assert_eq!(summary["average_tool_duration_ms"], 12_345);
        assert_eq!(summary["tool_duration_ms_by_tool"]["cmd.exec"], 12_345);
        assert_eq!(summary["max_tool_duration_ms_by_tool"]["cmd.exec"], 12_345);
        assert_eq!(summary["recent_signals"][0]["kind"], "slow_tool_result");
        assert!(
            summary["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|diagnostic| diagnostic["kind"] == "slow_tool_results")
        );
    }

    #[test]
    fn profiler_records_request_duration_and_slow_diagnostics() {
        let mut profiler = AgentProfiler::default();

        profiler.record_request(120);
        profiler.record_request_duration(1, 31_000);

        let summary = profiler.to_json();

        assert_eq!(summary["total_request_duration_ms"], 31_000);
        assert_eq!(summary["max_request_duration_ms"], 31_000);
        assert_eq!(summary["average_request_duration_ms"], 31_000);
        assert_eq!(summary["request_duration_ms_by_request"], json!([31_000]));
        assert_eq!(summary["recent_signals"][0]["kind"], "slow_spark_request");
        assert!(
            summary["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|diagnostic| diagnostic["kind"] == "slow_spark_requests")
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
        assert_eq!(summary["approx_context_window_tokens"], 128_000);
        assert_eq!(summary["max_approx_input_tokens"], 60);
        assert_eq!(summary["approx_input_tokens_by_request"], json!([30, 60]));
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
    fn profiler_diagnoses_near_context_window_before_input_guard() {
        let mut profiler = AgentProfiler::default();

        profiler.record_request(400_000);

        let diagnostics = profiler
            .to_json()
            .get("diagnostics")
            .and_then(Value::as_array)
            .cloned()
            .expect("diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic["kind"] == "near_context_window"
                && diagnostic["max_approx_input_tokens"] == 100_000
                && diagnostic["context_window_tokens"] == 128_000
        }));
        assert!(
            !diagnostics
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
    fn profiler_diagnoses_compaction_expansion() {
        let mut profiler = AgentProfiler::default();

        profiler.record_compaction(&json!({
            "method": "responses_compact",
            "forced": true,
            "duration_ms": 31_000,
            "before_chars": 200,
            "after_chars": 1200
        }));

        let diagnostics = profiler
            .to_json()
            .get("diagnostics")
            .and_then(Value::as_array)
            .cloned()
            .expect("diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic["kind"] == "compaction_expanded_context"
                && diagnostic["forced"] == true
                && diagnostic["method"] == "responses_compact"
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic["kind"] == "slow_compaction"
                && diagnostic["duration_ms"] == 31_000
                && diagnostic["forced"] == true
        }));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic["kind"] == "weak_compaction_shrink")
        );
    }

    #[test]
    fn profiler_summarizes_compaction_reports_without_raw_payload() {
        let mut profiler = AgentProfiler::default();

        profiler.record_compaction(&json!({
            "method": "responses_compact",
            "forced": true,
            "duration_ms": 1234,
            "before_chars": 200,
            "after_chars": 1200,
            "raw": {
                "id": "resp_123",
                "object": "response.compaction",
                "created_at": 12345,
                "usage": {"total_tokens": 42},
                "output": [
                    {
                        "type": "compaction_summary",
                        "encrypted_content": "very-secret-large-payload"
                    }
                ]
            }
        }));

        let summary = profiler.to_json();
        let report = &summary["compaction_reports"][0];

        assert!(report.get("raw").is_none());
        assert_eq!(report["method"], "responses_compact");
        assert_eq!(report["forced"], true);
        assert_eq!(report["duration_ms"], 1234);
        assert_eq!(report["raw_summary"]["id"], "resp_123");
        assert_eq!(report["raw_summary"]["output_items"], 1);
        assert_eq!(
            report["raw_summary"]["output_types"],
            json!(["compaction_summary"])
        );
        assert_eq!(report["raw_summary"]["usage"]["total_tokens"], 42);
    }

    #[test]
    fn profiler_reports_remote_compaction_local_pressure() {
        let mut profiler = AgentProfiler::default();

        profiler.record_compaction(&json!({
            "method": "responses_compact",
            "before_chars": 220_000,
            "after_chars": 100_000,
            "local_pressure": {
                "reason": "remote_compaction_above_threshold",
                "remote_after_chars": 190_000,
                "final_chars": 100_000,
                "made_progress": true
            }
        }));

        let diagnostics = profiler
            .to_json()
            .get("diagnostics")
            .and_then(Value::as_array)
            .cloned()
            .expect("diagnostics");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic["kind"] == "remote_compaction_local_pressure"
                && diagnostic["remote_after_chars"] == 190_000
                && diagnostic["final_chars"] == 100_000
                && diagnostic["made_progress"] == true
        }));
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
                "duration_ms": 1234,
                "raw": {
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
                }
            }))
            .expect("serialize response"),
        )
        .expect("write response");

        let summary = analyze_trace(dir.path()).expect("analyze trace");

        assert_eq!(summary["requests"], 1);
        assert_eq!(summary["tool_calls"], 1);
        assert_eq!(summary["tool_counts"]["fs.read"], 1);
        assert_eq!(summary["response_text_chars"], 4);
        assert_eq!(summary["request_duration_ms_by_request"], json!([1234]));
        assert_eq!(summary["max_request_duration_ms"], 1234);
        assert_eq!(summary["timeline"][0]["turn"], 1);
        assert!(
            summary["timeline"][0]["request_input_chars"]
                .as_u64()
                .expect("request chars")
                > 2
        );
        assert_eq!(summary["timeline"][0]["request_duration_ms"], 1234);
        assert_eq!(summary["timeline"][0]["response_text_chars"], 4);
        assert_eq!(summary["timeline"][0]["tool_calls"][0]["tool"], "fs.read");
    }

    #[test]
    fn analyze_trace_matches_retained_required_actions_to_tool_calls() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_request_with_required_action(
            dir.path(),
            "action_1=tool=fs.list path=src recursive=false",
        );
        write_turn_request_with_required_action(
            dir.path(),
            2,
            "action_1=tool=fs.list path=src recursive=false",
        );
        std::fs::write(
            dir.path().join("001-response.json"),
            serde_json::to_vec_pretty(&json!({
                "raw": {
                    "events": [{
                        "type": "response.output_item.done",
                        "output_index": 0,
                        "item": {
                            "type": "function_call",
                            "name": "fs_list",
                            "arguments": "{\"path\":\"src\",\"recursive\":false}"
                        }
                    }]
                }
            }))
            .expect("serialize response"),
        )
        .expect("write response");

        let summary = analyze_trace(dir.path()).expect("analyze trace");

        assert_eq!(
            summary["retained_required_actions"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(summary["retained_required_actions"][0]["tool"], "fs.list");
        assert_eq!(
            summary["retained_required_actions_executed"][0]["path"],
            "src"
        );
        assert_eq!(summary["retained_required_actions_missing"], json!([]));
        assert_eq!(summary["tool_calls_before_first_required_action"], 0);
    }

    #[test]
    fn analyze_trace_matches_retained_rename_required_action() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_request_with_required_action(
            dir.path(),
            "action_1=tool=fs.rename from=.spark-scenarios/file-ops/drafts/report-draft.md to=.spark-scenarios/file-ops/final/report.md",
        );
        std::fs::write(
            dir.path().join("001-response.json"),
            serde_json::to_vec_pretty(&json!({
                "raw": {
                    "events": [{
                        "type": "response.output_item.done",
                        "output_index": 0,
                        "item": {
                            "type": "function_call",
                            "name": "fs_rename",
                            "arguments": "{\"from\":\".spark-scenarios/file-ops/drafts/report-draft.md\",\"to\":\".spark-scenarios/file-ops/final/report.md\"}"
                        }
                    }]
                }
            }))
            .expect("serialize response"),
        )
        .expect("write response");

        let summary = analyze_trace(dir.path()).expect("analyze trace");

        assert_eq!(summary["retained_required_actions_missing"], json!([]));
        assert_eq!(
            summary["retained_required_actions_executed"][0]["tool"],
            "fs.rename"
        );
        assert_eq!(
            summary["retained_required_actions_executed"][0]["from"],
            ".spark-scenarios/file-ops/drafts/report-draft.md"
        );
        assert_eq!(
            summary["retained_required_actions_executed"][0]["to"],
            ".spark-scenarios/file-ops/final/report.md"
        );
    }

    #[test]
    fn analyze_trace_reports_profile_scenario_tool_expectations() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_trace_metadata_with_expected_tools(
            dir.path(),
            json!([["fs.write"], ["fs.rename"], ["fs.read"], ["fs.search"]]),
        );
        std::fs::write(
            dir.path().join("001-response.json"),
            serde_json::to_vec_pretty(&json!({
                "raw": {
                    "events": [
                        {
                            "type": "response.output_item.done",
                            "output_index": 0,
                            "item": {
                                "type": "function_call",
                                "name": "fs_write",
                                "arguments": "{\"path\":\"draft.md\",\"content\":\"hello\"}"
                            }
                        },
                        {
                            "type": "response.output_item.done",
                            "output_index": 1,
                            "item": {
                                "type": "function_call",
                                "name": "fs_rename",
                                "arguments": "{\"from\":\"draft.md\",\"to\":\"final.md\"}"
                            }
                        }
                    ]
                }
            }))
            .expect("serialize response"),
        )
        .expect("write response");

        let summary = analyze_trace(dir.path()).expect("analyze trace");

        assert_eq!(
            summary["profile_scenario_tool_expectations"]["total_groups"],
            4
        );
        assert_eq!(
            summary["profile_scenario_tool_expectations"]["satisfied_groups"],
            2
        );
        assert_eq!(
            summary["profile_scenario_tool_expectations"]["missing_groups"],
            json!([["fs.read"], ["fs.search"]])
        );
        assert!(
            summary["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|diagnostic| diagnostic["kind"] == "profile_scenario_expected_tools_missing")
        );
        assert!(format_trace_timeline(&summary).contains(
            "scenario-tools: satisfied=2/4 missing=2 groups=[fs.write,fs.rename,fs.read,fs.search]"
        ));
        assert!(
            format_trace_summary_row(".spark-runs/run-1", &summary).contains("scenario_tools=2/4")
        );
    }

    #[test]
    fn analyze_trace_reports_missing_profile_scenario_exact_tool_calls() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_trace_metadata_with_expected_tool_calls(
            dir.path(),
            json!([
                {
                    "tool": "fs.rename",
                    "from": ".spark-scenarios/file-ops/drafts/report-draft.md",
                    "to": ".spark-scenarios/file-ops/final/report.md"
                },
                {
                    "tool": "fs.read",
                    "path": ".spark-scenarios/file-ops/final/report.md"
                }
            ]),
        );
        std::fs::write(
            dir.path().join("001-response.json"),
            serde_json::to_vec_pretty(&json!({
                "raw": {
                    "events": [
                        {
                            "type": "response.output_item.done",
                            "output_index": 0,
                            "item": {
                                "type": "function_call",
                                "name": "fs_rename",
                                "arguments": "{\"from\":\".spark-scenarios/file-ops/drafts/report-draft.md\",\"to\":\".spark-scenarios/file-sops/final/report.md\"}"
                            }
                        },
                        {
                            "type": "response.output_item.done",
                            "output_index": 1,
                            "item": {
                                "type": "function_call",
                                "name": "fs_read",
                                "arguments": "{\"path\":\".spark-scenarios/file-ops/final/report.md\"}"
                            }
                        }
                    ]
                }
            }))
            .expect("serialize response"),
        )
        .expect("write response");

        let summary = analyze_trace(dir.path()).expect("analyze trace");

        assert_eq!(
            summary["profile_scenario_call_expectations"]["total_calls"],
            2
        );
        assert_eq!(
            summary["profile_scenario_call_expectations"]["satisfied_calls"],
            1
        );
        assert_eq!(
            summary["profile_scenario_call_expectations"]["missing_calls"][0]["tool"],
            "fs.rename"
        );
        assert!(
            summary["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|diagnostic| diagnostic["kind"] == "profile_scenario_expected_calls_missing")
        );
        assert!(
            format_trace_timeline(&summary).contains("scenario-calls: satisfied=1/2 missing=1")
        );
        assert!(
            format_trace_summary_row(".spark-runs/run-1", &summary).contains("scenario_calls=1/2")
        );
    }

    #[test]
    fn analyze_trace_reports_detour_before_retained_required_action() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_request_with_required_action(
            dir.path(),
            "action_1=tool=fs.list path=src recursive=false",
        );
        std::fs::write(
            dir.path().join("001-response.json"),
            serde_json::to_vec_pretty(&json!({
                "raw": {
                    "events": [
                        {
                            "type": "response.output_item.done",
                            "output_index": 0,
                            "item": {
                                "type": "function_call",
                                "name": "fs_list",
                                "arguments": "{\"path\":\".\",\"recursive\":false}"
                            }
                        },
                        {
                            "type": "response.output_item.done",
                            "output_index": 1,
                            "item": {
                                "type": "function_call",
                                "name": "fs_list",
                                "arguments": "{\"path\":\"src\",\"recursive\":false}"
                            }
                        }
                    ]
                }
            }))
            .expect("serialize response"),
        )
        .expect("write response");

        let summary = analyze_trace(dir.path()).expect("analyze trace");

        assert_eq!(summary["retained_required_actions_missing"], json!([]));
        assert_eq!(summary["tool_calls_before_first_required_action"], 1);
        assert!(
            summary["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|diagnostic| diagnostic["kind"] == "retained_required_action_detour")
        );
    }

    #[test]
    fn trace_file_sort_preserves_repeated_entry_sequence() {
        let first = Path::new("001-tool-result.json");
        let second = Path::new("001-tool-result-002.json");

        assert!(trace_file_sort_key(first) < trace_file_sort_key(second));
    }

    #[test]
    fn formats_trace_timeline_for_human_scan() {
        let summary = json!({
            "trace_metadata": {
                "model": "gpt-5.3-codex-spark",
                "context": {"profile_scenario": {"name": "compaction-pressure"}},
                "max_turns": null,
                "compact_after_chars": 160000,
                "max_input_chars": 500000
            },
            "diagnostics": [{"kind": "tool_failures"}],
            "retained_required_actions": [{"tool": "fs.list", "path": "src", "recursive": false}],
            "retained_required_actions_executed": [{"tool": "fs.list", "path": "src", "recursive": false}],
            "retained_required_actions_missing": [],
            "tool_calls_before_first_required_action": 0,
            "timeline": [{
                "turn": 1,
                "request_input_chars": 120000,
                "request_approx_tokens": 30000,
                "context_window_pct": 23.4375,
                "request_duration_ms": 1234,
                "response_text_chars": 42,
                "tool_calls": [{"tool": "fs.read", "signature": "fs.read:{\"path\":\"a.txt\"}"}],
                "tool_results": [{
                    "tool": "fs.read",
                    "ok": true,
                    "duration_ms": 9,
                    "output_chars": 512,
                    "cached_observation": true,
                    "truncated": false,
                    "timed_out": true,
                    "created_parent_dirs": ["nested"]
                }],
                "compactions": [{
                    "method": "responses_compact",
                    "before_chars": 200000,
                    "after_chars": 90000,
                    "remote_after_chars": 210000,
                    "remote_retained_pct": 105.0,
                    "local_pressure_final_chars": 90000
                }],
                "errors": [{"stage": "response", "error": "stream ended without response.completed"}]
            }]
        });

        let output = format_trace_timeline(&summary);

        assert!(output.contains("trace model=gpt-5.3-codex-spark scenario=compaction-pressure"));
        assert!(output.contains("diagnostics: tool_failures"));
        assert!(output.contains("required-actions: total=1 executed=1 missing=0 detours_before_first=0 actions=[tool=fs.list path=src recursive=false]"));
        assert!(output.contains("turn 1: input=120000 chars (~30000 tok, 23.4%)"));
        assert!(output.contains("calls=[fs.read]"));
        assert!(
            output.contains("results=[fs.read:ok 9ms 512 chars cached+timeout parents=nested]")
        );
        assert!(output.contains(
            "compactions=[responses_compact 200000->90000 remote=210000 105.0% local_pressure=210000->90000]"
        ));
        assert!(output.contains("errors=[response:stream ended without response.completed]"));
    }

    #[test]
    fn compaction_summary_reports_remote_replay_pressure_metrics() {
        let summary = summarize_compaction_report(&json!({
            "method": "responses_compact",
            "before_chars": 181900,
            "after_chars": 5430,
            "local_pressure": {
                "remote_after_chars": 183238,
                "final_chars": 5430,
                "made_progress": true
            }
        }));

        assert_eq!(summary["remote_after_chars"], 183238);
        assert_eq!(summary["local_pressure_final_chars"], 5430);
        assert!(
            (summary["remote_retained_pct"].as_f64().unwrap() - 100.73556899395272).abs() < 0.001
        );
        assert!(
            (summary["final_retained_pct"].as_f64().unwrap() - 2.9851566794942275).abs() < 0.001
        );
        assert!(
            (summary["local_pressure_reduction_pct"].as_f64().unwrap() - 97.03664087143497).abs()
                < 0.001
        );
    }

    #[test]
    fn formats_trace_summary_row_for_run_comparison() {
        let summary = json!({
            "trace_metadata": {
                "model": "gpt-5.3-codex-spark",
                "context": {"profile_scenario": {"name": "repo-survey"}}
            },
            "requests": 3,
            "max_approx_input_tokens": 42000,
            "max_context_window_pct": 32.8125,
            "max_request_duration_ms": 12345,
            "tool_calls": 7,
            "tool_failures": 1,
            "compactions": 2,
            "remote_compactions": 1,
            "fallback_compactions": 1,
            "compaction_reports": [{"local_pressure": {"made_progress": true}}],
            "diagnostics": [{"kind": "tool_failures"}, {"kind": "weak_compaction_shrink"}]
        });

        let row = format_trace_summary_row(".spark-runs/run-1", &summary);

        assert!(row.contains(".spark-runs/run-1 | model=gpt-5.3-codex-spark scenario=repo-survey"));
        assert!(row.contains("requests=3"));
        assert!(row.contains("max_tokens=42000 (32.8%)"));
        assert!(row.contains("tools=7 failures=1"));
        assert!(row.contains("compactions=2 remote=1 fallback=1 local_pressure=1"));
        assert!(row.contains("diagnostics=tool_failures,weak_compaction_shrink"));
    }

    #[test]
    fn extracts_profile_scenario_name_from_trace_summary() {
        let summary = json!({
            "trace_metadata": {
                "context": {"profile_scenario": {"name": "compaction-pressure"}}
            }
        });

        assert_eq!(
            trace_profile_scenario_name(&summary),
            Some("compaction-pressure")
        );
    }

    #[test]
    fn formats_trace_aggregate_row_for_run_comparison() {
        let summaries = vec![
            json!({
                "errors": [],
                "max_approx_input_tokens": 42000,
                "max_context_window_pct": 32.8125,
                "max_request_duration_ms": 1234,
                "tool_calls": 2,
                "tool_failures": 0,
                "compactions": 1,
                "remote_compactions": 1,
                "fallback_compactions": 0,
                "compaction_reports": [{"local_pressure": {"made_progress": true}}],
                "diagnostics": [{"kind": "remote_compaction_local_pressure"}]
            }),
            json!({
                "errors": [{"stage": "response", "error": "stream ended"}],
                "max_approx_input_tokens": 45000,
                "max_context_window_pct": 35.15625,
                "max_request_duration_ms": 0,
                "tool_calls": 0,
                "tool_failures": 0,
                "compactions": 1,
                "remote_compactions": 1,
                "fallback_compactions": 0,
                "compaction_reports": [{"local_pressure": {"made_progress": false}}],
                "diagnostics": [
                    {"kind": "request_failure"},
                    {"kind": "remote_compaction_local_pressure"}
                ]
            }),
        ];

        let row = format_trace_aggregate_row("compaction-pressure", &summaries);

        assert!(row.contains("compaction-pressure aggregate | runs=2 success=1 failure=1"));
        assert!(row.contains("max_tokens=45000 (35.2%)"));
        assert!(row.contains("tools=2 failures=0"));
        assert!(row.contains("compactions=2 remote=2 fallback=0 local_pressure=2"));
        assert!(row.contains("diagnostics=remote_compaction_local_pressure:2,request_failure:1"));
    }

    #[test]
    fn analyze_trace_reconstructs_tool_result_failures() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("001-tool-result.json"),
            serde_json::to_vec_pretty(&json!({
                "call_id": "call_1",
                "tool": "cmd.exec",
                "duration_ms": 12_345,
                "result": {
                    "ok": false,
                    "data": {"code": 1, "stdout_truncated": true, "stdout_chars": 40000, "timed_out": true},
                    "error": "command failed"
                }
            }))
            .expect("serialize first result"),
        )
        .expect("write first result");
        std::fs::write(
            dir.path().join("001-tool-result-002.json"),
            serde_json::to_vec_pretty(&json!({
                "call_id": "call_2",
                "tool": "fs.read",
                "args": {"path": "README.md"},
                "duration_ms": 4,
                "result": {
                    "ok": true,
                    "data": {"path": "README.md", "cached_observation": true, "created_parent_dirs": ["nested"]},
                    "error": null
                }
            }))
            .expect("serialize second result"),
        )
        .expect("write second result");

        let summary = analyze_trace(dir.path()).expect("analyze trace");

        assert_eq!(summary["tool_results"], 2);
        assert_eq!(summary["tool_failures"], 1);
        assert_eq!(summary["truncated_tool_results"], 1);
        assert_eq!(summary["readonly_tool_cache_hits"], 1);
        assert_eq!(summary["tool_failure_counts"]["cmd.exec"], 1);
        assert_eq!(summary["tool_truncation_counts"]["cmd.exec"], 1);
        assert_eq!(summary["total_tool_duration_ms"], 12_349);
        assert_eq!(summary["max_tool_duration_ms"], 12_345);
        assert_eq!(summary["tool_duration_ms_by_tool"]["cmd.exec"], 12_345);
        assert_eq!(summary["tool_duration_ms_by_tool"]["fs.read"], 4);
        assert_eq!(
            summary["timeline"][0]["tool_results"][0]["tool"],
            "cmd.exec"
        );
        assert_eq!(summary["timeline"][0]["tool_results"][0]["ok"], false);
        assert_eq!(summary["timeline"][0]["tool_results"][0]["truncated"], true);
        assert_eq!(summary["timeline"][0]["tool_results"][0]["timed_out"], true);
        assert_eq!(summary["timeline"][0]["tool_results"][1]["tool"], "fs.read");
        assert_eq!(
            summary["timeline"][0]["tool_results"][1]["cached_observation"],
            true
        );
        assert_eq!(
            summary["timeline"][0]["tool_results"][1]["created_parent_dirs"],
            json!(["nested"])
        );
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
        std::fs::write(
            dir.path().join("001-profile-summary-002.json"),
            serde_json::to_vec_pretty(&json!({
                "requests": 1,
                "stale": false,
                "compaction_reports": [{
                    "method": "responses_compact",
                    "before_chars": 10,
                    "after_chars": 20,
                    "raw": {
                        "id": "resp_old",
                        "output": [{
                            "type": "compaction_summary",
                            "encrypted_content": "old-raw"
                        }]
                    }
                }]
            }))
            .expect("serialize latest profile"),
        )
        .expect("write latest profile");

        let summary = analyze_trace(dir.path()).expect("analyze trace");

        assert_eq!(summary["requests"], 1);
        assert_eq!(summary["embedded_profile_summary"]["requests"], 1);
        assert_eq!(summary["embedded_profile_summary"]["stale"], false);
        assert!(
            summary["embedded_profile_summary"]["compaction_reports"][0]
                .get("raw")
                .is_none()
        );
        assert_eq!(
            summary["embedded_profile_summary"]["compaction_reports"][0]["raw_summary"]["id"],
            "resp_old"
        );
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
        assert_eq!(summary["timeline"][0]["errors"][0]["stage"], "response");
        assert!(
            summary["timeline"][0]["errors"][0]["error"]
                .as_str()
                .expect("timeline error")
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
        assert_eq!(summary["timeline"][0]["turn"], 2);
        assert_eq!(summary["timeline"][0]["errors"][0]["stage"], "max_turns");
        assert_eq!(summary["diagnostics"][0]["kind"], "request_failure");
    }
}
