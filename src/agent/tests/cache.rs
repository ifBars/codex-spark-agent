use super::*;
use crate::agent::cache::{
    CachedToolObservation, cached_readonly_result, is_cacheable_readonly_tool,
    should_cache_readonly_result,
};
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

#[test]
fn readonly_cache_hits_return_compact_reuse_hints() {
    let cached = CachedToolObservation {
        result: ToolResult {
            ok: true,
            data: json!({"content": "large file body"}),
            error: None,
        },
        first_turn: 3,
        hits: 2,
    };

    let result = cached_readonly_result("fs.read", &json!({"path": "README.md"}), &cached);

    assert!(result.ok);
    assert_eq!(result.data["cached_observation"], true);
    assert_eq!(result.data["content_reused"], true);
    assert_eq!(result.data["first_observed_turn"], 3);
    assert_eq!(result.data["cache_hits"], 2);
    assert_eq!(result.data["args"]["path"], "README.md");
    assert!(result.data.get("content").is_none());
}
