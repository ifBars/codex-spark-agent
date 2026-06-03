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

#[test]
fn tool_signature_is_stable_for_object_key_order() {
    let left = json!({"path": "a.txt", "offset": 1, "limit": 5});
    let right = json!({"limit": 5, "offset": 1, "path": "a.txt"});

    assert_eq!(
        tool_signature("fs.read", &left),
        tool_signature("fs.read", &right)
    );
}

#[test]
fn profiler_records_repeated_and_consecutive_tool_calls() {
    let args = json!({"path": "a.txt", "offset": 1, "limit": 5});
    let mut profiler = AgentProfiler::default();

    profiler.record_tool_call(1, "fs.read", &args);
    profiler.record_tool_call(2, "fs.read", &args);

    let summary = profiler.to_json();
    assert_eq!(summary["tool_calls"], 2);
    assert_eq!(summary["repeated_tool_calls"], 1);
    assert_eq!(summary["consecutive_duplicate_tool_calls"], 1);
    assert_eq!(summary["tool_counts"]["fs.read"], 2);
    assert_eq!(summary["recent_signals"][0]["kind"], "repeated_tool_call");
    assert_eq!(
        summary["recent_signals"][1]["kind"],
        "consecutive_duplicate_tool_call"
    );
}

#[test]
fn profiler_records_tool_only_turn_streaks() {
    let mut profiler = AgentProfiler::default();

    profiler.record_turn_activity(1, true, 0);
    profiler.record_turn_activity(2, true, 0);
    profiler.record_turn_activity(3, true, 0);
    profiler.record_turn_activity(4, false, 12);

    let summary = profiler.to_json();

    assert_eq!(summary["tool_only_turn_count"], 3);
    assert_eq!(summary["tool_only_turns"]["count"], 3);
    assert_eq!(summary["tool_only_turns"]["max_consecutive"], 3);
    assert_eq!(summary["tool_only_turns"]["turns"], json!([1, 2, 3]));
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "tool_only_turn_streak")
    );
    assert!(profiler.status_line().contains("tool_only_turns=3"));
}

#[test]
fn profiler_diagnoses_completion_starvation() {
    let mut profiler = AgentProfiler::default();

    for turn in 1..=8 {
        profiler.record_turn_activity(turn, true, 0);
    }

    let summary = profiler.to_json();
    let diagnostic = summary["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .find(|diagnostic| diagnostic["kind"] == "completion_starvation")
        .expect("completion starvation diagnostic");

    assert_eq!(diagnostic["level"], "warning");
    assert_eq!(diagnostic["tool_only_turns"], 8);
    assert_eq!(diagnostic["max_consecutive"], 8);
}

#[test]
fn profiler_records_readonly_cache_hits() {
    let args = json!({"path": "a.txt"});
    let mut profiler = AgentProfiler::default();

    profiler.record_readonly_tool_cache_hit(3, "fs.read", &args);

    let summary = profiler.to_json();
    assert_eq!(summary["readonly_tool_cache_hits"], 1);
    assert_eq!(
        summary["recent_signals"][0]["kind"],
        "readonly_tool_cache_hit"
    );
}

#[test]
fn profiler_records_tool_failures() {
    let mut profiler = AgentProfiler::default();

    profiler.record_tool_result(
        1,
        "cmd.exec",
        false,
        &json!({"code": 1}),
        128,
        250,
        Some("command exited with code 1"),
    );

    let summary = profiler.to_json();

    assert_eq!(summary["tool_results"], 1);
    assert_eq!(summary["tool_failures"], 1);
    assert_eq!(summary["tool_failure_counts"]["cmd.exec"], 1);
    assert_eq!(summary["recent_signals"][0]["kind"], "tool_failure");
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "tool_failures")
    );
}

