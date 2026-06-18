use serde_json::json;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

use super::*;

mod cache;
mod compaction;
mod goal;
mod snapshot;
mod subagent;
mod trace;

#[tokio::test]
async fn run_with_cancel_stops_before_first_request_when_token_is_cancelled() {
    let temp = TempDir::new().expect("tempdir");
    let mut runner = AgentRunner::new(
        test_auth_tokens(),
        temp.path().to_path_buf(),
        crate::DEFAULT_MODEL.to_string(),
        None,
        false,
        false,
        crate::DEFAULT_COMPACT_AFTER_CHARS,
        crate::DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS,
        crate::DEFAULT_MAX_INPUT_CHARS,
        false,
        None,
        false,
        None,
        crate::tools::AgentMode::Work,
    )
    .expect("runner");
    runner.use_buffered_display();
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    let error = runner
        .run_with_cancel("hello", cancellation)
        .await
        .expect_err("cancelled run should error");

    assert_eq!(error.to_string(), "run cancelled");
    assert_eq!(runner.request_seq, 0);
    assert!(runner.take_display_events().iter().any(
        |event| matches!(event, AgentDisplayEvent::Warning(text) if text.contains("run cancelled"))
    ));
}

#[test]
fn memory_is_off_by_default_and_reported_in_status() {
    let temp = TempDir::new().expect("tempdir");
    let runner = AgentRunner::new(
        test_auth_tokens(),
        temp.path().to_path_buf(),
        crate::DEFAULT_MODEL.to_string(),
        None,
        false,
        false,
        crate::DEFAULT_COMPACT_AFTER_CHARS,
        crate::DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS,
        crate::DEFAULT_MAX_INPUT_CHARS,
        false,
        None,
        false,
        None,
        crate::tools::AgentMode::Work,
    )
    .expect("runner");

    assert!(!runner.memory_enabled());
    assert!(runner.profile_status().contains("memory=off"));
    assert_eq!(runner.profile_summary()["memory_enabled"], false);
    assert!(!runner.snapshot().memory_enabled);
}

fn test_auth_tokens() -> crate::auth::AuthTokens {
    crate::auth::AuthTokens {
        id_token: "id".to_string(),
        access_token: "access".to_string(),
        refresh_token: "refresh".to_string(),
        expires_at: i64::MAX,
        account_id: Some("acct_test".to_string()),
    }
}
