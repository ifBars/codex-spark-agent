use super::*;
use crate::agent::cache::{is_cacheable_readonly_tool, should_cache_readonly_result};
use crate::tools::ToolResult;

#[test]
fn readonly_cache_policy_keeps_successes_and_failures() {
    assert!(is_cacheable_readonly_tool("fs.stat"));
    assert!(should_cache_readonly_result(&ToolResult {
        ok: true,
        data: json!({"path": "README.md"}),
        error: None,
    }));
    assert!(should_cache_readonly_result(&ToolResult {
        ok: false,
        data: json!({}),
        error: Some("failed to read missing.txt".to_string()),
    }));
    assert!(!should_cache_readonly_result(&ToolResult {
        ok: false,
        data: json!({}),
        error: None,
    }));
}
