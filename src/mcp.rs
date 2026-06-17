use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::config;
use crate::tools::{ToolDescriptor, ToolResult};

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const MCP_TIMEOUT: Duration = Duration::from_secs(30);
const MCP_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);
const MCP_TOOL_PREFIX: &str = "mcp__";

#[derive(Debug, Clone, Default)]
pub(crate) struct McpRegistry {
    bindings: HashMap<String, McpToolBinding>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct McpToolBinding {
    server_name: String,
    tool_name: String,
    description: Option<String>,
    input_schema: Value,
    config: McpServerConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpJsonRoot {
    #[serde(default)]
    mcp_servers: HashMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct McpTomlRoot {
    #[serde(default)]
    mcp_servers: HashMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct McpServerConfig {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    cwd: Option<PathBuf>,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    http_headers: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ToolsListResult {
    #[serde(default)]
    tools: Vec<McpTool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpTool {
    name: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    input_schema: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Debug)]
struct McpHttpResponse {
    body: Value,
    session_id: Option<String>,
}

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

impl McpRegistry {
    pub(crate) async fn discover(cwd: &Path) -> Self {
        let mut registry = Self::default();
        if mcp_disabled_by_env() {
            return registry;
        }
        for (server_name, config) in load_mcp_servers(cwd) {
            if config.enabled == Some(false) {
                continue;
            }
            match tokio::time::timeout(
                MCP_DISCOVERY_TIMEOUT,
                registry.discover_server(&server_name, config),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    registry
                        .warnings
                        .push(format!("MCP server `{server_name}` unavailable: {error}"));
                }
                Err(error) => {
                    registry
                        .warnings
                        .push(format!("MCP server `{server_name}` unavailable: {error}"));
                }
            }
        }
        registry
    }

    pub(crate) fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub(crate) fn tools(&self) -> Vec<ToolDescriptor> {
        self.bindings
            .iter()
            .map(|(local_name, binding)| ToolDescriptor {
                name: local_name.clone(),
                description: format!(
                    "MCP tool `{}` from server `{}`. {}",
                    binding.tool_name,
                    binding.server_name,
                    binding_description(local_name, binding)
                ),
                input_schema: binding_schema(binding),
                hosted_type: None,
                hosted_config: None,
            })
            .collect()
    }

    pub(crate) fn is_mcp_tool(tool_name: &str) -> bool {
        tool_name.starts_with(MCP_TOOL_PREFIX)
    }

    pub(crate) async fn invoke(&self, tool_name: &str, args: Value) -> ToolResult {
        let Some(binding) = self.bindings.get(tool_name) else {
            return ToolResult {
                ok: false,
                data: json!({
                    "tool": tool_name,
                    "error_kind": "unknown_mcp_tool",
                    "hint": "Use one of the advertised MCP tool names exactly."
                }),
                error: Some(format!("unknown MCP tool: {tool_name}")),
            };
        };

        match call_tool(binding, args).await {
            Ok(result) => ToolResult {
                ok: !result
                    .get("isError")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                data: json!({
                    "server": binding.server_name,
                    "tool": binding.tool_name,
                    "result": result,
                }),
                error: result
                    .get("isError")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    .then(|| "MCP tool returned isError=true".to_string()),
            },
            Err(error) => ToolResult {
                ok: false,
                data: json!({
                    "server": binding.server_name,
                    "tool": binding.tool_name,
                    "error": error.to_string(),
                }),
                error: Some(error.to_string()),
            },
        }
    }

    async fn discover_server(&mut self, server_name: &str, config: McpServerConfig) -> Result<()> {
        let tools = list_tools(&config).await?;
        for tool in tools {
            let local_name = local_mcp_tool_name(server_name, &tool.name);
            self.bindings.insert(
                local_name,
                McpToolBinding {
                    server_name: server_name.to_string(),
                    tool_name: tool.name,
                    description: tool.description.or(tool.title),
                    input_schema: tool.input_schema.unwrap_or_else(default_tool_schema),
                    config: McpServerConfig {
                        enabled: config.enabled,
                        command: config.command.clone(),
                        args: config.args.clone(),
                        cwd: config.cwd.clone(),
                        env: config.env.clone(),
                        url: config.url.clone(),
                        http_headers: config.http_headers.clone(),
                    },
                },
            );
        }
        Ok(())
    }
}

fn mcp_disabled_by_env() -> bool {
    std::env::var("SPARK_DISABLE_MCP")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn binding_description(local_name: &str, binding: &McpToolBinding) -> String {
    binding.description.clone().unwrap_or_default()
        + &format!(" Call with function name `{local_name}`.")
        + binding
            .config
            .url
            .as_ref()
            .map(|_| " Transport: HTTP.")
            .unwrap_or(" Transport: stdio.")
}

fn binding_schema(binding: &McpToolBinding) -> Value {
    binding.input_schema.clone()
}

fn default_tool_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": true
    })
}

fn load_mcp_servers(cwd: &Path) -> HashMap<String, McpServerConfig> {
    let mut servers = HashMap::new();
    if let Ok(app_dir) = config::app_dir() {
        merge_toml_servers(&mut servers, &app_dir.join("config.toml"));
    }
    if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
        merge_toml_servers(&mut servers, &home.join(".codex").join("config.toml"));
    }
    merge_json_servers(&mut servers, &cwd.join(".mcp.json"));
    merge_json_servers(&mut servers, &cwd.join(".spark").join("mcp.json"));
    servers
}

fn merge_toml_servers(target: &mut HashMap<String, McpServerConfig>, path: &Path) {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(root) = toml::from_str::<McpTomlRoot>(&raw) else {
        return;
    };
    target.extend(root.mcp_servers);
}

fn merge_json_servers(target: &mut HashMap<String, McpServerConfig>, path: &Path) {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(root) = serde_json::from_str::<McpJsonRoot>(&raw) else {
        return;
    };
    target.extend(root.mcp_servers);
}

async fn list_tools(config: &McpServerConfig) -> Result<Vec<McpTool>> {
    if config.url.is_none() {
        let result = stdio_initialized_request(config, "tools/list", json!({})).await?;
        let parsed: ToolsListResult =
            serde_json::from_value(result).context("failed to parse MCP tools/list response")?;
        return Ok(parsed.tools);
    }
    let session = initialize(config).await?;
    initialized(config, session.as_deref()).await?;
    let result = mcp_request(config, session.as_deref(), "tools/list", json!({})).await?;
    let parsed: ToolsListResult =
        serde_json::from_value(result).context("failed to parse MCP tools/list response")?;
    Ok(parsed.tools)
}

async fn call_tool(binding: &McpToolBinding, args: Value) -> Result<Value> {
    if binding.config.url.is_none() {
        return stdio_initialized_request(
            &binding.config,
            "tools/call",
            json!({
                "name": binding.tool_name,
                "arguments": args,
            }),
        )
        .await;
    }
    let session = initialize(&binding.config).await?;
    initialized(&binding.config, session.as_deref()).await?;
    mcp_request(
        &binding.config,
        session.as_deref(),
        "tools/call",
        json!({
            "name": binding.tool_name,
            "arguments": args,
        }),
    )
    .await
}

async fn initialize(config: &McpServerConfig) -> Result<Option<String>> {
    let response = mcp_request_raw(
        config,
        None,
        "initialize",
        json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": "codex-spark-agent",
                "version": env!("CARGO_PKG_VERSION"),
            }
        }),
    )
    .await?;
    parse_json_rpc_result(&response.body)?;
    Ok(response.session_id)
}

