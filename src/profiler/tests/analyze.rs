use super::*;

#[test]
fn analyze_trace_reconstructs_tool_calls_from_stream_events() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("001-request-input.json"),
        serde_json::to_vec_pretty(&json!({
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "read"}]}]
        }))
        .expect("serialize request"),
    )
    .expect("write request");
    std::fs::write(
        dir.path().join("001-response.json"),
        serde_json::to_vec_pretty(&json!({
            "duration_ms": 1234,
            "raw": {
                "response": {"output": []},
                "events": [
                    {
                        "type": "response.output_item.done",
                        "output_index": 0,
                        "item": {
                            "type": "function_call",
                            "name": "fs_read",
                            "arguments": "{\"path\":\"a.txt\",\"limit\":5}"
                        }
                    },
                    {
                        "type": "response.output_item.done",
                        "output_index": 1,
                        "item": {
                            "type": "message",
                            "content": [{"type": "output_text", "text": "done"}]
                        }
                    }
                ]
            }
        }))
        .expect("serialize response"),
    )
    .expect("write response");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["requests"], 1);
    assert_eq!(summary["tool_calls"], 1);
    assert_eq!(summary["tool_counts"]["fs.read"], 1);
    assert_eq!(summary["response_text_chars"], 4);
    assert_eq!(summary["request_duration_ms_by_request"], json!([1234]));
    assert_eq!(summary["max_request_duration_ms"], 1234);
    assert_eq!(summary["timeline"][0]["turn"], 1);
    assert!(
        summary["timeline"][0]["request_input_chars"]
            .as_u64()
            .expect("request chars")
            > 2
    );
    assert_eq!(summary["timeline"][0]["request_duration_ms"], 1234);
    assert_eq!(summary["timeline"][0]["response_text_chars"], 4);
    assert_eq!(summary["timeline"][0]["tool_calls"][0]["tool"], "fs.read");
}

#[test]
fn analyze_trace_skips_artifact_directories() {
    let dir = tempfile::tempdir().expect("tempdir");
    let artifacts = dir.path().join("browser-artifacts");
    std::fs::create_dir(&artifacts).expect("create artifacts dir");
    std::fs::write(artifacts.join("screenshot.png"), b"not json").expect("write artifact");
    std::fs::write(dir.path().join("browser-smoke.mjs"), "not json").expect("write script");
    std::fs::write(dir.path().join("screenshot.png"), b"not json").expect("write screenshot");
    std::fs::write(
        dir.path().join("001-request-input.json"),
        serde_json::to_vec_pretty(&json!({
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "read"}]}]
        }))
        .expect("serialize request"),
    )
    .expect("write request");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["requests"], 1);
}

#[test]
fn analyze_trace_reports_post_compaction_regrowth() {
    let dir = tempfile::tempdir().expect("tempdir");
    let large_followup = "x".repeat(120_000);
    std::fs::write(
        dir.path().join("001-request-input.json"),
        serde_json::to_vec_pretty(&json!({
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "compact soon"}]}]
        }))
        .expect("serialize request"),
    )
    .expect("write first request");
    std::fs::write(
        dir.path().join("002-compaction.json"),
        serde_json::to_vec_pretty(&json!({
            "method": "responses_compact",
            "trigger": "size_threshold",
            "before_chars": 220_000,
            "after_chars": 10_000
        }))
        .expect("serialize compaction"),
    )
    .expect("write compaction");
    std::fs::write(
        dir.path().join("002-request-input.json"),
        serde_json::to_vec_pretty(&json!({
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "after compact"}]}]
        }))
        .expect("serialize request"),
    )
    .expect("write second request");
    std::fs::write(
        dir.path().join("003-request-input.json"),
        serde_json::to_vec_pretty(&json!({
            "input": [{"role": "user", "content": [{"type": "input_text", "text": large_followup}]}]
        }))
        .expect("serialize request"),
    )
    .expect("write third request");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["compaction_regrowth"]["count"], 1);
    assert!(
        summary["compaction_regrowth"]["max_next_request_growth_chars"]
            .as_u64()
            .expect("regrowth")
            >= 40_000
    );
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "post_compaction_context_regrowth")
    );
}

#[test]
fn analyze_trace_matches_retained_required_actions_to_tool_calls() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_request_with_required_action(
        dir.path(),
        "action_1=tool=fs.list path=src recursive=false",
    );
    write_turn_request_with_required_action(
        dir.path(),
        2,
        "action_1=tool=fs.list path=src recursive=false",
    );
    std::fs::write(
        dir.path().join("001-response.json"),
        serde_json::to_vec_pretty(&json!({
            "raw": {
                "events": [{
                    "type": "response.output_item.done",
                    "output_index": 0,
                    "item": {
                        "type": "function_call",
                        "name": "fs_list",
                        "arguments": "{\"path\":\"src\",\"recursive\":false}"
                    }
                }]
            }
        }))
        .expect("serialize response"),
    )
    .expect("write response");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(
        summary["retained_required_actions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(summary["retained_required_actions"][0]["tool"], "fs.list");
    assert_eq!(
        summary["retained_required_actions_executed"][0]["path"],
        "src"
    );
    assert_eq!(summary["retained_required_actions_missing"], json!([]));
    assert_eq!(summary["tool_calls_before_first_required_action"], 0);
}

#[test]
fn analyze_trace_matches_retained_rename_required_action() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_request_with_required_action(
        dir.path(),
        "action_1=tool=fs.rename from=.spark-scenarios/file-ops/drafts/report-draft.md to=.spark-scenarios/file-ops/final/report.md",
    );
    std::fs::write(
            dir.path().join("001-response.json"),
            serde_json::to_vec_pretty(&json!({
                "raw": {
                    "events": [{
                        "type": "response.output_item.done",
                        "output_index": 0,
                        "item": {
                            "type": "function_call",
                            "name": "fs_rename",
                            "arguments": "{\"from\":\".spark-scenarios/file-ops/drafts/report-draft.md\",\"to\":\".spark-scenarios/file-ops/final/report.md\"}"
                        }
                    }]
                }
            }))
            .expect("serialize response"),
        )
        .expect("write response");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["retained_required_actions_missing"], json!([]));
    assert_eq!(
        summary["retained_required_actions_executed"][0]["tool"],
        "fs.rename"
    );
    assert_eq!(
        summary["retained_required_actions_executed"][0]["from"],
        ".spark-scenarios/file-ops/drafts/report-draft.md"
    );
    assert_eq!(
        summary["retained_required_actions_executed"][0]["to"],
        ".spark-scenarios/file-ops/final/report.md"
    );
}

