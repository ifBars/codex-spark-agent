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

const AGENT_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

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
    #[serde(default = "default_agent_snapshot_schema_version")]
    pub schema_version: u32,
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
        interactive: bool,
        session_name: Option<String>,
        new_session: bool,
        trace_context: Option<Value>,
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
            profile,
            interactive,
            session_name,
            new_session,
            context: trace_context,
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
            schema_version: AGENT_SNAPSHOT_SCHEMA_VERSION,
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

fn default_agent_snapshot_schema_version() -> u32 {
    AGENT_SNAPSHOT_SCHEMA_VERSION
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

fn format_compaction_notice(report: &Value) -> String {
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

    let mut compacted_tool_outputs = 0usize;
    let mut compacted_messages = 0usize;
    let keep_full_from = output_indexes.len().saturating_sub(2);
    for (ordinal, index) in output_indexes.iter().copied().enumerate() {
        let keep_recent = ordinal >= keep_full_from;
        let max_output_chars = if keep_recent { 16_000 } else { 4_000 };
        if compact_output_item(&mut input[index], max_output_chars)? {
            compacted_tool_outputs += 1;
        }
    }

    let mid = serde_json::to_string(input)?.len();
    if mid > max_chars {
        let has_compaction_summary = input.iter().any(|item| {
            matches!(
                item.get("type").and_then(Value::as_str),
                Some("compaction" | "context_compaction")
            )
        });
        let message_indexes = input
            .iter()
            .enumerate()
            .filter(|(_, item)| item.get("role").and_then(Value::as_str).is_some())
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let keep_messages_from = if has_compaction_summary {
            message_indexes.len()
        } else {
            message_indexes.len().saturating_sub(8)
        };
        for (ordinal, index) in message_indexes.iter().copied().enumerate() {
            if !has_compaction_summary && ordinal >= keep_messages_from {
                continue;
            }
            if compact_message_item(&mut input[index], 800)? {
                compacted_messages += 1;
            }
        }

        while serde_json::to_string(input)?.len() > max_chars {
            let mut made_progress = false;
            for index in message_indexes.iter().rev().copied() {
                if compact_message_item(&mut input[index], 1200)? {
                    compacted_messages += 1;
                    made_progress = true;
                    break;
                }
            }
            if !made_progress {
                break;
            }
        }
    }

    let after = serde_json::to_string(input)?.len();
    let compacted = compacted_tool_outputs + compacted_messages;
    if compacted == 0 {
        return Ok(None);
    }

    Ok(Some(json!({
        "before_chars": before,
        "after_chars": after,
        "compacted_outputs": compacted,
        "compacted_tool_outputs": compacted_tool_outputs,
        "compacted_messages": compacted_messages,
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
        "fs.write" | "fs.replace" | "fs.edit" | "fs.rename" | "cmd.exec"
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
        if raw.starts_with("[spark local message compaction]") {
            continue;
        }
        if raw.len() <= max_chars {
            continue;
        }
        let preview = compact_text(raw, max_chars);
        let retained_intent = retained_intent_block(raw);
        *text = Value::String(format!(
            "[spark local message compaction]\noriginal_chars={}\npreview_chars={}\nretained=head+tail\nexact_content=omitted; rerun/read the relevant source if exact text matters\n{}\n[/spark local message compaction]\n{}",
            raw.len(),
            preview.len(),
            retained_intent,
            preview
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

fn retained_intent_block(raw: &str) -> String {
    let lines = retained_intent_lines(raw, 12);
    let actions = retained_action_lines(&lines);
    let mut block = format!("retained_intent_lines={}", lines.len());
    for (index, line) in lines.iter().enumerate() {
        block.push('\n');
        block.push_str(&format!("intent_{}={}", index + 1, line));
    }
    block.push('\n');
    block.push_str(&format!("required_actions={}", actions.len()));
    for (index, action) in actions.iter().enumerate() {
        block.push('\n');
        block.push_str(&format!("action_{}={}", index + 1, action));
    }
    block
}

fn retained_intent_lines(raw: &str, max_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("row ") {
            continue;
        }
        if line
            == "Synthetic payload follows. Preserve the high-level instruction above; payload rows are intentionally repetitive profiling filler."
        {
            break;
        }
        if is_high_signal_intent_line(line) {
            lines.push(line.to_string());
            if lines.len() >= max_lines {
                break;
            }
        }
    }
    lines
}

fn is_high_signal_intent_line(line: &str) -> bool {
    line.starts_with("Profile scenario:")
        || line.starts_with("This prompt")
        || line.starts_with("Let the harness")
        || line.starts_with("Do not ")
        || line.starts_with("After any compaction")
        || line.starts_with("- ")
        || mentions_native_file_tool_action(line)
        || line.chars().next().is_some_and(|ch| ch.is_ascii_digit())
}

fn mentions_native_file_tool_action(line: &str) -> bool {
    (line.contains("fs.list")
        || line.contains("fs.read")
        || line.contains("fs.search")
        || line.contains("fs.replace")
        || line.contains("fs.edit")
        || line.contains("fs.write")
        || line.contains("fs.rename"))
        && (line.contains("use ") || line.contains("call ") || line.contains("run "))
}

fn retained_action_lines(lines: &[String]) -> Vec<String> {
    let mut actions = Vec::new();
    for line in lines {
        if let Some(action) = parse_native_tool_action(line) {
            actions.push(action);
        }
    }
    actions.sort();
    actions.dedup();
    actions
}

fn parse_native_tool_action(line: &str) -> Option<String> {
    if let Some(action) = parse_rename_tool_action(line) {
        return Some(action);
    }
    const TOOLS: [&str; 6] = [
        "fs.list",
        "fs.read",
        "fs.search",
        "fs.replace",
        "fs.edit",
        "fs.write",
    ];
    for tool in TOOLS {
        if let Some(action) = parse_file_tool_action(line, tool) {
            return Some(action);
        }
    }
    None
}

fn parse_rename_tool_action(line: &str) -> Option<String> {
    let normalized = line.trim().trim_end_matches('.');
    let tool = "fs.rename";
    let tool_index = normalized.find(tool)?;
    let before = normalized[..tool_index].trim_end();
    if !before.ends_with("use")
        && !before.ends_with("call")
        && !before.ends_with("run")
        && !before.ends_with("using")
    {
        return None;
    }
    let after_tool = normalized[tool_index + tool.len()..].trim_start();
    let after_tool = after_tool
        .strip_prefix("to move ")
        .or_else(|| after_tool.strip_prefix("move "))
        .or_else(|| after_tool.strip_prefix("from "))
        .unwrap_or(after_tool);
    let (from, to) = after_tool.split_once(" to ")?;
    let from = clean_action_path(from);
    let to = clean_action_path(to);
    if from.is_empty() || to.is_empty() {
        return Some(format!("tool={tool}"));
    }
    Some(format!("tool={tool} from={from} to={to}"))
}

fn parse_file_tool_action(line: &str, tool: &str) -> Option<String> {
    let normalized = line.trim().trim_end_matches('.');
    let tool_index = normalized.find(tool)?;
    let before = normalized[..tool_index].trim_end();
    if !before.ends_with("use")
        && !before.ends_with("call")
        && !before.ends_with("run")
        && !before.ends_with("using")
    {
        return None;
    }
    let after_tool = normalized[tool_index + tool.len()..].trim_start();
    let after_tool = after_tool
        .strip_prefix("on ")
        .or_else(|| after_tool.strip_prefix("in "))
        .or_else(|| after_tool.strip_prefix("for "))
        .unwrap_or(after_tool);
    if after_tool.is_empty() {
        return Some(format!("tool={tool}"));
    }
    let (path, tail) = after_tool
        .split_once(" with ")
        .map_or((after_tool, ""), |(path, tail)| (path, tail));
    let mut path = path.trim().trim_matches('`');
    if let Some((prefix, _)) = path.split_once(", then") {
        path = prefix.trim();
    }
    if let Some((prefix, _)) = path.split_once(" for ") {
        path = prefix.trim();
    }
    let path = clean_action_path(path);
    if path.is_empty() {
        return Some(format!("tool={tool}"));
    }
    let mut action = format!("tool={tool} path={path}");
    if tool == "fs.list" {
        let recursive = if tail.contains("recursive=false") {
            "false"
        } else if tail.contains("recursive=true") {
            "true"
        } else {
            "unspecified"
        };
        action.push_str(&format!(" recursive={recursive}"));
    }
    Some(action)
}

fn clean_action_path(path: &str) -> String {
    path.trim()
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim_end_matches(',')
        .trim_end_matches(':')
        .trim()
        .to_string()
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
    profile: bool,
    interactive: bool,
    session_name: Option<String>,
    new_session: bool,
    context: Option<Value>,
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
                "profile": metadata.profile,
                "interactive": metadata.interactive,
                "session": metadata.session_name,
                "new_session": metadata.new_session,
                "context": metadata.context,
                "compact_after_chars": metadata.compact_after_chars,
                "compact_after_approx_tokens": approx_token_count_from_chars(metadata.compact_after_chars),
                "max_input_chars": metadata.max_input_chars,
                "max_input_approx_tokens": approx_token_count_from_chars(metadata.max_input_chars),
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
    fn remote_compaction_summary_compacts_replayed_large_user_message() {
        let remote_output = vec![
            json!({
                "type": "message",
                "role": "user",
                "status": "completed",
                "id": "msg_replayed",
                "content": [{
                    "type": "input_text",
                    "text": format!("important instruction\n{}\nfinal instruction", "x".repeat(180_000))
                }]
            }),
            json!({
                "type": "compaction_summary",
                "encrypted_content": "encrypted-summary",
            }),
        ];

        let (replacement, pressure) =
            compact_remote_history_to_threshold(&[], remote_output, 160_000)
                .expect("compact remote history");
        let pressure = pressure.expect("local pressure report");
        let final_chars = serde_json::to_string(&replacement)
            .expect("serialize replacement")
            .len();

        assert_eq!(pressure["made_progress"], true);
        assert!(final_chars < 160_000);
        assert_eq!(replacement[1]["type"], "compaction");
        let retained = message_text_from_value(&replacement[0]);
        assert!(retained.contains("important instruction"));
        assert!(retained.contains("final instruction"));
        assert!(retained.contains("[spark local message compaction]"));
        assert!(retained.contains("exact_content=omitted"));
    }

    #[test]
    fn local_compaction_report_splits_tool_outputs_and_messages() {
        let mut input = (0..12)
            .flat_map(|index| {
                [
                    json!({
                        "role": "user",
                        "content": [{
                            "type": "input_text",
                            "text": format!("message {index} {}", "m".repeat(10_000))
                        }]
                    }),
                    json!({
                        "type": "function_call_output",
                        "call_id": format!("call_{index}"),
                        "output": "o".repeat(10_000)
                    }),
                ]
            })
            .collect::<Vec<_>>();

        let report = compact_input_locally(&mut input, 40_000)
            .expect("local compact")
            .expect("report");

        assert!(
            report["compacted_tool_outputs"]
                .as_u64()
                .expect("tool outputs")
                > 0
        );
        assert!(report["compacted_messages"].as_u64().expect("messages") > 0);
        assert_eq!(
            report["compacted_outputs"],
            json!(
                report["compacted_tool_outputs"].as_u64().unwrap()
                    + report["compacted_messages"].as_u64().unwrap()
            )
        );
    }

    #[test]
    fn local_compaction_can_shrink_single_large_recent_user_message() {
        let mut input = vec![json!({
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": format!("must keep start\n{}\nmust keep end", "x".repeat(180_000))
            }]
        })];

        let report = compact_input_locally(&mut input, 40_000)
            .expect("local compact")
            .expect("report");
        let final_chars = serde_json::to_string(&input)
            .expect("serialize compacted input")
            .len();
        let retained = message_text_from_value(&input[0]);

        assert_eq!(report["compacted_tool_outputs"], 0);
        assert_eq!(report["compacted_messages"], 1);
        assert!(final_chars < 40_000);
        assert!(retained.contains("must keep start"));
        assert!(retained.contains("must keep end"));
        assert!(retained.contains("[spark local message compaction]"));
        assert!(retained.contains("retained=head+tail"));
    }

    #[test]
    fn compact_message_item_is_idempotent_for_local_handoff() {
        let mut item = json!({
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": format!("first\n{}\nlast", "x".repeat(20_000))
            }]
        });

        assert!(compact_message_item(&mut item, 1200).expect("first compact"));
        let once = message_text_from_value(&item);
        assert!(!compact_message_item(&mut item, 1200).expect("second compact"));
        let twice = message_text_from_value(&item);

        assert_eq!(once, twice);
        assert!(once.contains("preview_chars="));
    }

    #[test]
    fn local_compaction_handoff_retains_intent_lines_without_filler_rows() {
        let raw = format!(
            "Profile scenario: compaction-pressure.\n\
             This prompt intentionally creates long-context pressure below Spark's 128k context window.\n\
             Let the harness compact automatically if its threshold is crossed.\n\
             Do not restate the synthetic payload. After any compaction, use fs.list on src with recursive=false, then answer with:\n\
             - whether the task remained understandable,\n\
             - which tool you used,\n\
             Next, use fs.read on README.md.\n\
             Then use fs.write on .spark-scenarios/file-ops/drafts/report-draft.md.\n\
             Then use fs.rename to move .spark-scenarios/file-ops/drafts/report-draft.md to .spark-scenarios/file-ops/final/report.md.\n\
             Synthetic payload follows. Preserve the high-level instruction above; payload rows are intentionally repetitive profiling filler.\n\
             row 00001: {}\n\
             row 00002: {}\n",
            "x".repeat(4000),
            "y".repeat(4000)
        );

        let lines = retained_intent_lines(&raw, 12);
        let block = retained_intent_block(&raw);

        assert!(lines.iter().any(|line| line.contains("Profile scenario")));
        assert!(lines.iter().any(|line| line.contains("fs.list on src")));
        assert!(lines.iter().any(|line| line.contains("which tool")));
        assert!(!lines.iter().any(|line| line.starts_with("row ")));
        assert!(block.contains("retained_intent_lines="));
        assert!(block.contains("intent_1=Profile scenario: compaction-pressure."));
        assert!(block.contains("required_actions=4"));
        assert!(block.contains("action_1=tool=fs.list path=src recursive=false"));
        assert!(block.contains("action_2=tool=fs.read path=README.md"));
        assert!(block.contains(
            "action_3=tool=fs.rename from=.spark-scenarios/file-ops/drafts/report-draft.md to=.spark-scenarios/file-ops/final/report.md"
        ));
        assert!(block.contains(
            "action_4=tool=fs.write path=.spark-scenarios/file-ops/drafts/report-draft.md"
        ));
    }

    #[test]
    fn parses_required_native_file_tool_actions_from_intent_lines() {
        let list_action = parse_native_tool_action(
            "Do not restate the synthetic payload. After any compaction, use fs.list on src with recursive=false, then answer with:",
        )
        .expect("list action");
        let read_action =
            parse_native_tool_action("Next, use fs.read on `README.md`.").expect("read action");
        let search_action = parse_native_tool_action("Then run fs.search in src for compact.")
            .expect("search action");
        let write_action = parse_native_tool_action(
            "Then use fs.write on .spark-scenarios/file-ops/drafts/report-draft.md with a short markdown report.",
        )
        .expect("write action");
        let rename_action = parse_native_tool_action(
            "Then use fs.rename to move .spark-scenarios/file-ops/drafts/report-draft.md to .spark-scenarios/file-ops/final/report.md.",
        )
        .expect("rename action");

        assert_eq!(list_action, "tool=fs.list path=src recursive=false");
        assert_eq!(read_action, "tool=fs.read path=README.md");
        assert_eq!(search_action, "tool=fs.search path=src");
        assert_eq!(
            write_action,
            "tool=fs.write path=.spark-scenarios/file-ops/drafts/report-draft.md"
        );
        assert_eq!(
            rename_action,
            "tool=fs.rename from=.spark-scenarios/file-ops/drafts/report-draft.md to=.spark-scenarios/file-ops/final/report.md"
        );
    }

    #[test]
    fn local_compaction_report_keeps_aggregate_output_count() {
        let mut input = (0..4)
            .map(|index| {
                json!({
                    "type": "function_call_output",
                    "call_id": format!("call_{index}"),
                    "output": "o".repeat(20_000)
                })
            })
            .collect::<Vec<_>>();

        let report = compact_input_locally(&mut input, 40_000)
            .expect("local compact")
            .expect("report");

        assert_eq!(
            report["compacted_outputs"],
            report["compacted_tool_outputs"]
        );
        assert_eq!(report["compacted_messages"], 0);
    }

    #[test]
    fn compaction_notice_summarizes_remote_report() {
        let notice = format_compaction_notice(&json!({
            "method": "responses_compact",
            "duration_ms": 1234,
            "before_chars": 220_000,
            "after_chars": 80_000
        }));

        assert_eq!(
            notice,
            "compaction: responses_compact 220000->80000 chars in 1234ms"
        );
    }

    #[test]
    fn compaction_notice_marks_local_pressure() {
        let notice = format_compaction_notice(&json!({
            "method": "responses_compact",
            "duration_ms": 1234,
            "before_chars": 220_000,
            "after_chars": 100_000,
            "local_pressure": {
                "made_progress": true
            }
        }));

        assert!(notice.contains("local_pressure=applied"));
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
    fn trace_metadata_includes_approx_token_thresholds() {
        let dir = tempfile::tempdir().expect("tempdir");
        let writer = TraceWriter::new(
            dir.path().to_path_buf(),
            TraceMetadata {
                cwd: dir.path().to_path_buf(),
                model: "gpt-5.3-codex-spark".to_string(),
                max_turns: None,
                compact_after_chars: 120_000,
                max_input_chars: 480_000,
                profile: true,
                interactive: true,
                session_name: Some("demo-session".to_string()),
                new_session: true,
                context: Some(json!({
                    "profile_scenario": {
                        "name": "compaction-pressure",
                        "target_tokens": 45_000
                    }
                })),
            },
        )
        .expect("trace writer");

        let metadata = std::fs::read_to_string(writer.dir.join("000-trace-metadata.json"))
            .expect("read metadata");
        let metadata = serde_json::from_str::<Value>(&metadata).expect("parse metadata");

        assert_eq!(metadata["compact_after_approx_tokens"], 30_000);
        assert_eq!(metadata["max_input_approx_tokens"], 120_000);
        assert_eq!(metadata["context_window_tokens"], 128_000);
        assert_eq!(metadata["profile"], true);
        assert_eq!(metadata["interactive"], true);
        assert_eq!(metadata["session"], "demo-session");
        assert_eq!(metadata["new_session"], true);
        assert_eq!(
            metadata["context"]["profile_scenario"]["name"],
            "compaction-pressure"
        );
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
            schema_version: AGENT_SNAPSHOT_SCHEMA_VERSION,
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

        assert_eq!(decoded.schema_version, AGENT_SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(decoded.input, snapshot.input);
        assert_eq!(decoded.request_seq, 7);
        assert_eq!(decoded.loaded_skills, vec!["demo"]);
        assert_eq!(decoded.profiler.to_json()["requests"], 0);
    }

    #[test]
    fn agent_snapshot_defaults_schema_version_for_existing_sessions() {
        let decoded = serde_json::from_value::<AgentSnapshot>(json!({
            "input": [{
                "role": "user",
                "content": [{"type": "input_text", "text": "hello"}]
            }],
            "request_seq": 1,
            "profiler": AgentProfiler::default(),
            "loaded_skills": []
        }))
        .expect("deserialize old snapshot");

        assert_eq!(decoded.schema_version, AGENT_SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(decoded.request_seq, 1);
    }
}
