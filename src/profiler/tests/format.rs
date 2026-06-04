use super::*;

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