async fn initialized(config: &McpServerConfig, session_id: Option<&str>) -> Result<()> {
    mcp_notification(config, session_id, "notifications/initialized").await
}

async fn mcp_request(
    config: &McpServerConfig,
    session_id: Option<&str>,
    method: &str,
    params: Value,
) -> Result<Value> {
    let response = mcp_request_raw(config, session_id, method, params).await?;
    parse_json_rpc_result(&response.body)
}

async fn mcp_request_raw(
    config: &McpServerConfig,
    session_id: Option<&str>,
    method: &str,
    params: Value,
) -> Result<McpHttpResponse> {
    let id = REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    let body = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    if config.url.is_some() {
        http_json_rpc(config, session_id, body).await
    } else {
        stdio_json_rpc(config, Some(id), body).await
    }
}

async fn mcp_notification(
    config: &McpServerConfig,
    session_id: Option<&str>,
    method: &str,
) -> Result<()> {
    let body = json!({
        "jsonrpc": "2.0",
        "method": method,
    });
    if config.url.is_some() {
        let _ = http_json_rpc(config, session_id, body).await?;
    } else {
        let _ = stdio_json_rpc(config, None, body).await?;
    }
    Ok(())
}

async fn stdio_initialized_request(
    config: &McpServerConfig,
    method: &str,
    params: Value,
) -> Result<Value> {
    let initialize_id = REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    let request_id = REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    let initialize_body = json!({
        "jsonrpc": "2.0",
        "id": initialize_id,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": "codex-spark-agent",
                "version": env!("CARGO_PKG_VERSION"),
            }
        },
    });
    let initialized_body = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
    });
    let request_body = json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": method,
        "params": params,
    });

    let command = config
        .command
        .as_deref()
        .context("MCP stdio server missing command")?;
    let mut child = Command::new(command);
    child.args(&config.args);
    if let Some(cwd) = &config.cwd {
        child.current_dir(cwd);
    }
    child.envs(&config.env);
    child
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = child
        .spawn()
        .with_context(|| format!("failed to start MCP server `{command}`"))?;
    let mut stdin = child.stdin.take().context("failed to open MCP stdin")?;
    let stdout = child.stdout.take().context("failed to open MCP stdout")?;
    let mut lines = BufReader::new(stdout).lines();

    write_stdio_message(&mut stdin, &initialize_body).await?;
    let initialize_response = read_stdio_response(&mut lines, initialize_id).await?;
    parse_json_rpc_result(&initialize_response)?;
    write_stdio_message(&mut stdin, &initialized_body).await?;
    write_stdio_message(&mut stdin, &request_body).await?;
    let response = read_stdio_response(&mut lines, request_id).await?;
    drop(stdin);
    let _ = child.start_kill();
    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
    parse_json_rpc_result(&response)
}

