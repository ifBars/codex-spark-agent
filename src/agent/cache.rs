use serde_json::Value;

use crate::agent::AgentRunner;
use crate::profiler::tool_signature;
use crate::tools::{ToolResult, invoke};

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
}

pub(in crate::agent) fn is_cacheable_readonly_tool(tool_name: &str) -> bool {
    matches!(tool_name, "fs.read" | "fs.list" | "fs.stat" | "fs.search")
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
