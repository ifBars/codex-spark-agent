use anyhow::Result;
use serde_json::{Value, json};

use crate::agent::compaction::{compaction_trigger_for_turn, format_compaction_notice};
use crate::agent::{AgentRunner, TOOL_ONLY_STREAK_COMPACTION_TRIGGER};
use crate::client::{function_calls, output_items_for_next_input, response_text};
use crate::tools::builtin_tools;

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

    pub(super) async fn run_until_idle(&mut self) -> Result<()> {
        let tools = builtin_tools();

        let mut turn = 0usize;
        let mut last_tool_only_compaction_streak = 0usize;
        loop {
            turn += 1;
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
                    eprintln!("{}", format_compaction_notice(&report));
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
            self.profiler.record_request(input_chars);
            if let Some(trace) = &mut self.trace {
                trace.write(
                    self.request_seq,
                    "request-input",
                    &json!({"input": self.input, "tools": tools}),
                )?;
            }

            let request_started = std::time::Instant::now();
            let (response, raw) = match self.client.responses_create(&self.input, &tools).await {
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
            if !text.trim().is_empty() {
                println!("{text}");
            }

            self.input.extend(output_items_for_next_input(&raw));

            let calls = function_calls(&response);
            self.profiler
                .record_turn_activity(self.request_seq, !calls.is_empty(), text.len());
            if !text.is_empty() {
                last_tool_only_compaction_streak = 0;
            }
            if calls.is_empty() {
                self.emit_profile_summary()?;
                return Ok(());
            }

            for (call_id, tool_name, args) in calls {
                self.profiler
                    .record_tool_call(self.request_seq, &tool_name, &args);
                println!("\n> {tool_name} {}", serde_json::to_string(&args)?);
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

    pub(super) fn emit_profile_summary(&mut self) -> Result<()> {
        let summary = self.profile_summary();
        if let Some(trace) = &mut self.trace {
            trace.write(self.request_seq, "profile-summary", &summary)?;
        }
        if self.profile {
            println!("{}", serde_json::to_string_pretty(&summary)?);
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
        self.emit_profile_summary()
    }
}