async fn write_stdio_message(stdin: &mut tokio::process::ChildStdin, body: &Value) -> Result<()> {
    stdin
        .write_all(format!("{body}\n").as_bytes())
        .await
        .context("failed to write MCP request")?;
    stdin.flush().await.context("failed to flush MCP request")
}

async fn read_stdio_response(
    lines: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    response_id: u64,
) -> Result<Value> {
    loop {
        let line = tokio::time::timeout(MCP_TIMEOUT, lines.next_line())
            .await
            .context("MCP stdio response timed out")?
            .context("failed to read MCP stdio response")?;
        let Some(line) = line else {
            anyhow::bail!("MCP stdio server exited before response {response_id}");
        };
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line)
            .with_context(|| format!("failed to parse MCP stdio JSON: {line}"))?;
        if value.get("id").and_then(Value::as_u64) == Some(response_id) {
            return Ok(value);
        }
    }
}

async fn http_json_rpc(
    config: &McpServerConfig,
    session_id: Option<&str>,
    body: Value,
) -> Result<McpHttpResponse> {
    let url = config
        .url
        .as_deref()
        .context("MCP HTTP server missing url")?;
    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/event-stream"),
    );
    if let Some(session_id) = session_id {
        headers.insert("mcp-session-id", HeaderValue::from_str(session_id)?);
    }
    for (name, value) in &config.http_headers {
        headers.insert(
            HeaderName::from_bytes(name.as_bytes())
                .with_context(|| format!("invalid MCP HTTP header name `{name}`"))?,
            HeaderValue::from_str(value)
                .with_context(|| format!("invalid MCP HTTP header value for `{name}`"))?,
        );
    }
    let response = tokio::time::timeout(
        MCP_TIMEOUT,
        client.post(url).headers(headers).json(&body).send(),
    )
    .await
    .context("MCP HTTP request timed out")?
    .context("MCP HTTP request failed")?;
    let session_id = response
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!("MCP HTTP request failed ({status}): {text}");
    }
    let body = if content_type.contains("text/event-stream") {
        parse_sse_json(&text)?
    } else if text.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&text)
            .with_context(|| format!("failed to parse MCP HTTP JSON: {text}"))?
    };
    Ok(McpHttpResponse { body, session_id })
}

