use std::collections::{HashMap, HashSet};
use std::time::Instant;

use anyhow::Result;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::agent::compaction::{compaction_trigger_for_turn, format_compaction_notice};
use crate::agent::{AgentRunner, TOOL_ONLY_STREAK_COMPACTION_TRIGGER};
use crate::client::{
    ReasoningDisplayUpdate, WebSearchDisplayUpdate, function_calls, output_items_for_next_input,
    output_text_delta, reasoning_display_update, response_text, web_search_display_update,
};
use crate::tools::{builtin_tools, tools_for_mode};

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

    pub(super) async fn run_until_idle(&mut self, cancellation: CancellationToken) -> Result<()> {
        let tools = tools_for_mode(builtin_tools(), self.mode);

        let mut turn = 0usize;
        let mut last_tool_only_compaction_streak = 0usize;
        let mut last_tool_only_notice_streak = 0usize;
        loop {
            turn += 1;
            if cancellation.is_cancelled() {
                return self.record_cancelled(turn, "turn_start");
            }
            if let Some(max_turns) = self.max_turns
                && turn > max_turns
            {
                let message = format!("stopped after {max_turns} turns without completion");
                self.record_terminal_error(self.request_seq + 1, "max_turns", &message)?;
                anyhow::bail!(message);
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
            let mut streamed_text = String::new();
            let mut hosted_search_starts = HashMap::<String, Instant>::new();
            let mut hosted_search_queries = HashMap::<String, Option<String>>::new();
            let mut hosted_search_displayed = HashSet::<String>::new();
            let (response, raw) = match client
                .responses_create_with_event_handler(&request_input, &tools, |event| {
                    if let Some(update) = reasoning_display_update(event) {
                        match update {
                            ReasoningDisplayUpdate::Started => self.emit_reasoning_start(),
                            ReasoningDisplayUpdate::Summary(text) => {
                                self.emit_reasoning_summary(&text);
                            }
                            ReasoningDisplayUpdate::Finished => self.emit_reasoning_finish(),
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
                                let error = (!ok).then_some("hosted web search did not complete");
                                self.emit_tool_result("web.search", ok, duration_ms, 0, error);
                            }
                        }
                    }
                    if let Some(delta) = output_text_delta(event) {
                        streamed_text.push_str(delta);
                        self.emit_assistant_delta(delta);
                    }
                })
                .await
            {
                Ok(result) => result,
                Err(error) => {
                    self.record_terminal_error(self.request_seq, "response", &error.to_string())?;
                    return Err(error);
                }
            };
            let request_duration_ms = request_started.elapsed().as_millis() as u64;
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

            self.input.extend(output_items_for_next_input(&raw));

            let calls = function_calls(&response);
            self.profiler
                .record_turn_activity(self.request_seq, !calls.is_empty(), text.len());
            if !text.is_empty() {
                last_tool_only_compaction_streak = 0;
                last_tool_only_notice_streak = 0;
            }
            if calls.is_empty() {
                self.emit_profile_summary()?;
                return Ok(());
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

    fn record_cancelled(&mut self, turn: usize, stage: &str) -> Result<()> {
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

fn web_search_display_args(query: Option<String>) -> String {
    match query {
        Some(query) => json!({ "query": query }).to_string(),
        None => json!({ "query": "hosted web search" }).to_string(),
    }
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
    use super::tool_only_notice_interval;

    #[test]
    fn tool_only_notice_arrives_before_compaction_threshold() {
        assert_eq!(tool_only_notice_interval(12), 6);
        assert_eq!(tool_only_notice_interval(8), 4);
        assert_eq!(tool_only_notice_interval(3), 3);
        assert_eq!(tool_only_notice_interval(0), 0);
    }
}
