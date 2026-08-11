use serde_json::{Value, json};
use std::time::Instant;

use futures_util::future::join_all;

use crate::agent::AgentRunner;
use crate::mcp::McpRegistry;
use crate::profiler::tool_signature;
use crate::tools::{
    ToolResult, invoke_local_read_with_read_roots, invoke_with_read_roots,
    is_local_filesystem_tool, is_readonly_tool,
};

const MAX_PARALLEL_LOCAL_READS: usize = 8;

#[derive(Debug, Clone)]
pub(super) struct CachedToolObservation {
    pub(super) result: ToolResult,
    pub(super) first_turn: usize,
    pub(super) hits: usize,
}

pub(super) struct TimedToolResult {
    pub(super) result: ToolResult,
    pub(super) duration_ms: u64,
}

impl AgentRunner {
    pub(super) async fn invoke_with_cache(&mut self, tool_name: &str, args: Value) -> ToolResult {
        if !self.tool_access.allows(tool_name) {
            let message = format!("tool `{tool_name}` is blocked by the configured tool policy");
            return ToolResult {
                ok: false,
                data: json!({"error_kind": "tool_blocked", "tool": tool_name, "message": message}),
                error: Some(message),
            };
        }
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
        if tool_name == "tool.search" {
            self.ensure_mcp_registry().await;
            return self.search_and_activate_tools(args);
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

    /// Executes one model-emitted batch of independent local reads concurrently while preserving
    /// the call order expected by the Responses API. Mutation, shell, network, browser, MCP, and
    /// subagent tools intentionally stay on the serial path in the agent loop.
    pub(super) async fn invoke_parallel_local_reads(
        &mut self,
        calls: &[(String, Value)],
    ) -> Vec<TimedToolResult> {
        debug_assert!(
            calls
                .iter()
                .all(|(tool_name, _)| is_parallel_local_read(tool_name))
        );

        let mut results = (0..calls.len()).map(|_| None).collect::<Vec<_>>();
        let mut pending = Vec::new();

        for (index, (tool_name, args)) in calls.iter().enumerate() {
            if self.local_filesystem_only
                && let Some(budget) = &mut self.local_filesystem_tool_budget
                && !budget.try_consume()
            {
                let message = "local filesystem evidence budget reached; synthesize from the evidence already gathered";
                results[index] = Some(TimedToolResult {
                    result: ToolResult {
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
                    },
                    duration_ms: 0,
                });
                continue;
            }

            let signature = tool_signature(tool_name, args);
            if let Some(cached) = self.readonly_tool_cache.get_mut(&signature) {
                cached.hits += 1;
                self.profiler
                    .record_readonly_tool_cache_hit(self.request_seq, tool_name, args);
                results[index] = Some(TimedToolResult {
                    result: cached_readonly_result(tool_name, args, cached),
                    duration_ms: 0,
                });
                continue;
            }

            pending.push((index, signature, tool_name.clone(), args.clone()));
        }

        let cwd = self.cwd.clone();
        let read_roots = self.read_roots.clone();
        let mode = self.mode;
        let mut completed = Vec::with_capacity(pending.len());
        for wave in pending.chunks(MAX_PARALLEL_LOCAL_READS) {
            let joined = join_all(wave.iter().map(|(_, _, tool_name, args)| {
                let cwd = cwd.clone();
                let read_roots = read_roots.clone();
                let tool_name = tool_name.clone();
                let args = args.clone();
                tokio::task::spawn_blocking(move || {
                    let started = Instant::now();
                    let result = invoke_local_read_with_read_roots(
                        &cwd,
                        &read_roots,
                        mode,
                        &tool_name,
                        args,
                    );
                    TimedToolResult {
                        result,
                        duration_ms: started.elapsed().as_millis() as u64,
                    }
                })
            }))
            .await;
            completed.extend(joined.into_iter().map(|joined| match joined {
                Ok(timed) => timed,
                Err(error) => TimedToolResult {
                    result: ToolResult {
                        ok: false,
                        data: json!({
                            "error_kind": "parallel_tool_join",
                            "message": error.to_string(),
                        }),
                        error: Some(format!("parallel local read task failed: {error}")),
                    },
                    duration_ms: 0,
                },
            }));
        }

        for ((index, signature, _, _), timed) in pending.into_iter().zip(completed) {
            if should_cache_readonly_result(&timed.result) {
                self.readonly_tool_cache.insert(
                    signature,
                    CachedToolObservation {
                        result: timed.result.clone(),
                        first_turn: self.request_seq,
                        hits: 0,
                    },
                );
            }
            results[index] = Some(timed);
        }

        results
            .into_iter()
            .map(|result| result.expect("every parallel read has a result"))
            .collect()
    }
}

pub(in crate::agent) fn is_parallel_local_read(tool_name: &str) -> bool {
    is_local_filesystem_tool(tool_name)
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

    #[tokio::test]
    async fn parallel_local_reads_preserve_order_cache_results_and_budget() {
        let dir = TempDir::new().expect("tempdir");
        std::fs::write(dir.path().join("first.txt"), "first").expect("first fixture");
        std::fs::write(dir.path().join("second.txt"), "second").expect("second fixture");
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
        runner.set_local_filesystem_tool_budget(3);
        let calls = vec![
            ("fs.read".to_string(), json!({"path": "first.txt"})),
            ("fs.read".to_string(), json!({"path": "second.txt"})),
        ];

        let first = runner.invoke_parallel_local_reads(&calls).await;
        assert_eq!(first[0].result.data["path"], "first.txt");
        assert_eq!(first[1].result.data["path"], "second.txt");

        let cached = runner.invoke_parallel_local_reads(&calls).await;
        assert_eq!(cached[0].result.data["cached_observation"], true);
        assert_eq!(cached[1].result.data["error_kind"], "tool_budget_reached");
        assert_eq!(
            runner.profile_summary()["local_filesystem_tool_budget"],
            json!({"max": 3, "used": 3, "remaining": 0})
        );
    }
}
