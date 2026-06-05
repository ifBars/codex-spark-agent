use serde_json::{Value, json};

use crate::agent::AgentRunner;
use crate::profiler::tool_signature;
use crate::tools::{ToolResult, invoke_with_read_roots, is_readonly_tool};

#[derive(Debug, Clone)]
pub(super) struct CachedToolObservation {
    pub(super) result: ToolResult,
    pub(super) first_turn: usize,
    pub(super) hits: usize,
}

impl AgentRunner {
    pub(super) async fn invoke_with_cache(&mut self, tool_name: &str, args: Value) -> ToolResult {
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
