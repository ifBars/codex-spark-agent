use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use anyhow::Result;
use rand::Rng;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::agent::cache::is_parallel_local_read;
use crate::agent::compaction::{compaction_trigger_for_turn, format_compaction_notice};
use crate::agent::{AgentRunner, TOOL_ONLY_STREAK_COMPACTION_TRIGGER};
use crate::client::{
    ReasoningDisplayUpdate, WebSearchDisplayUpdate, function_calls, output_items_for_next_input,
    output_text_delta, reasoning_display_update, response_text, web_search_display_update,
};
#[cfg(test)]
use crate::tools::{ToolDescriptor, builtin_tools, is_local_filesystem_tool, tools_for_mode};

const DEFAULT_TOOL_ONLY_NOTICE_INTERVAL: usize = 6;
const DEFAULT_SPARK_RESPONSE_DEADLINE_SECS: u64 = 120;
const MIN_SPARK_RESPONSE_DEADLINE_SECS: u64 = 10;
const MAX_SPARK_RESPONSE_DEADLINE_SECS: u64 = 900;

impl AgentRunner {
    pub(super) fn push_user_message(&mut self, prompt: &str) {
        self.reset_deferred_tool_surface();
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

            let tools = self.tools_for_current_loop();

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
                    "request input is {input_chars} JSON chars, above the configured model-context guard of {}; split the prompt or lower retained context",
                    self.max_input_chars
                );
                self.record_terminal_error(self.request_seq + 1, "input_guard", &message)?;
                anyhow::bail!(message);
            }

            self.request_seq += 1;
            if cancellation.is_cancelled() {
                return self.record_cancelled(self.request_seq, "before_request");
            }
            let request_input_chars = if previous_response_id.is_some() {
                serde_json::to_string(
                    &self.input[continuation_input_start.min(self.input.len())..],
                )?
                .len()
            } else {
                input_chars
            };
            let instruction_chars = self.client.request_instruction_chars();
            let tool_schema_chars = self.client.request_tool_schema_chars(&tools)?;
            self.profiler.record_request(input_chars);
            self.profiler.record_request_footprint(
                request_input_chars,
                instruction_chars,
                tool_schema_chars,
                tools.len(),
            );
            self.emit_request_start(self.request_seq, input_chars);
            if let Some(trace) = &mut self.trace {
                trace.write(
                    self.request_seq,
                    "request-input",
                    &json!({
                        "input": self.input,
                        "tools": tools,
                        "footprint": {
                            "request_input_chars": request_input_chars,
                            "instruction_chars": instruction_chars,
                            "tool_schema_chars": tool_schema_chars,
                            "tool_count": tools.len(),
                            "estimated_total_chars": request_input_chars
                                .saturating_add(instruction_chars)
                                .saturating_add(tool_schema_chars),
                        }
                    }),
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
                let mut attempt_reasoning_active = false;
                let response_future = client.responses_create_with_event_handler(
                    &request_input,
                    &tools,
                    previous_response_id.as_deref(),
                    continuation_input_start,
                    |event| {
                        if let Some(update) = reasoning_display_update(event) {
                            match update {
                                ReasoningDisplayUpdate::Started => {
                                    attempt_reasoning_active = true;
                                    self.emit_reasoning_start();
                                }
                                ReasoningDisplayUpdate::Summary(text) => {
                                    self.emit_reasoning_summary(&text);
                                }
                                ReasoningDisplayUpdate::Finished => {
                                    attempt_reasoning_active = false;
                                    self.emit_reasoning_finish();
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
                                    hosted_search_queries.insert(id.clone(), Some(query.clone()));
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
                                        hosted_search_queries.remove(&id).and_then(|query| query)
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
                                    self.emit_tool_result("web.search", ok, duration_ms, 0, error);
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
                );
                let response_result =
                    if let Some(deadline) = spark_response_deadline(client.model()) {
                        match tokio::time::timeout(deadline, response_future).await {
                            Ok(result) => result,
                            Err(_) => {
                                if attempt_reasoning_active {
                                    self.emit_reasoning_finish();
                                }
                                let message = format!(
                                    "Spark response exceeded the {}s per-response deadline",
                                    deadline.as_secs()
                                );
                                let retry_over_http = attempt_streamed_text.is_empty()
                                    && client.switch_to_http_transport().await;
                                self.profiler.record_response_deadline(
                                    self.request_seq,
                                    deadline.as_millis().min(u64::MAX as u128) as u64,
                                    retry_over_http,
                                );
                                if retry_over_http {
                                    response_retries += 1;
                                    self.emit_transport_fallback(
                                        "WebSocket",
                                        "HTTP/SSE",
                                        message.clone(),
                                    );
                                    if let Some(trace) = &mut self.trace {
                                        trace.write(
                                            self.request_seq,
                                            "response-deadline",
                                            &json!({
                                                "deadline_ms": deadline.as_millis(),
                                                "transport": "responses_websocket",
                                                "recovery": "retry_full_history_over_http",
                                            }),
                                        )?;
                                    }
                                    continue;
                                }
                                self.record_terminal_error(
                                    self.request_seq,
                                    "response_deadline",
                                    &message,
                                )?;
                                anyhow::bail!(message);
                            }
                        }
                    } else {
                        response_future.await
                    };
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
            self.profiler.record_response_usage(&raw);
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
            if should_parallelize_tool_batch(&calls) {
                if cancellation.is_cancelled() {
                    return self.record_cancelled(self.request_seq, "before_tool_batch");
                }
                for (_, tool_name, args) in &calls {
                    self.profiler
                        .record_tool_call(self.request_seq, tool_name, args);
                    self.emit_tool_call(tool_name, &serde_json::to_string(args)?);
                }
                let parallel_calls = calls
                    .iter()
                    .map(|(_, tool_name, args)| (tool_name.clone(), args.clone()))
                    .collect::<Vec<_>>();
                let results = self.invoke_parallel_local_reads(&parallel_calls).await;
                for ((call_id, tool_name, args), timed) in calls.into_iter().zip(results) {
                    let result = timed.result;
                    let output = serde_json::to_string(&result)?;
                    self.profiler.record_tool_result(
                        self.request_seq,
                        &tool_name,
                        result.ok,
                        &result.data,
                        output.len(),
                        timed.duration_ms,
                        result.error.as_deref(),
                    );
                    self.emit_tool_result(
                        &tool_name,
                        result.ok,
                        timed.duration_ms,
                        output.len(),
                        result.error.as_deref(),
                    );
                    if let Some(trace) = &mut self.trace {
                        trace.write(
                            self.request_seq,
                            "tool-result",
                            &json!({"call_id": call_id, "tool": tool_name, "args": args, "duration_ms": timed.duration_ms, "parallel": true, "result": result}),
                        )?;
                    }
                    self.input.push(json!({
                        "type": "function_call_output",
                        "call_id": call_id,
                        "output": output,
                    }));
                }
                continue;
            }
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
                "text": tool_only_recovery_notice(tool_only_streak, self.local_filesystem_only)
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

fn should_parallelize_tool_batch(calls: &[(String, String, Value)]) -> bool {
    calls.len() > 1
        && calls
            .iter()
            .all(|(_, tool_name, _)| is_parallel_local_read(tool_name))
}

fn response_output_tokens(raw: &Value) -> Option<u64> {
    crate::profiler::ResponseUsage::from_response_raw(raw).and_then(|usage| usage.output_tokens)
}

fn average_tokens_per_second(output_tokens: Option<u64>, duration_ms: u64) -> Option<f64> {
    let output_tokens = output_tokens?;
    if duration_ms == 0 {
        return None;
    }
    Some(output_tokens as f64 * 1_000.0 / duration_ms as f64)
}

fn spark_response_deadline(model: &str) -> Option<Duration> {
    let configured = std::env::var("SPARK_RESPONSE_DEADLINE_SECONDS").ok();
    spark_response_deadline_with_override(model, configured.as_deref())
}

fn spark_response_deadline_with_override(
    model: &str,
    configured: Option<&str>,
) -> Option<Duration> {
    if !model.to_ascii_lowercase().contains("codex-spark") {
        return None;
    }
    let seconds = match configured.and_then(|value| value.parse::<u64>().ok()) {
        Some(0) => return None,
        Some(seconds) => seconds.clamp(
            MIN_SPARK_RESPONSE_DEADLINE_SECS,
            MAX_SPARK_RESPONSE_DEADLINE_SECS,
        ),
        None => DEFAULT_SPARK_RESPONSE_DEADLINE_SECS,
    };
    Some(Duration::from_secs(seconds))
}

#[cfg(test)]
fn local_filesystem_brief_tools() -> Vec<ToolDescriptor> {
    filter_local_filesystem_tools(tools_for_mode(
        builtin_tools(),
        crate::tools::AgentMode::Ask,
    ))
}

#[cfg(test)]
fn filter_local_filesystem_tools(tools: Vec<ToolDescriptor>) -> Vec<ToolDescriptor> {
    tools
        .into_iter()
        .filter(|tool| is_local_filesystem_tool(&tool.name))
        .collect()
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
        DEFAULT_TOOL_ONLY_NOTICE_INTERVAL
    } else {
        (compact_after_tool_only_turns / 2)
            .max(4)
            .min(compact_after_tool_only_turns)
    }
}

fn tool_only_recovery_notice(tool_only_streak: usize, local_filesystem_only: bool) -> String {
    if local_filesystem_only {
        return format!(
            "[spark harness recovery]\nYou have made {tool_only_streak} consecutive tool-only turns. Stop broad investigation now. Synthesize the requested read-only repository brief from the local evidence already gathered, including citations, risks/unknowns, and the next inspection. Do not repeat read-only calls or enumerate new areas unless a specific citation cannot be completed."
        );
    }
    format!(
        "[spark harness recovery]\nYou have made {tool_only_streak} consecutive tool-only turns. Stop piecemeal trial-and-error now. Re-evaluate the current artifact against the complete evidence already gathered. If an edit is needed, consolidate the next coherent change and run the smallest required validation. If validation passes, provide the final answer. Use further tools only to resolve a concrete compiler, test, or command failure."
    )
}

#[cfg(test)]
mod tests {
    use super::{
        average_tokens_per_second, local_filesystem_brief_tools, response_output_tokens,
        should_parallelize_tool_batch, should_retry_response_stream_error,
        spark_response_deadline_with_override, stream_retry_base_delay, tool_only_notice_interval,
        tool_only_recovery_notice,
    };
    use crate::agent::AgentRunner;
    use crate::auth::AuthTokens;
    use crate::tools::AgentMode;
    use serde_json::json;
    use std::time::Duration;
    use tempfile::TempDir;

    fn auth_tokens() -> AuthTokens {
        AuthTokens {
            id_token: "id".to_string(),
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            expires_at: i64::MAX,
            account_id: None,
        }
    }

    fn runner() -> (TempDir, AgentRunner) {
        let dir = TempDir::new().expect("tempdir");
        let runner = AgentRunner::new(
            auth_tokens(),
            dir.path().to_path_buf(),
            crate::DEFAULT_MODEL.to_string(),
            false,
            false,
            crate::DEFAULT_COMPACT_AFTER_CHARS,
            crate::DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS,
            crate::DEFAULT_MAX_INPUT_CHARS,
            false,
            None,
            false,
            None,
            AgentMode::Ask,
        )
        .expect("runner");
        (dir, runner)
    }

    #[test]
    fn tool_only_notice_arrives_before_compaction_threshold() {
        assert_eq!(tool_only_notice_interval(12), 6);
        assert_eq!(tool_only_notice_interval(8), 4);
        assert_eq!(tool_only_notice_interval(3), 3);
        assert_eq!(tool_only_notice_interval(0), 6);
    }

    #[test]
    fn disabled_tool_only_compaction_keeps_completion_nudges() {
        let (_dir, mut runner) = runner();
        let mut last_notice_streak = 0;

        assert_eq!(runner.compact_after_tool_only_turns, 0);
        assert!(!runner.maybe_push_tool_only_notice(5, &mut last_notice_streak));
        assert!(runner.maybe_push_tool_only_notice(6, &mut last_notice_streak));
        assert_eq!(last_notice_streak, 6);
        assert!(
            serde_json::to_string(&runner.input)
                .expect("notice input")
                .contains("Stop piecemeal trial-and-error now")
        );
    }

    #[test]
    fn tool_only_recovery_requires_progress_or_completion() {
        let notice = tool_only_recovery_notice(6, false);

        assert!(notice.contains("Stop piecemeal trial-and-error now"));
        assert!(notice.contains("consolidate the next coherent change"));
        assert!(notice.contains("provide the final answer"));
        assert!(notice.contains("compiler, test, or command failure"));
    }

    #[test]
    fn local_filesystem_notice_requires_evidence_synthesis_not_implementation() {
        let notice = tool_only_recovery_notice(6, true);

        assert!(notice.contains("Synthesize the requested read-only repository brief"));
        assert!(notice.contains("citations, risks/unknowns, and the next inspection"));
        assert!(!notice.contains("implementation/edit"));
    }

    #[test]
    fn exhausted_local_filesystem_budget_advertises_no_tool_schemas() {
        let (_dir, mut runner) = runner();
        runner.enforce_local_filesystem_only();
        runner.set_local_filesystem_tool_budget(0);

        assert!(runner.tools_for_current_loop().is_empty());
    }

    #[test]
    fn ordinary_runner_defers_specialist_tools_without_a_budget() {
        let (_dir, runner) = runner();
        let names = runner
            .tools_for_current_loop()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();

        assert!(names.contains(&"fs.read".to_string()));
        assert!(names.contains(&"tool.search".to_string()));
        assert!(!names.contains(&"web.search".to_string()));
        assert!(!names.contains(&"subagent.run".to_string()));
    }

    #[test]
    fn tool_search_activates_one_matching_specialist_for_the_current_goal() {
        let (_dir, mut runner) = runner();
        runner.disable_mcp();

        let result = runner.search_and_activate_tools(json!({
            "query": "public web current facts",
            "limit": 1,
        }));
        assert!(result.ok);
        assert_eq!(result.data["activated"][0]["name"], "web.search");
        assert!(
            runner
                .tools_for_current_loop()
                .iter()
                .any(|tool| tool.name == "web.search")
        );

        runner.push_user_message("start a separate local task");
        assert!(
            runner
                .tools_for_current_loop()
                .iter()
                .all(|tool| tool.name != "web.search")
        );
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
    fn response_deadline_is_spark_specific_and_configurable() {
        assert_eq!(
            spark_response_deadline_with_override("gpt-5.3-codex-spark", None),
            Some(Duration::from_secs(120))
        );
        assert_eq!(
            spark_response_deadline_with_override("gpt-5.3-codex-spark", Some("30")),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            spark_response_deadline_with_override("gpt-5.3-codex-spark", Some("0")),
            None
        );
        assert_eq!(
            spark_response_deadline_with_override("gpt-5.6-luna", None),
            None
        );
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

    #[test]
    fn local_repo_brief_schemas_exclude_network_execution_and_writes() {
        let names = local_filesystem_brief_tools()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["fs.read", "fs.list", "fs.stat", "fs.search"]);
        for forbidden in [
            "web.search",
            "cmd.exec",
            "browser.run",
            "fs.write",
            "subagent.run",
        ] {
            assert!(!names.iter().any(|name| name == forbidden));
        }
    }

    #[test]
    fn only_multi_call_local_read_batches_run_in_parallel() {
        let reads = vec![
            (
                "call-1".to_string(),
                "fs.read".to_string(),
                json!({"path": "a"}),
            ),
            (
                "call-2".to_string(),
                "fs.search".to_string(),
                json!({"query": "b"}),
            ),
        ];
        assert!(should_parallelize_tool_batch(&reads));

        let one_read = vec![reads[0].clone()];
        assert!(!should_parallelize_tool_batch(&one_read));

        let mut mixed = reads;
        mixed.push((
            "call-3".to_string(),
            "fs.write".to_string(),
            json!({"path": "c", "content": "value"}),
        ));
        assert!(!should_parallelize_tool_batch(&mixed));
    }
}