async fn stdio_json_rpc(
    config: &McpServerConfig,
    response_id: Option<u64>,
    body: Value,
) -> Result<McpHttpResponse> {
    let command = config
        .command
        .as_deref()
        .context("MCP stdio server missing command")?;
    let mut child = Command::new(command);
    child.args(&config.args);
    if let Some(cwd) = &config.cwd {
        child.current_dir(cwd);
    }
    child.envs(&config.env);
    child
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = child
        .spawn()
        .with_context(|| format!("failed to start MCP server `{command}`"))?;
    let mut stdin = child.stdin.take().context("failed to open MCP stdin")?;
    let stdout = child.stdout.take().context("failed to open MCP stdout")?;
    let stderr = child.stderr.take();
    let mut lines = BufReader::new(stdout).lines();

    stdin
        .write_all(format!("{body}\n").as_bytes())
        .await
        .context("failed to write MCP request")?;
    stdin.flush().await.context("failed to flush MCP request")?;

    let mut response = json!({});
    if let Some(response_id) = response_id {
        loop {
            let line = tokio::time::timeout(MCP_TIMEOUT, lines.next_line())
                .await
                .context("MCP stdio response timed out")?
                .context("failed to read MCP stdio response")?;
            let Some(line) = line else {
                break;
            };
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(&line)
                .with_context(|| format!("failed to parse MCP stdio JSON: {line}"))?;
            if value.get("id").and_then(Value::as_u64) == Some(response_id) {
                response = value;
                break;
            }
        }
    }
    drop(stdin);
    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
    if let Some(stderr) = stderr {
        let mut stderr = BufReader::new(stderr);
        let mut line = String::new();
        let _ = stderr.read_line(&mut line).await;
    }
    Ok(McpHttpResponse {
        body: response,
        session_id: None,
    })
}

fn parse_json_rpc_result(value: &Value) -> Result<Value> {
    let response: JsonRpcResponse = serde_json::from_value(value.clone())
        .with_context(|| format!("failed to parse MCP JSON-RPC response: {value}"))?;
    if let Some(error) = response.error {
        anyhow::bail!("MCP error {}: {}", error.code, error.message);
    }
    Ok(response.result.unwrap_or_else(|| json!({})))
}

fn parse_sse_json(text: &str) -> Result<Value> {
    for line in text.lines() {
        let line = line.trim();
        let Some(payload) = line.strip_prefix("data:") else {
            continue;
        };
        let payload = payload.trim();
        if payload.is_empty() || payload == "[DONE]" {
            continue;
        }
        return serde_json::from_str(payload)
            .with_context(|| format!("failed to parse MCP SSE payload: {payload}"));
    }
    Ok(json!({}))
}

fn local_mcp_tool_name(server_name: &str, tool_name: &str) -> String {
    format!(
        "{MCP_TOOL_PREFIX}{}__{}",
        sanitize_name(server_name),
        sanitize_name(tool_name)
    )
}

fn sanitize_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{local_mcp_tool_name, parse_json_rpc_result, parse_sse_json};
    use serde_json::json;

    #[test]
    fn mcp_tool_names_are_stable_function_names() {
        assert_eq!(
            local_mcp_tool_name("context7", "resolve-library-id"),
            "mcp__context7__resolve-library-id"
        );
        assert_eq!(
            local_mcp_tool_name("windows.fast", "read basename"),
            "mcp__windows_fast__read_basename"
        );
    }

    #[test]
    fn json_rpc_errors_become_failures() {
        let error = parse_json_rpc_result(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {"code": -32601, "message": "missing"}
        }))
        .expect_err("error response should fail");

        assert!(error.to_string().contains("missing"));
    }

    #[test]
    fn sse_parser_reads_first_json_data_event() {
        let parsed = parse_sse_json(
            "event: message\ndata: {\"jsonrpc\":\"2.0\",\"result\":{\"ok\":true}}\n\n",
        )
        .expect("sse should parse");

        assert_eq!(parsed["result"]["ok"], true);
    }
}
