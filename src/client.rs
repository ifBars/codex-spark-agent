use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::auth::AuthTokens;
use crate::tools::ToolDescriptor;

const CODEX_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const CODEX_RESPONSES_COMPACT_URL: &str = "https://chatgpt.com/backend-api/codex/responses/compact";
pub(crate) const DEFAULT_SPARK_AGENT_REASONING_EFFORT: &str = "medium";

#[derive(Debug, Clone)]
pub struct SparkClient {
    http: reqwest::Client,
    pub auth: AuthTokens,
    model: String,
    reasoning_effort: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: Option<String>,
    #[serde(default)]
    pub output: Vec<ResponseItem>,
    #[serde(flatten)]
    pub extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseItem {
    Message {
        #[serde(default)]
        content: Vec<MessageContent>,
        #[serde(flatten)]
        extra: Value,
    },
    FunctionCall {
        call_id: String,
        name: String,
        #[serde(default)]
        arguments: Value,
        #[serde(flatten)]
        extra: Value,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
    OutputText {
        text: String,
    },
    Text {
        text: String,
    },
    #[serde(other)]
    Other,
}

impl SparkClient {
    pub fn new(auth: AuthTokens, model: String) -> Self {
        Self::new_with_reasoning_effort(auth, model, DEFAULT_SPARK_AGENT_REASONING_EFFORT)
    }

    pub(crate) fn new_with_reasoning_effort(
        auth: AuthTokens,
        model: String,
        reasoning_effort: impl Into<String>,
    ) -> Self {
        Self {
            http: reqwest::Client::new(),
            auth,
            model,
            reasoning_effort: reasoning_effort.into(),
        }
    }

    pub(crate) fn reasoning_effort(&self) -> &str {
        &self.reasoning_effort
    }

    pub(crate) fn set_reasoning_effort(&mut self, reasoning_effort: impl Into<String>) {
        self.reasoning_effort = reasoning_effort.into();
    }

    pub async fn responses_create_with_event_handler(
        &self,
        input: &[Value],
        tools: &[ToolDescriptor],
        on_event: impl FnMut(&Value),
    ) -> Result<(Response, Value)> {
        let body = json!({
            "model": self.model,
            "instructions": spark_system_prompt(),
            "input": input,
            "tools": tools.iter().map(tool_to_wire).collect::<Vec<_>>(),
            "tool_choice": "auto",
            "parallel_tool_calls": true,
            "reasoning": {
                "effort": self.reasoning_effort.as_str(),
            },
            "store": false,
            "stream": true,
        });

        self.send_streaming_body(body, "Spark request", on_event)
            .await
    }

    pub async fn compile_skill_summary(&self, name: &str, raw_skill: &str) -> Result<String> {
        let prompt = format!(
            r#"Compile this Codex-style SKILL.md into a compact Spark-agent skill payload.

Rules:
- Preserve concrete trigger conditions, hard requirements, workflow steps, validation commands, and repo-specific constraints.
- Remove examples that are not operationally necessary.
- Do not invent behavior not present in the source.
- Keep it concise but complete enough for a coding agent to follow without reading the full source.
- Return markdown only.

Skill name: {name}

SKILL.md:
```markdown
{raw_skill}
```"#
        );
        let body = json!({
            "model": self.model,
            "instructions": skill_compiler_prompt(),
            "input": [{
                "role": "user",
                "content": [{"type": "input_text", "text": prompt}]
            }],
            "tools": [],
            "parallel_tool_calls": false,
            "store": false,
            "stream": true,
        });

        let (response, _) = self
            .send_streaming_body(body, "Spark skill compile request", |_| {})
            .await?;
        let text = response_text(&response);
        if text.trim().is_empty() {
            anyhow::bail!("Spark returned an empty skill summary");
        }
        Ok(text)
    }

    pub async fn responses_create_judge(
        &self,
        input: &[Value],
        reasoning_effort: &str,
        on_event: impl FnMut(&Value),
    ) -> Result<(Response, Value)> {
        let body = json!({
            "model": self.model,
            "instructions": judge_system_prompt(),
            "input": input,
            "tools": [],
            "parallel_tool_calls": false,
            "reasoning": {
                "effort": reasoning_effort,
            },
            "store": false,
            "stream": true,
        });

        self.send_streaming_body(body, "Benchmark judge request", on_event)
            .await
    }

    async fn send_streaming_body(
        &self,
        body: Value,
        context: &str,
        mut on_event: impl FnMut(&Value),
    ) -> Result<(Response, Value)> {
        let mut request = self
            .http
            .post(CODEX_RESPONSES_URL)
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", self.auth.access_token),
            )
            .header("originator", "spark")
            .json(&body);

        if let Some(account_id) = &self.auth.account_id {
            request = request.header("ChatGPT-Account-Id", account_id);
        }

        let mut response = request
            .send()
            .await
            .with_context(|| format!("{context} failed"))?;
        let status = response.status();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            let raw = response.text().await.unwrap_or_default();
            anyhow::bail!("ChatGPT auth failed ({status}): {raw}");
        }
        if !status.is_success() {
            let raw = response.text().await.unwrap_or_default();
            anyhow::bail!("Spark request failed ({status}): {raw}");
        }

        let mut buffer = String::new();
        let mut events = Vec::new();
        let mut completed: Option<Value> = None;
        while let Some(chunk) = response
            .chunk()
            .await
            .context("failed to read Spark stream")?
        {
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(newline_idx) = buffer.find('\n') {
                let mut line = buffer[..newline_idx].to_string();
                if line.ends_with('\r') {
                    line.pop();
                }
                buffer.drain(..=newline_idx);
                if let Some(value) = parse_sse_json_line(&line) {
                    on_event(&value);
                    if value.get("type").and_then(Value::as_str) == Some("response.completed") {
                        completed = value.get("response").cloned();
                    }
                    events.push(value);
                }
            }
        }
        if !buffer.trim().is_empty()
            && let Some(value) = parse_sse_json_line(buffer.trim_end_matches('\r'))
        {
            on_event(&value);
            if value.get("type").and_then(Value::as_str) == Some("response.completed") {
                completed = value.get("response").cloned();
            }
            events.push(value);
        }

        let raw = response_from_stream(completed, &events)?;
        let parsed = serde_json::from_value::<Response>(raw.clone())
            .with_context(|| format!("failed to parse Spark response: {raw}"))?;
        Ok((parsed, json!({"response": raw, "events": events})))
    }

