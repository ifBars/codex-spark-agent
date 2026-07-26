use super::*;
use std::collections::HashMap;

use crate::agent::trace::{TraceMetadata, TraceWriter};

#[test]
fn trace_writer_keeps_multiple_same_turn_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut writer = TraceWriter {
        dir: dir.path().to_path_buf(),
        file_counts: HashMap::new(),
    };

    writer
        .write(1, "tool-result", &json!({"tool": "fs.read"}))
        .expect("write first");
    writer
        .write(1, "tool-result", &json!({"tool": "cmd.exec"}))
        .expect("write second");

    assert!(dir.path().join("001-tool-result.json").exists());
    assert!(dir.path().join("001-tool-result-002.json").exists());
}

#[test]
fn trace_metadata_includes_approx_token_thresholds() {
    let dir = tempfile::tempdir().expect("tempdir");
    let writer = TraceWriter::new(
        dir.path().to_path_buf(),
        TraceMetadata {
            cwd: dir.path().to_path_buf(),
            model: "gpt-5.3-codex-spark".to_string(),
            compact_after_chars: 120_000,
            compact_after_tool_only_turns: 12,
            max_input_chars: 480_000,
            profile: true,
            interactive: true,
            session_name: Some("demo-session".to_string()),
            new_session: true,
            mode: crate::tools::AgentMode::Ask,
            context: Some(json!({
                "profile_scenario": {
                    "name": "compaction-pressure",
                    "target_tokens": 45_000
                }
            })),
        },
    )
    .expect("trace writer");

    let metadata =
        std::fs::read_to_string(writer.dir.join("000-trace-metadata.json")).expect("read metadata");
    let metadata = serde_json::from_str::<Value>(&metadata).expect("parse metadata");

    assert_eq!(metadata["compact_after_approx_tokens"], 30_000);
    assert_eq!(metadata["compact_after_tool_only_turns"], 12);
    assert_eq!(metadata["max_input_approx_tokens"], 120_000);
    assert_eq!(metadata["context_window_tokens"], 128_000);
    assert_eq!(metadata["profile"], true);
    assert_eq!(metadata["interactive"], true);
    assert_eq!(metadata["session"], "demo-session");
    assert_eq!(metadata["new_session"], true);
    assert_eq!(metadata["mode"], "ask");
    assert_eq!(
        metadata["context"]["profile_scenario"]["name"],
        "compaction-pressure"
    );
}