#[test]
fn profiler_records_tool_result_truncation() {
    let mut profiler = AgentProfiler::default();

    profiler.record_tool_result(
        1,
        "cmd.exec",
        true,
        &json!({
            "stdout_truncated": true,
            "stderr_truncated": false,
            "stdout_chars": 40_000,
            "stderr_chars": 0
        }),
        24_512,
        400,
        None,
    );

    let summary = profiler.to_json();

    assert_eq!(summary["truncated_tool_results"], 1);
    assert_eq!(summary["tool_truncation_counts"]["cmd.exec"], 1);
    assert_eq!(
        summary["recent_signals"][0]["kind"],
        "tool_result_truncated"
    );
    assert_eq!(
        summary["recent_signals"][0]["truncation"]["stdout_truncated"],
        true
    );
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "tool_result_truncation")
    );
}

#[test]
fn profiler_records_parent_dirs_created_by_mutation_tools() {
    let mut profiler = AgentProfiler::default();

    profiler.record_tool_result(
        3,
        "fs.rename",
        true,
        &json!({"created_parent_dirs": ["nested", "nested/final"]}),
        128,
        1,
        None,
    );
    let summary = profiler.to_json();

    assert_eq!(summary["mutation_parent_dir_creations"], 2);
    assert_eq!(
        summary["mutation_parent_dir_creation_counts"]["fs.rename"],
        2
    );
    assert_eq!(
        summary["recent_signals"][0]["kind"],
        "mutation_created_parent_dirs"
    );
    assert_eq!(
        summary["recent_signals"][0]["created_parent_dirs"],
        json!(["nested", "nested/final"])
    );
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "mutation_created_parent_dirs")
    );
}

#[test]
fn profiler_records_tool_duration_and_slow_diagnostics() {
    let mut profiler = AgentProfiler::default();

    profiler.record_tool_result(1, "cmd.exec", true, &json!({}), 64, 12_345, None);

    let summary = profiler.to_json();

    assert_eq!(summary["total_tool_duration_ms"], 12_345);
    assert_eq!(summary["max_tool_duration_ms"], 12_345);
    assert_eq!(summary["average_tool_duration_ms"], 12_345);
    assert_eq!(summary["tool_duration_ms_by_tool"]["cmd.exec"], 12_345);
    assert_eq!(summary["max_tool_duration_ms_by_tool"]["cmd.exec"], 12_345);
    assert_eq!(summary["recent_signals"][0]["kind"], "slow_tool_result");
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "slow_tool_results")
    );
}

#[test]
fn profiler_records_request_duration_and_slow_diagnostics() {
    let mut profiler = AgentProfiler::default();

    profiler.record_request(120);
    profiler.record_request_duration(1, 31_000);

    let summary = profiler.to_json();

    assert_eq!(summary["total_request_duration_ms"], 31_000);
    assert_eq!(summary["max_request_duration_ms"], 31_000);
    assert_eq!(summary["average_request_duration_ms"], 31_000);
    assert_eq!(summary["request_duration_ms_by_request"], json!([31_000]));
    assert_eq!(summary["recent_signals"][0]["kind"], "slow_spark_request");
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "slow_spark_requests")
    );
}

#[test]
fn profiler_records_input_size_sequence_and_errors() {
    let mut profiler = AgentProfiler::default();

    profiler.record_request(120);
    profiler.record_request(240);
    profiler.record_error(2, "response", "stream ended without response.completed");

    let summary = profiler.to_json();
    assert_eq!(summary["requests"], 2);
    assert_eq!(summary["max_input_chars"], 240);
    assert_eq!(summary["input_chars_by_request"], json!([120, 240]));
    assert_eq!(summary["approx_context_window_tokens"], 128_000);
    assert_eq!(summary["max_approx_input_tokens"], 60);
    assert_eq!(summary["approx_input_tokens_by_request"], json!([30, 60]));
    assert_eq!(summary["errors"][0]["turn"], 2);
    assert_eq!(summary["recent_signals"][0]["kind"], "error");
    assert_eq!(summary["diagnostics"][0]["kind"], "request_failure");
}

#[test]
fn profiler_diagnoses_duplicate_tool_loops_and_input_pressure() {
    let args = json!({"path": "a.txt"});
    let mut profiler = AgentProfiler::default();

    profiler.record_request(470_000);
    profiler.record_tool_call(1, "fs.read", &args);
    profiler.record_tool_call(2, "fs.read", &args);

    let diagnostics = profiler
        .to_json()
        .get("diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .expect("diagnostics");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "consecutive_duplicate_tool_calls")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "near_input_guard")
    );
}

