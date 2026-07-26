use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use anyhow::Result;
use rand::Rng;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::agent::compaction::{compaction_trigger_for_turn, format_compaction_notice};
use crate::agent::{AgentRunner, TOOL_ONLY_STREAK_COMPACTION_TRIGGER};
use crate::client::{
    ReasoningDisplayUpdate, WebSearchDisplayUpdate, function_calls, output_items_for_next_input,
    output_text_delta, reasoning_display_update, response_text, web_search_display_update,
};
use crate::tools::{ToolDescriptor, builtin_tools, tools_for_mode};

impl AgentRunner {
    pub(super) fn push_user_message(&mut self, prompt: &str) {
        self.input.push(json!({
            "role": "user",
            "content": [
                {
                    "type": "input_text",
                    "text": prompt,
                }
            ]
        }));
    }

    pub(super) async fn run_until_idle(
        &mut self,
        cancellation: CancellationToken,
    ) -> Result<String> {
        self.ensure_mcp_registry().await;
        let tools = self.tools_for_current_loop();

        let mut turn = 0usize;
        let mut last_tool_only_compaction_streak = 0usize;
        let mut last_tool_only_notice_streak = 0usize;
        let mut previous_response_id = None::<String>;
        let mut continuation_input_start = 0usize;
        loop {
            turn += 1;
            if cancellation.is_cancelled() {
                return self.record_cancelled(turn, "turn_start");
            }

            let tool_only_streak = self.profiler.current_tool_only_turn_streak();
            let compaction_trigger = compaction_trigger_for_turn(
                self.compact_after_chars,
                self.compact_after_tool_only_turns,
                tool_only_streak,
                last_tool_only_compaction_streak,
                &self.input,
            )?;

            if compaction_trigger.is_some() {
                self.emit_compaction_start(
                    compaction_trigger,
                    serde_json::to_string(&self.input)?.len(),
                );
            }
            if cancellation.is_cancelled() {
                return self.record_cancelled(self.request_seq + 1, "before_compaction");
            }
            match self
                .compact_once(&tools, compaction_trigger.is_some(), compaction_trigger)
                .await
            {
                Ok(Some(report)) => {
                    previous_response_id = None;
                    continuation_input_start = 0;
                    if report.get("trigger").and_then(Value::as_str)
                        == Some(TOOL_ONLY_STREAK_COMPACTION_TRIGGER)
                    {
                        last_tool_only_compaction_streak = tool_only_streak;
                    }
                    self.profiler.record_compaction(&report);
                    if let Some(trace) = &mut self.trace {
                        trace.write(self.request_seq + 1, "compaction", &report)?;
                    }
                    self.emit_compaction_finish(format_compaction_notice(&report));
                }
                Ok(None) => {}
                Err(error) => {
                    self.record_terminal_error(
                        self.request_seq + 1,
                        "compaction",
                        &error.to_string(),
                    )?;
                    return Err(error);
                }
            }

            if self.maybe_push_tool_only_notice(tool_only_streak, &mut last_tool_only_notice_streak)
            {
                if let Some(trace) = &mut self.trace {
                    trace.write(
                        self.request_seq + 1,
                        "harness-notice",
                        &json!({
                            "kind": "tool_only_streak_completion_nudge",
                            "tool_only_streak": tool_only_streak,
                        }),
                    )?;
                }
            }

            let input_chars = serde_json::to_string(&self.input)?.len();
            if input_chars > self.max_input_chars {
                let message = format!(
                    "request input is {input_chars} JSON chars, above max-input-chars {}; Spark has a 128k context window, so split the prompt or lower retained context",
                    self.max_input_chars
                );
                self.record_terminal_error(self.request_seq + 1, "input_guard", &message)?;
                anyhow::bail!(message);
            }

            self.request_seq += 1;
            if cancellation.is_cancelled() {
                return self.record_cancelled(self.request_seq, "before_request");
            }
            self.profiler.record_request(input_chars);
            self.emit_request_start(self.request_seq, input_chars);
            if let Some(trace) = &mut self.trace {
                trace.write(
                    self.request_seq,
                    "request-input",
                    &json!({"input": self.input, "tools": tools}),
                )?;
            }

            let request_started = std::time::Instant::now();
            let client = self.client.clone();
            let request_input = self.input.clone();
            let streamed_text;
            let mut time_to_first_token_ms = None;
            let mut generation_started = None;
            let mut response_retries = 0;
            let (response, raw) = loop {
                let mut attempt_streamed_text = String::new();
                let mut hosted_search_starts = HashMap::<String, Instant>::new();
                let mut hosted_search_queries = HashMap::<String, Option<String>>::new();
                let mut hosted_search_displayed = HashSet::<String>::new();
                let response_result = client
                    .responses_create_with_event_handler(
                        &request_input,
                        &tools,
                        previous_response_id.as_deref(),
                        continuation_input_start,
                        |event| {
                            if let Some(update) = reasoning_display_update(event) {
                                match update {
                                    ReasoningDisplayUpdate::Started => self.emit_reasoning_start(),
                                    ReasoningDisplayUpdate::Summary(text) => {
                                        self.emit_reasoning_summary(&text);
                                    }
                                    ReasoningDisplayUpdate::Finished => {
                                        self.emit_reasoning_finish()
                                    }
                                }
                            }
                            if let Some(update) = web_search_display_update(event) {
                                match update {
                                    WebSearchDisplayUpdate::Started { id, query } => {
                                        hosted_search_starts.insert(id.clone(), Instant::now());
                                        hosted_search_queries.insert(id.clone(), query.clone());
                                        if query.is_some() {
                                            hosted_search_displayed.insert(id);
                                            self.emit_tool_batch_start(1);
                                            self.emit_tool_call(
                                                "web.search",
                                                &web_search_display_args(query),
                                            );
                                        }
                                    }
                                    WebSearchDisplayUpdate::Query { id, query } => {
                                        hosted_search_queries
                                            .insert(id.clone(), Some(query.clone()));
                                        if hosted_search_displayed.insert(id) {
                                            self.emit_tool_batch_start(1);
                                            self.emit_tool_call(
                                                "web.search",
                                                &web_search_display_args(Some(query)),
                                            );
                                        }
                                    }
                                    WebSearchDisplayUpdate::Finished { id, query, ok } => {
                                        let started = hosted_search_starts.remove(&id);
                                        let query = query.or_else(|| {
                                            hosted_search_queries
                                                .remove(&id)
                                                .and_then(|query| query)
                                        });
                                        if hosted_search_displayed.insert(id) {
                                            self.emit_tool_batch_start(1);
                                            self.emit_tool_call(
                                                "web.search",
                                                &web_search_display_args(query.clone()),
                                            );
                                        }
                                        let duration_ms = started
                                            .map(|started| started.elapsed().as_millis() as u64)
                                            .unwrap_or(0);
                                        let error =
                                            (!ok).then_some("hosted web search did not complete");
                                        self.emit_tool_result(
                                            "web.search",
                                            ok,
                                            duration_ms,
                                            0,
                                            error,
                                        );
                                    }
                                }
                            }
                            if let Some(delta) = output_text_delta(event) {
                                if generation_started.is_none() {
                                    generation_started = Some(Instant::now());
                                    time_to_first_token_ms = Some(
                                        request_started.elapsed().as_millis().min(u64::MAX as u128)
                                            as u64,
                                    );
                                }
                                attempt_streamed_text.push_str(delta);
                                self.emit_assistant_delta(delta);
                            }
                        },
                    )
                    .await;
                match response_result {
                    Ok(result) => {
                        if response_retries > 0 {
                            self.emit_connection_recovered(response_retries);
                        }
                        streamed_text = attempt_streamed_text;
                        break result;
                    }
                    Err(error)
                        if should_retry_response_stream_error(&error)
                            && attempt_streamed_text.is_empty() =>
                    {
                        if response_retries >= DEFAULT_STREAM_MAX_RETRIES {
                            if client.switch_to_http_transport().await {
                                self.emit_transport_fallback(
                                    "WebSocket",
                                    "HTTP/SSE",
                                    format!("{error:#}"),
                                );
                                response_retries = 0;
                                continue;
                            }
                            self.record_terminal_error(
                                self.request_seq,
                                "response",
                                &error.to_string(),
                            )?;
                            return Err(error);
                        }

                        response_retries += 1;
                        let delay = stream_retry_delay(response_retries);
                        self.emit_connection_retry(
                            response_retries,
                            DEFAULT_STREAM_MAX_RETRIES,
                            delay.as_millis().min(u64::MAX as u128) as u64,
                            format!("{error:#}"),
                        );
                        if let Some(trace) = &mut self.trace {
                            trace.write(
                                self.request_seq,
                                "response-retry",
                                &json!({
                                    "stage": "response",
                                    "error": format!("{error:#}"),
                                    "retry": response_retries,
                                    "max_retries": DEFAULT_STREAM_MAX_RETRIES,
                                    "delay_ms": delay.as_millis(),
                                    "transport": if client.websocket_enabled() {
                                        "responses_websocket"
                                    } else {
                                        "responses_http"
                                    },
                                }),
                            )?;
                        }
                        tokio::select! {
                            _ = cancellation.cancelled() => {
                                return self.record_cancelled(self.request_seq, "response_retry_delay");
                            }
                            _ = tokio::time::sleep(delay) => {}
                        }
                        continue;
                    }
                    Err(error) => {
                        self.record_terminal_error(
                            self.request_seq,
                            "response",
                            &error.to_string(),
                        )?;
                        return Err(error);
                    }
                }
            };
            let request_duration_ms = request_started.elapsed().as_millis() as u64;
            let output_tokens = response_output_tokens(&raw);
            let generation_duration_ms = generation_started
                .map(|started| started.elapsed().as_millis().min(u64::MAX as u128) as u64)
                .unwrap_or(request_duration_ms);
            let average_tokens_per_second =
                average_tokens_per_second(output_tokens, generation_duration_ms);
            self.profiler
                .record_request_duration(self.request_seq, request_duration_ms);
            if let Some(trace) = &mut self.trace {
                trace.write(
                    self.request_seq,
                    "response",
                    &json!({"duration_ms": request_duration_ms, "raw": raw}),
                )?;
            }

            let text = response_text(&response);
            self.profiler.record_response_text(&text);
            if streamed_text.is_empty() && !text.trim().is_empty() {
                self.emit_assistant_message(&text);
            } else if let Some(missing_suffix) = text.strip_prefix(&streamed_text)
                && !missing_suffix.is_empty()
            {
                self.emit_assistant_delta(missing_suffix);
            }
            self.emit_response_complete(
                request_duration_ms,
                output_tokens,
                time_to_first_token_ms,
                average_tokens_per_second,
            );

            self.input.extend(output_items_for_next_input(&raw));
            if raw.get("transport").and_then(Value::as_str) == Some("responses_websocket") {
                previous_response_id = response.id.clone();
                continuation_input_start = self.input.len();
            } else {
                previous_response_id = None;
                continuation_input_start = 0;
            }

            let calls = function_calls(&response);
            self.profiler
                .record_turn_activity(self.request_seq, !calls.is_empty(), text.len());
            if !text.is_empty() {
                last_tool_only_compaction_streak = 0;
                last_tool_only_notice_streak = 0;
            }
            if calls.is_empty() {
                self.record_goal_decision_from_assistant(&text);
                self.emit_profile_summary()?;
                return Ok(text);
            }

            self.emit_tool_batch_start(calls.len());
            for (call_id, tool_name, args) in calls {
                if cancellation.is_cancelled() {
                    return self.record_cancelled(self.request_seq, "before_tool");
                }
                self.profiler
                    .record_tool_call(self.request_seq, &tool_name, &args);
                self.emit_tool_call(&tool_name, &serde_json::to_string(&args)?);
                let tool_started = std::time::Instant::now();
                let result = self.invoke_with_cache(&tool_name, args.clone()).await;
                let duration_ms = tool_started.elapsed().as_millis() as u64;

                let output = serde_json::to_string(&result)?;
                self.profiler.record_tool_result(
                    self.request_seq,
                    &tool_name,
                    result.ok,
                    &result.data,
                    output.len(),
                    duration_ms,
                    result.error.as_deref(),
                );
                self.emit_tool_result(
                    &tool_name,
                    result.ok,
                    duration_ms,
                    output.len(),
                    result.error.as_deref(),
                );
                if let Some(trace) = &mut self.trace {
                    trace.write(
                        self.request_seq,
                        "tool-result",
                        &json!({"call_id": call_id, "tool": tool_name, "args": args, "duration_ms": duration_ms, "result": result}),
                    )?;
                }
                self.input.push(json!({
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": output,
                }));
            }
        }
    }

