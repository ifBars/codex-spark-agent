use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::auth::{self, AuthTokens};
use crate::client::{SparkClient, function_calls, output_items_for_next_input, response_text};
use crate::config;
use crate::profiler::{
    AgentProfiler, SPARK_CONTEXT_WINDOW_TOKENS, approx_token_count_from_chars, context_window_pct,
    tool_signature,
};
use crate::tools::{ToolResult, builtin_tools, invoke};

pub struct AgentRunner {
    client: SparkClient,
    cwd: PathBuf,
    input: Vec<Value>,
    max_turns: Option<usize>,
    trace: Option<TraceWriter>,
    compact_after_chars: usize,
    max_input_chars: usize,
    request_seq: usize,
    profile: bool,
    profiler: AgentProfiler,
    readonly_tool_cache: HashMap<String, CachedToolObservation>,
    loaded_skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub input: Vec<Value>,
    pub request_seq: usize,
    pub profiler: AgentProfiler,
    #[serde(default)]
    pub loaded_skills: Vec<String>,
}

#[derive(Debug, Clone)]
struct CachedToolObservation {
    result: ToolResult,
    first_turn: usize,
    hits: usize,
}

impl AgentRunner {
    pub fn new(
        mut auth_tokens: AuthTokens,
        cwd: PathBuf,
        model: String,
        max_turns: Option<usize>,
        trace: bool,
        profile: bool,
        compact_after_chars: usize,
        max_input_chars: usize,
    ) -> Result<Self> {
        if auth::is_expired(&auth_tokens) {
            println!("Refreshing ChatGPT token...");
            auth_tokens = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(auth::refresh(&auth_tokens))
            })?;
            config::save_auth(&auth_tokens)?;
        }

        let trace_metadata = TraceMetadata {
            cwd: cwd.clone(),
            model: model.clone(),
            max_turns,
            compact_after_chars,
            max_input_chars,
        };

        Ok(Self {
            client: SparkClient::new(auth_tokens, model),
            cwd: cwd.clone(),
            input: Vec::new(),
            max_turns,
            trace: if trace {
                Some(TraceWriter::new(cwd, trace_metadata)?)
            } else {
                None
            },
            compact_after_chars,
            max_input_chars,
            request_seq: 0,
            profile,
            profiler: AgentProfiler::default(),
            readonly_tool_cache: HashMap::new(),
            loaded_skills: Vec::new(),
        })
    }

    pub async fn run(&mut self, prompt: &str) -> Result<()> {
        self.push_user_message(prompt);
        self.run_until_idle().await
    }

    pub fn clear_conversation(&mut self) {
        self.input.clear();
        self.request_seq = 0;
        self.profiler = AgentProfiler::default();
        self.readonly_tool_cache.clear();
        self.loaded_skills.clear();
    }

    pub fn input_chars(&self) -> Result<usize> {
        Ok(serde_json::to_string(&self.input)?.len())
    }

    pub fn profile_status(&self) -> String {
        let live =
            context_pressure_json(&self.input, self.compact_after_chars, self.max_input_chars)
                .unwrap_or_else(|error| json!({"error": error.to_string()}));
        let live_input_chars = live
            .get("input_chars")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let live_approx_tokens = live
            .get("approx_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let live_context_pct = live
            .get("context_window_pct")
            .and_then(Value::as_f64)
            .unwrap_or_default();
        format!(
            "{}; live_input_chars={}, live_approx_input_tokens={} ({:.1}% of 128k), compact_after_chars={}, max_input_chars={}",
            self.profiler.status_line(),
            live_input_chars,
            live_approx_tokens,
            live_context_pct,
            self.compact_after_chars,
            self.max_input_chars
        )
    }

    pub fn profile_summary(&self) -> Value {
        let mut summary = self.profiler.to_json();
        if let Some(object) = summary.as_object_mut() {
            object.insert(
                "live_context".to_string(),
                context_pressure_json(&self.input, self.compact_after_chars, self.max_input_chars)
                    .unwrap_or_else(|error| json!({"error": error.to_string()})),
            );
        }
        summary
    }

    pub fn snapshot(&self) -> AgentSnapshot {
        AgentSnapshot {
            input: self.input.clone(),
            request_seq: self.request_seq,
            profiler: self.profiler.clone(),
            loaded_skills: self.loaded_skills.clone(),
        }
    }

    pub fn restore_snapshot(&mut self, snapshot: AgentSnapshot) {
        self.input = snapshot.input;
        self.request_seq = snapshot.request_seq;
        self.profiler = snapshot.profiler;
        self.loaded_skills = snapshot.loaded_skills;
        self.readonly_tool_cache.clear();
    }

    pub fn loaded_skills(&self) -> &[String] {
        &self.loaded_skills
    }

    pub async fn compact_now(&mut self) -> Result<Option<Value>> {
        let tools = builtin_tools();
        let report = self.compact_once(&tools, true).await?;
        if let Some(report) = &report {
            self.profiler.record_compaction(report);
            if let Some(trace) = &mut self.trace {
                trace.write(self.request_seq + 1, "compaction", report)?;
            }
            self.emit_profile_summary()?;
        }
        Ok(report)
    }

    pub fn load_skill_context(&mut self, name: &str, summary: &str) -> bool {
        if self.loaded_skills.iter().any(|loaded| loaded == name) {
            return false;
        }
        self.input.push(json!({
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": format!("[spark skill loaded: {name}]\n{summary}")
            }]
        }));
        self.loaded_skills.push(name.to_string());
        true
    }

    pub async fn compile_skill_summary(&self, name: &str, raw_skill: &str) -> Result<String> {
        self.client.compile_skill_summary(name, raw_skill).await
    }

    pub fn load_session(&mut self, path: &std::path::Path) -> Result<bool> {
        if !path.exists() {
            return Ok(false);
        }
        let body = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read session {}", path.display()))?;
        let snapshot = serde_json::from_str::<AgentSnapshot>(&body)
            .with_context(|| format!("failed to parse session {}", path.display()))?;
        self.restore_snapshot(snapshot);
        Ok(true)
    }

    pub fn save_session(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(&self.snapshot())?)
            .with_context(|| format!("failed to write session {}", path.display()))?;
        Ok(())
    }

    fn push_user_message(&mut self, prompt: &str) {
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

    async fn run_until_idle(&mut self) -> Result<()> {
        let tools = builtin_tools();

        let mut turn = 0usize;
        loop {
            turn += 1;
            if let Some(max_turns) = self.max_turns
                && turn > max_turns
            {
                let message = format!("stopped after {max_turns} turns without completion");
                self.record_terminal_error(self.request_seq + 1, "max_turns", &message)?;
                anyhow::bail!(message);
            }

            match self.compact_once(&tools, false).await {
                Ok(Some(report)) => {
                    self.profiler.record_compaction(&report);
                    if let Some(trace) = &mut self.trace {
                        trace.write(self.request_seq + 1, "compaction", &report)?;
                    }
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

    async fn invoke_with_cache(&mut self, tool_name: &str, args: Value) -> ToolResult {
        let signature = tool_signature(tool_name, &args);
        if is_cacheable_readonly_tool(tool_name)
            && let Some(cached) = self.readonly_tool_cache.get_mut(&signature)
        {
            cached.hits += 1;
            self.profiler
                .record_readonly_tool_cache_hit(self.request_seq, tool_name, &args);
            let mut result = cached.result.clone();
            if let Some(data) = result.data.as_object_mut() {
                data.insert("cached_observation".to_string(), Value::Bool(true));
                data.insert(
                    "first_observed_turn".to_string(),
                    Value::Number(cached.first_turn.into()),
                );
                data.insert("cache_hits".to_string(), Value::Number(cached.hits.into()));
            }
            return result;
        }

        let result = invoke(&self.cwd, tool_name, args).await;
        if is_cacheable_readonly_tool(tool_name) && should_cache_readonly_result(&result) {
            self.readonly_tool_cache.insert(
                signature,
                CachedToolObservation {
                    result: result.clone(),
                    first_turn: self.request_seq,
                    hits: 0,
                },
            );
        } else if invalidates_readonly_tool_cache(tool_name) {
            self.readonly_tool_cache.clear();
        }
        result
    }

    fn emit_profile_summary(&mut self) -> Result<()> {
        let summary = self.profile_summary();
        if let Some(trace) = &mut self.trace {
            trace.write(self.request_seq, "profile-summary", &summary)?;
        }
        if self.profile {
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Ok(())
    }

    fn record_terminal_error(&mut self, turn: usize, stage: &str, error: &str) -> Result<()> {
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

    async fn compact_once(
        &mut self,
        tools: &[crate::tools::ToolDescriptor],
        force: bool,
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
                let (replacement, pressure_report) = compact_remote_history_to_threshold(
                    &compact_input,
                    remote_output,
                    self.compact_after_chars,
                )?;
                let after = serde_json::to_string(&replacement)?.len();
                self.input = replacement;
                Ok(Some(json!({
                    "method": "responses_compact",
                    "forced": force,
                    "duration_ms": duration_ms,
                    "before_chars": before,
                    "compact_request_chars": serde_json::to_string(&compact_input)?.len(),
                    "after_chars": after,
                    "threshold_chars": self.compact_after_chars,
                    "local_pressure": pressure_report,
                    "raw": raw,
                })))
            }
            Err(error) => {
                let duration_ms = compaction_started.elapsed().as_millis() as u64;
                let fallback = compact_input_locally(&mut self.input, self.compact_after_chars)?;
                if let Some(report) = fallback {
                    Ok(Some(json!({
                        "method": "local_fallback",
                        "forced": force,
                        "duration_ms": duration_ms,
                        "remote_error": error.to_string(),
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

fn compact_remote_history_to_threshold(
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

fn context_pressure_json(
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

fn compact_input_locally(input: &mut Vec<Value>, max_chars: usize) -> Result<Option<Value>> {
    if max_chars == 0 {
        return Ok(None);
    }

    let before = serde_json::to_string(input)?.len();
    if before <= max_chars {
        return Ok(None);
    }

    let output_indexes = input
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            item.get("type").and_then(Value::as_str) == Some("function_call_output")
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    let mut compacted = 0usize;
    let keep_full_from = output_indexes.len().saturating_sub(2);
    for (ordinal, index) in output_indexes.iter().copied().enumerate() {
        let keep_recent = ordinal >= keep_full_from;
        let max_output_chars = if keep_recent { 16_000 } else { 4_000 };
        if compact_output_item(&mut input[index], max_output_chars)? {
            compacted += 1;
        }
    }

    let mid = serde_json::to_string(input)?.len();
    if mid > max_chars {
        let message_indexes = input
            .iter()
            .enumerate()
            .filter(|(_, item)| item.get("role").and_then(Value::as_str).is_some())
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let keep_messages_from = message_indexes.len().saturating_sub(8);
        for (ordinal, index) in message_indexes.iter().copied().enumerate() {
            if ordinal >= keep_messages_from {
                continue;
            }
            if compact_message_item(&mut input[index], 800)? {
                compacted += 1;
            }
        }
    }

    let after = serde_json::to_string(input)?.len();
    if compacted == 0 {
        return Ok(None);
    }

    Ok(Some(json!({
        "before_chars": before,
        "after_chars": after,
        "compacted_outputs": compacted,
        "threshold_chars": max_chars,
    })))
}

fn is_cacheable_readonly_tool(tool_name: &str) -> bool {
    matches!(tool_name, "fs.read" | "fs.list" | "fs.search")
}

fn should_cache_readonly_result(result: &ToolResult) -> bool {
    result.ok || result.error.is_some()
}

fn invalidates_readonly_tool_cache(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "fs.write" | "fs.replace" | "fs.edit" | "cmd.exec"
    )
}

fn trim_codex_generated_tail_to_fit(mut input: Vec<Value>, max_chars: usize) -> Result<Vec<Value>> {
    while serde_json::to_string(&input)?.len() > max_chars {
        let Some(last) = input.last() else {
            break;
        };
        if !is_codex_generated_item(last) {
            break;
        }
        input.pop();
    }
    Ok(input)
}

fn install_remote_compaction_history(
    prompt_input: &[Value],
    remote_output: Vec<Value>,
) -> Vec<Value> {
    let mut replacement = process_remote_compaction_output(remote_output);
    if !replacement.is_empty() {
        return replacement;
    }

    replacement = retained_user_messages_for_remote_compaction_v2(prompt_input, 20_000);
    if replacement.is_empty() {
        replacement = prompt_input
            .iter()
            .rev()
            .filter(|item| is_real_user_message(item) || is_assistant_message(item))
            .take(8)
            .cloned()
            .collect::<Vec<_>>();
        replacement.reverse();
    }
    replacement
}

fn process_remote_compaction_output(items: Vec<Value>) -> Vec<Value> {
    items
        .into_iter()
        .filter_map(normalize_compacted_history_item)
        .collect()
}

fn normalize_compacted_history_item(mut item: Value) -> Option<Value> {
    match item.get("type").and_then(Value::as_str) {
        Some("compaction" | "compaction_summary") => Some(json!({
            "type": "compaction",
            "encrypted_content": item.get("encrypted_content")?.clone(),
        })),
        Some("context_compaction") => Some(json!({
            "type": "context_compaction",
            "encrypted_content": item.get("encrypted_content").cloned(),
        })),
        Some("compaction_trigger") => None,
        Some("message") | None => {
            if should_keep_compacted_history_item(&item) {
                strip_response_only_fields(&mut item);
                Some(item)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn retained_user_messages_for_remote_compaction_v2(
    input: &[Value],
    max_tokens: usize,
) -> Vec<Value> {
    let mut remaining = max_tokens;
    let mut retained_reversed = Vec::new();
    for item in input.iter().rev() {
        if !is_real_user_message(item) {
            continue;
        }
        if remaining == 0 {
            continue;
        }
        let token_count = approx_token_count(&message_text_from_value(item)).max(1);
        if token_count <= remaining {
            retained_reversed.push(item.clone());
            remaining = remaining.saturating_sub(token_count);
        } else if let Some(truncated) = truncate_message_text_to_token_budget(item, remaining) {
            retained_reversed.push(truncated);
            remaining = 0;
        }
    }
    retained_reversed.reverse();
    retained_reversed
}

fn should_keep_compacted_history_item(item: &Value) -> bool {
    match item.get("type").and_then(Value::as_str) {
        Some("compaction" | "compaction_summary" | "context_compaction") => true,
        Some("compaction_trigger") => false,
        Some("message") | None => is_assistant_message(item) || is_real_user_message(item),
        _ => false,
    }
}

fn strip_response_only_fields(item: &mut Value) {
    if let Some(object) = item.as_object_mut() {
        object.remove("id");
        object.remove("status");
    }
}

fn is_codex_generated_item(item: &Value) -> bool {
    matches!(
        item.get("type").and_then(Value::as_str),
        Some(
            "function_call"
                | "function_call_output"
                | "tool_search_call"
                | "tool_search_output"
                | "reasoning"
                | "web_search_call"
                | "image_generation_call"
        )
    )
}

fn is_assistant_message(item: &Value) -> bool {
    item.get("role").and_then(Value::as_str) == Some("assistant")
}

fn is_real_user_message(item: &Value) -> bool {
    item.get("role").and_then(Value::as_str) == Some("user")
        && !message_text_from_value(item).trim().is_empty()
}

fn message_text_from_value(item: &Value) -> String {
    item.get("content")
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

fn truncate_message_text_to_token_budget(item: &Value, max_tokens: usize) -> Option<Value> {
    let mut cloned = item.clone();
    let content = cloned.get_mut("content")?.as_array_mut()?;
    let mut remaining = max_tokens;
    let mut any_text = false;
    for part in content {
        let Some(text) = part.get_mut("text") else {
            continue;
        };
        let Some(raw) = text.as_str() else {
            continue;
        };
        if remaining == 0 {
            *text = Value::String(String::new());
            continue;
        }
        let tokens = approx_token_count(raw);
        if tokens <= remaining {
            remaining = remaining.saturating_sub(tokens);
            any_text = any_text || !raw.is_empty();
        } else {
            *text = Value::String(truncate_text_tokens(raw, remaining));
            remaining = 0;
            any_text = true;
        }
    }
    any_text.then_some(cloned)
}

fn approx_token_count(text: &str) -> usize {
    text.len().div_ceil(4)
}

fn truncate_text_tokens(text: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens.saturating_mul(4);
    text.chars().take(max_chars).collect()
}

fn compact_output_item(item: &mut Value, max_output_chars: usize) -> Result<bool> {
    let Some(output) = item.get_mut("output") else {
        return Ok(false);
    };
    let Some(raw) = output.as_str() else {
        return Ok(false);
    };
    if raw.len() <= max_output_chars {
        return Ok(false);
    }

    let parsed = serde_json::from_str::<Value>(raw).unwrap_or_else(|_| json!({"raw": raw}));
    let compact = json!({
        "ok": parsed.get("ok").cloned().unwrap_or(Value::Bool(true)),
        "compacted": true,
        "original_chars": raw.len(),
        "preview": compact_text(raw, max_output_chars),
        "note": "Older tool output compacted by spark harness; rerun the relevant tool if exact content is needed.",
    });
    *output = Value::String(serde_json::to_string(&compact)?);
    Ok(true)
}

fn compact_message_item(item: &mut Value, max_chars: usize) -> Result<bool> {
    let Some(content) = item.get_mut("content").and_then(Value::as_array_mut) else {
        return Ok(false);
    };
    let mut changed = false;
    for part in content {
        let Some(text) = part.get_mut("text") else {
            continue;
        };
        let Some(raw) = text.as_str() else {
            continue;
        };
        if raw.starts_with("[older conversation turn compacted;") {
            continue;
        }
        if raw.len() <= max_chars {
            continue;
        }
        *text = Value::String(format!(
            "[older conversation turn compacted; original_chars={}]\n{}",
            raw.len(),
            compact_text(raw, max_chars)
        ));
        changed = true;
    }
    Ok(changed)
}

fn compact_text(raw: &str, max_chars: usize) -> String {
    let head_len = max_chars.saturating_mul(3) / 4;
    let tail_len = max_chars.saturating_sub(head_len).saturating_sub(64);
    let head = raw.chars().take(head_len).collect::<String>();
    let tail_vec = raw.chars().rev().take(tail_len).collect::<Vec<_>>();
    let tail = tail_vec.into_iter().rev().collect::<String>();
    format!("{head}\n...[compacted]...\n{tail}")
}

struct TraceWriter {
    dir: PathBuf,
    file_counts: HashMap<String, usize>,
}

struct TraceMetadata {
    cwd: PathBuf,
    model: String,
    max_turns: Option<usize>,
    compact_after_chars: usize,
    max_input_chars: usize,
}

impl TraceWriter {
    fn new(cwd: PathBuf, metadata: TraceMetadata) -> Result<Self> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let dir = cwd.join(".spark-runs").join(format!("run-{now_ms}"));
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("000-trace-metadata.json"),
            serde_json::to_vec_pretty(&json!({
                "schema_version": 1,
                "started_at_unix_ms": now_ms,
                "cwd": metadata.cwd,
                "model": metadata.model,
                "max_turns": metadata.max_turns,
                "compact_after_chars": metadata.compact_after_chars,
                "max_input_chars": metadata.max_input_chars,
                "context_window_tokens": SPARK_CONTEXT_WINDOW_TOKENS,
            }))?,
        )?;
        eprintln!("trace: {}", dir.display());
        Ok(Self {
            dir,
            file_counts: HashMap::new(),
        })
    }

    fn write(&mut self, turn: usize, kind: &str, value: &Value) -> Result<()> {
        let key = format!("{turn:03}-{kind}");
        let count = self.file_counts.entry(key.clone()).or_insert(0);
        *count += 1;
        let filename = if *count == 1 {
            format!("{key}.json")
        } else {
            format!("{key}-{count:03}.json")
        };
        let path = self.dir.join(filename);
        std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn remote_compaction_summary_alias_normalizes_to_codex_compaction_item() {
        let output = vec![
            json!({
                "type": "message",
                "id": "msg_keep_id_out",
                "status": "completed",
                "role": "user",
                "content": [{"type": "input_text", "text": "keep the user request"}]
            }),
            json!({
                "type": "compaction_summary",
                "id": "cmp_drop_id",
                "encrypted_content": "encrypted-summary"
            }),
        ];

        let replacement = process_remote_compaction_output(output);

        assert_eq!(replacement.len(), 2);
        assert_eq!(replacement[0]["role"], "user");
        assert!(replacement[0].get("id").is_none());
        assert!(replacement[0].get("status").is_none());
        assert_eq!(
            replacement[1],
            json!({
                "type": "compaction",
                "encrypted_content": "encrypted-summary"
            })
        );
    }

    #[test]
    fn remote_compaction_output_drops_stale_developer_and_tool_items() {
        let output = vec![
            json!({
                "type": "message",
                "role": "developer",
                "content": [{"type": "input_text", "text": "stale instructions"}]
            }),
            json!({
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "{}"
            }),
            json!({
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "kept assistant"}]
            }),
            json!({
                "type": "compaction",
                "encrypted_content": "encrypted-summary"
            }),
        ];

        let replacement = process_remote_compaction_output(output);

        assert_eq!(replacement.len(), 2);
        assert_eq!(replacement[0]["role"], "assistant");
        assert_eq!(replacement[1]["type"], "compaction");
    }

    #[test]
    fn fallback_remote_compaction_history_retains_recent_user_messages_under_budget() {
        let prompt_input = vec![
            json!({
                "role": "developer",
                "content": [{"type": "input_text", "text": "not retained"}]
            }),
            json!({
                "role": "user",
                "content": [{"type": "input_text", "text": "first"}]
            }),
            json!({
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "{}"
            }),
            json!({
                "role": "user",
                "content": [{"type": "input_text", "text": "second"}]
            }),
        ];

        let replacement = install_remote_compaction_history(&prompt_input, Vec::new());

        assert_eq!(replacement.len(), 2);
        assert_eq!(message_text_from_value(&replacement[0]), "first");
        assert_eq!(message_text_from_value(&replacement[1]), "second");
    }

    #[test]
    fn remote_compaction_above_threshold_gets_local_pressure_pass() {
        let remote_output = (0..12)
            .map(|index| {
                json!({
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": format!("remote retained message {index} {}", "x".repeat(10_000))
                    }]
                })
            })
            .collect::<Vec<_>>();

        let (replacement, pressure) =
            compact_remote_history_to_threshold(&[], remote_output, 90_000)
                .expect("compact remote history");
        let final_chars = serde_json::to_string(&replacement)
            .expect("serialize replacement")
            .len();
        let pressure = pressure.expect("local pressure report");
        let remote_after_chars = pressure["remote_after_chars"]
            .as_u64()
            .expect("remote after chars") as usize;

        assert_eq!(pressure["reason"], "remote_compaction_above_threshold");
        assert!(remote_after_chars > 90_000);
        assert!(final_chars < remote_after_chars);
        assert_eq!(pressure["made_progress"], true);
        assert_eq!(
            pressure["final_chars"].as_u64().expect("final chars") as usize,
            final_chars
        );
    }

    #[test]
    fn context_pressure_reports_live_thresholds() {
        let input = vec![json!({
            "role": "user",
            "content": [{"type": "input_text", "text": "x".repeat(120)}]
        })];

        let pressure = context_pressure_json(&input, 40, 10_000).expect("context pressure");

        assert!(pressure["input_chars"].as_u64().expect("input chars") > 40);
        assert_eq!(pressure["compact_after_exceeded"], true);
        assert_eq!(pressure["max_input_exceeded"], false);
        assert_eq!(pressure["context_window_tokens"], 128_000);
    }

    #[test]
    fn trace_writer_keeps_multiple_same_turn_entries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut writer = TraceWriter {
            dir: dir.path().to_path_buf(),
            file_counts: HashMap::new(),
        };

        writer
            .write(1, "tool-result", &json!({"tool": "fs.read"}))
            .expect("write first");
        writer
            .write(1, "tool-result", &json!({"tool": "cmd.exec"}))
            .expect("write second");

        assert!(dir.path().join("001-tool-result.json").exists());
        assert!(dir.path().join("001-tool-result-002.json").exists());
    }

    #[test]
    fn readonly_cache_policy_keeps_successes_and_failures() {
        assert!(should_cache_readonly_result(&ToolResult {
            ok: true,
            data: json!({"path": "README.md"}),
            error: None,
        }));
        assert!(should_cache_readonly_result(&ToolResult {
            ok: false,
            data: json!({}),
            error: Some("failed to read missing.txt".to_string()),
        }));
        assert!(!should_cache_readonly_result(&ToolResult {
            ok: false,
            data: json!({}),
            error: None,
        }));
    }

    #[test]
    fn agent_snapshot_round_trips_history_and_profile() {
        let snapshot = AgentSnapshot {
            input: vec![json!({
                "role": "user",
                "content": [{"type": "input_text", "text": "hello"}]
            })],
            request_seq: 7,
            profiler: AgentProfiler::default(),
            loaded_skills: vec!["demo".to_string()],
        };

        let encoded = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let decoded =
            serde_json::from_str::<AgentSnapshot>(&encoded).expect("deserialize snapshot");

        assert_eq!(decoded.input, snapshot.input);
        assert_eq!(decoded.request_seq, 7);
        assert_eq!(decoded.loaded_skills, vec!["demo"]);
        assert_eq!(decoded.profiler.to_json()["requests"], 0);
    }
}