#[test]
fn profiler_diagnoses_near_context_window_before_input_guard() {
    let mut profiler = AgentProfiler::default();

    profiler.record_request(400_000);

    let diagnostics = profiler
        .to_json()
        .get("diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .expect("diagnostics");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["kind"] == "near_context_window"
            && diagnostic["max_approx_input_tokens"] == 100_000
            && diagnostic["context_window_tokens"] == 128_000
    }));
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "near_input_guard")
    );
}

#[test]
fn profiler_diagnoses_weak_compaction() {
    let mut profiler = AgentProfiler::default();

    profiler.record_compaction(&json!({
        "method": "responses_compact",
        "before_chars": 100_000,
        "after_chars": 75_000
    }));

    let summary = profiler.to_json();
    assert_eq!(summary["remote_compactions"], 1);
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "weak_compaction_shrink")
    );
}

#[test]
fn profiler_diagnoses_compaction_expansion() {
    let mut profiler = AgentProfiler::default();

    profiler.record_compaction(&json!({
        "method": "responses_compact",
        "forced": true,
        "duration_ms": 31_000,
        "before_chars": 200,
        "after_chars": 1200
    }));

    let diagnostics = profiler
        .to_json()
        .get("diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .expect("diagnostics");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["kind"] == "compaction_expanded_context"
            && diagnostic["forced"] == true
            && diagnostic["method"] == "responses_compact"
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["kind"] == "slow_compaction"
            && diagnostic["duration_ms"] == 31_000
            && diagnostic["forced"] == true
    }));
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "weak_compaction_shrink")
    );
}

#[test]
fn profiler_summarizes_compaction_reports_without_raw_payload() {
    let mut profiler = AgentProfiler::default();

    profiler.record_compaction(&json!({
        "method": "responses_compact",
        "forced": true,
        "duration_ms": 1234,
        "before_chars": 200,
        "after_chars": 1200,
        "raw": {
            "id": "resp_123",
            "object": "response.compaction",
            "created_at": 12345,
            "usage": {"total_tokens": 42},
            "output": [
                {
                    "type": "compaction_summary",
                    "encrypted_content": "very-secret-large-payload"
                }
            ]
        }
    }));

    let summary = profiler.to_json();
    let report = &summary["compaction_reports"][0];

    assert!(report.get("raw").is_none());
    assert_eq!(report["method"], "responses_compact");
    assert_eq!(report["forced"], true);
    assert_eq!(report["duration_ms"], 1234);
    assert_eq!(report["raw_summary"]["id"], "resp_123");
    assert_eq!(report["raw_summary"]["output_items"], 1);
    assert_eq!(
        report["raw_summary"]["output_types"],
        json!(["compaction_summary"])
    );
    assert_eq!(report["raw_summary"]["usage"]["total_tokens"], 42);
}

#[test]
fn profiler_reports_remote_compaction_local_pressure() {
    let mut profiler = AgentProfiler::default();

    profiler.record_compaction(&json!({
        "method": "responses_compact",
        "before_chars": 220_000,
        "after_chars": 100_000,
        "local_pressure": {
            "reason": "remote_compaction_above_threshold",
            "remote_after_chars": 190_000,
            "final_chars": 100_000,
            "made_progress": true
        }
    }));

    let diagnostics = profiler
        .to_json()
        .get("diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .expect("diagnostics");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["kind"] == "remote_compaction_local_pressure"
            && diagnostic["remote_after_chars"] == 190_000
            && diagnostic["final_chars"] == 100_000
            && diagnostic["made_progress"] == true
    }));
}

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

#[test]
fn trace_file_sort_preserves_repeated_entry_sequence() {
    let first = Path::new("001-tool-result.json");
    let second = Path::new("001-tool-result-002.json");

    assert!(trace_file_sort_key(first) < trace_file_sort_key(second));
}

