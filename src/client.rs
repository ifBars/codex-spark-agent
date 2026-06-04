use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::auth::AuthTokens;
use crate::tools::ToolDescriptor;

const CODEX_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const CODEX_RESPONSES_COMPACT_URL: &str = "https://chatgpt.com/backend-api/codex/responses/compact";

#[derive(Debug, Clone)]
pub struct SparkClient {
    http: reqwest::Client,
    pub auth: AuthTokens,
    model: String,
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
        Self {
            http: reqwest::Client::new(),
            auth,
            model,
        }
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

Use the available native tools when they help. Answer directly when they do not.
When a user names a project, library, or repo ambiguously, first take a small look at the current workspace before assuming they mean a public product or SDK.
When a user gives explicit paths, files, or a narrow workspace scope, start there instead of listing the repository root.
Batch independent tool calls in the same turn when possible, especially reads, searches, and writes for a known set of files.
After required evidence is gathered and validation passes, stop calling tools and provide the final answer.
When validation fails, inspect the first concrete failure, make a targeted code or config change, and do not rerun the same failing command again unless something relevant changed.
When finished, provide the final answer as a normal assistant message."#
}

fn skill_compiler_prompt() -> &'static str {
    r#"You compile Codex agent skills for GPT-5.3-Codex-Spark.

Produce compact operational guidance, not a prose summary. Preserve source-grounded requirements and validation steps. Do not add generic advice."#
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Response, response_from_stream, response_text, spark_system_prompt};

    #[test]
    fn spark_system_prompt_prefers_workspace_peek_for_ambiguous_repo_names() {
        let prompt = spark_system_prompt();

        assert!(prompt.contains("first take a small look at the current workspace"));
        assert!(prompt.contains("before assuming they mean a public product or SDK"));
        assert!(prompt.contains("explicit paths, files, or a narrow workspace scope"));
        assert!(prompt.contains("instead of listing the repository root"));
        assert!(prompt.contains("Batch independent tool calls in the same turn"));
        assert!(prompt.contains("After required evidence is gathered and validation passes"));
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
}
