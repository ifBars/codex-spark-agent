use anyhow::Result;
use serde_json::{Value, json};

use crate::agent::{AgentRunner, TOOL_ONLY_STREAK_COMPACTION_TRIGGER};
use crate::profiler::{
    SPARK_CONTEXT_WINDOW_TOKENS, approx_token_count_from_chars, context_window_pct,
};
use crate::tools::builtin_tools;

#[path = "compaction/retention.rs"]
pub(in crate::agent) mod retention;

use retention::{
    append_post_compaction_verification_notice, compact_input_locally,
    install_remote_compaction_history, trim_codex_generated_tail_to_fit,
};

impl AgentRunner {
    pub async fn compact_now(&mut self) -> Result<Option<Value>> {
        let tools = builtin_tools();
        if !self.input.is_empty() {
            self.emit_compaction_start(Some("manual"), serde_json::to_string(&self.input)?.len());
        }
        let report = self.compact_once(&tools, true, Some("manual")).await?;
        if let Some(report) = &report {
            self.profiler.record_compaction(report);
            if let Some(trace) = &mut self.trace {
                trace.write(self.request_seq + 1, "compaction", report)?;
            }
            self.emit_compaction_finish(format_compaction_notice(report));
            self.emit_profile_summary()?;
        }
        Ok(report)
    }

    pub(super) async fn compact_once(
        &mut self,
        tools: &[crate::tools::ToolDescriptor],
        force: bool,
        trigger: Option<&'static str>,
    ) -> Result<Option<Value>> {
        if self.input.is_empty() {
            return Ok(None);
        }

        let before = serde_json::to_string(&self.input)?.len();
        if !force && (self.compact_after_chars == 0 || before <= self.compact_after_chars) {
            return Ok(None);
        }

        let compact_input =
            trim_codex_generated_tail_to_fit(self.input.clone(), self.max_input_chars)?;
        let compaction_started = std::time::Instant::now();
        match self.client.responses_compact(&compact_input, tools).await {
            Ok((remote_output, raw)) => {
                let duration_ms = compaction_started.elapsed().as_millis() as u64;
                let (mut replacement, pressure_report) = compact_remote_history_to_threshold(
                    &compact_input,
                    remote_output,
                    self.compact_after_chars,
                )?;
                let verification_notice =
                    append_post_compaction_verification_notice(&mut replacement, &compact_input);
                let after = serde_json::to_string(&replacement)?.len();
                self.input = replacement;
                Ok(Some(json!({
                    "method": "responses_compact",
                    "forced": force,
                    "trigger": trigger,
                    "duration_ms": duration_ms,
                    "before_chars": before,
                    "compact_request_chars": serde_json::to_string(&compact_input)?.len(),
                    "after_chars": after,
                    "threshold_chars": self.compact_after_chars,
                    "local_pressure": pressure_report,
                    "post_compaction_verification_notice": verification_notice.is_some(),
                    "post_compaction_required_actions": verification_notice
                        .map(|notice| notice.required_actions)
                        .unwrap_or(0),
                    "raw": raw,
                })))
            }
            Err(error) => {
                let duration_ms = compaction_started.elapsed().as_millis() as u64;
                let fallback = compact_input_locally(&mut self.input, self.compact_after_chars)?;
                if let Some(report) = fallback {
                    let verification_notice =
                        append_post_compaction_verification_notice(&mut self.input, &compact_input);
                    let after = serde_json::to_string(&self.input)?.len();
                    Ok(Some(json!({
                        "method": "local_fallback",
                        "forced": force,
                        "trigger": trigger,
                        "duration_ms": duration_ms,
                        "remote_error": error.to_string(),
                        "after_chars": after,
                        "post_compaction_verification_notice": verification_notice.is_some(),
                        "post_compaction_required_actions": verification_notice
                            .map(|notice| notice.required_actions)
                            .unwrap_or(0),
                        "fallback": report,
                    })))
                } else {
                    anyhow::bail!(
                        "remote compaction failed and local fallback made no progress: {error:#}"
                    );
                }
            }
        }
    }
}

pub(super) fn compaction_trigger_for_turn(
    compact_after_chars: usize,
    compact_after_tool_only_turns: usize,
    tool_only_streak: usize,
    last_tool_only_compaction_streak: usize,
    input: &[Value],
) -> Result<Option<&'static str>> {
    let input_chars = serde_json::to_string(input)?.len();
    if compact_after_chars != 0 && input_chars > compact_after_chars {
        return Ok(Some("size_threshold"));
    }
    if compact_after_tool_only_turns != 0
        && tool_only_streak >= compact_after_tool_only_turns
        && last_tool_only_compaction_streak == 0
        && should_compact_for_tool_only_streak(compact_after_chars, input_chars)
    {
        return Ok(Some(TOOL_ONLY_STREAK_COMPACTION_TRIGGER));
    }
    Ok(None)
}