    fn record_cancelled<T>(&mut self, turn: usize, stage: &str) -> Result<T> {
        let message = "run cancelled";
        self.record_terminal_error(turn, stage, message)?;
        anyhow::bail!(message)
    }

    fn maybe_push_tool_only_notice(
        &mut self,
        tool_only_streak: usize,
        last_notice_streak: &mut usize,
    ) -> bool {
        let notice_interval = tool_only_notice_interval(self.compact_after_tool_only_turns);
        if notice_interval == 0
            || tool_only_streak < notice_interval
            || tool_only_streak.saturating_sub(*last_notice_streak) < notice_interval
        {
            return false;
        }

        *last_notice_streak = tool_only_streak;
        self.input.push(json!({
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": format!(
                    "[spark harness notice]\nYou have made {tool_only_streak} consecutive tool-only turns. If you have enough evidence to answer the user's question, stop calling tools now and provide the final assistant response. Avoid repeating exact read-only calls; cached observations mean the same data was already returned. Use another tool only for a concrete missing fact."
                )
            }]
        }));
        true
    }

    pub(super) fn emit_profile_summary(&mut self) -> Result<()> {
        let summary = self.profile_summary();
        if let Some(trace) = &mut self.trace {
            trace.write(self.request_seq, "profile-summary", &summary)?;
        }
        if self.profile {
            self.emit_profile_message(serde_json::to_string_pretty(&summary)?);
        }
        Ok(())
    }

    pub(super) fn record_terminal_error(
        &mut self,
        turn: usize,
        stage: &str,
        error: &str,
    ) -> Result<()> {
        self.profiler.record_error(turn, stage, error);
        if let Some(trace) = &mut self.trace {
            trace.write(
                turn,
                &format!("{stage}-error"),
                &json!({"stage": stage, "error": error}),
            )?;
        }
        self.emit_warning(format!("{stage}: {error}"));
        self.emit_profile_summary()
    }
}