    pub async fn responses_compact(
        &self,
        input: &[Value],
        tools: &[ToolDescriptor],
    ) -> Result<(Vec<Value>, Value)> {
        if input.is_empty() {
            return Ok((Vec::new(), json!({ "output": [] })));
        }

        let body = json!({
            "model": self.model,
            "instructions": spark_system_prompt(),
            "input": input,
            "tools": tools.iter().map(tool_to_wire).collect::<Vec<_>>(),
            "parallel_tool_calls": true,
            "reasoning": {
                "effort": self.reasoning_effort.as_str(),
            },
        });

        let mut request = self
            .http
            .post(CODEX_RESPONSES_COMPACT_URL)
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", self.auth.access_token),
            )
            .header("originator", "spark")
            .json(&body);

        if let Some(account_id) = &self.auth.account_id {
            request = request.header("ChatGPT-Account-Id", account_id);
        }

        let response = request
            .send()
            .await
            .context("Spark compact request failed")?;
        let status = response.status();
        let raw_text = response.text().await.unwrap_or_default();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            anyhow::bail!("ChatGPT auth failed during compaction ({status}): {raw_text}");
        }
        if !status.is_success() {
            anyhow::bail!("Spark compact request failed ({status}): {raw_text}");
        }

        let raw: Value = serde_json::from_str(&raw_text)
            .with_context(|| format!("failed to parse compact response: {raw_text}"))?;
        let output = compact_output_items(&raw)?;
        Ok((output, raw))
    }
}

fn compact_output_items(raw: &Value) -> Result<Vec<Value>> {
    if let Some(items) = raw.as_array() {
        return Ok(items.clone());
    }
    if let Some(items) = raw.get("output").and_then(Value::as_array) {
        return Ok(items.clone());
    }
    if let Some(items) = raw
        .get("response")
        .and_then(|response| response.get("output"))
        .and_then(Value::as_array)
    {
        return Ok(items.clone());
    }
    anyhow::bail!("compact response did not include output items: {raw}");
}

