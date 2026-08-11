//! One-shot, machine-readable Spark runner for trusted host applications.

use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::agent::AgentRunner;
use crate::mcp::{McpHttpServer, McpRegistry};
use crate::tools::{AgentMode, ToolAccessPolicy};
use crate::{
    DEFAULT_COMPACT_AFTER_CHARS, DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS, DEFAULT_MAX_INPUT_CHARS,
    config,
};

pub(crate) const AUTOMATION_SCHEMA_VERSION: &str = "spark.automation.v1";
const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_PROMPT_CHARS: usize = 200_000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutomationRequest {
    schema_version: String,
    request_id: String,
    cwd: PathBuf,
    prompt: String,
    model: String,
    reasoning_effort: String,
    output_schema: Value,
    #[serde(default = "default_output_schema_name")]
    output_schema_name: String,
    #[serde(default)]
    read_roots: Vec<PathBuf>,
    #[serde(default)]
    system_prompt: Option<String>,
    #[serde(default)]
    tool_policy: AutomationToolPolicy,
    #[serde(default)]
    mcp_servers: Vec<AutomationMcpServer>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AutomationToolPolicy {
    #[serde(default)]
    workspace_writes: bool,
    /// Spark's command tool is not an OS sandbox. Hosts must provide their own
    /// process/container boundary before opting into it.
    #[serde(default)]
    allow_unsandboxed_commands: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutomationMcpServer {
    name: String,
    url: String,
    #[serde(default)]
    bearer_token_env_var: Option<String>,
}

#[derive(Debug, Serialize)]
struct AutomationResponse {
    schema_version: &'static str,
    request_id: String,
    status: &'static str,
    final_response: String,
    tool_policy: AutomationToolPolicy,
    warnings: Vec<&'static str>,
}

pub(crate) async fn run_stdio() -> Result<()> {
    let request = match read_request() {
        Ok(request) => request,
        Err(error) => {
            write_failure(None, &error)?;
            return Err(error);
        }
    };
    let request_id = request.request_id.clone();
    match run_request(request).await {
        Ok(response) => {
            println!("{}", serde_json::to_string(&response)?);
            Ok(())
        }
        Err(error) => {
            write_failure(Some(&request_id), &error)?;
            Err(error)
        }
    }
}

fn write_failure(request_id: Option<&str>, error: &anyhow::Error) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "schema_version": AUTOMATION_SCHEMA_VERSION,
            "request_id": request_id,
            "status": "failed",
            "error": error.to_string(),
        }))?
    );
    Ok(())
}

fn read_request() -> Result<AutomationRequest> {
    let mut input = Vec::new();
    std::io::stdin()
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut input)
        .context("failed to read Spark automation request from stdin")?;
    if input.len() > MAX_REQUEST_BYTES {
        anyhow::bail!("Spark automation request exceeds {MAX_REQUEST_BYTES} bytes");
    }
    parse_request(&input)
}

fn parse_request(input: &[u8]) -> Result<AutomationRequest> {
    let input = input.strip_prefix(b"\xef\xbb\xbf").unwrap_or(input);
    serde_json::from_slice(input).context("failed to parse Spark automation request JSON")
}

async fn run_request(request: AutomationRequest) -> Result<AutomationResponse> {
    validate_request(&request)?;
    let cwd = canonical_directory(&request.cwd, "workspace")?;
    let read_roots = request
        .read_roots
        .iter()
        .map(|root| canonical_directory(root, "read root"))
        .collect::<Result<Vec<_>>>()?;

    let auth = config::load_auth()?;
    let mut runner = AgentRunner::new_with_reasoning_effort(
        auth,
        cwd,
        request.model,
        request.reasoning_effort,
        /* trace */ false,
        /* profile */ false,
        DEFAULT_COMPACT_AFTER_CHARS,
        DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS,
        DEFAULT_MAX_INPUT_CHARS,
        /* interactive */ false,
        None,
        /* new_session */ true,
        None,
        AgentMode::Work,
    )?;
    runner.use_buffered_display();
    runner.set_read_roots(read_roots);
    runner.set_system_prompt(request.system_prompt);
    runner.set_output_schema(request.output_schema_name, request.output_schema);

    let tool_policy = request.tool_policy;
    runner.set_tool_access(ToolAccessPolicy {
        workspace_writes: tool_policy.workspace_writes,
        command_execution: tool_policy.allow_unsandboxed_commands,
        github_cli: false,
        hosted_web_search: false,
        browser: false,
        subagents: false,
        mcp: !request.mcp_servers.is_empty(),
    });
    let mcp_servers = request
        .mcp_servers
        .into_iter()
        .map(|server| McpHttpServer {
            name: server.name,
            url: server.url,
            bearer_token_env_var: server.bearer_token_env_var,
        })
        .collect();
    runner.set_explicit_mcp_registry(McpRegistry::from_http_servers(mcp_servers).await?);

    let final_response = runner
        .run_with_cancel_to_text(&request.prompt, CancellationToken::new())
        .await?;
    serde_json::from_str::<Value>(&final_response)
        .context("Spark returned a non-JSON final response for a structured automation run")?;

    Ok(AutomationResponse {
        schema_version: AUTOMATION_SCHEMA_VERSION,
        request_id: request.request_id,
        status: "completed",
        final_response,
        tool_policy,
        warnings: tool_policy
            .allow_unsandboxed_commands
            .then_some("cmd.exec is enabled without a Spark-provided OS sandbox")
            .into_iter()
            .collect(),
    })
}