#[test]
fn formats_trace_timeline_for_human_scan() {
    let summary = json!({
        "trace_metadata": {
            "model": "gpt-5.3-codex-spark",
            "context": {"profile_scenario": {"name": "compaction-pressure"}},
            "max_turns": null,
            "compact_after_chars": 160000,
            "max_input_chars": 500000
        },
        "diagnostics": [{"kind": "tool_failures"}],
        "retained_required_actions": [{"tool": "fs.list", "path": "src", "recursive": false}],
        "retained_required_actions_executed": [{"tool": "fs.list", "path": "src", "recursive": false}],
        "retained_required_actions_missing": [],
        "tool_calls_before_first_required_action": 0,
        "timeline": [{
            "turn": 1,
            "request_input_chars": 120000,
            "request_approx_tokens": 30000,
            "context_window_pct": 23.4375,
            "request_duration_ms": 1234,
            "response_text_chars": 42,
            "tool_calls": [{"tool": "fs.read", "signature": "fs.read:{\"path\":\"a.txt\"}"}],
            "tool_results": [{
                "tool": "fs.read",
                "ok": true,
                "duration_ms": 9,
                "output_chars": 512,
                "cached_observation": true,
                "truncated": false,
                "timed_out": true,
                "created_parent_dirs": ["nested"]
            }],
            "compactions": [{
                "method": "responses_compact",
                "trigger": "tool_only_streak",
                "before_chars": 200000,
                "after_chars": 90000,
                "remote_after_chars": 210000,
                "remote_retained_pct": 105.0,
                "local_pressure_final_chars": 90000
            }],
            "errors": [{"stage": "response", "error": "stream ended without response.completed"}]
        }]
    });

    let output = format_trace_timeline(&summary);

    assert!(output.contains("trace model=gpt-5.3-codex-spark scenario=compaction-pressure"));
    assert!(output.contains("diagnostics: tool_failures"));
    assert!(output.contains("required-actions: total=1 executed=1 missing=0 detours_before_first=0 actions=[tool=fs.list path=src recursive=false]"));
    assert!(output.contains("turn 1: input=120000 chars (~30000 tok, 23.4%)"));
    assert!(output.contains("calls=[fs.read]"));
    assert!(output.contains("results=[fs.read:ok 9ms 512 chars cached+timeout parents=nested]"));
    assert!(output.contains(
            "compactions=[responses_compact 200000->90000 trigger=tool_only_streak remote=210000 105.0% local_pressure=210000->90000]"
        ));
    assert!(output.contains("errors=[response:stream ended without response.completed]"));
}

#[test]
fn compaction_summary_reports_remote_replay_pressure_metrics() {
    let summary = summarize_compaction_report(&json!({
        "method": "responses_compact",
        "trigger": "tool_only_streak",
        "before_chars": 181900,
        "after_chars": 5430,
        "local_pressure": {
            "remote_after_chars": 183238,
            "final_chars": 5430,
            "made_progress": true
        }
    }));

    assert_eq!(summary["trigger"], "tool_only_streak");
    assert_eq!(summary["remote_after_chars"], 183238);
    assert_eq!(summary["local_pressure_final_chars"], 5430);
    assert!((summary["remote_retained_pct"].as_f64().unwrap() - 100.73556899395272).abs() < 0.001);
    assert!((summary["final_retained_pct"].as_f64().unwrap() - 2.9851566794942275).abs() < 0.001);
    assert!(
        (summary["local_pressure_reduction_pct"].as_f64().unwrap() - 97.03664087143497).abs()
            < 0.001
    );
}

