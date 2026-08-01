use serde_json::{Value, json};

use crate::agent::AgentRunner;
use crate::mcp::McpRegistry;
use crate::profiler::tool_signature;
use crate::tools::{
    ToolResult, invoke_with_read_roots, is_local_filesystem_tool, is_readonly_tool,
};

#[derive(Debug, Clone)]
pub(super) struct CachedToolObservation {
    pub(super) result: ToolResult,
    pub(super) first_turn: usize,
    pub(super) hits: usize,
}

impl AgentRunner {
    pub(super) async fn invoke_with_cache(&mut self, tool_name: &str, args: Value) -> ToolResult {
        if self.local_filesystem_only && !is_local_filesystem_tool(tool_name) {
            let message =
                format!("tool `{tool_name}` is blocked by the local-filesystem-only capability");
            return ToolResult {
                ok: false,
                data: json!({"error_kind": "tool_blocked", "tool": tool_name, "message": message}),
                error: Some(message),
            };
        }
        if self.local_filesystem_only
            && is_local_filesystem_tool(tool_name)
            && let Some(budget) = &mut self.local_filesystem_tool_budget
            && !budget.try_consume()
        {
            let message = "local filesystem evidence budget reached; synthesize from the evidence already gathered";
            return ToolResult {
                ok: false,
                data: json!({
                    "error_kind": "tool_budget_reached",
                    "tool": tool_name,
                    "max": budget.max,
                    "used": budget.used,
                    "remaining": 0,
                    "message": message,
                }),
                error: Some(message.to_string()),
            };
        }
        if tool_name.starts_with("subagent.") {
            return Box::pin(self.invoke_subagent_tool(tool_name, args)).await;
        }
        if let Some(message) = self.delegated_tool_scope_error(tool_name, &args) {
            return ToolResult {
                ok: false,
                data: json!({"error_kind": "delegated_write_scope", "tool": tool_name, "args": args, "message": message}),
                error: Some(message),
            };
        }
        if McpRegistry::is_mcp_tool(tool_name) {
            self.ensure_mcp_registry().await;
            if let Some(registry) = &self.mcp_registry {
                return registry.invoke(tool_name, args).await;
            }
        }

        let signature = tool_signature(tool_name, &args);
        if is_cacheable_readonly_tool(tool_name)
            && let Some(cached) = self.readonly_tool_cache.get_mut(&signature)
        {
            cached.hits += 1;
            self.profiler
                .record_readonly_tool_cache_hit(self.request_seq, tool_name, &args);
            return cached_readonly_result(tool_name, &args, cached);
        }

        let result =
            invoke_with_read_roots(&self.cwd, &self.read_roots, self.mode, tool_name, args).await;
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
}

pub(in crate::agent) fn is_cacheable_readonly_tool(tool_name: &str) -> bool {
    is_readonly_tool(tool_name)
}

pub(in crate::agent) fn should_cache_readonly_result(result: &ToolResult) -> bool {
    result.ok || result.error.is_some()
}

pub(in crate::agent) fn invalidates_readonly_tool_cache(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "fs.write" | "fs.replace" | "fs.edit" | "fs.rename" | "cmd.exec"
    )
}

pub(in crate::agent) fn cached_readonly_result(
    tool_name: &str,
    args: &Value,
    cached: &CachedToolObservation,
) -> ToolResult {
    ToolResult {
        ok: cached.result.ok,
        data: json!({
            "cached_observation": true,
            "content_reused": true,
            "first_observed_turn": cached.first_turn,
            "cache_hits": cached.hits,
            "tool": tool_name,
            "args": args,
            "hint": "This exact read-only tool call was already observed. Use the previous observation or compacted summary instead of repeating the same call. Request a different path, offset, limit, or query only if more evidence is still needed.",
            "original_error": cached.result.error,
        }),
        error: cached.result.error.clone(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    fn auth_tokens() -> crate::auth::AuthTokens {
        crate::auth::AuthTokens {
            id_token: "id".to_string(),
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            expires_at: i64::MAX,
            account_id: None,
        }
    }

    #[tokio::test]
    async fn local_filesystem_capability_blocks_stale_hosted_and_mcp_calls() {
        let dir = TempDir::new().expect("tempdir");
        let mut runner = AgentRunner::new(
            auth_tokens(),
            dir.path().to_path_buf(),
            crate::DEFAULT_MODEL.to_string(),
            false,
            false,
            crate::DEFAULT_COMPACT_AFTER_CHARS,
            crate::DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS,
            crate::DEFAULT_MAX_INPUT_CHARS,
            false,
            None,
            false,
            None,
            crate::tools::AgentMode::Ask,
        )
        .expect("runner");
        runner.enforce_local_filesystem_only();

        for tool in [
            "web.search",
            "mcp__example__tool",
            "subagent.run",
            "fs.write",
        ] {
            let result = runner.invoke_with_cache(tool, json!({})).await;
            assert!(!result.ok, "{tool} must be blocked");
            assert_eq!(result.data["error_kind"], "tool_blocked");
        }
    }

    #[tokio::test]
    async fn repo_brief_evidence_budget_executes_only_the_first_sixteen_calls() {
        let dir = TempDir::new().expect("tempdir");
        for index in 0..17 {
            std::fs::write(dir.path().join(format!("evidence-{index}.txt")), "evidence")
                .expect("fixture");
        }
        let mut runner = AgentRunner::new(
            auth_tokens(),
            dir.path().to_path_buf(),
            crate::DEFAULT_MODEL.to_string(),
            false,
            false,
            crate::DEFAULT_COMPACT_AFTER_CHARS,
            crate::DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS,
            crate::DEFAULT_MAX_INPUT_CHARS,
            false,
            None,
            false,
            None,
            crate::tools::AgentMode::Ask,
        )
        .expect("runner");
        runner.enforce_local_filesystem_only();
        runner.set_local_filesystem_tool_budget(16);

        for index in 0..16 {
            let result = runner
                .invoke_with_cache("fs.stat", json!({"path": format!("evidence-{index}.txt")}))
                .await;
            assert!(result.ok, "call {index} should execute");
        }

        let blocked = runner
            .invoke_with_cache("fs.stat", json!({"path": "evidence-16.txt"}))
            .await;
        assert!(!blocked.ok);
        assert_eq!(blocked.data["error_kind"], "tool_budget_reached");
        assert_eq!(blocked.data["max"], 16);
        assert_eq!(blocked.data["used"], 16);
        assert_eq!(blocked.data["remaining"], 0);
        assert_eq!(
            runner.profile_summary()["local_filesystem_tool_budget"],
            json!({"max": 16, "used": 16, "remaining": 0})
        );
    }
}
