use serde_json::{Value, json};

use super::{
    AgentProfiler, SLOW_SPARK_REQUEST_MS, SLOW_TOOL_RESULT_MS, SPARK_CONTEXT_WINDOW_TOKENS,
    approx_token_count_from_chars, context_window_pct,
};

impl AgentProfiler {
    pub(super) fn diagnostics(&self) -> Vec<Value> {
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

        if self.response_deadlines_exceeded > 0 {
            diagnostics.push(json!({
                "level": "warning",
                "kind": "response_deadline_exceeded",
                "message": "One or more Spark responses exceeded the per-response deadline. Inspect the deadline turns and transport fallback before changing the limit.",
                "count": self.response_deadlines_exceeded,
                "turns": self.response_deadline_turns,
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