#[test]
fn formats_trace_summary_row_for_run_comparison() {
    let summary = json!({
        "trace_metadata": {
            "model": "gpt-5.3-codex-spark",
            "context": {"profile_scenario": {"name": "repo-survey"}}
        },
        "requests": 3,
        "max_approx_input_tokens": 42000,
        "max_context_window_pct": 32.8125,
        "max_request_duration_ms": 12345,
        "tool_calls": 7,
        "tool_failures": 1,
        "compactions": 2,
        "remote_compactions": 1,
        "fallback_compactions": 1,
        "compaction_reports": [{"local_pressure": {"made_progress": true}}],
        "diagnostics": [{"kind": "tool_failures"}, {"kind": "weak_compaction_shrink"}]
    });

    let row = format_trace_summary_row(".spark-runs/run-1", &summary);

    assert!(row.contains(".spark-runs/run-1 | model=gpt-5.3-codex-spark scenario=repo-survey"));
    assert!(row.contains("requests=3"));
    assert!(row.contains("max_tokens=42000 (32.8%)"));
    assert!(row.contains("tools=7 failures=1"));
    assert!(row.contains("compactions=2 remote=1 fallback=1 local_pressure=1"));
    assert!(row.contains("diagnostics=tool_failures,weak_compaction_shrink"));
}

#[test]
fn extracts_profile_scenario_name_from_trace_summary() {
    let summary = json!({
        "trace_metadata": {
            "context": {"profile_scenario": {"name": "compaction-pressure"}}
        }
    });

    assert_eq!(
        trace_profile_scenario_name(&summary),
        Some("compaction-pressure")
    );
}

#[test]
fn formats_trace_aggregate_row_for_run_comparison() {
    let summaries = vec![
        json!({
            "errors": [],
            "max_approx_input_tokens": 42000,
            "max_context_window_pct": 32.8125,
            "max_request_duration_ms": 1234,
            "tool_calls": 2,
            "tool_failures": 1,
            "tool_failure_recovery": {
                "failed_tool_results": 1,
                "recovered_failures": 1,
                "unrecovered_failures": 0
            },
            "profile_scenario_tool_expectations": {
                "total_groups": 3,
                "satisfied_groups": 3
            },
            "profile_scenario_call_expectations": {
                "total_calls": 4,
                "satisfied_calls": 4,
                "extra_calls_after_satisfied": 2,
                "extra_turns_after_satisfied": 1,
                "context_growth_after_satisfied_chars": 12000
            },
            "compactions": 1,
            "remote_compactions": 1,
            "fallback_compactions": 0,
            "compaction_reports": [{"local_pressure": {"made_progress": true}}],
            "diagnostics": [{"kind": "remote_compaction_local_pressure"}]
        }),
        json!({
            "errors": [{"stage": "response", "error": "stream ended"}],
            "max_approx_input_tokens": 45000,
            "max_context_window_pct": 35.15625,
            "max_request_duration_ms": 0,
            "tool_calls": 0,
            "tool_failures": 1,
            "tool_failure_recovery": {
                "failed_tool_results": 1,
                "recovered_failures": 0,
                "unrecovered_failures": 1
            },
            "profile_scenario_tool_expectations": {
                "total_groups": 3,
                "satisfied_groups": 2
            },
            "profile_scenario_call_expectations": {
                "total_calls": 4,
                "satisfied_calls": 3,
                "extra_calls_after_satisfied": 5,
                "extra_turns_after_satisfied": 3,
                "context_growth_after_satisfied_chars": 34000
            },
            "compactions": 1,
            "remote_compactions": 1,
            "fallback_compactions": 0,
            "compaction_reports": [{"local_pressure": {"made_progress": false}}],
            "diagnostics": [
                {"kind": "request_failure"},
                {"kind": "remote_compaction_local_pressure"}
            ]
        }),
    ];

    let row = format_trace_aggregate_row("compaction-pressure", &summaries);

    assert!(row.contains("compaction-pressure aggregate | runs=2 success=1 failure=1"));
    assert!(row.contains("max_tokens=45000 (35.2%)"));
    assert!(row.contains("tools=2 failures=2 recoveries=1/2"));
    assert!(row.contains("scenario_tools=5/6 scenario_calls=7/8"));
    assert!(row.contains("scenario_overrun_calls=7 scenario_overrun_turns=4"));
    assert!(row.contains("max_overrun_turns=3 scenario_overrun_context=46000"));
    assert!(row.contains("compactions=2 remote=2 fallback=0 local_pressure=2"));
    assert!(row.contains("diagnostics=remote_compaction_local_pressure:2,request_failure:1"));
}

