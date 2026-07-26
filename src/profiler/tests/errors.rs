use super::*;

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
fn analyze_trace_treats_alternate_file_mutation_as_recovery() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("001-tool-result.json"),
        serde_json::to_vec_pretty(&json!({
            "call_id": "call_1",
            "tool": "fs.replace",
            "args": {"path": "src/status_map.ts"},
            "duration_ms": 2,
            "result": {
                "ok": false,
                "data": {
                    "error_kind": "expected_replacements_mismatch",
                    "message": "expected 1 replacements but found 2"
                },
                "error": "expected 1 replacements but found 2"
            }
        }))
        .expect("serialize failed result"),
    )
    .expect("write failed result");
    std::fs::write(
        dir.path().join("002-tool-result.json"),
        serde_json::to_vec_pretty(&json!({
            "call_id": "call_2",
            "tool": "fs.edit",
            "args": {"path": "src/status_map.ts"},
            "duration_ms": 3,
            "result": {
                "ok": true,
                "data": {"path": "src/status_map.ts"},
                "error": null
            }
        }))
        .expect("serialize recovered result"),
    )
    .expect("write recovered result");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["tool_failure_recovery"]["failed_tool_results"], 1);
    assert_eq!(summary["tool_failure_recovery"]["recovered_failures"], 1);
    assert_eq!(summary["tool_failure_recovery"]["unrecovered_failures"], 0);
    assert_eq!(
        summary["tool_failure_recovery"]["recovered"][0]["recovered_by_tool"],
        "fs.edit"
    );
    assert_eq!(
        summary["tool_failure_recovery"]["by_tool"]["fs.replace"]["recovered"],
        1
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
}

#[test]
fn analyze_trace_treats_successful_post_run_validation_as_cmd_recovery() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("001-tool-result.json"),
        serde_json::to_vec_pretty(&json!({
            "call_id": "call_1",
            "tool": "cmd.exec",
            "args": {"command": "validate stale refs", "workdir": ".spark-scenarios/config-migration"},
            "duration_ms": 12,
            "result": {
                "ok": false,
                "data": {"code": 1},
                "error": "command exited with code 1"
            }
        }))
        .expect("serialize failed validation result"),
    )
    .expect("write failed validation result");
    std::fs::write(
        dir.path().join("002-tool-result.json"),
        serde_json::to_vec_pretty(&json!({
            "call_id": "call_2",
            "tool": "fs.write",
            "args": {"path": ".spark-scenarios/config-migration/docs/config.md"},
            "duration_ms": 3,
            "result": {
                "ok": true,
                "data": {"path": ".spark-scenarios/config-migration/docs/config.md"},
                "error": null
            }
        }))
        .expect("serialize repair result"),
    )
    .expect("write repair result");
    std::fs::write(
        dir.path().join("scenario-validation.json"),
        serde_json::to_vec_pretty(&json!({
            "scenario": "config-migration",
            "workdir": ".spark-scenarios/config-migration",
            "command": ["powershell", "-NoProfile", "-Command", "validate"],
            "exit_code": 0,
            "timed_out": false,
            "duration_ms": 50,
            "stdout": "validation passed",
            "stderr": ""
        }))
        .expect("serialize validation artifact"),
    )
    .expect("write validation artifact");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["tool_failure_recovery"]["failed_tool_results"], 1);
    assert_eq!(summary["tool_failure_recovery"]["recovered_failures"], 1);
    assert_eq!(summary["tool_failure_recovery"]["unrecovered_failures"], 0);
    assert_eq!(
        summary["tool_failure_recovery"]["recovered"][0]["recovered_by_tool"],
        "scenario-validation"
    );
    assert_eq!(
        summary["tool_failure_recovery"]["by_tool"]["cmd.exec"]["recovered"],
        1
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
}

#[test]
fn analyze_trace_does_not_recover_cmd_failure_when_post_run_validation_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("001-tool-result.json"),
        serde_json::to_vec_pretty(&json!({
            "call_id": "call_1",
            "tool": "cmd.exec",
            "args": {"command": "validate stale refs"},
            "duration_ms": 12,
            "result": {
                "ok": false,
                "data": {"code": 1},
                "error": "command exited with code 1"
            }
        }))
        .expect("serialize failed validation result"),
    )
    .expect("write failed validation result");
    std::fs::write(
        dir.path().join("002-tool-result.json"),
        serde_json::to_vec_pretty(&json!({
            "call_id": "call_2",
            "tool": "fs.write",
            "args": {"path": ".spark-scenarios/config-migration/docs/config.md"},
            "duration_ms": 3,
            "result": {
                "ok": true,
                "data": {"path": ".spark-scenarios/config-migration/docs/config.md"},
                "error": null
            }
        }))
        .expect("serialize repair result"),
    )
    .expect("write repair result");
    std::fs::write(
        dir.path().join("scenario-validation.json"),
        serde_json::to_vec_pretty(&json!({
            "scenario": "config-migration",
            "workdir": ".spark-scenarios/config-migration",
            "command": ["powershell", "-NoProfile", "-Command", "validate"],
            "exit_code": 1,
            "timed_out": false,
            "duration_ms": 50,
            "stdout": "",
            "stderr": "validation failed"
        }))
        .expect("serialize validation artifact"),
    )
    .expect("write validation artifact");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["tool_failure_recovery"]["failed_tool_results"], 1);
    assert_eq!(summary["tool_failure_recovery"]["recovered_failures"], 0);
    assert_eq!(summary["tool_failure_recovery"]["unrecovered_failures"], 1);
    assert_eq!(
        summary["tool_failure_recovery"]["by_tool"]["cmd.exec"]["unrecovered"],
        1
    );
    assert!(
        summary["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "tool_failure_unrecovered")
    );
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
        dir.path().join("002-validation-error.json"),
        serde_json::to_vec_pretty(&json!({
            "stage": "validation",
            "error": "local validation failed"
        }))
        .expect("serialize error"),
    )
    .expect("write error");

    let summary = analyze_trace(dir.path()).expect("analyze trace");

    assert_eq!(summary["errors"][0]["turn"], 2);
    assert_eq!(summary["errors"][0]["stage"], "validation");
    assert_eq!(summary["timeline"][0]["turn"], 2);
    assert_eq!(summary["timeline"][0]["errors"][0]["stage"], "validation");
    assert_eq!(summary["diagnostics"][0]["kind"], "request_failure");
}
