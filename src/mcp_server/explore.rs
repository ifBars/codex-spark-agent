use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::repo_brief::{self, RepoBriefRequest};

const MAX_STARTING_PATHS: usize = 32;

/// Backward-compatible MCP argument shape for `explore_repo`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExploreArgs {
    task: String,
    #[serde(default)]
    cwd: Option<std::path::PathBuf>,
    #[serde(default)]
    starting_paths: Vec<String>,
    #[serde(default)]
    context: Option<String>,
    #[serde(default)]
    reasoning_effort: Option<String>,
    #[serde(default)]
    trace: bool,
}

pub(super) fn tool_definition() -> Value {
    json!({
        "name": "explore_repo",
        "title": "Explore a repository with Spark",
        "description": "Use proactively for non-trivial read-only repository exploration, file discovery, call-chain tracing, architecture reconnaissance, and hypothesis checks before the parent Codex agent edits or decides. This runs GPT-5.3-Codex-Spark through the dedicated Spark harness and returns a compact evidence brief. Do not use for a single known file or lookup that one direct read/search can answer.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "task": {"type": "string", "description": "One concrete exploration question with the desired evidence."},
                "cwd": {"type": "string", "description": "Absolute repository or workspace directory. Defaults to the MCP server process working directory."},
                "starting_paths": {"type": "array", "items": {"type": "string"}, "maxItems": MAX_STARTING_PATHS, "description": "Optional repository-relative files or directories Spark should inspect first."},
                "context": {"type": "string", "description": "Only task-relevant parent Codex constraints, decisions, skill guidance, or memory. Do not forward secrets or the full parent transcript."},
                "reasoning_effort": {"type": "string", "enum": ["low", "medium", "high", "xhigh"], "default": "medium"},
                "trace": {"type": "boolean", "default": false, "description": "Save the harness trace under the explored workspace's ignored .spark-runs directory."}
            },
            "required": ["task"],
            "additionalProperties": false
        }
    })
}

pub(super) async fn run(arguments: Value, cancellation: CancellationToken) -> Result<String> {
    let args: ExploreArgs =
        serde_json::from_value(arguments).context("invalid explore_repo arguments")?;
    repo_brief::run_mcp(
        RepoBriefRequest {
            question: args.task,
            cwd: args.cwd,
            paths: args.starting_paths,
            context: args.context,
            reasoning_effort: args.reasoning_effort,
            trace: args.trace,
        },
        cancellation,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::tool_definition;

    #[test]
    fn explorer_tool_remains_a_text_only_mcp_contract_without_deadline() {
        let tool = tool_definition();
        let schema = &tool["inputSchema"]["properties"];
        assert!(schema.get("timeout_seconds").is_none());
        assert_eq!(tool["name"], "explore_repo");
        assert!(tool.get("outputSchema").is_none());
    }
}
