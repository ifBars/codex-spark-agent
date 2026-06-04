use super::*;

#[test]
fn analyze_trace_reports_profile_scenario_tool_expectations() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_trace_metadata_with_expected_tools(
        dir.path(),
        json!([["fs.write"], ["fs.rename"], ["fs.read"], ["fs.search"]]),
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
                            "name": "fs_write",
                            "arguments": "{\"path\":\"draft.md\",\"content\":\"hello\"}"
                        }
                    },
                    {
                        "type": "response.output_item.done",
                        "output_index": 1,
                        "item": {
                            "type": "function_call",
                            "name": "fs_rename",
                            "arguments": "{\"from\":\"draft.md\",\"to\":\"final.md\"}"
                        }
                    }
                ]
            }
        }))
        .expect("serialize response"),
    )
    .expect("write response");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(
        summary["profile_scenario_tool_expectations"]["total_groups"],
        4
    );
    assert_eq!(
        summary["profile_scenario_tool_expectations"]["satisfied_groups"],
        2
    );
    assert_eq!(
        summary["profile_scenario_tool_expectations"]["missing_groups"],
        json!([["fs.read"], ["fs.search"]])
    );
    assert!(
        summary["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "profile_scenario_expected_tools_missing")
    );
    assert!(format_trace_timeline(&summary).contains(
        "scenario-tools: satisfied=2/4 missing=2 groups=[fs.write,fs.rename,fs.read,fs.search]"
    ));
    assert!(format_trace_summary_row(".spark-runs/run-1", &summary).contains("scenario_tools=2/4"));
}

#[test]
fn analyze_trace_reports_missing_profile_scenario_exact_tool_calls() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_trace_metadata_with_expected_tool_calls(
        dir.path(),
        json!([
            {
                "tool": "fs.rename",
                "from": ".spark-scenarios/file-ops/drafts/report-draft.md",
                "to": ".spark-scenarios/file-ops/final/report.md"
            },
            {
                "tool": "fs.read",
                "path": ".spark-scenarios/file-ops/final/report.md"
            }
        ]),
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
                                "name": "fs_rename",
                                "arguments": "{\"from\":\".spark-scenarios/file-ops/drafts/report-draft.md\",\"to\":\".spark-scenarios/file-sops/final/report.md\"}"
                            }
                        },
                        {
                            "type": "response.output_item.done",
                            "output_index": 1,
                            "item": {
                                "type": "function_call",
                                "name": "fs_read",
                                "arguments": "{\"path\":\".spark-scenarios/file-ops/final/report.md\"}"
                            }
                        }
                    ]
                }
            }))
            .expect("serialize response"),
        )
        .expect("write response");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(
        summary["profile_scenario_call_expectations"]["total_calls"],
        2
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["satisfied_calls"],
        1
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["missing_calls"][0]["tool"],
        "fs.rename"
    );
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "profile_scenario_expected_calls_missing")
    );
    assert!(format_trace_timeline(&summary).contains("scenario-calls: satisfied=1/2 missing=1"));
    assert!(format_trace_summary_row(".spark-runs/run-1", &summary).contains("scenario_calls=1/2"));
}

#[test]
fn analyze_trace_reports_extra_calls_after_expected_calls_satisfied() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_trace_metadata_with_expected_tool_calls(
        dir.path(),
        json!([
            {
                "tool": "fs.read",
                "path": "src/main.rs"
            },
            {
                "tool": "fs.search",
                "path": "src"
            }
        ]),
    );
    for (turn, (tool_name, arguments)) in [
        ("fs_read", "{\"path\":\"src/main.rs\"}"),
        (
            "fs_search",
            "{\"path\":\"src\",\"query\":\"load_skill_mentions\"}",
        ),
        ("fs_read", "{\"path\":\"src/skills.rs\"}"),
    ]
    .into_iter()
    .enumerate()
    {
        let turn = turn + 1;
        std::fs::write(
            dir.path().join(format!("{turn:03}-request-input.json")),
            serde_json::to_vec_pretty(&json!({
                "input": [{
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": "x".repeat(turn * 100)
                    }]
                }]
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
                            "name": tool_name,
                            "arguments": arguments
                        }
                    }]
                }
            }))
            .expect("serialize response"),
        )
        .expect("write response");
    }

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(
        summary["profile_scenario_call_expectations"]["satisfied_calls"],
        2
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["first_satisfied_call_index"],
        1
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["first_satisfied_turn"],
        2
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["final_tool_call_turn"],
        3
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["extra_calls_after_satisfied"],
        1
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["extra_turns_after_satisfied"],
        1
    );
    assert!(
        summary["profile_scenario_call_expectations"]["context_growth_after_satisfied_chars"]
            .as_u64()
            .expect("context growth")
            > 0
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["extra_tool_calls"][0]["turn"],
        3
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["extra_tool_calls"][0]["tool"],
        "fs.read"
    );
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| {
                diagnostic["kind"] == "profile_scenario_extra_calls_after_expected"
            })
    );
    assert!(format_trace_timeline(&summary).contains("extra_after=1 extra_turns=1"));
    assert!(
        format_trace_summary_row(".spark-runs/run-1", &summary)
            .contains("scenario_calls=2/2 extra_calls=1 extra_turns=1")
    );
}