pub fn output_text_delta(event: &Value) -> Option<&str> {
    if event.get("type").and_then(Value::as_str) != Some("response.output_text.delta") {
        return None;
    }
    event.get("delta").and_then(Value::as_str)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReasoningDisplayUpdate {
    Started,
    Summary(String),
    Finished,
}

pub fn reasoning_display_update(event: &Value) -> Option<ReasoningDisplayUpdate> {
    let event_type = event.get("type").and_then(Value::as_str)?;
    match event_type {
        "response.output_item.added" => {
            let item = event.get("item")?;
            (item.get("type").and_then(Value::as_str) == Some("reasoning"))
                .then_some(ReasoningDisplayUpdate::Started)
        }
        "response.output_item.done" => {
            let item = event.get("item")?;
            if item.get("type").and_then(Value::as_str) != Some("reasoning") {
                return None;
            }
            reasoning_summary_text(item)
                .map(ReasoningDisplayUpdate::Summary)
                .or(Some(ReasoningDisplayUpdate::Finished))
        }
        "response.reasoning_summary_text.delta" | "response.reasoning_summary.delta" => event
            .get("delta")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(|text| ReasoningDisplayUpdate::Summary(text.to_string())),
        _ => None,
    }
}

fn reasoning_summary_text(item: &Value) -> Option<String> {
    let text = item
        .get("summary")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (!text.trim().is_empty()).then_some(text)
}

fn reconstruct_output_from_events(events: &[Value]) -> Vec<Value> {
    let mut indexed = Vec::new();
    for event in events {
        if event.get("type").and_then(Value::as_str) != Some("response.output_item.done") {
            continue;
        }
        let Some(item) = event.get("item").cloned() else {
            continue;
        };
        let index = event
            .get("output_index")
            .and_then(Value::as_u64)
            .unwrap_or(indexed.len() as u64);
        indexed.push((index, item));
    }
    indexed.sort_by_key(|(index, _)| *index);
    indexed.into_iter().map(|(_, item)| item).collect()
}

fn response_from_stream(completed: Option<Value>, events: &[Value]) -> Result<Value> {
    let output = reconstruct_output_from_events(events);
    if let Some(mut raw) = completed {
        if raw
            .get("output")
            .and_then(Value::as_array)
            .is_none_or(|items| items.is_empty())
            && !output.is_empty()
            && let Some(object) = raw.as_object_mut()
        {
            object.insert("output".to_string(), Value::Array(output));
        }
        return Ok(raw);
    }

    if output.is_empty() {
        anyhow::bail!("Spark stream ended without response.completed");
    }

    let mut raw = events
        .iter()
        .rev()
        .filter_map(|event| event.get("response").cloned())
        .next()
        .unwrap_or_else(|| json!({"object": "response"}));
    if let Some(object) = raw.as_object_mut() {
        object.insert("output".to_string(), Value::Array(output));
        object.insert("status".to_string(), Value::String("completed".to_string()));
        object.insert("incomplete_details".to_string(), Value::Null);
        object.insert("spark_harness_reconstructed".to_string(), Value::Bool(true));
        return Ok(raw);
    }

    Ok(json!({
        "object": "response",
        "status": "completed",
        "output": output,
        "incomplete_details": null,
        "spark_harness_reconstructed": true,
    }))
}

fn parse_sse_json_line(line: &str) -> Option<Value> {
    let trimmed = line.trim();
    let payload = trimmed.strip_prefix("data:")?.trim();
    if payload == "[DONE]" || payload.is_empty() {
        return None;
    }
    serde_json::from_str(payload).ok()
}

pub fn response_text(response: &Response) -> String {
    let mut out = String::new();
    for item in &response.output {
        let ResponseItem::Message { content, .. } = item else {
            continue;
        };
        for part in content {
            match part {
                MessageContent::OutputText { text } | MessageContent::Text { text } => {
                    out.push_str(text)
                }
                MessageContent::Other => {}
            }
        }
    }
    out
}

pub fn function_calls(response: &Response) -> Vec<(String, String, Value)> {
    response
        .output
        .iter()
        .filter_map(|item| {
            let ResponseItem::FunctionCall {
                call_id,
                name,
                arguments,
                ..
            } = item
            else {
                return None;
            };
            let args = function_arguments_to_value(arguments);
            Some((call_id.clone(), wire_tool_name_to_local(name), args))
        })
        .collect()
}

pub fn output_items_for_next_input(raw_response: &Value) -> Vec<Value> {
    raw_response
        .get("response")
        .unwrap_or(raw_response)
        .get("output")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(minimal_carry_forward_item)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn minimal_carry_forward_item(item: &Value) -> Option<Value> {
    match item.get("type").and_then(Value::as_str)? {
        "function_call" => Some(json!({
            "type": "function_call",
            "call_id": item.get("call_id")?.clone(),
            "name": item.get("name")?.clone(),
            "arguments": item.get("arguments").cloned().unwrap_or_else(|| json!("{}")),
        })),
        "web_search_call" => Some(item.clone()),
        "message" => {
            let role = item
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("assistant");
            let text = message_text_from_value(item);
            if text.trim().is_empty() {
                return None;
            }
            Some(json!({
                "role": role,
                "content": [
                    {
                        "type": if role == "assistant" { "output_text" } else { "input_text" },
                        "text": text,
                    }
                ]
            }))
        }
        _ => None,
    }
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

fn tool_to_wire(tool: &ToolDescriptor) -> Value {
    if let Some(hosted_type) = &tool.hosted_type {
        let mut object = tool
            .hosted_config
            .clone()
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        object.insert("type".to_string(), Value::String(hosted_type.clone()));
        return Value::Object(object);
    }
    json!({
        "type": "function",
        "name": local_tool_name_to_wire(&tool.name),
        "description": tool.description,
        "parameters": normalize_schema(&tool.input_schema),
    })
}

fn local_tool_name_to_wire(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn wire_tool_name_to_local(name: &str) -> String {
    match name {
        "fs_read" => "fs.read",
        "fs_list" => "fs.list",
        "fs_stat" => "fs.stat",
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

fn normalize_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let mut out = map.clone();
            if out.get("type").and_then(Value::as_str) == Some("object")
                && !out.contains_key("properties")
            {
                out.insert("properties".to_string(), json!({}));
            }
            if let Some(Value::Object(properties)) = out.get("properties").cloned() {
                let normalized = properties
                    .into_iter()
                    .map(|(key, value)| (key, normalize_schema(&value)))
                    .collect();
                out.insert("properties".to_string(), Value::Object(normalized));
            }
            if let Some(items) = out.get("items").cloned() {
                let normalized = match items {
                    Value::Object(_) => normalize_schema(&items),
                    Value::Array(items) => items
                        .first()
                        .map(normalize_schema)
                        .unwrap_or_else(|| json!({})),
                    Value::String(kind) => json!({ "type": kind }),
                    _ => json!({}),
                };
                out.insert("items".to_string(), normalized);
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

fn function_arguments_to_value(arguments: &Value) -> Value {
    match arguments {
        Value::String(raw) => serde_json::from_str(raw)
            .unwrap_or_else(|error| json!({"_parse_error": error.to_string(), "_raw": raw})),
        Value::Object(_) => arguments.clone(),
        Value::Null => json!({}),
        other => json!({"_raw": other}),
    }
}

fn spark_system_prompt() -> &'static str {
    r#"You are GPT-5.3-Codex-Spark running inside a compact coding agent harness.

Use the available native and hosted tools when they help. Answer directly when tools do not help.

# How you work

Persist until the task is handled end-to-end within the current turn whenever feasible. Do not stop at analysis or partial fixes when the user asked for implementation. Carry changes through focused verification and a clear final answer unless the user explicitly pauses or redirects you.

Start from the user's concrete anchor. When a user gives explicit paths, files, symptoms, benchmark rows, or a narrow workspace scope, inspect those first instead of listing the repository root. When a user names a project, library, or repo ambiguously, first take a small look at the current workspace before assuming they mean a public product or SDK.

Gather enough evidence before writing. Prefer bounded file reads and targeted searches over broad output. Batch independent tool calls in the same turn when possible, especially reads, searches, and writes for a known set of files.

Use hosted web search for current external facts when local files are insufficient, and cite sources when web search informs the answer.

# Quality bar

For nontrivial work, spend a small bounded extra pass before finalizing: confirm the relevant files or facts, check the artifact against the user's stated requirements, and run or name the most focused validation that applies. Do not stop at the first plausible answer when one more targeted read, search, or command would materially improve confidence.

Adapt effort to quality risk:
- Low-risk tasks are exact-path reads, simple recovery probes, narrow answers, or mechanical edits with explicit requirements. Complete the required evidence path, do one direct verification if needed, then stop. Do not add unrelated probes just to look thorough.
- High-risk tasks are scaffolding runnable projects, multi-file edits, migrations, bug fixes, issue triage, reports, essays, architecture surveys, ambiguous repo questions, or any task where missing one requirement can make the final answer wrong. For these, spend extra effort before finalizing: make a compact requirement checklist, inspect the relevant files or data, verify created artifacts by reading or running them, and run focused tests or validation when available.

Prefer correctness and completeness over being maximally fast. It is acceptable to spend a little more time gathering evidence, checking edge cases, or verifying output when that reduces the chance of a wrong or shallow answer. Keep this bounded: avoid broad repo sweeps, repeated equivalent reads, or validation loops that do not follow from new evidence.

When you write files, do not assume the write is correct. For high-risk tasks, re-open or otherwise verify the important outputs and check exact required strings, schemas, command results, or browser-runnable entrypoints before finalizing.

For data reports, compute from source rows instead of intuition. Treat headers correctly, show the formula or ranking rule you used, and when identifying highest risk prefer explicit risk signals such as severity, open status, and age over simple volume counts.

# Repository instructions

Repos may contain AGENTS.md files. These files give project instructions such as coding conventions, structure, and test commands.
- The scope of an AGENTS.md file is the directory tree rooted at the folder containing it.
- For every file you touch, obey any AGENTS.md file whose scope includes that file.
- More deeply nested AGENTS.md files take precedence over parent AGENTS.md files.
- Direct system, developer, and user instructions take precedence over AGENTS.md instructions.
- AGENTS.md content included in the current input is already available; do not reread it unless the task needs a fresh file check or a subdirectory may have additional instructions.

# Editing and validation

Keep edits focused on the requested behavior. Do not revert or overwrite changes you did not make unless the user explicitly asks.

When validation is relevant, run the narrowest meaningful check first, then broader checks if the change has wider blast radius. When validation fails, inspect the first concrete failure, make a targeted code or config change, and do not rerun the same failing command again unless something relevant changed.

After required evidence is gathered and validation passes, stop calling tools and provide the final answer. When finished, provide the final answer as a normal assistant message."#
}

fn judge_system_prompt() -> &'static str {
    "You are an expert software benchmark judge. Score only from supplied evidence, call out uncertainty, and return the requested JSON shape without markdown."
}

fn skill_compiler_prompt() -> &'static str {
    r#"You compile Codex agent skills for GPT-5.3-Codex-Spark.

Produce compact operational guidance, not a prose summary. Preserve source-grounded requirements and validation steps. Do not add generic advice."#
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::auth::AuthTokens;

    use super::{
        DEFAULT_SPARK_AGENT_REASONING_EFFORT, ReasoningDisplayUpdate, Response, SparkClient,
        output_items_for_next_input, reasoning_display_update, response_from_stream, response_text,
        spark_system_prompt, tool_to_wire,
    };

    fn test_auth_tokens() -> AuthTokens {
        AuthTokens {
            id_token: "id".to_string(),
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            expires_at: i64::MAX,
            account_id: None,
        }
    }

    #[test]
    fn spark_client_tracks_default_and_custom_reasoning_effort() {
        let default_client = SparkClient::new(test_auth_tokens(), "model".to_string());
        assert_eq!(
            default_client.reasoning_effort(),
            DEFAULT_SPARK_AGENT_REASONING_EFFORT
        );

        let mut custom_client =
            SparkClient::new_with_reasoning_effort(test_auth_tokens(), "model".to_string(), "high");
        assert_eq!(custom_client.reasoning_effort(), "high");

        custom_client.set_reasoning_effort("low");
        assert_eq!(custom_client.reasoning_effort(), "low");
    }

    #[test]
    fn spark_system_prompt_prefers_workspace_peek_for_ambiguous_repo_names() {
        let prompt = spark_system_prompt();

        assert!(prompt.contains("Persist until the task is handled end-to-end"));
        assert!(prompt.contains("first take a small look at the current workspace"));
        assert!(prompt.contains("before assuming they mean a public product or SDK"));
        assert!(prompt.contains("explicit paths, files, symptoms, benchmark rows"));
        assert!(prompt.contains("instead of listing the repository root"));
        assert!(prompt.contains("Repos may contain AGENTS.md files"));
        assert!(prompt.contains("More deeply nested AGENTS.md files take precedence"));
        assert!(prompt.contains("AGENTS.md content included in the current input"));
        assert!(prompt.contains("Keep edits focused on the requested behavior"));
        assert!(prompt.contains("Adapt effort to quality risk"));
        assert!(prompt.contains("High-risk tasks are scaffolding runnable projects"));
        assert!(prompt.contains("Low-risk tasks are exact-path reads"));
        assert!(prompt.contains("do not assume the write is correct"));
        assert!(prompt.contains("when identifying highest risk prefer explicit risk signals"));
        assert!(prompt.contains("Batch independent tool calls in the same turn"));
        assert!(prompt.contains("After required evidence is gathered and validation passes"));
        assert!(prompt.contains("Use hosted web search for current external facts"));
        assert!(prompt.contains("cite sources when web search informs the answer"));
    }

    #[test]
    fn hosted_web_search_tool_serializes_as_responses_tool() {
        let tool = crate::tools::builtin_tools()
            .into_iter()
            .find(|tool| tool.name == "web.search")
            .expect("web search tool should be advertised");

        let wire = tool_to_wire(&tool);

        assert_eq!(wire["type"], "web_search");
        assert_eq!(wire["search_context_size"], "medium");
        assert!(wire.get("name").is_none());
        assert!(wire.get("parameters").is_none());
    }

    #[test]
    fn web_search_calls_are_carried_forward_for_next_input() {
        let items = output_items_for_next_input(&json!({
            "output": [
                {
                    "type": "web_search_call",
                    "id": "ws_test",
                    "status": "completed",
                    "action": {
                        "type": "search",
                        "queries": ["current rust release"]
                    }
                }
            ]
        }));

        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "web_search_call");
        assert_eq!(items[0]["id"], "ws_test");
    }

    #[test]
    fn reconstructs_response_when_stream_lacks_completed_event() {
        let events = vec![
            json!({
                "type": "response.created",
                "response": {
                    "id": "resp_test",
                    "object": "response",
                    "status": "in_progress",
                    "output": []
                }
            }),
            json!({
                "type": "response.output_item.done",
                "output_index": 0,
                "item": {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {"type": "output_text", "text": "done"}
                    ]
                }
            }),
        ];

        let raw = response_from_stream(None, &events).expect("stream should be reconstructed");
        let parsed =
            serde_json::from_value::<Response>(raw.clone()).expect("response should parse");

        assert_eq!(response_text(&parsed), "done");
        assert_eq!(
            raw.get("spark_harness_reconstructed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn missing_completed_still_errors_without_output_items() {
        let error = response_from_stream(
            None,
            &[json!({
                "type": "response.created",
                "response": {"id": "resp_test", "output": []}
            })],
        )
        .expect_err("empty stream should stay an error");

        assert!(
            error
                .to_string()
                .contains("Spark stream ended without response.completed")
        );
    }

    #[test]
    fn reasoning_display_update_surfaces_only_safe_reasoning_events() {
        assert_eq!(
            reasoning_display_update(&json!({
                "type": "response.output_item.added",
                "item": {"type": "reasoning", "summary": []}
            })),
            Some(ReasoningDisplayUpdate::Started)
        );
        assert_eq!(
            reasoning_display_update(&json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "reasoning",
                    "summary": [{"type": "summary_text", "text": "Checked the likely files."}]
                }
            })),
            Some(ReasoningDisplayUpdate::Summary(
                "Checked the likely files.".to_string()
            ))
        );
        assert_eq!(
            reasoning_display_update(&json!({
                "type": "response.reasoning_text.delta",
                "delta": "raw hidden reasoning"
            })),
            None
        );
    }
}