#[test]
fn formats_trace_aggregate_json_for_batch_analysis() {
    let summaries = vec![
        json!({
            "errors": [],
            "max_approx_input_tokens": 45_000,
            "max_context_window_pct": 35.2,
            "max_request_duration_ms": 1200,
            "tool_calls": 4,
            "tool_failures": 1,
            "compactions": 1,
            "remote_compactions": 1,
            "fallback_compactions": 0,
            "tool_failure_recovery": {
                "failed_tool_results": 1,
                "recovered_failures": 1
            },
            "profile_scenario_tool_expectations": {
                "satisfied_groups": 3,
                "total_groups": 4
            },
            "profile_scenario_call_expectations": {
                "satisfied_calls": 4,
                "total_calls": 5,
                "extra_calls_after_satisfied": 3,
                "extra_turns_after_satisfied": 2,
                "context_growth_after_satisfied_chars": 8000
            },
            "diagnostics": [{"kind": "tool_failure_recovered"}],
            "compaction_reports": [{"local_pressure": {"after_chars": 1000}}],
            "tool_only_turns": {
                "count": 2,
                "max_consecutive": 2,
                "turns": [1, 2]
            }
        }),
        json!({
            "errors": [{"stage": "response"}],
            "max_approx_input_tokens": 50_000,
            "max_context_window_pct": 39.1,
            "max_request_duration_ms": 2400,
            "tool_calls": 2,
            "tool_failures": 0,
            "compactions": 0,
            "remote_compactions": 0,
            "fallback_compactions": 0,
            "profile_scenario_tool_expectations": {
                "satisfied_groups": 2,
                "total_groups": 4
            },
            "profile_scenario_call_expectations": {
                "satisfied_calls": 3,
                "total_calls": 5,
                "extra_calls_after_satisfied": 4,
                "extra_turns_after_satisfied": 1,
                "context_growth_after_satisfied_chars": 3000
            },
            "tool_only_turns": {
                "count": 1,
                "max_consecutive": 1,
                "turns": [1]
            },
            "diagnostics": [{"kind": "request_failure"}]
        }),
    ];

    let aggregate = trace_aggregate_json("tool-recovery", &summaries);

    assert_eq!(aggregate["label"], "tool-recovery");
    assert_eq!(aggregate["runs"], 2);
    assert_eq!(aggregate["success"], 1);
    assert_eq!(aggregate["failure"], 1);
    assert_eq!(aggregate["max_approx_input_tokens"], 50_000);
    assert_eq!(aggregate["max_request_duration_ms"], 2400);
    assert_eq!(aggregate["tool_calls"], 6);
    assert_eq!(aggregate["tool_failures"], 1);
    assert_eq!(aggregate["recovered_tool_failures"], 1);
    assert_eq!(aggregate["failed_tool_results"], 1);
    assert_eq!(aggregate["local_pressure_compactions"], 1);
    assert_eq!(aggregate["tool_only_turns"], 3);
    assert_eq!(aggregate["max_tool_only_turn_streak"], 2);
    assert_eq!(aggregate["scenario_tools"]["satisfied"], 5);
    assert_eq!(aggregate["scenario_tools"]["total"], 8);
    assert_eq!(aggregate["scenario_calls"]["satisfied"], 7);
    assert_eq!(aggregate["scenario_calls"]["total"], 10);
    assert_eq!(
        aggregate["scenario_overrun"]["extra_calls_after_satisfied"],
        7
    );
    assert_eq!(
        aggregate["scenario_overrun"]["extra_turns_after_satisfied"],
        3
    );
    assert_eq!(
        aggregate["scenario_overrun"]["max_extra_turns_after_satisfied"],
        2
    );
    assert_eq!(
        aggregate["scenario_overrun"]["context_growth_after_satisfied_chars"],
        11_000
    );
    assert_eq!(
        aggregate["scenario_overrun"]["max_context_growth_after_satisfied_chars"],
        8_000
    );
    assert_eq!(aggregate["diagnostics"]["tool_failure_recovered"], 1);
    assert_eq!(aggregate["diagnostics"]["request_failure"], 1);
}