fn should_compact_for_tool_only_streak(compact_after_chars: usize, input_chars: usize) -> bool {
    if compact_after_chars == 0 {
        return false;
    }
    input_chars >= tool_only_compaction_min_chars(compact_after_chars)
}

fn tool_only_compaction_min_chars(compact_after_chars: usize) -> usize {
    compact_after_chars.saturating_mul(9).div_ceil(10)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{compaction_trigger_for_turn, tool_only_compaction_min_chars};
    use crate::agent::TOOL_ONLY_STREAK_COMPACTION_TRIGGER;

    #[test]
    fn tool_only_compaction_waits_for_meaningful_input_pressure() {
        let small_input = vec![json!({
            "role": "user",
            "content": [{"type": "input_text", "text": "x".repeat(120)}]
        })];
        let medium_input = vec![json!({
            "role": "user",
            "content": [{"type": "input_text", "text": "x".repeat(90_000)}]
        })];
        let near_threshold_input = vec![json!({
            "role": "user",
            "content": [{"type": "input_text", "text": "x".repeat(145_000)}]
        })];

        let trigger = compaction_trigger_for_turn(160_000, 12, 12, 0, &small_input)
            .expect("trigger decision");
        assert_eq!(trigger, None);

        let trigger = compaction_trigger_for_turn(160_000, 12, 12, 0, &medium_input)
            .expect("trigger decision");
        assert_eq!(trigger, None);

        let trigger = compaction_trigger_for_turn(160_000, 12, 12, 0, &near_threshold_input)
            .expect("trigger decision");
        assert_eq!(trigger, Some(TOOL_ONLY_STREAK_COMPACTION_TRIGGER));
    }

    #[test]
    fn tool_only_compaction_waits_until_near_size_threshold() {
        assert_eq!(tool_only_compaction_min_chars(160_000), 144_000);
        assert_eq!(tool_only_compaction_min_chars(20_000), 18_000);
    }

    #[test]
    fn tool_only_compaction_disabled_without_size_threshold() {
        let input = vec![json!({
            "role": "user",
            "content": [{"type": "input_text", "text": "x".repeat(200_000)}]
        })];

        let trigger = compaction_trigger_for_turn(0, 12, 12, 0, &input).expect("trigger decision");
        assert_eq!(trigger, None);
    }
}

pub(in crate::agent) fn compact_remote_history_to_threshold(
    prompt_input: &[Value],
    remote_output: Vec<Value>,
    max_chars: usize,
) -> Result<(Vec<Value>, Option<Value>)> {
    let mut replacement = install_remote_compaction_history(prompt_input, remote_output);
    let remote_after_chars = serde_json::to_string(&replacement)?.len();
    if max_chars == 0 || remote_after_chars <= max_chars {
        return Ok((replacement, None));
    }

    let pressure_report = compact_input_locally(&mut replacement, max_chars)?;
    let final_chars = serde_json::to_string(&replacement)?.len();
    Ok((
        replacement,
        Some(json!({
            "reason": "remote_compaction_above_threshold",
            "remote_after_chars": remote_after_chars,
            "final_chars": final_chars,
            "made_progress": pressure_report.is_some(),
            "fallback": pressure_report,
        })),
    ))
}

pub(super) fn format_compaction_notice(report: &Value) -> String {
    let method = report
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let before = report
        .get("before_chars")
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".to_string());
    let after = report
        .get("after_chars")
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .or_else(|| {
            report
                .pointer("/fallback/after_chars")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "?".to_string());
    let duration_ms = report
        .get("duration_ms")
        .and_then(Value::as_u64)
        .map(|value| format!("{value}ms"))
        .unwrap_or_else(|| "?ms".to_string());
    let pressure = report
        .get("local_pressure")
        .and_then(|pressure| pressure.get("made_progress"))
        .and_then(Value::as_bool)
        .map(|made_progress| {
            if made_progress {
                " local_pressure=applied"
            } else {
                " local_pressure=no_progress"
            }
        })
        .unwrap_or("");

    format!("compaction: {method} {before}->{after} chars in {duration_ms}{pressure}")
}

pub(super) fn context_pressure_json(
    input: &[Value],
    compact_after_chars: usize,
    max_input_chars: usize,
) -> Result<Value> {
    let input_chars = serde_json::to_string(input)?.len();
    Ok(json!({
        "input_chars": input_chars,
        "approx_input_tokens": approx_token_count_from_chars(input_chars),
        "context_window_tokens": SPARK_CONTEXT_WINDOW_TOKENS,
        "context_window_pct": context_window_pct(input_chars),
        "compact_after_chars": compact_after_chars,
        "compact_after_exceeded": compact_after_chars != 0 && input_chars > compact_after_chars,
        "max_input_chars": max_input_chars,
        "max_input_exceeded": input_chars > max_input_chars,
    }))
}
