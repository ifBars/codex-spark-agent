use std::path::Path;

use serde_json::json;

use super::*;

fn write_request_with_required_action(dir: &Path, action: &str) {
    write_turn_request_with_required_action(dir, 1, action);
}

fn write_turn_request_with_required_action(dir: &Path, turn: usize, action: &str) {
    std::fs::write(
            dir.join(format!("{turn:03}-request-input.json")),
            serde_json::to_vec_pretty(&json!({
                "input": [{
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": format!(
                            "[spark local message compaction]\nrequired_actions=1\n{action}\n[/spark local message compaction]"
                        )
                    }]
                }]
            }))
            .expect("serialize request"),
        )
        .expect("write request");
}

fn write_trace_metadata_with_expected_tools(dir: &Path, groups: Value) {
    std::fs::write(
        dir.join("000-trace-metadata.json"),
        serde_json::to_vec_pretty(&json!({
            "model": "gpt-5.3-codex-spark",
            "context": {
                "profile_scenario": {
                    "name": "file-ops",
                    "expected_tool_groups": groups,
                }
            }
        }))
        .expect("serialize metadata"),
    )
    .expect("write metadata");
}

fn write_trace_metadata_with_expected_tool_calls(dir: &Path, calls: Value) {
    std::fs::write(
        dir.join("000-trace-metadata.json"),
        serde_json::to_vec_pretty(&json!({
            "model": "gpt-5.3-codex-spark",
            "context": {
                "profile_scenario": {
                    "name": "file-ops",
                    "expected_tool_calls": calls,
                }
            }
        }))
        .expect("serialize metadata"),
    )
    .expect("write metadata");
}

fn write_trace_metadata_with_expected_skills(dir: &Path, skills: Value) {
    std::fs::write(
        dir.join("000-trace-metadata.json"),
        serde_json::to_vec_pretty(&json!({
            "model": "gpt-5.3-codex-spark",
            "context": {
                "profile_scenario": {
                    "name": "skill-use",
                    "expected_skills": skills,
                }
            }
        }))
        .expect("serialize metadata"),
    )
    .expect("write metadata");
}

mod analyze;
mod core;
mod errors;
mod expectations;
mod format;