#[test]
fn analyze_trace_reconstructs_tool_result_failures() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
            dir.path().join("001-tool-result.json"),
            serde_json::to_vec_pretty(&json!({
                "call_id": "call_1",
                "tool": "cmd.exec",
                "duration_ms": 12_345,
                "result": {
                    "ok": false,
                    "data": {"code": 1, "stdout_truncated": true, "stdout_chars": 40000, "timed_out": true},
                    "error": "command failed"
                }
            }))
            .expect("serialize first result"),
        )
        .expect("write first result");
    std::fs::write(
            dir.path().join("001-tool-result-002.json"),
            serde_json::to_vec_pretty(&json!({
                "call_id": "call_2",
                "tool": "fs.read",
                "args": {"path": "README.md"},
                "duration_ms": 4,
                "result": {
                    "ok": true,
                    "data": {"path": "README.md", "cached_observation": true, "created_parent_dirs": ["nested"]},
                    "error": null
                }
            }))
            .expect("serialize second result"),
        )
        .expect("write second result");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["tool_results"], 2);
    assert_eq!(summary["tool_failures"], 1);
    assert_eq!(summary["truncated_tool_results"], 1);
    assert_eq!(summary["readonly_tool_cache_hits"], 1);
    assert_eq!(summary["tool_failure_counts"]["cmd.exec"], 1);
    assert_eq!(summary["tool_truncation_counts"]["cmd.exec"], 1);
    assert_eq!(summary["total_tool_duration_ms"], 12_349);
    assert_eq!(summary["max_tool_duration_ms"], 12_345);
    assert_eq!(summary["tool_duration_ms_by_tool"]["cmd.exec"], 12_345);
    assert_eq!(summary["tool_duration_ms_by_tool"]["fs.read"], 4);
    assert_eq!(
        summary["timeline"][0]["tool_results"][0]["tool"],
        "cmd.exec"
    );
    assert_eq!(summary["timeline"][0]["tool_results"][0]["ok"], false);
    assert_eq!(summary["timeline"][0]["tool_results"][0]["truncated"], true);
    assert_eq!(summary["timeline"][0]["tool_results"][0]["timed_out"], true);
    assert_eq!(summary["timeline"][0]["tool_results"][1]["tool"], "fs.read");
    assert_eq!(
        summary["timeline"][0]["tool_results"][1]["cached_observation"],
        true
    );
    assert_eq!(
        summary["timeline"][0]["tool_results"][1]["created_parent_dirs"],
        json!(["nested"])
    );
}

#[test]
fn analyze_trace_reports_recovered_tool_failures() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("001-tool-result.json"),
        serde_json::to_vec_pretty(&json!({
            "call_id": "call_1",
            "tool": "fs.read",
            "args": {"path": "missing.md"},
            "duration_ms": 2,
            "result": {
                "ok": false,
                "data": {
                    "error_kind": "not_found_or_unavailable",
                    "message": "failed to read missing.md"
                },
                "error": "failed to read missing.md"
            }
        }))
        .expect("serialize failed result"),
    )
    .expect("write failed result");
    std::fs::write(
        dir.path().join("002-tool-result.json"),
        serde_json::to_vec_pretty(&json!({
            "call_id": "call_2",
            "tool": "fs.read",
            "args": {"path": "README.md"},
            "duration_ms": 3,
            "result": {
                "ok": true,
                "data": {"path": "README.md"},
                "error": null
            }
        }))
        .expect("serialize recovered result"),
    )
    .expect("write recovered result");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["tool_failures"], 1);
    assert_eq!(summary["tool_failure_recovery"]["failed_tool_results"], 1);
    assert_eq!(summary["tool_failure_recovery"]["recovered_failures"], 1);
    assert_eq!(summary["tool_failure_recovery"]["unrecovered_failures"], 0);
    assert_eq!(
        summary["tool_failure_recovery"]["by_tool"]["fs.read"]["recovered"],
        1
    );
    assert_eq!(
        summary["timeline"][0]["tool_results"][0]["error_kind"],
        "not_found_or_unavailable"
    );
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "tool_failure_recovered")
    );
    assert!(
        !summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "tool_failure_unrecovered")
    );
    assert!(
        format_trace_timeline(&summary)
            .contains("tool-recovery: recovered=1/1 unrecovered=0 by_tool=[fs.read:1/1]")
    );
    assert!(format_trace_timeline(&summary).contains("results=[fs.read:fail 2ms"));
    assert!(format_trace_timeline(&summary).contains("not_found_or_unavailable"));
    assert!(format_trace_summary_row(".spark-runs/run-1", &summary).contains("recoveries=1/1"));
}