fn response_output_tokens(raw: &Value) -> Option<u64> {
    raw.get("response")
        .unwrap_or(raw)
        .get("usage")
        .and_then(|usage| usage.get("output_tokens"))
        .and_then(Value::as_u64)
}

fn average_tokens_per_second(output_tokens: Option<u64>, duration_ms: u64) -> Option<f64> {
    let output_tokens = output_tokens?;
    if duration_ms == 0 {
        return None;
    }
    Some(output_tokens as f64 * 1_000.0 / duration_ms as f64)
}

impl AgentRunner {
    fn tools_for_current_loop(&self) -> Vec<ToolDescriptor> {
        let mut tools = tools_for_mode(builtin_tools(), self.mode)
            .into_iter()
            .filter(|tool| self.subagent_depth == 0 || tool.name != "subagent.run")
            .collect::<Vec<_>>();
        if self.mode == crate::tools::AgentMode::Work
            && let Some(registry) = &self.mcp_registry
        {
            tools.extend(registry.tools());
        }
        tools
    }
}

fn web_search_display_args(query: Option<String>) -> String {
    match query {
        Some(query) => json!({ "query": query }).to_string(),
        None => json!({ "query": "hosted web search" }).to_string(),
    }
}

fn should_retry_response_stream_error(error: &anyhow::Error) -> bool {
    let message = format!("{error:#}").to_ascii_lowercase();
    [
        "stream ended without response.completed",
        "websocket closed before response.completed",
        "responses websocket",
        "failed to read spark stream",
        "error sending request",
        "connection closed before message completed",
        "timed out",
        "timeout waiting for",
    ]
    .iter()
    .any(|needle| message.contains(needle))
        || retryable_http_status(&message)
}