fn validate_request(request: &AutomationRequest) -> Result<()> {
    if request.schema_version != AUTOMATION_SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported Spark automation schema '{}'",
            request.schema_version
        );
    }
    validate_identifier(&request.request_id, 128, "request_id")?;
    validate_identifier(&request.output_schema_name, 64, "output_schema_name")?;
    if request.prompt.trim().is_empty() || request.prompt.chars().count() > MAX_PROMPT_CHARS {
        anyhow::bail!("prompt must contain 1 to {MAX_PROMPT_CHARS} characters");
    }
    if request.model.trim().is_empty() || request.model.len() > 128 {
        anyhow::bail!("model must contain 1 to 128 characters");
    }
    if !matches!(
        request.reasoning_effort.as_str(),
        "minimal" | "low" | "medium" | "high" | "xhigh" | "max"
    ) {
        anyhow::bail!("reasoning_effort is not supported by the automation protocol");
    }
    if !request.output_schema.is_object() {
        anyhow::bail!("output_schema must be a JSON object");
    }
    Ok(())
}

fn validate_identifier(value: &str, max_len: usize, field: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > max_len
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        anyhow::bail!(
            "{field} must contain only letters, numbers, underscores, dashes, or dots and be at most {max_len} characters"
        );
    }
    Ok(())
}

fn canonical_directory(path: &PathBuf, label: &str) -> Result<PathBuf> {
    let canonical = std::fs::canonicalize(path).with_context(|| {
        format!(
            "Spark automation {label} does not exist: {}",
            path.display()
        )
    })?;
    if !canonical.is_dir() {
        anyhow::bail!(
            "Spark automation {label} is not a directory: {}",
            path.display()
        );
    }
    Ok(canonical)
}

fn default_output_schema_name() -> String {
    "automation_output".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> AutomationRequest {
        AutomationRequest {
            schema_version: AUTOMATION_SCHEMA_VERSION.to_string(),
            request_id: "diffuin-job-42".to_string(),
            cwd: PathBuf::from("."),
            prompt: "Return the requested artifact.".to_string(),
            model: "gpt-5.3-codex-spark".to_string(),
            reasoning_effort: "medium".to_string(),
            output_schema: serde_json::json!({"type": "object"}),
            output_schema_name: default_output_schema_name(),
            read_roots: Vec::new(),
            system_prompt: None,
            tool_policy: AutomationToolPolicy::default(),
            mcp_servers: Vec::new(),
        }
    }

    #[test]
    fn accepts_diffuin_shaped_request_contract() {
        validate_request(&request()).expect("valid request");
    }

    #[test]
    fn rejects_unknown_protocol_and_non_object_schema() {
        let mut request = request();
        request.schema_version = "spark.automation.v2".to_string();
        assert!(validate_request(&request).is_err());

        request.schema_version = AUTOMATION_SCHEMA_VERSION.to_string();
        request.output_schema = serde_json::json!([]);
        assert!(validate_request(&request).is_err());
    }

    #[test]
    fn defaults_to_no_writes_and_no_commands() {
        let policy = AutomationToolPolicy::default();
        assert!(!policy.workspace_writes);
        assert!(!policy.allow_unsandboxed_commands);
    }

    #[test]
    fn parser_accepts_a_utf8_bom_from_windows_hosts() {
        let body = serde_json::json!({
            "schema_version": AUTOMATION_SCHEMA_VERSION,
            "request_id": "job-42",
            "cwd": ".",
            "prompt": "Return the artifact.",
            "model": "gpt-5.3-codex-spark",
            "reasoning_effort": "medium",
            "output_schema": {"type": "object"}
        })
        .to_string();
        let bytes = [b"\xef\xbb\xbf".as_slice(), body.as_bytes()].concat();

        let parsed = parse_request(&bytes).expect("parse BOM-prefixed request");

        assert_eq!(parsed.request_id, "job-42");
    }
}