#[test]
fn analyze_trace_recomputes_even_when_profile_summary_exists() {
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
        dir.path().join("001-profile-summary.json"),
        serde_json::to_vec_pretty(&json!({
            "requests": 999,
            "stale": true
        }))
        .expect("serialize profile"),
    )
    .expect("write profile");
    std::fs::write(
        dir.path().join("001-profile-summary-002.json"),
        serde_json::to_vec_pretty(&json!({
            "requests": 1,
            "stale": false,
            "compaction_reports": [{
                "method": "responses_compact",
                "before_chars": 10,
                "after_chars": 20,
                "raw": {
                    "id": "resp_old",
                    "output": [{
                        "type": "compaction_summary",
                        "encrypted_content": "old-raw"
                    }]
                }
            }]
        }))
        .expect("serialize latest profile"),
    )
    .expect("write latest profile");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["requests"], 1);
    assert_eq!(summary["embedded_profile_summary"]["requests"], 1);
    assert_eq!(summary["embedded_profile_summary"]["stale"], false);
    assert!(
        summary["embedded_profile_summary"]["compaction_reports"][0]
            .get("raw")
            .is_none()
    );
    assert_eq!(
        summary["embedded_profile_summary"]["compaction_reports"][0]["raw_summary"]["id"],
        "resp_old"
    );
}

#[test]
fn analyze_trace_includes_trace_metadata() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("000-trace-metadata.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "model": "gpt-5.3-codex-spark",
            "compact_after_chars": 160000,
            "max_input_chars": 500000
        }))
        .expect("serialize metadata"),
    )
    .expect("write metadata");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["trace_metadata"]["schema_version"], 1);
    assert_eq!(summary["trace_metadata"]["model"], "gpt-5.3-codex-spark");
    assert_eq!(summary["trace_metadata"]["compact_after_chars"], 160000);
}

#[test]
fn analyze_trace_reports_response_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("001-request-input.json"),
        serde_json::to_vec_pretty(&json!({
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "large prompt"}]}]
        }))
        .expect("serialize request"),
    )
    .expect("write request");
    std::fs::write(
        dir.path().join("001-response-error.json"),
        serde_json::to_vec_pretty(&json!({
            "stage": "response",
            "error": "Spark stream ended without response.completed"
        }))
        .expect("serialize error"),
    )
    .expect("write error");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["requests"], 1);
    assert_eq!(summary["errors"][0]["stage"], "response");
    assert!(
        summary["errors"][0]["error"]
            .as_str()
            .expect("error text")
            .contains("without response.completed")
    );
    assert_eq!(summary["timeline"][0]["errors"][0]["stage"], "response");
    assert!(
        summary["timeline"][0]["errors"][0]["error"]
            .as_str()
            .expect("timeline error")
            .contains("without response.completed")
    );
}

#[test]
fn analyze_trace_reports_generic_terminal_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("002-max_turns-error.json"),
        serde_json::to_vec_pretty(&json!({
            "stage": "max_turns",
            "error": "stopped after 1 turns without completion"
        }))
        .expect("serialize error"),
    )
    .expect("write error");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["errors"][0]["turn"], 2);
    assert_eq!(summary["errors"][0]["stage"], "max_turns");
    assert_eq!(summary["timeline"][0]["turn"], 2);
    assert_eq!(summary["timeline"][0]["errors"][0]["stage"], "max_turns");
    assert_eq!(summary["diagnostics"][0]["kind"], "request_failure");
}