const DEFAULT_STREAM_MAX_RETRIES: u64 = 5;
const STREAM_RETRY_INITIAL_DELAY_MS: u64 = 200;

fn stream_retry_delay(attempt: u64) -> Duration {
    let base = stream_retry_base_delay(attempt);
    let jitter = rand::thread_rng().gen_range(0.9..1.1);
    Duration::from_millis((base.as_millis() as f64 * jitter) as u64)
}

fn stream_retry_base_delay(attempt: u64) -> Duration {
    let exponent = attempt.saturating_sub(1).min(10) as u32;
    Duration::from_millis(STREAM_RETRY_INITIAL_DELAY_MS.saturating_mul(1_u64 << exponent))
}

fn retryable_http_status(message: &str) -> bool {
    message.contains("(408)")
        || message.contains("(429)")
        || (500..=599).any(|status| message.contains(&format!("({status})")))
}

fn tool_only_notice_interval(compact_after_tool_only_turns: usize) -> usize {
    if compact_after_tool_only_turns == 0 {
        0
    } else {
        (compact_after_tool_only_turns / 2)
            .max(4)
            .min(compact_after_tool_only_turns)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        average_tokens_per_second, response_output_tokens, should_retry_response_stream_error,
        stream_retry_base_delay, tool_only_notice_interval,
    };
    use serde_json::json;

    #[test]
    fn tool_only_notice_arrives_before_compaction_threshold() {
        assert_eq!(tool_only_notice_interval(12), 6);
        assert_eq!(tool_only_notice_interval(8), 4);
        assert_eq!(tool_only_notice_interval(3), 3);
        assert_eq!(tool_only_notice_interval(0), 0);
    }

    #[test]
    fn response_stream_retry_is_limited_to_missing_completed() {
        let retryable = anyhow::anyhow!("Spark stream ended without response.completed");
        let websocket = anyhow::anyhow!(
            "Responses WebSocket closed before response.completed: keepalive ping timeout"
        );
        let server = anyhow::anyhow!("Spark request failed (503): unavailable");
        let other = anyhow::anyhow!("request input is too large");

        assert!(should_retry_response_stream_error(&retryable));
        assert!(should_retry_response_stream_error(&websocket));
        assert!(should_retry_response_stream_error(&server));
        assert!(!should_retry_response_stream_error(&other));
    }

    #[test]
    fn response_stream_retry_uses_codex_aligned_exponential_backoff() {
        assert_eq!(stream_retry_base_delay(1).as_millis(), 200);
        assert_eq!(stream_retry_base_delay(2).as_millis(), 400);
        assert_eq!(stream_retry_base_delay(3).as_millis(), 800);
        assert_eq!(stream_retry_base_delay(5).as_millis(), 3_200);
    }

    #[test]
    fn response_usage_drives_average_output_token_rate() {
        let raw = json!({
            "response": {
                "usage": {"input_tokens": 120, "output_tokens": 42, "total_tokens": 162}
            }
        });

        let output_tokens = response_output_tokens(&raw);
        assert_eq!(output_tokens, Some(42));
        assert_eq!(average_tokens_per_second(output_tokens, 2_000), Some(21.0));
        assert_eq!(average_tokens_per_second(output_tokens, 0), None);
        assert_eq!(average_tokens_per_second(None, 2_000), None);
    }
}
