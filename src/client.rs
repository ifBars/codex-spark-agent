use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async_with_config};

use crate::auth::AuthTokens;
use crate::tools::ToolDescriptor;

const CODEX_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const CODEX_RESPONSES_WS_URL: &str = "wss://chatgpt.com/backend-api/codex/responses";
const CODEX_RESPONSES_COMPACT_URL: &str = "https://chatgpt.com/backend-api/codex/responses/compact";
const CODEX_MODELS_URL: &str = "https://chatgpt.com/backend-api/codex/models";
const WEBSOCKET_CONNECT_TIMEOUT: Duration = Duration::from_secs(6);
const WEBSOCKET_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
// The ChatGPT Codex WebSocket gates protocol support using this header. Keep it
// aligned with the Responses WebSocket generation implemented below.
const CODEX_WEBSOCKET_PROTOCOL_VERSION: &str = "0.145.0";
pub(crate) const DEFAULT_SPARK_AGENT_REASONING_EFFORT: &str = "medium";

type ResponsesWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone)]
pub struct SparkClient {
    http: reqwest::Client,
    websocket: Arc<Mutex<Option<ResponsesWebSocket>>>,
    websocket_enabled: Arc<AtomicBool>,
    pub auth: AuthTokens,
    model: String,
    reasoning_effort: String,
    system_prompt: Option<String>,
    memory_context: Option<String>,
    structured_output: Option<StructuredOutput>,
}

#[derive(Debug, Clone)]
struct StructuredOutput {
    name: String,
    schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: Option<String>,
    #[serde(default)]
    pub output: Vec<ResponseItem>,
    #[serde(flatten)]
    pub extra: Value,
}

#[derive(Debug)]
pub(crate) struct CompactionResponse {
    pub(crate) output: Vec<Value>,
    pub(crate) raw: Value,
    pub(crate) method: &'static str,
    pub(crate) v2_error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    models: Vec<ModelMetadata>,
}

