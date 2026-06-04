use super::*;

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
