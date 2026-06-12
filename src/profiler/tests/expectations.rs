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

#[test]
fn analyze_trace_allows_optional_calls_after_expected_calls_satisfied() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_trace_metadata_with_expected_and_optional_tool_calls(
        dir.path(),
        json!([
            {
                "tool": "fs.read",
                "path": "src/quote.ts"
            },
            {
                "tool": "cmd.exec",
                "command": "bun test"
            }
        ]),
        json!([
            {
                "tool": "fs.read",
                "path": "src/quote.ts"
            }
        ]),
    );
    for (turn, (tool_name, arguments)) in [
        ("fs_read", "{\"path\":\"src/quote.ts\"}"),
        ("cmd_exec", "{\"command\":\"bun test\"}"),
        ("fs_read", "{\"path\":\"src/quote.ts\"}"),
    ]
    .into_iter()
    .enumerate()
    {
        let turn = turn + 1;
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
        summary["profile_scenario_call_expectations"]["optional_calls_satisfied"],
        1
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["extra_calls_after_satisfied"],
        0
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["extra_tool_calls"],
        json!([])
    );
    assert!(
        !summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| {
                diagnostic["kind"] == "profile_scenario_extra_calls_after_expected"
            })
    );
    assert!(format_trace_summary_row(".spark-runs/run-1", &summary).contains("scenario_calls=2/2"));
    assert!(!format_trace_summary_row(".spark-runs/run-1", &summary).contains("extra_calls="));
}

#[test]
fn analyze_trace_consumes_expected_calls_in_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_trace_metadata_with_expected_tool_calls(
        dir.path(),
        json!([
            {
                "tool": "fs.read",
                "path": "config/app.json"
            },
            {
                "tool": "fs.write",
                "path": "config/app.json"
            },
            {
                "tool": "fs.read",
                "path": "config/app.json"
            }
        ]),
    );
    write_function_call_trace(
        dir.path(),
        1,
        "call_read",
        "fs_read",
        "{\"path\":\"config/app.json\"}",
    );
    write_tool_result_trace(
        dir.path(),
        1,
        "call_read",
        "fs.read",
        json!({"path": "config/app.json"}),
        true,
    );
    write_function_call_trace(
        dir.path(),
        2,
        "call_write",
        "fs_write",
        "{\"path\":\"config/app.json\",\"content\":\"updated\"}",
    );
    write_tool_result_trace(
        dir.path(),
        2,
        "call_write",
        "fs.write",
        json!({"path": "config/app.json"}),
        true,
    );

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(
        summary["profile_scenario_call_expectations"]["satisfied_calls"],
        2
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["missing_calls"]
            .as_array()
            .expect("missing calls")
            .len(),
        1
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["missing_calls"][0]["tool"],
        "fs.read"
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["missing_calls"][0]["path"],
        "config/app.json"
    );
}

#[test]
fn analyze_trace_allows_same_turn_parallel_calls_to_satisfy_ordered_expectations() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_trace_metadata_with_expected_tool_calls(
        dir.path(),
        json!([
            {
                "tool": "fs.read",
                "path": ".spark-scenarios/github-issue-triage/issue.md"
            },
            {
                "tool": "fs.read",
                "path": ".spark-scenarios/github-issue-triage/src/cachePolicy.ts"
            },
            {
                "tool": "fs.read",
                "path": ".spark-scenarios/github-issue-triage/logs/warehouse-import.log"
            }
        ]),
    );
    write_function_call_trace(
        dir.path(),
        1,
        "call_issue",
        "fs_read",
        "{\"path\":\".spark-scenarios/github-issue-triage/issue.md\"}",
    );
    write_tool_result_trace(
        dir.path(),
        1,
        "call_issue",
        "fs.read",
        json!({"path": ".spark-scenarios/github-issue-triage/issue.md"}),
        true,
    );
    std::fs::write(
        dir.path().join("002-response.json"),
        serde_json::to_vec_pretty(&json!({
            "raw": {
                "response": {
                    "output": [
                        {
                            "type": "function_call",
                            "call_id": "call_log",
                            "name": "fs_read",
                            "arguments": "{\"path\":\".spark-scenarios/github-issue-triage/logs/warehouse-import.log\"}"
                        },
                        {
                            "type": "function_call",
                            "call_id": "call_cache",
                            "name": "fs_read",
                            "arguments": "{\"path\":\".spark-scenarios/github-issue-triage/src/cachePolicy.ts\"}"
                        }
                    ]
                }
            }
        }))
        .expect("serialize response"),
    )
    .expect("write response");
    std::fs::write(
        dir.path().join("002-tool-result.json"),
        serde_json::to_vec_pretty(&json!({
            "tool": "fs.read",
            "call_id": "call_log",
            "args": {"path": ".spark-scenarios/github-issue-triage/logs/warehouse-import.log"},
            "duration_ms": 1,
            "result": {"ok": true, "error": null, "data": {}}
        }))
        .expect("serialize log result"),
    )
    .expect("write log result");
    std::fs::write(
        dir.path().join("002-tool-result-002.json"),
        serde_json::to_vec_pretty(&json!({
            "tool": "fs.read",
            "call_id": "call_cache",
            "args": {"path": ".spark-scenarios/github-issue-triage/src/cachePolicy.ts"},
            "duration_ms": 1,
            "result": {"ok": true, "error": null, "data": {}}
        }))
        .expect("serialize cache result"),
    )
    .expect("write cache result");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(
        summary["profile_scenario_call_expectations"]["satisfied_calls"],
        3
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["missing_calls"],
        json!([])
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["extra_calls_after_satisfied"],
        0
    );
}