#[derive(Debug, Deserialize)]
struct ModelMetadata {
    slug: String,
    context_window: Option<usize>,
    max_context_window: Option<usize>,
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
            websocket: Arc::new(Mutex::new(None)),
            websocket_enabled: Arc::new(AtomicBool::new(true)),
            auth,
            model,
            reasoning_effort: reasoning_effort.into(),
            system_prompt: None,
            memory_context: None,
            structured_output: None,
        }
    }

    pub(crate) fn reasoning_effort(&self) -> &str {
        &self.reasoning_effort
    }

    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn clone_with_model_and_reasoning_effort(
        &self,
        model: impl Into<String>,
        reasoning_effort: impl Into<String>,
    ) -> Self {
        Self {
            http: self.http.clone(),
            websocket: Arc::new(Mutex::new(None)),
            websocket_enabled: Arc::new(AtomicBool::new(true)),
            auth: self.auth.clone(),
            model: model.into(),
            reasoning_effort: reasoning_effort.into(),
            system_prompt: None,
            memory_context: self.memory_context.clone(),
            structured_output: None,
        }
    }

    pub(crate) fn set_reasoning_effort(&mut self, reasoning_effort: impl Into<String>) {
        self.reasoning_effort = reasoning_effort.into();
    }

    pub(crate) fn set_system_prompt(&mut self, system_prompt: impl Into<Option<String>>) {
        self.system_prompt = system_prompt.into();
    }

    pub(crate) fn set_memory_context(&mut self, memory_context: impl Into<Option<String>>) {
        self.memory_context = memory_context.into();
    }

    pub(crate) fn set_output_schema(&mut self, name: impl Into<String>, schema: Value) {
        self.structured_output = Some(StructuredOutput {
            name: name.into(),
            schema,
        });
    }

    fn instructions(&self) -> String {
        spark_system_prompt_with_context(
            self.system_prompt.as_deref(),
            self.memory_context.as_deref(),
        )
    }

    pub(crate) fn request_instruction_chars(&self) -> usize {
        self.instructions().len()
    }

    pub(crate) fn request_tool_schema_chars(&self, tools: &[ToolDescriptor]) -> Result<usize> {
        serde_json::to_string(&tools.iter().map(tool_to_wire).collect::<Vec<_>>())
            .map(|encoded| encoded.len())
            .context("failed to encode Responses tool schemas")
    }

    pub(crate) async fn model_context_window_tokens(&self) -> Result<usize> {
        let mut request = self
            .http
            .get(CODEX_MODELS_URL)
            .query(&[("client_version", CODEX_WEBSOCKET_PROTOCOL_VERSION)])
            .header(
                "Authorization",
                format!("Bearer {}", self.auth.access_token),
            )
            .header("originator", "spark");
        if let Some(account_id) = &self.auth.account_id {
            request = request.header("ChatGPT-Account-Id", account_id);
        }

        let response = tokio::time::timeout(Duration::from_secs(5), request.send())
            .await
            .context("model metadata request timed out")?
            .context("model metadata request failed")?;
        let status = response.status();
        let raw_text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("model metadata request failed ({status}): {raw_text}");
        }
        let response: ModelsResponse = serde_json::from_str(&raw_text)
            .with_context(|| format!("failed to parse model metadata response: {raw_text}"))?;
        let model = response
            .models
            .into_iter()
            .find(|candidate| candidate.slug == self.model)
            .with_context(|| format!("model metadata did not include {}", self.model))?;
        model
            .context_window
            .or(model.max_context_window)
            .filter(|tokens| *tokens > 0)
            .with_context(|| format!("model metadata for {} has no context window", self.model))
    }

    pub async fn responses_create_with_event_handler(
        &self,
        input: &[Value],
        tools: &[ToolDescriptor],
        previous_response_id: Option<&str>,
        continuation_input_start: usize,
        on_event: impl FnMut(&Value),
    ) -> Result<(Response, Value)> {
        let body = self.responses_request_body(input, tools);

        self.send_streaming_body_with_continuation(
            body,
            "Spark request",
            previous_response_id,
            continuation_input_start,
            on_event,
        )
        .await
    }

    fn responses_request_body(&self, input: &[Value], tools: &[ToolDescriptor]) -> Value {
        let mut body = json!({
            "model": self.model,
            "instructions": self.instructions(),
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
        if let Some(output) = &self.structured_output {
            body["text"] = json!({
                "format": {
                    "type": "json_schema",
                    "name": output.name,
                    "strict": true,
                    "schema": output.schema,
                }
            });
        }
        body
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
        match self
            .send_streaming_body_with_continuation(body.clone(), context, None, 0, &mut on_event)
            .await
        {
            Ok(result) => Ok(result),
            Err(_) if self.switch_to_http_transport().await => {
                self.send_streaming_body_with_continuation(body, context, None, 0, on_event)
                    .await
            }
            Err(error) => Err(error),
        }
    }

    async fn send_streaming_body_with_continuation(
        &self,
        body: Value,
        context: &str,
        previous_response_id: Option<&str>,
        continuation_input_start: usize,
        mut on_event: impl FnMut(&Value),
    ) -> Result<(Response, Value)> {
        if self.websocket_enabled() {
            return self
                .send_websocket_body(
                    &body,
                    previous_response_id,
                    continuation_input_start,
                    &mut on_event,
                )
                .await
                .map_err(|error| error.error.context(context.to_string()));
        }

        self.send_http_streaming_body(body, context, on_event).await
    }

    pub(crate) fn websocket_enabled(&self) -> bool {
        self.websocket_enabled.load(Ordering::Relaxed)
    }

    pub(crate) async fn switch_to_http_transport(&self) -> bool {
        if !self.websocket_enabled.swap(false, Ordering::Relaxed) {
            return false;
        }
        self.websocket.lock().await.take();
        true
    }

    async fn send_http_streaming_body(
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
        Ok((
            parsed,
            json!({"response": raw, "events": events, "transport": "responses_http"}),
        ))
    }

    async fn send_websocket_body(
        &self,
        body: &Value,
        previous_response_id: Option<&str>,
        continuation_input_start: usize,
        on_event: &mut impl FnMut(&Value),
    ) -> std::result::Result<(Response, Value), WebSocketAttemptError> {
        let mut socket_guard = self.websocket.lock().await;
        if socket_guard.is_none() {
            let socket = self
                .connect_websocket()
                .await
                .map_err(WebSocketAttemptError::before_stream)?;
            *socket_guard = Some(socket);
        }

        let payload = websocket_request_body(body, previous_response_id, continuation_input_start)
            .map_err(WebSocketAttemptError::before_stream)?;
        let socket = socket_guard
            .as_mut()
            .expect("websocket is initialized before streaming");
        let result = run_websocket_response(socket, payload, on_event).await;
        if result.is_err() {
            *socket_guard = None;
        }
        result
    }

    async fn connect_websocket(&self) -> Result<ResponsesWebSocket> {
        let mut request = CODEX_RESPONSES_WS_URL
            .into_client_request()
            .context("failed to build Responses WebSocket request")?;
        let headers = request.headers_mut();
        let mut authorization =
            HeaderValue::from_str(&format!("Bearer {}", self.auth.access_token))?;
        authorization.set_sensitive(true);
        headers.insert("Authorization", authorization);
        headers.insert("originator", HeaderValue::from_static("spark"));
        headers.insert(
            "OpenAI-Beta",
            HeaderValue::from_static("responses_websockets=2026-02-06"),
        );
        headers.insert(
            "User-Agent",
            HeaderValue::from_static(concat!("spark/", env!("CARGO_PKG_VERSION"))),
        );
        headers.insert(
            "version",
            HeaderValue::from_static(CODEX_WEBSOCKET_PROTOCOL_VERSION),
        );
        if let Some(account_id) = &self.auth.account_id {
            let mut account_id = HeaderValue::from_str(account_id)?;
            account_id.set_sensitive(true);
            headers.insert("ChatGPT-Account-Id", account_id);
        }

        let (socket, _) = tokio::time::timeout(
            WEBSOCKET_CONNECT_TIMEOUT,
            connect_async_with_config(request, None, true),
        )
        .await
        .context("Responses WebSocket connection timed out")?
        .context("Responses WebSocket connection failed")?;
        Ok(socket)
    }

    pub async fn responses_compact(
        &self,
        input: &[Value],
        tools: &[ToolDescriptor],
    ) -> Result<CompactionResponse> {
        if input.is_empty() {
            return Ok(CompactionResponse {
                output: Vec::new(),
                raw: json!({ "output": [] }),
                method: "responses_compaction_v2",
                v2_error: None,
            });
        }

        match self.responses_compact_v2(input, tools).await {
            Ok((output, raw)) => {
                return Ok(CompactionResponse {
                    output,
                    raw,
                    method: "responses_compaction_v2",
                    v2_error: None,
                });
            }
            Err(v2_error) => {
                let v2_error = format!("{v2_error:#}");
                let (output, raw) = self.responses_compact_legacy(input, tools).await?;
                return Ok(CompactionResponse {
                    output,
                    raw,
                    method: "responses_compact",
                    v2_error: Some(v2_error),
                });
            }
        }
    }

    async fn responses_compact_v2(
        &self,
        input: &[Value],
        tools: &[ToolDescriptor],
    ) -> Result<(Vec<Value>, Value)> {
        let body = compaction_v2_body(
            &self.model,
            &self.instructions(),
            self.reasoning_effort.as_str(),
            input,
            tools,
        );
        let (_, raw) = self
            .send_streaming_body(body, "Spark compaction v2 request", |_| {})
            .await?;
        let output = compact_output_items(&raw)?;
        if !output.iter().any(is_compaction_item) {
            anyhow::bail!("compaction v2 response did not include a compaction item: {raw}");
        }
        Ok((output, raw))
    }

    async fn responses_compact_legacy(
        &self,
        input: &[Value],
        tools: &[ToolDescriptor],
    ) -> Result<(Vec<Value>, Value)> {
        let body = json!({
            "model": self.model,
            "instructions": self.instructions(),
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

struct WebSocketAttemptError {
    error: anyhow::Error,
}

impl WebSocketAttemptError {
    fn before_stream(error: anyhow::Error) -> Self {
        Self { error }
    }

    fn during_stream(error: anyhow::Error, _emitted_event: bool) -> Self {
        Self { error }
    }
}

fn websocket_request_body(
    body: &Value,
    previous_response_id: Option<&str>,
    continuation_input_start: usize,
) -> Result<String> {
    let mut request = body.clone();
    let object = request
        .as_object_mut()
        .context("Responses request body must be an object")?;
    object.insert("type".to_string(), json!("response.create"));
    if let Some(previous_response_id) = previous_response_id {
        object.insert(
            "previous_response_id".to_string(),
            json!(previous_response_id),
        );
        let input = object
            .get("input")
            .and_then(Value::as_array)
            .context("Responses request input must be an array")?;
        object.insert(
            "input".to_string(),
            Value::Array(input[continuation_input_start.min(input.len())..].to_vec()),
        );
    }
    serde_json::to_string(&request).context("failed to encode Responses WebSocket request")
}

async fn run_websocket_response(
    socket: &mut ResponsesWebSocket,
    payload: String,
    on_event: &mut impl FnMut(&Value),
) -> std::result::Result<(Response, Value), WebSocketAttemptError> {
    tokio::time::timeout(
        WEBSOCKET_IDLE_TIMEOUT,
        socket.send(Message::Text(payload.into())),
    )
    .await
    .map_err(|_| {
        WebSocketAttemptError::before_stream(anyhow::anyhow!(
            "timed out sending Responses WebSocket request"
        ))
    })?
    .map_err(|error| {
        WebSocketAttemptError::before_stream(anyhow::anyhow!(
            "failed to send Responses WebSocket request: {error}"
        ))
    })?;

    let mut events = Vec::new();
    let mut completed = None;
    loop {
        let message = tokio::time::timeout(WEBSOCKET_IDLE_TIMEOUT, socket.next())
            .await
            .map_err(|_| {
                WebSocketAttemptError::during_stream(
                    anyhow::anyhow!("idle timeout waiting for Responses WebSocket"),
                    !events.is_empty(),
                )
            })?
            .ok_or_else(|| {
                WebSocketAttemptError::during_stream(
                    anyhow::anyhow!("Responses WebSocket closed before response.completed"),
                    !events.is_empty(),
                )
            })?
            .map_err(|error| {
                WebSocketAttemptError::during_stream(
                    anyhow::anyhow!("failed to read Responses WebSocket: {error}"),
                    !events.is_empty(),
                )
            })?;

        match message {
            Message::Text(text) => {
                let value: Value = serde_json::from_str(text.as_str()).map_err(|error| {
                    WebSocketAttemptError::during_stream(
                        anyhow::anyhow!("invalid Responses WebSocket event: {error}"),
                        !events.is_empty(),
                    )
                })?;
                if value.get("type").and_then(Value::as_str) == Some("error") {
                    return Err(WebSocketAttemptError::during_stream(
                        anyhow::anyhow!("Responses WebSocket returned an error: {value}"),
                        !events.is_empty(),
                    ));
                }
                on_event(&value);
                if value.get("type").and_then(Value::as_str) == Some("response.completed") {
                    completed = value.get("response").cloned();
                }
                events.push(value);
                if completed.is_some() {
                    break;
                }
            }
            Message::Ping(payload) => {
                socket.send(Message::Pong(payload)).await.map_err(|error| {
                    WebSocketAttemptError::during_stream(
                        anyhow::anyhow!("failed to answer Responses WebSocket ping: {error}"),
                        !events.is_empty(),
                    )
                })?;
            }
            Message::Pong(_) => {}
            Message::Close(frame) => {
                return Err(WebSocketAttemptError::during_stream(
                    anyhow::anyhow!(
                        "Responses WebSocket closed before response.completed: {frame:?}"
                    ),
                    !events.is_empty(),
                ));
            }
            Message::Binary(_) | Message::Frame(_) => {}
        }
    }

    let raw = response_from_stream(completed, &events)
        .map_err(|error| WebSocketAttemptError::during_stream(error, !events.is_empty()))?;
    let parsed = serde_json::from_value::<Response>(raw.clone()).map_err(|error| {
        WebSocketAttemptError::during_stream(
            anyhow::anyhow!("failed to parse Spark response: {error}; response={raw}"),
            !events.is_empty(),
        )
    })?;
    Ok((
        parsed,
        json!({"response": raw, "events": events, "transport": "responses_websocket"}),
    ))
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

fn is_compaction_item(item: &Value) -> bool {
    matches!(
        item.get("type").and_then(Value::as_str),
        Some("compaction" | "context_compaction")
    )
}

fn compaction_v2_body(
    model: &str,
    instructions: &str,
    reasoning_effort: &str,
    input: &[Value],
    tools: &[ToolDescriptor],
) -> Value {
    let mut compact_input = input.to_vec();
    compact_input.push(json!({ "type": "compaction_trigger" }));
    json!({
        "model": model,
        "instructions": instructions,
        "input": compact_input,
        "tools": tools.iter().map(tool_to_wire).collect::<Vec<_>>(),
        "tool_choice": "auto",
        "parallel_tool_calls": true,
        "reasoning": { "effort": reasoning_effort },
        "store": false,
        "stream": true,
    })
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSearchDisplayUpdate {
    Started {
        id: String,
        query: Option<String>,
    },
    Query {
        id: String,
        query: String,
    },
    Finished {
        id: String,
        query: Option<String>,
        ok: bool,
    },
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

pub fn web_search_display_update(event: &Value) -> Option<WebSearchDisplayUpdate> {
    let event_type = event.get("type").and_then(Value::as_str)?;
    if event_type.starts_with("response.web_search_call.") {
        let id = event
            .get("item_id")
            .and_then(Value::as_str)
            .or_else(|| event.get("id").and_then(Value::as_str))
            .unwrap_or("web_search")
            .to_string();
        return web_search_query(event).map(|query| WebSearchDisplayUpdate::Query { id, query });
    }
    let item = event.get("item")?;
    if item.get("type").and_then(Value::as_str) != Some("web_search_call") {
        return None;
    }
    let id = item
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| event.get("item_id").and_then(Value::as_str))
        .unwrap_or("web_search")
        .to_string();
    let query = web_search_query(item);
    match event_type {
        "response.output_item.added" => Some(WebSearchDisplayUpdate::Started { id, query }),
        "response.output_item.done" => {
            let ok = item
                .get("status")
                .and_then(Value::as_str)
                .is_none_or(|status| status == "completed");
            Some(WebSearchDisplayUpdate::Finished { id, query, ok })
        }
        _ => None,
    }
}

fn web_search_query(item: &Value) -> Option<String> {
    query_from_value(item).filter(|query| !query.trim().is_empty())
}

fn query_from_value(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            for key in ["query", "search_query", "search_terms", "q"] {
                if let Some(query) = object
                    .get(key)
                    .and_then(Value::as_str)
                    .filter(|query| !query.trim().is_empty())
                {
                    return Some(query.to_string());
                }
            }
            if let Some(query) = object
                .get("queries")
                .and_then(Value::as_array)
                .and_then(|queries| queries.first())
                .and_then(Value::as_str)
                .filter(|query| !query.trim().is_empty())
            {
                return Some(query.to_string());
            }
            object.values().find_map(query_from_value)
        }
        Value::Array(values) => values.iter().find_map(query_from_value),
        _ => None,
    }
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
        "gh_read" => "gh.read",
        "tool_search" => "tool.search",
        "browser_run" => "browser.run",
        "subagent_run" => "subagent.run",
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

Start from the user's concrete anchor. When a user gives explicit paths, files, symptoms, benchmark rows, or a narrow workspace scope, inspect those first instead of listing the repository root. If exact files are named, read those files directly before any root listing or discovery command; only discover around them when an exact path fails or the instruction is ambiguous. When a user names a project, library, or repo ambiguously, first take a small look at the current workspace before assuming they mean a public product or SDK.

Gather enough evidence before writing. Prefer bounded file reads and targeted searches over broad output. Batch independent tool calls in the same turn when possible, especially reads, searches, and writes for a known set of files.

Core workspace tools are loaded by default. Specialist web, GitHub, browser, subagent, and MCP capabilities are deferred to keep each request focused. Use `tool.search` only when the current task genuinely needs one of those capabilities; do not search for tools when the supplied local evidence is sufficient.

For example-driven or demonstration-only tasks, derive one compact rule table before editing: account separately for every output field, check the candidate rule against every supplied example, and reject ad hoc constants or case-specific guesses. Prefer one coherent implementation over piecemeal trial-and-error. When the prompt says the supplied local evidence is the complete specification, do not search the web or unrelated sources for an answer.

Use hosted web search for current external facts when local files are insufficient, and cite sources when web search informs the answer.

Use `subagent.run` for one bounded helper, or `subagent.spawn` plus `subagent.wait` for independent concurrent work. Keep each task concrete and merge the compact briefs yourself. Default workers are read-only; only use `mode=work` with explicit, non-overlapping relative `ownership` paths when a delegated patch is genuinely needed. Explore inherits Spark; research, review, and plan use gpt-5.6-luna unless `SPARK_ADVANCED_SUBAGENT_MODEL` or `model` overrides it. Use `subagent.list`, `subagent.followup`, `subagent.steer`, and `subagent.cancel` to manage lifecycle. Do not use workers for trivial single-file reads or to avoid making the final decision yourself.

# Quality bar

For nontrivial work, spend a small bounded extra pass before finalizing: confirm the relevant files or facts, check the artifact against the user's stated requirements, and run or name the most focused validation that applies. Do not stop at the first plausible answer when one more targeted read, search, or command would materially improve confidence.

Adapt effort to quality risk:
- Low-risk tasks are exact-path reads, simple recovery probes, narrow answers, or mechanical edits with explicit requirements. Complete the required evidence path, do one direct verification if needed, then stop. Do not add unrelated probes just to look thorough.
- High-risk tasks are scaffolding runnable projects, multi-file edits, migrations, bug fixes, issue triage, reports, essays, architecture surveys, ambiguous repo questions, or any task where missing one requirement can make the final answer wrong. For these, spend extra effort before finalizing: make a compact requirement checklist, inspect the relevant files or data, verify created artifacts by reading or running them, and run focused tests or validation when available.

Prefer correctness and completeness over being maximally fast. It is acceptable to spend a little more time gathering evidence, checking edge cases, or verifying output when that reduces the chance of a wrong or shallow answer. Keep this bounded: avoid broad repo sweeps, repeated equivalent reads, or validation loops that do not follow from new evidence.

When a touched seam already exposes typed aliases, direct methods, or native properties for the requested behavior, use that established shape. Do not add speculative reflection, catch-and-ignore paths, or compatibility indirection merely to avoid resolving a local type or member. Use reflection only when the local seam already establishes it as the required compatibility pattern, or when a direct typed attempt has produced concrete evidence that the type surface differs.

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

fn spark_system_prompt_with_context(
    custom_system_prompt: Option<&str>,
    memory_context: Option<&str>,
) -> String {
    let base = spark_system_prompt();
    let memory_context = memory_context
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let custom_system_prompt = custom_system_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if memory_context.is_none() && custom_system_prompt.is_none() {
        return base.to_string();
    }

    let mut prompt = base.to_string();
    if let Some(memory_context) = memory_context {
        prompt.push_str("\n\n# Spark memory\n\n");
        prompt.push_str(memory_context);
    }
    if let Some(custom_system_prompt) = custom_system_prompt {
        prompt.push_str("\n\n# Harness instructions\n\n");
        prompt.push_str(custom_system_prompt);
    }
    prompt
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
        WebSearchDisplayUpdate, compaction_v2_body, is_compaction_item,
        output_items_for_next_input, reasoning_display_update, response_from_stream, response_text,
        spark_system_prompt, spark_system_prompt_with_context, tool_to_wire,
        web_search_display_update, websocket_request_body,
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
    fn compaction_v2_appends_trigger_to_normal_responses_input() {
        let body = compaction_v2_body(
            "gpt-test",
            "instructions",
            "medium",
            &[json!({"role": "user", "content": "hello"})],
            &[],
        );
        let input = body["input"].as_array().expect("input array");

        assert_eq!(input.len(), 2);
        assert_eq!(input[1]["type"], "compaction_trigger");
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["reasoning"]["effort"], "medium");
        assert!(is_compaction_item(&json!({"type": "compaction"})));
    }

    #[test]
    fn websocket_first_request_preserves_full_responses_body() {
        let body = json!({
            "model": "gpt-test",
            "input": [{"role": "user", "content": "hello"}],
            "tools": [],
            "stream": true
        });

        let payload = websocket_request_body(&body, None, 0).expect("websocket payload");
        let payload: serde_json::Value = serde_json::from_str(&payload).expect("json payload");

        assert_eq!(payload["type"], "response.create");
        assert_eq!(payload["input"], body["input"]);
        assert!(payload.get("previous_response_id").is_none());
    }

    #[test]
    fn websocket_continuation_sends_only_new_items_and_previous_response_id() {
        let body = json!({
            "model": "gpt-test",
            "input": [
                {"role": "user", "content": "hello"},
                {"type": "function_call", "call_id": "call_1"},
                {"type": "function_call_output", "call_id": "call_1", "output": "done"}
            ],
            "tools": [],
            "stream": true
        });

        let payload = websocket_request_body(&body, Some("resp_1"), 2)
            .expect("websocket continuation payload");
        let payload: serde_json::Value = serde_json::from_str(&payload).expect("json payload");

        assert_eq!(payload["type"], "response.create");
        assert_eq!(payload["previous_response_id"], "resp_1");
        assert_eq!(payload["input"].as_array().map(Vec::len), Some(1));
        assert_eq!(payload["input"][0]["type"], "function_call_output");
    }

    #[test]
    fn spark_system_prompt_prefers_workspace_peek_for_ambiguous_repo_names() {
        let prompt = spark_system_prompt();

        assert!(prompt.contains("Persist until the task is handled end-to-end"));
        assert!(prompt.contains("first take a small look at the current workspace"));
        assert!(prompt.contains("before assuming they mean a public product or SDK"));
        assert!(prompt.contains("explicit paths, files, symptoms, benchmark rows"));
        assert!(prompt.contains("instead of listing the repository root"));
        assert!(prompt.contains("Do not add speculative reflection"));
        assert!(prompt.contains("concrete evidence that the type surface differs"));
        assert!(prompt.contains("If exact files are named, read those files directly"));
        assert!(prompt.contains("only discover around them when an exact path fails"));
        assert!(prompt.contains("Repos may contain AGENTS.md files"));
        assert!(prompt.contains("More deeply nested AGENTS.md files take precedence"));
        assert!(prompt.contains("AGENTS.md content included in the current input"));
        assert!(prompt.contains("Keep edits focused on the requested behavior"));
        assert!(prompt.contains("Adapt effort to quality risk"));
        assert!(prompt.contains("derive one compact rule table before editing"));
        assert!(prompt.contains("do not search the web or unrelated sources"));
        assert!(prompt.contains("High-risk tasks are scaffolding runnable projects"));
        assert!(prompt.contains("Low-risk tasks are exact-path reads"));
        assert!(prompt.contains("do not assume the write is correct"));
        assert!(prompt.contains("when identifying highest risk prefer explicit risk signals"));
        assert!(prompt.contains("Batch independent tool calls in the same turn"));
        assert!(prompt.contains("After required evidence is gathered and validation passes"));
        assert!(prompt.contains("Use hosted web search for current external facts"));
        assert!(prompt.contains("cite sources when web search informs the answer"));
        assert!(prompt.contains("subagent.run"));
    }

    #[test]
    fn spark_system_prompt_appends_custom_harness_instructions() {
        let prompt = spark_system_prompt_with_context(Some("You are Relay in Discord."), None);

        assert!(prompt.contains("You are GPT-5.3-Codex-Spark"));
        assert!(prompt.contains("# Harness instructions"));
        assert!(prompt.contains("You are Relay in Discord."));
    }

    #[test]
    fn spark_system_prompt_can_include_memory_context_before_custom_instructions() {
        let prompt = spark_system_prompt_with_context(
            Some("Use terse final answers."),
            Some("Prefer bun for JavaScript tooling."),
        );

        let memory_index = prompt.find("# Spark memory").expect("memory section");
        let custom_index = prompt
            .find("# Harness instructions")
            .expect("custom section");
        assert!(memory_index < custom_index);
        assert!(prompt.contains("Prefer bun for JavaScript tooling."));
        assert!(prompt.contains("Use terse final answers."));
    }

    #[test]
    fn structured_output_is_sent_as_strict_responses_text_format() {
        let mut client = SparkClient::new(test_auth_tokens(), "gpt-5.3-codex-spark".to_string());
        client.set_output_schema(
            "diffuin_artifact",
            json!({
                "type": "object",
                "properties": {"summary": {"type": "string"}},
                "required": ["summary"],
                "additionalProperties": false
            }),
        );

        let body = client.responses_request_body(&[], &[]);

        assert_eq!(body["text"]["format"]["type"], "json_schema");
        assert_eq!(body["text"]["format"]["name"], "diffuin_artifact");
        assert_eq!(body["text"]["format"]["strict"], true);
        assert_eq!(
            body["text"]["format"]["schema"]["required"],
            json!(["summary"])
        );
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
    fn subagent_wire_tool_name_maps_back_to_local_name() {
        let response = Response {
            id: Some("resp_test".to_string()),
            output: vec![super::ResponseItem::FunctionCall {
                call_id: "call_subagent".to_string(),
                name: "subagent_run".to_string(),
                arguments: json!({"kind": "review", "task": "Check the patch"}),
                extra: json!({}),
            }],
            extra: json!({}),
        };

        let calls = super::function_calls(&response);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "subagent.run");
        assert_eq!(calls[0].2["kind"], "review");
    }

    #[test]
    fn browser_wire_tool_name_maps_back_to_local_name() {
        let response = Response {
            id: Some("resp_test".to_string()),
            output: vec![super::ResponseItem::FunctionCall {
                call_id: "call_browser".to_string(),
                name: "browser_run".to_string(),
                arguments: json!({"url": "https://example.com"}),
                extra: json!({}),
            }],
            extra: json!({}),
        };

        let calls = super::function_calls(&response);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "browser.run");
        assert_eq!(calls[0].2["url"], "https://example.com");
    }

    #[test]
    fn github_wire_tool_name_maps_back_to_local_name() {
        let response = Response {
            id: Some("resp_test".to_string()),
            output: vec![super::ResponseItem::FunctionCall {
                call_id: "call_github".to_string(),
                name: "gh_read".to_string(),
                arguments: json!({"args": ["pr", "view", "23"]}),
                extra: json!({}),
            }],
            extra: json!({}),
        };

        let calls = super::function_calls(&response);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "gh.read");
        assert_eq!(calls[0].2["args"][0], "pr");
    }

    #[test]
    fn deferred_tool_search_wire_name_maps_back_to_local_name() {
        let response = Response {
            id: Some("resp_test".to_string()),
            output: vec![super::ResponseItem::FunctionCall {
                call_id: "call_tool_search".to_string(),
                name: "tool_search".to_string(),
                arguments: json!({"query": "public web search"}),
                extra: json!({}),
            }],
            extra: json!({}),
        };

        let calls = super::function_calls(&response);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "tool.search");
    }

    #[test]
    fn web_search_display_update_surfaces_hosted_search_lifecycle() {
        assert_eq!(
            web_search_display_update(&json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "web_search_call",
                    "id": "ws_test",
                    "action": {
                        "type": "search",
                        "queries": ["current rust release"]
                    }
                }
            })),
            Some(WebSearchDisplayUpdate::Started {
                id: "ws_test".to_string(),
                query: Some("current rust release".to_string()),
            })
        );

        assert_eq!(
            web_search_display_update(&json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "web_search_call",
                    "id": "ws_test",
                    "status": "completed",
                    "action": {
                        "type": "search",
                        "queries": ["current rust release"]
                    }
                }
            })),
            Some(WebSearchDisplayUpdate::Finished {
                id: "ws_test".to_string(),
                query: Some("current rust release".to_string()),
                ok: true,
            })
        );
    }

    #[test]
    fn web_search_display_update_reads_query_from_stream_event() {
        assert_eq!(
            web_search_display_update(&json!({
                "type": "response.web_search_call.searching",
                "item_id": "ws_test",
                "query": "Obsession 2025 film plot"
            })),
            Some(WebSearchDisplayUpdate::Query {
                id: "ws_test".to_string(),
                query: "Obsession 2025 film plot".to_string(),
            })
        );

        assert_eq!(
            web_search_display_update(&json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "web_search_call",
                    "id": "ws_test",
                    "status": "completed"
                }
            })),
            Some(WebSearchDisplayUpdate::Finished {
                id: "ws_test".to_string(),
                query: None,
                ok: true,
            })
        );
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