#[test]
fn analyze_trace_reports_tool_only_turn_streaks() {
    let dir = tempfile::tempdir().expect("tempdir");
    for turn in 1..=8 {
        std::fs::write(
            dir.path().join(format!("{turn:03}-request-input.json")),
            serde_json::to_vec_pretty(&json!({
                "input": [{"role": "user", "content": [{"type": "input_text", "text": "demo"}]}]
            }))
            .expect("serialize request"),
        )
        .expect("write request");
        std::fs::write(
            dir.path().join(format!("{turn:03}-response.json")),
            serde_json::to_vec_pretty(&json!({
                "raw": {
                    "events": [{
                        "type": "response.output_item.done",
                        "output_index": 0,
                        "item": {
                            "type": "function_call",
                            "name": "fs_read",
                            "arguments": "{\"path\":\"src/main.rs\"}"
                        }
                    }]
                }
            }))
            .expect("serialize response"),
        )
        .expect("write response");
    }

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["tool_only_turns"]["count"], 8);
    assert_eq!(summary["tool_only_turns"]["max_consecutive"], 8);
    assert_eq!(
        summary["tool_only_turns"]["turns"],
        json!([1, 2, 3, 4, 5, 6, 7, 8])
    );
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "tool_only_turn_streak")
    );
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "completion_starvation")
    );
    assert!(
        format_trace_timeline(&summary)
            .contains("tool-only-turns: count=8 max_consecutive=8 turns=[1,2,3,4,5,6,7,8]")
    );
    assert!(
        format_trace_summary_row(".spark-runs/run-1", &summary)
            .contains("tool_only=8 max_tool_only_streak=8")
    );
}

#[test]
fn analyze_trace_reports_profile_scenario_skill_expectations() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_trace_metadata_with_expected_skills(
        dir.path(),
        json!(["rust-patterns", "missing-skill"]),
    );
    std::fs::write(
            dir.path().join("001-request-input.json"),
            serde_json::to_vec_pretty(&json!({
                "input": [
                    {
                        "role": "user",
                        "content": [{
                            "type": "input_text",
                            "text": "[spark skill loaded: rust-patterns]\nSpark skill: rust-patterns\n\nDescription: Demo"
                        }]
                    },
                    {
                        "role": "user",
                        "content": [{
                            "type": "input_text",
                            "text": "Profile scenario: skill-use. Apply @rust-patterns."
                        }]
                    }
                ]
            }))
            .expect("serialize request"),
        )
        .expect("write request");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["loaded_skill_contexts"], json!(["rust-patterns"]));
    assert_eq!(
        summary["profile_scenario_skill_expectations"]["total_skills"],
        2
    );
    assert_eq!(
        summary["profile_scenario_skill_expectations"]["satisfied_skills"],
        1
    );
    assert_eq!(
        summary["profile_scenario_skill_expectations"]["missing_skills"],
        json!(["missing-skill"])
    );
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "profile_scenario_expected_skills_missing")
    );
    assert!(format_trace_timeline(&summary).contains("skills=[rust-patterns]"));
    assert!(format_trace_timeline(&summary).contains("scenario-skills: satisfied=1/2 missing=1"));
    assert!(
        format_trace_summary_row(".spark-runs/run-1", &summary).contains("scenario_skills=1/2")
    );
}

#[test]
fn analyze_trace_reports_detour_before_retained_required_action() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_request_with_required_action(
        dir.path(),
        "action_1=tool=fs.list path=src recursive=false",
    );
    std::fs::write(
        dir.path().join("001-response.json"),
        serde_json::to_vec_pretty(&json!({
            "raw": {
                "events": [
                    {
                        "type": "response.output_item.done",
                        "output_index": 0,
                        "item": {
                            "type": "function_call",
                            "name": "fs_list",
                            "arguments": "{\"path\":\".\",\"recursive\":false}"
                        }
                    },
                    {
                        "type": "response.output_item.done",
                        "output_index": 1,
                        "item": {
                            "type": "function_call",
                            "name": "fs_list",
                            "arguments": "{\"path\":\"src\",\"recursive\":false}"
                        }
                    }
                ]
            }
        }))
        .expect("serialize response"),
    )
    .expect("write response");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["retained_required_actions_missing"], json!([]));
    assert_eq!(summary["tool_calls_before_first_required_action"], 1);
    assert!(
        summary["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "retained_required_action_detour")
    );
}