#[test]
fn analyze_trace_satisfies_expected_calls_on_matching_result_status() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_trace_metadata_with_expected_tool_calls(
        dir.path(),
        json!([{
            "tool": "cmd.exec",
            "command": "bun test"
        }]),
    );

    write_function_call_trace(
        dir.path(),
        1,
        "call_failed",
        "cmd_exec",
        "{\"command\":\"bun test\",\"workdir\":\"C:\\\\bad\"}",
    );
    write_tool_result_trace(
        dir.path(),
        1,
        "call_failed",
        "cmd.exec",
        json!({"command": "bun test", "workdir": "C:\\bad"}),
        false,
    );
    write_function_call_trace(
        dir.path(),
        2,
        "call_install",
        "cmd_exec",
        "{\"command\":\"bun install\",\"workdir\":\".spark-scenarios/react-calculator\"}",
    );
    write_tool_result_trace(
        dir.path(),
        2,
        "call_install",
        "cmd.exec",
        json!({"command": "bun install", "workdir": ".spark-scenarios/react-calculator"}),
        true,
    );
    write_function_call_trace(
        dir.path(),
        3,
        "call_ok",
        "cmd_exec",
        "{\"command\":\"bun test\",\"workdir\":\".spark-scenarios/react-calculator\"}",
    );
    write_tool_result_trace(
        dir.path(),
        3,
        "call_ok",
        "cmd.exec",
        json!({"command": "bun test", "workdir": ".spark-scenarios/react-calculator"}),
        true,
    );

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(
        summary["profile_scenario_call_expectations"]["satisfied_calls"],
        1
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["first_satisfied_call_index"],
        2
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["first_satisfied_turn"],
        3
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["extra_calls_after_satisfied"],
        0
    );
}

#[test]
fn analyze_trace_satisfies_expected_call_with_alternative_tools() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_trace_metadata_with_expected_tool_calls(
        dir.path(),
        json!([{
            "tools": ["fs.edit", "fs.replace", "fs.write"],
            "path": ".spark-scenarios/multi-file-patch/src/routes.ts"
        }]),
    );

    write_function_call_trace(
        dir.path(),
        1,
        "call_write",
        "fs_write",
        "{\"path\":\".spark-scenarios/multi-file-patch/src/routes.ts\",\"content\":\"updated\"}",
    );
    write_tool_result_trace(
        dir.path(),
        1,
        "call_write",
        "fs.write",
        json!({"path": ".spark-scenarios/multi-file-patch/src/routes.ts"}),
        true,
    );

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(
        summary["profile_scenario_call_expectations"]["satisfied_calls"],
        1
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["missing_calls"],
        json!([])
    );
}

#[test]
fn analyze_trace_allows_expected_failing_probe_when_declared() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_trace_metadata_with_expected_tool_calls(
        dir.path(),
        json!([{
            "tool": "fs.read",
            "path": ".spark-scenarios/tool-recovery/source/missing-note.md",
            "ok": false
        }]),
    );

    write_function_call_trace(
        dir.path(),
        1,
        "call_missing",
        "fs_read",
        "{\"path\":\".spark-scenarios/tool-recovery/source/missing-note.md\"}",
    );
    write_tool_result_trace(
        dir.path(),
        1,
        "call_missing",
        "fs.read",
        json!({"path": ".spark-scenarios/tool-recovery/source/missing-note.md"}),
        false,
    );

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(
        summary["profile_scenario_call_expectations"]["satisfied_calls"],
        1
    );
    assert_eq!(
        summary["profile_scenario_call_expectations"]["missing_calls"],
        json!([])
    );
}

fn write_function_call_trace(
    dir: &std::path::Path,
    turn: usize,
    call_id: &str,
    name: &str,
    arguments: &str,
) {
    std::fs::write(
        dir.join(format!("{turn:03}-response.json")),
        serde_json::to_vec_pretty(&json!({
            "raw": {
                "events": [{
                    "type": "response.output_item.done",
                    "output_index": 0,
                    "item": {
                        "type": "function_call",
                        "call_id": call_id,
                        "name": name,
                        "arguments": arguments
                    }
                }]
            }
        }))
        .expect("serialize response"),
    )
    .expect("write response");
}

fn write_tool_result_trace(
    dir: &std::path::Path,
    turn: usize,
    call_id: &str,
    tool: &str,
    args: serde_json::Value,
    ok: bool,
) {
    std::fs::write(
        dir.join(format!("{turn:03}-tool-result.json")),
        serde_json::to_vec_pretty(&json!({
            "tool": tool,
            "call_id": call_id,
            "args": args,
            "duration_ms": 1,
            "result": {
                "ok": ok,
                "error": if ok { serde_json::Value::Null } else { json!("failed") },
                "data": {}
            }
        }))
        .expect("serialize tool result"),
    )
    .expect("write tool result");
}
