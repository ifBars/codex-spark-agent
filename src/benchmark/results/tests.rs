use serde_json::json;

use super::*;

fn read_external_agent_report_rows(
    cwd: &Path,
    paths: &[PathBuf],
    label: &str,
) -> Result<Vec<CodexCliBenchmarkRow>> {
    Ok(read_external_agent_report_rows_with_skips(cwd, paths, label)?.rows)
}

fn aggregate_comparison(suite: &str, rows: &[ComparisonRow]) -> Value {
    aggregate_comparison_with_diagnostics(suite, rows, ComparisonDiagnostics::default())
}

fn empty_comparison_inputs() -> Value {
    json!({
        "harness_reports": [],
        "codex_cli_reports": [],
        "opencode_reports": [],
    })
}

fn comparison_row(runner: &str, scenario: &str) -> ComparisonRow {
    ComparisonRow {
        runner: runner.to_string(),
        suite: "real-world".to_string(),
        scenario: scenario.to_string(),
        attempts: 1,
        successful_attempts: 1,
        model: "spark".to_string(),
        reasoning_effort: "medium".to_string(),
        command_path: String::new(),
        command_version: String::new(),
        completion_score: 100.0,
        quality_score: 100.0,
        process_score: 100.0,
        llm_solution_score: None,
        llm_process_score: None,
        llm_confidence: None,
        llm_notes: String::new(),
        efficiency_index: None,
        benchmark_index: None,
        score: 100.0,
        task_quality_score: 100.0,
        efficiency_score: 100.0,
        harness_pressure_score: 100.0,
        success: true,
        validation_exit_code: Some(0),
        validation_timed_out: false,
        duration_ms: 10_000,
        tool_or_item_calls: 10,
        input_tokens: 10_000,
        output_tokens: 0,
        source_files: 2,
        source_bytes: 2_000,
        failure_points: String::new(),
        source: "run".to_string(),
    }
}

fn indexed(mut rows: Vec<ComparisonRow>) -> Vec<ComparisonRow> {
    apply_comparison_indices(&mut rows, "codex-cli");
    rows
}

fn external_report_json(runner: &str, scenario: &str) -> Value {
    json!({
        "rows": [{
            "runner": runner,
            "suite": "real-world",
            "scenario": scenario,
            "repeat_index": 1,
            "model": "gpt-test",
            "command_path": r"C:\Users\ghost\.bun\bin\codex.exe",
            "command_version": "codex-cli 0.139.0",
            "score": 100.0,
            "success": true,
            "exit_code": 0,
            "timed_out": false,
            "duration_ms": 10000,
            "json_events": 1,
            "non_json_stdout_lines": 0,
            "stderr_lines": 0,
            "actionable_stderr_lines": 0,
            "turns": 1,
            "completed_items": 1,
            "agent_messages": 1,
            "tool_items": 1,
            "input_tokens": 1000,
            "cached_input_tokens": 0,
            "output_tokens": 100,
            "reasoning_output_tokens": 0,
            "expected_artifacts": 0,
            "present_artifacts": 0,
            "validation_exit_code": 0,
            "validation_timed_out": false,
            "browser_validation_present": false,
            "browser_validation_exit_code": null,
            "browser_validation_timed_out": false,
            "browser_screenshot": "",
            "source_files": 1,
            "source_bytes": 100,
            "final_message_chars": 100,
            "run_dir": "run",
            "failure_points": ""
        }]
    })
}

fn benchmark_row() -> BenchmarkRunRow {
    BenchmarkRunRow {
        run_id: "run-1".to_string(),
        trace_dir: ".spark-runs/run-1".to_string(),
        suite: "real-world".to_string(),
        scenario: "react-calculator-scaffold".to_string(),
        model: "spark".to_string(),
        reasoning_effort: "medium".to_string(),
        completion_score: 0.0,
        quality_score: 0.0,
        process_score: 0.0,
        efficiency_index: None,
        benchmark_index: None,
        score: 0.0,
        task_quality_score: 0.0,
        efficiency_score: 0.0,
        harness_pressure_score: 0.0,
        success: true,
        validation_present: true,
        validation_exit_code: Some(0),
        validation_timed_out: false,
        browser_validation_present: false,
        browser_validation_exit_code: None,
        browser_validation_timed_out: false,
        browser_screenshot: String::new(),
        requests: 4,
        tool_calls: 4,
        max_approx_input_tokens: 1000,
        max_context_window_pct: 1.0,
        max_request_duration_ms: 1000,
        total_duration_ms: 1500,
        source_files: 0,
        source_bytes: 0,
        compactions: 0,
        tool_failures: 0,
        recovered_tool_failures: 0,
        unrecovered_tool_failures: 0,
        truncated_tool_results: 0,
        repeated_tool_calls: 0,
        tool_only_turns: 3,
        max_tool_only_streak: 3,
        expected_tool_groups: 2,
        satisfied_tool_groups: 2,
        expected_tool_calls: 2,
        satisfied_tool_calls: 2,
        extra_calls_after_satisfied: 0,
        extra_turns_after_satisfied: 0,
        context_growth_after_satisfied_chars: 0,
        diagnostics: String::new(),
        failure_points: String::new(),
    }
}

#[test]
fn reasoning_effort_reads_nested_profile_scenario_metadata() {
    let summary = json!({
        "trace_metadata": {
            "context": {
                "profile_scenario": {
                    "reasoning_effort": "high"
                }
            }
        }
    });

    assert_eq!(reasoning_effort(&summary), Some("high"));
}

#[test]
fn benchmark_index_separates_successful_completion_ties() {
    let codex = comparison_row("codex-cli", "matched");
    let mut spark = comparison_row("spark-harness", "matched");
    spark.duration_ms = 2_500;
    spark.input_tokens = 2_500;
    spark.tool_or_item_calls = 5;

    let rows = indexed(vec![codex, spark]);
    let spark = rows
        .iter()
        .find(|row| row.runner == "spark-harness")
        .unwrap();
    let codex = rows.iter().find(|row| row.runner == "codex-cli").unwrap();

    assert_eq!(spark.completion_score, 100.0);
    assert_eq!(codex.completion_score, 100.0);
    assert_eq!(codex.benchmark_index, Some(100.0));
    assert!(spark.benchmark_index.unwrap() > 100.0);
}

#[test]
fn harness_report_inputs_accept_saved_benchmark_report_rows() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut row = benchmark_row();
    row.scenario = "config-migration".to_string();
    let report = json!({
        "suite": "real-world",
        "rows": [row],
        "aggregate": {}
    });
    let path = dir.path().join("real-world-report.json");
    std::fs::write(&path, serde_json::to_string(&report).expect("json")).expect("write");

    let rows = collect_benchmark_rows_from_harness_input_paths(
        dir.path(),
        ProfileBenchmarkSuiteKind::RealWorld,
        &[path],
    )
    .expect("rows");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].scenario, "config-migration");
    assert_eq!(rows[0].suite, "real-world");
}

#[test]
fn harness_report_inputs_reject_wrong_suite_report_rows() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut row = benchmark_row();
    row.suite = "scaffolding".to_string();
    let report = json!({
        "suite": "scaffolding",
        "rows": [row],
        "aggregate": {}
    });
    let path = dir.path().join("scaffolding-report.json");
    std::fs::write(&path, serde_json::to_string(&report).expect("json")).expect("write");

    let error = collect_benchmark_rows_from_harness_input_paths(
        dir.path(),
        ProfileBenchmarkSuiteKind::RealWorld,
        &[path],
    )
    .expect_err("wrong suite should fail")
    .to_string();

    assert!(error.contains("belongs to suite 'scaffolding', expected 'real-world'"));
}

#[test]
fn benchmark_comparison_records_input_report_provenance() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut harness_row = benchmark_row();
    harness_row.scenario = "config-migration".to_string();
    harness_row.success = true;
    harness_row.completion_score = 100.0;
    harness_row.quality_score = 100.0;
    harness_row.process_score = 100.0;
    harness_row.score = 100.0;
    harness_row.task_quality_score = 100.0;
    harness_row.efficiency_score = 100.0;
    harness_row.harness_pressure_score = 100.0;
    harness_row.validation_exit_code = Some(0);
    let harness_report = json!({
        "suite": "real-world",
        "generated_at_unix_ms": 10,
        "rows": [harness_row],
        "aggregate": {}
    });
    let harness_path = dir.path().join("harness-report.json");
    std::fs::write(
        &harness_path,
        serde_json::to_string(&harness_report).expect("harness json"),
    )
    .expect("write harness report");

    let mut codex_report = external_report_json("codex-cli", "config-migration");
    codex_report["generated_at_unix_ms"] = json!(20);
    let codex_path = dir.path().join("codex-report.json");
    std::fs::write(
        &codex_path,
        serde_json::to_string(&codex_report).expect("codex json"),
    )
    .expect("write codex report");

    let output = write_benchmark_comparison(BenchmarkComparisonOptions {
        cwd: dir.path().to_path_buf(),
        suite: ProfileBenchmarkSuiteKind::RealWorld,
        limit: 50,
        all_runs: false,
        harness_reports: vec![harness_path.clone()],
        codex_cli_reports: vec![codex_path.clone()],
        opencode_reports: Vec::new(),
        llm_judge_report: None,
        group_by_reasoning: false,
        group_by_model: false,
        output_dir: dir.path().join("out"),
    })
    .expect("write comparison");

    let report: Value = serde_json::from_str(
        &std::fs::read_to_string(&output.json_path).expect("read comparison json"),
    )
    .expect("comparison json");
    assert_eq!(
        report["inputs"]["harness_reports"][0]["path"],
        harness_path.display().to_string()
    );
    assert_eq!(
        report["inputs"]["codex_cli_reports"][0]["path"],
        codex_path.display().to_string()
    );
    assert_eq!(report["inputs"]["harness_reports"][0]["row_count"], 1);
    assert_eq!(
        report["inputs"]["codex_cli_reports"][0]["scenarios"][0],
        "config-migration"
    );
    assert_eq!(report["inputs"]["freshness"]["input_count"], 2);
    assert!(
        report["inputs"]["freshness"]["modified_span_ms"]
            .as_u64()
            .is_some()
    );
    assert_eq!(
        report["aggregate"]["diagnostics"]["comparison_validity"]["mixed_input_warning"],
        false
    );
    assert_eq!(
        report["aggregate"]["diagnostics"]["comparison_validity"]["excluded_provider_api_rows"],
        0
    );
    assert_eq!(
        report["aggregate"]["diagnostics"]["comparison_validity"]["directional_until_fresh_paired_run"],
        false
    );

    let html = std::fs::read_to_string(&output.html_path).expect("read comparison html");
    assert!(html.contains("Report Inputs"));
    assert!(html.contains("Input freshness"));
    assert!(html.contains("harness-report.json"));
    assert!(html.contains("codex-report.json"));
}

#[test]
fn comparison_input_freshness_warns_for_mixed_report_ages() {
    let mut inputs = json!({
        "harness_reports": [{"modified_unix_ms": 1_000u64}],
        "codex_cli_reports": [{"modified_unix_ms": 3_601_000u64}],
        "opencode_reports": [],
    });
    let freshness = input_freshness_summary(&inputs);
    inputs
        .as_object_mut()
        .expect("inputs object")
        .insert("freshness".to_string(), freshness);

    assert_eq!(inputs["freshness"]["input_count"], 2);
    assert_eq!(inputs["freshness"]["modified_span_ms"], 3_600_000u64);
    assert_eq!(inputs["freshness"]["modified_span_label"], "1.0h");
    assert_eq!(inputs["freshness"]["mixed_input_warning"], false);

    inputs["codex_cli_reports"][0]["modified_unix_ms"] = json!(3_602_000u64);
    let freshness = input_freshness_summary(&inputs);
    inputs
        .as_object_mut()
        .expect("inputs object")
        .insert("freshness".to_string(), freshness);

    assert_eq!(inputs["freshness"]["mixed_input_warning"], true);
    let html = comparison_input_table(&inputs);
    assert!(html.contains("Input freshness warning"));
    assert!(html.contains("1.0h older"));
}

#[test]
fn comparison_html_places_freshness_caveat_near_headline() {
    let rows = indexed(vec![
        comparison_row("spark-harness", "one"),
        comparison_row("codex-cli", "one"),
    ]);
    let mut aggregate = aggregate_comparison_with_diagnostics(
        "real-world",
        &rows,
        ComparisonDiagnostics {
            skipped_spark_infrastructure_failures: 1,
            skipped_spark_infrastructure_scenarios: BTreeMap::from([("one".to_string(), 1)]),
            skipped_spark_infrastructure_retry_hints: BTreeMap::new(),
            ..ComparisonDiagnostics::default()
        },
    );
    let mut inputs = json!({
        "harness_reports": [{"modified_unix_ms": 1_000u64}],
        "codex_cli_reports": [{"modified_unix_ms": 7_201_000u64}],
        "opencode_reports": [],
    });
    let freshness = input_freshness_summary(&inputs);
    inputs
        .as_object_mut()
        .expect("inputs object")
        .insert("freshness".to_string(), freshness);
    annotate_comparison_validity(&mut aggregate, &inputs);

    let html = comparison_rows_to_html("real-world", &rows, &aggregate, &inputs);

    assert_eq!(
        aggregate["diagnostics"]["comparison_validity"]["mixed_input_warning"],
        true
    );
    assert_eq!(
        aggregate["diagnostics"]["comparison_validity"]["excluded_provider_api_rows"],
        1
    );
    assert_eq!(
        aggregate["diagnostics"]["comparison_validity"]["directional_until_fresh_paired_run"],
        true
    );
    assert_eq!(
        aggregate["diagnostics"]["comparison_validity"]["caveats"][0],
        "selected input reports span 2.0h"
    );
    assert!(html.contains("Comparison freshness caveat"));
    assert!(html.contains("selected input reports span 2.0h"));
    assert!(html.contains("1 provider/API failure row(s) were excluded"));
    assert!(html.find("Comparison freshness caveat") < html.find("Evidence Ledger"));
}

#[test]
fn comparison_directional_failure_message_uses_validity_caveats() {
    let clean = json!({
        "diagnostics": {
            "comparison_validity": {
                "directional_until_fresh_paired_run": false,
                "caveats": []
            }
        }
    });
    assert!(comparison_directional_failure_message(&clean).is_none());

    let directional = json!({
        "diagnostics": {
            "comparison_validity": {
                "directional_until_fresh_paired_run": true,
                "caveats": [
                    "selected input reports span 3.5d",
                    "15 provider/API failure row(s) were excluded"
                ]
            }
        }
    });
    let message = comparison_directional_failure_message(&directional).expect("message");

    assert!(message.contains("benchmark comparison is directional"));
    assert!(message.contains("selected input reports span 3.5d"));
    assert!(message.contains("15 provider/API failure row(s) were excluded"));
}

#[test]
fn aggregate_rows_reports_harness_request_failure_scenarios() {
    let mut row = benchmark_row();
    row.scenario = "config-migration".to_string();
    row.success = false;
    row.diagnostics = "request_failure".to_string();

    let aggregate = aggregate_rows("real-world", &[row]);

    assert_eq!(aggregate["diagnostics"]["request_failure"], 1);
    assert_eq!(
        aggregate["diagnostics"]["request_failure_scenarios"]["config-migration"],
        1
    );
}

#[test]
fn rows_to_html_reports_harness_request_failure_scenarios() {
    let mut row = benchmark_row();
    row.scenario = "config-migration".to_string();
    row.success = false;
    row.diagnostics = "request_failure".to_string();
    let aggregate = aggregate_rows("real-world", &[row.clone(), row]);

    let html = rows_to_html("real-world", &[], &aggregate);

    assert!(html.contains("Request failure scenarios: config-migration x2."));
    assert!(html.contains("Provider/API failures are tracked separately"));
}

#[test]
fn harness_request_failure_rows_are_skipped_for_comparison() {
    let dir = tempfile::tempdir().expect("tempdir");
    let trace_dir = dir.path().join("spark-quota-error");
    std::fs::create_dir_all(&trace_dir).expect("trace dir");
    std::fs::write(
        trace_dir.join("002-response-error.json"),
        r#"{"error":"Spark request failed (429 Too Many Requests): {\"error\":{\"type\":\"usage_limit_reached\",\"message\":\"The usage limit has been reached\",\"resets_in_seconds\":21328,\"resets_at\":1781138326}}"}"#,
    )
    .expect("write trace error");

    let mut failed = benchmark_row();
    failed.trace_dir = trace_dir.to_string_lossy().to_string();
    failed.scenario = "config-migration".to_string();
    failed.success = false;
    failed.diagnostics = "request_failure".to_string();
    let mut kept = benchmark_row();
    kept.scenario = "ops-report".to_string();

    let rows = filter_harness_request_failure_rows(dir.path(), vec![failed, kept]);

    assert_eq!(rows.rows.len(), 1);
    assert_eq!(rows.rows[0].scenario, "ops-report");
    assert_eq!(rows.skipped_request_failures, 1);
    assert_eq!(
        rows.skipped_request_failure_scenarios["config-migration"],
        1
    );
    assert_eq!(
        rows.skipped_request_failure_retry_hints["config-migration"],
        "try again after 2026-06-11T00:38:46Z"
    );
}

#[test]
fn harness_local_request_failures_remain_comparable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let trace_dir = dir.path().join("spark-max-turns-error");
    std::fs::create_dir_all(&trace_dir).expect("trace dir");
    std::fs::write(
        trace_dir.join("002-max_turns-error.json"),
        r#"{"stage":"max_turns","error":"stopped after 0 turns without completion"}"#,
    )
    .expect("write trace error");

    let mut failed = benchmark_row();
    failed.trace_dir = trace_dir.to_string_lossy().to_string();
    failed.scenario = "precise-patch".to_string();
    failed.success = false;
    failed.diagnostics = "request_failure".to_string();
    failed.failure_points = "max_turns".to_string();

    let rows = filter_harness_request_failure_rows(dir.path(), vec![failed]);

    assert_eq!(rows.rows.len(), 1);
    assert_eq!(rows.rows[0].scenario, "precise-patch");
    assert_eq!(rows.skipped_request_failures, 0);
    assert!(rows.skipped_request_failure_scenarios.is_empty());
}

#[test]
fn failure_points_include_benchmark_specific_signals() {
    let summary = json!({
        "errors": [],
        "truncated_tool_results": 1,
        "repeated_tool_calls": 2,
        "tool_only_turns": {"max_consecutive": 12},
        "max_context_window_pct": 24.0,
        "compactions": 1,
        "profile_scenario_call_expectations": {
            "extra_calls_after_satisfied": 3
        }
    });

    let points = failure_points(
        "react-calculator-scaffold",
        &summary,
        2,
        2,
        4,
        4,
        0,
        false,
        None,
        false,
        false,
        None,
        false,
    );

    assert!(points.contains(&"truncated_tool_result".to_string()));
    assert!(points.contains(&"repeated_tool_call".to_string()));
    assert!(points.contains(&"extra_calls_after_expected".to_string()));
    assert!(points.contains(&"long_tool_only_streak".to_string()));
    assert!(points.contains(&"high_context_pressure".to_string()));
    assert!(points.contains(&"compaction_needed".to_string()));
}

#[test]
fn failure_points_include_request_failure_diagnostics() {
    let summary = json!({
        "errors": [
            {
                "stage": "response",
                "error": "Spark request failed (429 Too Many Requests)"
            }
        ],
        "diagnostics": [
            {
                "kind": "request_failure",
                "level": "error",
                "message": "One or more Spark turns failed."
            }
        ]
    });

    let points = failure_points(
        "precise-patch",
        &summary,
        0,
        0,
        0,
        0,
        0,
        false,
        None,
        false,
        false,
        None,
        false,
    );

    assert!(points.contains(&"request_failure".to_string()));
    assert!(points.contains(&"terminal_error".to_string()));
}

#[test]
fn expected_repeated_source_reads_do_not_count_as_repeat_pressure() {
    let summary = json!({
        "repeated_tool_calls": 1,
        "profile_scenario_call_expectations": {
            "expected_calls": [
                {"tool": "fs.read", "path": ".spark-scenarios/rust-failing-test-bugfix/src/lib.rs"},
                {"tools": ["fs.edit", "fs.replace", "fs.write"], "path": ".spark-scenarios/rust-failing-test-bugfix/src/lib.rs"},
                {"tool": "cmd.exec", "command": "cargo test"},
                {"tool": "fs.read", "path": ".spark-scenarios/rust-failing-test-bugfix/src/lib.rs"}
            ],
            "extra_calls_after_satisfied": 0
        }
    });

    assert_eq!(expected_repeated_tool_calls(&summary), 1);
    assert_eq!(unexpected_repeated_tool_calls(&summary), 0);

    let points = failure_points(
        "rust-failing-test-bugfix",
        &summary,
        3,
        3,
        4,
        4,
        0,
        true,
        Some(0),
        false,
        false,
        None,
        false,
    );

    assert!(!points.contains(&"repeated_tool_call".to_string()));
}

#[test]
fn unexpected_repeated_calls_still_count_as_repeat_pressure() {
    let summary = json!({
        "repeated_tool_calls": 1,
        "profile_scenario_call_expectations": {
            "expected_calls": [
                {"tool": "fs.read", "path": "src/lib.rs"},
                {"tool": "cmd.exec", "command": "cargo test"}
            ],
            "extra_calls_after_satisfied": 0
        }
    });

    assert_eq!(expected_repeated_tool_calls(&summary), 0);
    assert_eq!(unexpected_repeated_tool_calls(&summary), 1);
}

#[test]
fn survey_extra_calls_are_not_exact_completion_pressure() {
    let summary = json!({
        "errors": [],
        "truncated_tool_results": 0,
        "repeated_tool_calls": 0,
        "tool_only_turns": {"max_consecutive": 4},
        "max_context_window_pct": 1.0,
        "compactions": 0,
        "profile_scenario_call_expectations": {
            "extra_calls_after_satisfied": 6
        }
    });

    let points = failure_points(
        "benchmark-design-survey",
        &summary,
        2,
        2,
        4,
        4,
        0,
        false,
        None,
        false,
        false,
        None,
        false,
    );

    assert!(!points.contains(&"extra_calls_after_expected".to_string()));
}

#[test]
fn bounded_real_world_triage_review_and_merge_penalize_extra_calls_after_satisfied() {
    for scenario in [
        "ci-failure-triage",
        "pull-request-review",
        "dependency-upgrade-triage",
        "merge-conflict-resolution",
    ] {
        assert!(
            exact_completion_pressure_scenario(scenario),
            "{scenario} should penalize unnecessary work after required evidence is satisfied"
        );

        let mut row = benchmark_row();
        row.scenario = scenario.to_string();
        row.success = true;
        row.completion_score = 100.0;
        row.quality_score = 100.0;
        row.extra_calls_after_satisfied = 2;
        row.extra_turns_after_satisfied = 1;
        row.max_tool_only_streak = 4;

        assert!(
            process_score(&row) < 100.0,
            "{scenario} extra work should reduce process score"
        );
    }
}

#[test]
fn codex_cli_baseline_average_normalizes_to_100() {
    let rows = indexed(vec![
        comparison_row("codex-cli", "one"),
        comparison_row("spark-harness", "one"),
        comparison_row("codex-cli", "two"),
        comparison_row("spark-harness", "two"),
    ]);
    let aggregate = aggregate_comparison("real-world", &rows);

    assert_eq!(
        aggregate["matched_runner_benchmark_index_averages"]["codex-cli"],
        100.0
    );
}

#[test]
fn external_agent_reader_merges_multiple_report_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let first = dir.path().join("codex-one.json");
    let second = dir.path().join("codex-two.json");
    std::fs::write(
        &first,
        serde_json::to_string(&external_report_json("codex-cli", "one")).expect("json"),
    )
    .expect("write first report");
    std::fs::write(
        &second,
        serde_json::to_string(&external_report_json("codex-cli", "two")).expect("json"),
    )
    .expect("write second report");

    let rows =
        read_external_agent_report_rows(dir.path(), &[first, second], "Codex CLI").expect("rows");

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].scenario, "one");
    assert_eq!(rows[1].scenario, "two");
}

#[test]
fn external_agent_command_provenance_reaches_comparison_rows() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("codex-cli.json");
    std::fs::write(
        &path,
        serde_json::to_string(&external_report_json("codex-cli", "one")).expect("json"),
    )
    .expect("write report");

    let rows = read_external_agent_report_rows(dir.path(), &[path], "Codex CLI").expect("rows");
    let comparison = comparison_row_from_external_agent(&rows[0]);

    assert_eq!(
        comparison.command_path,
        r"C:\Users\ghost\.bun\bin\codex.exe"
    );
    assert_eq!(comparison.command_version, "codex-cli 0.139.0");
    assert!(
        serde_json::to_value(&comparison)
            .expect("comparison row json")
            .get("command_version")
            .is_some()
    );
    let csv = comparison_rows_to_csv(&[comparison.clone()]);
    assert!(csv.contains("codex-cli 0.139.0"));
    let html_rows = vec![comparison];
    let aggregate = aggregate_comparison("real-world", &html_rows);
    let html = comparison_rows_to_html(
        "real-world",
        &html_rows,
        &aggregate,
        &empty_comparison_inputs(),
    );
    assert!(html.contains("<th>Command</th>"));
    assert!(html.contains("codex-cli 0.139.0"));
}

#[test]
fn external_agent_reader_skips_infrastructure_api_failures() {
    let dir = tempfile::tempdir().expect("tempdir");
    let run_dir = dir.path().join("run-infra-error");
    std::fs::create_dir_all(&run_dir).expect("run dir");
    std::fs::write(
            run_dir.join("stdout.jsonl"),
            r#"{"type":"error","error":{"name":"APIError","data":{"message":"Insufficient balance. Manage your billing here.","statusCode":401}}}"#,
        )
        .expect("stdout");

    let mut report = external_report_json("opencode", "one");
    report["rows"][0]["run_dir"] = json!("run-infra-error");
    let path = dir.path().join("opencode.json");
    std::fs::write(&path, serde_json::to_string(&report).expect("json")).expect("write report");

    let rows = read_external_agent_report_rows(dir.path(), &[path], "opencode").expect("rows");

    assert!(rows.is_empty());
}

#[test]
fn external_agent_reader_skips_codex_usage_limit_failures() {
    let dir = tempfile::tempdir().expect("tempdir");
    let run_dir = dir.path().join("run-codex-usage-limit");
    std::fs::create_dir_all(&run_dir).expect("run dir");
    std::fs::write(
            run_dir.join("stdout.jsonl"),
            r#"{"type":"error","message":"You've hit your usage limit for GPT-5.3-Codex-Spark. Switch to another model now, or try again at 5:38 PM."}
{"type":"turn.failed","error":{"message":"You've hit your usage limit for GPT-5.3-Codex-Spark. Switch to another model now, or try again at 5:38 PM."}}"#,
        )
        .expect("stdout");

    let mut report = external_report_json("codex-cli", "one");
    report["rows"][0]["run_dir"] = json!("run-codex-usage-limit");
    report["rows"][0]["success"] = json!(false);
    report["rows"][0]["exit_code"] = json!(1);
    report["rows"][0]["agent_messages"] = json!(0);
    report["rows"][0]["final_message_chars"] = json!(0);
    report["rows"][0]["failure_points"] =
        json!("nonzero_exit;missing_final_message;validation_failed");
    let path = dir.path().join("codex-cli.json");
    std::fs::write(&path, serde_json::to_string(&report).expect("json")).expect("write report");

    let rows = read_external_agent_report_rows(dir.path(), &[path], "Codex CLI").expect("rows");

    assert!(rows.is_empty());
}

#[test]
fn external_agent_reader_reports_skipped_infrastructure_count() {
    let dir = tempfile::tempdir().expect("tempdir");
    let run_dir = dir.path().join("run-codex-usage-limit");
    std::fs::create_dir_all(&run_dir).expect("run dir");
    std::fs::write(
        run_dir.join("stdout.jsonl"),
        r#"{"type":"error","message":"You've hit your usage limit for GPT-5.3-Codex-Spark."}"#,
    )
    .expect("stdout");

    let mut report = external_report_json("codex-cli", "one");
    report["rows"][0]["run_dir"] = json!("run-codex-usage-limit");
    report["rows"][0]["success"] = json!(false);
    report["rows"][0]["failure_points"] = json!("request_failure;nonzero_exit");
    report["rows"][0]["provider_retry_hint"] = json!("try again at 5:38 PM");
    let path = dir.path().join("codex-cli.json");
    std::fs::write(&path, serde_json::to_string(&report).expect("json")).expect("write report");

    let rows =
        read_external_agent_report_rows_with_skips(dir.path(), &[path], "Codex CLI").expect("rows");

    assert!(rows.rows.is_empty());
    assert_eq!(rows.skipped_infrastructure_failures, 1);
    assert_eq!(rows.skipped_infrastructure_scenarios["one"], 1);
    assert_eq!(
        rows.skipped_infrastructure_retry_hints["one"],
        "try again at 5:38 PM"
    );
}

#[test]
fn external_agent_reader_recovers_retry_hint_from_request_failure_logs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let run_dir = dir.path().join("run-codex-usage-limit");
    std::fs::create_dir_all(&run_dir).expect("run dir");
    std::fs::write(
        run_dir.join("stdout.jsonl"),
        r#"{"type":"error","message":"You've hit your usage limit for GPT-5.3-Codex-Spark. Switch to another model now, or try again at 5:38 PM."}"#,
    )
    .expect("stdout");

    let mut report = external_report_json("codex-cli", "one");
    report["rows"][0]["run_dir"] = json!("run-codex-usage-limit");
    report["rows"][0]["success"] = json!(false);
    report["rows"][0]["failure_points"] = json!("request_failure;nonzero_exit");
    report["rows"][0]["provider_retry_hint"] = json!("");
    let path = dir.path().join("codex-cli.json");
    std::fs::write(&path, serde_json::to_string(&report).expect("json")).expect("write report");

    let rows =
        read_external_agent_report_rows_with_skips(dir.path(), &[path], "Codex CLI").expect("rows");

    assert!(rows.rows.is_empty());
    assert_eq!(rows.skipped_infrastructure_failures, 1);
    assert_eq!(
        rows.skipped_infrastructure_retry_hints["one"],
        "try again at 5:38 PM"
    );
}

#[test]
fn external_agent_reader_trusts_request_failure_without_run_logs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut report = external_report_json("codex-cli", "one");
    report["rows"][0]["run_dir"] = json!("missing-run-dir");
    report["rows"][0]["success"] = json!(false);
    report["rows"][0]["failure_points"] = json!("request_failure;nonzero_exit");
    let path = dir.path().join("codex-cli.json");
    std::fs::write(&path, serde_json::to_string(&report).expect("json")).expect("write report");

    let rows =
        read_external_agent_report_rows_with_skips(dir.path(), &[path], "Codex CLI").expect("rows");

    assert!(rows.rows.is_empty());
    assert_eq!(rows.skipped_infrastructure_failures, 1);
}

#[test]
fn skipped_infrastructure_scenarios_text_includes_counts() {
    let scenarios = BTreeMap::from([
        ("config-migration".to_string(), 2),
        ("ops-report".to_string(), 1),
    ]);

    let text = skipped_infrastructure_scenarios_text(&scenarios);

    assert_eq!(text, ": config-migration x2, ops-report");
}

#[test]
fn skipped_infrastructure_rows_message_includes_scenario_counts() {
    let scenarios = BTreeMap::from([
        ("config-migration".to_string(), 2),
        ("ops-report".to_string(), 1),
    ]);

    let text = infrastructure::skipped_infrastructure_rows_message(
        3,
        "Codex CLI infrastructure/API failure",
        &scenarios,
    );

    assert_eq!(
        text,
        "benchmark_compare skipped 3 Codex CLI infrastructure/API failure row(s): config-migration x2, ops-report"
    );
}

#[test]
fn comparison_aggregate_reports_skipped_external_infrastructure_rows() {
    let rows = indexed(vec![
        comparison_row("spark-harness", "one"),
        comparison_row("codex-cli", "one"),
        comparison_row("opencode", "one"),
    ]);

    let aggregate = aggregate_comparison_with_diagnostics(
        "real-world",
        &rows,
        ComparisonDiagnostics {
            skipped_spark_infrastructure_failures: 1,
            skipped_spark_infrastructure_scenarios: BTreeMap::from([(
                "precise-patch".to_string(),
                1,
            )]),
            skipped_spark_infrastructure_retry_hints: BTreeMap::from([(
                "precise-patch".to_string(),
                "try again after 2026-06-11T00:38:46Z".to_string(),
            )]),
            skipped_codex_infrastructure_failures: 2,
            skipped_codex_infrastructure_scenarios: BTreeMap::from([
                ("config-migration".to_string(), 1),
                ("ops-report".to_string(), 1),
            ]),
            skipped_codex_infrastructure_retry_hints: BTreeMap::from([(
                "config-migration".to_string(),
                "try again at 5:38 PM".to_string(),
            )]),
            skipped_opencode_infrastructure_failures: 1,
            skipped_opencode_infrastructure_scenarios: BTreeMap::from([(
                "typescript-reducer-bugfix".to_string(),
                1,
            )]),
            skipped_opencode_infrastructure_retry_hints: BTreeMap::new(),
        },
    );

    assert_eq!(
        aggregate["diagnostics"]["skipped_infrastructure_failures"]["spark-harness"],
        1
    );
    assert_eq!(
        aggregate["diagnostics"]["skipped_infrastructure_failures"]["codex-cli"],
        2
    );
    assert_eq!(
        aggregate["diagnostics"]["skipped_infrastructure_failures"]["opencode"],
        1
    );
    assert_eq!(
        aggregate["diagnostics"]["total_skipped_infrastructure_failures"],
        4
    );
    assert_eq!(
        aggregate["diagnostics"]["skipped_infrastructure_scenarios"]["spark-harness"]["precise-patch"],
        1
    );
    assert_eq!(
        aggregate["diagnostics"]["skipped_infrastructure_scenarios"]["codex-cli"]["config-migration"],
        1
    );
    assert_eq!(
        aggregate["diagnostics"]["skipped_infrastructure_scenarios"]["codex-cli"]["ops-report"],
        1
    );
    assert_eq!(
        aggregate["diagnostics"]["skipped_infrastructure_scenarios"]["opencode"]["typescript-reducer-bugfix"],
        1
    );
    assert_eq!(
        aggregate["diagnostics"]["skipped_infrastructure_retry_hints"]["spark-harness"]["precise-patch"],
        "try again after 2026-06-11T00:38:46Z"
    );
    assert_eq!(
        aggregate["diagnostics"]["skipped_infrastructure_retry_hints"]["codex-cli"]["config-migration"],
        "try again at 5:38 PM"
    );
}

#[test]
fn comparison_html_reports_partial_external_infrastructure_skips() {
    let rows = indexed(vec![
        comparison_row("spark-harness", "one"),
        comparison_row("codex-cli", "one"),
        comparison_row("opencode", "one"),
    ]);
    let aggregate = aggregate_comparison_with_diagnostics(
        "real-world",
        &rows,
        ComparisonDiagnostics {
            skipped_spark_infrastructure_failures: 1,
            skipped_spark_infrastructure_scenarios: BTreeMap::from([(
                "precise-patch".to_string(),
                1,
            )]),
            skipped_spark_infrastructure_retry_hints: BTreeMap::from([(
                "precise-patch".to_string(),
                "try again after 2026-06-11T00:38:46Z".to_string(),
            )]),
            skipped_codex_infrastructure_failures: 2,
            skipped_codex_infrastructure_scenarios: BTreeMap::from([(
                "config-migration".to_string(),
                2,
            )]),
            skipped_codex_infrastructure_retry_hints: BTreeMap::from([(
                "config-migration".to_string(),
                "try again at 5:38 PM".to_string(),
            )]),
            skipped_opencode_infrastructure_failures: 1,
            skipped_opencode_infrastructure_scenarios: BTreeMap::from([(
                "typescript-reducer-bugfix".to_string(),
                1,
            )]),
            skipped_opencode_infrastructure_retry_hints: BTreeMap::new(),
        },
    );

    let html = comparison_rows_to_html("real-world", &rows, &aggregate, &empty_comparison_inputs());

    assert!(html.contains("Spark harness skipped 1 provider/API infrastructure failure row(s)"));
    assert!(html.contains("Codex CLI skipped 2 provider/API infrastructure failure row(s)"));
    assert!(html.contains("OpenCode skipped 1 provider/API infrastructure failure row(s)"));
    assert!(html.contains("Valid Spark rows remain in the comparison"));
    assert!(html.contains("Valid native rows remain in the comparison"));
    assert!(html.contains("Valid OpenCode rows remain in the comparison"));
    assert!(html.contains("Skipped scenarios: precise-patch."));
    assert!(html.contains("Skipped scenarios: config-migration x2."));
    assert!(html.contains("Skipped scenarios: typescript-reducer-bugfix."));
    assert!(
        html.contains("Retry hint for skipped scenarios: try again after 2026-06-11T00:38:46Z.")
    );
    assert!(html.contains("Retry hint for skipped scenarios: try again at 5:38 PM."));
}

#[test]
fn comparison_html_groups_mixed_retry_hints_by_hint() {
    let rows = indexed(vec![comparison_row("spark-harness", "one")]);
    let aggregate = aggregate_comparison_with_diagnostics(
        "real-world",
        &rows,
        ComparisonDiagnostics {
            skipped_spark_infrastructure_failures: 3,
            skipped_spark_infrastructure_scenarios: BTreeMap::from([
                ("config-migration".to_string(), 1),
                ("multi-file-patch".to_string(), 1),
                ("precise-patch".to_string(), 1),
            ]),
            skipped_spark_infrastructure_retry_hints: BTreeMap::from([
                (
                    "config-migration".to_string(),
                    "try again after 2026-06-11T00:38:46Z".to_string(),
                ),
                (
                    "multi-file-patch".to_string(),
                    "try again after 2026-06-11T00:38:46Z".to_string(),
                ),
                (
                    "precise-patch".to_string(),
                    "try again after 2026-06-11T01:00:00Z".to_string(),
                ),
            ]),
            ..ComparisonDiagnostics::default()
        },
    );

    let html = comparison_rows_to_html("real-world", &rows, &aggregate, &empty_comparison_inputs());

    assert!(html.contains(
        "Retry hints: config-migration, multi-file-patch: try again after 2026-06-11T00:38:46Z"
    ));
    assert!(html.contains("precise-patch: try again after 2026-06-11T01:00:00Z"));
}

#[test]
fn codex_cli_scenario_indices_can_vary_while_average_stays_baseline() {
    let codex_one = comparison_row("codex-cli", "one");
    let mut codex_two = comparison_row("codex-cli", "two");
    codex_two.quality_score = 70.0;
    codex_two.task_quality_score = 70.0;

    let rows = indexed(vec![
        codex_one,
        comparison_row("spark-harness", "one"),
        codex_two,
        comparison_row("spark-harness", "two"),
    ]);
    let aggregate = aggregate_comparison("real-world", &rows);
    let codex_indices = rows
        .iter()
        .filter(|row| row.runner == "codex-cli")
        .map(|row| row.benchmark_index.expect("index"))
        .collect::<Vec<_>>();

    assert_eq!(
        aggregate["matched_runner_benchmark_index_averages"]["codex-cli"],
        100.0
    );
    assert!(codex_indices.iter().any(|index| *index != 100.0));
}

#[test]
fn spark_can_exceed_100_when_quality_matches_and_efficiency_wins() {
    let codex = comparison_row("codex-cli", "one");
    let mut spark = comparison_row("spark-harness", "one");
    spark.duration_ms = 5_000;
    spark.input_tokens = 4_000;
    spark.tool_or_item_calls = 4;
    spark.source_bytes = 1_000;

    let rows = indexed(vec![spark, codex]);
    let aggregate = aggregate_comparison("real-world", &rows);

    assert_eq!(aggregate["winner"]["runner"], "spark-harness");
    assert!(
        aggregate["matched_runner_benchmark_index_averages"]["spark-harness"]
            .as_f64()
            .unwrap()
            > 100.0
    );
    assert_eq!(aggregate["headline"]["winner"], "spark-harness");
    assert_eq!(aggregate["headline"]["baseline_runner"], "codex-cli");
    assert_eq!(aggregate["headline"]["baseline_benchmark_index"], 100.0);
    assert!(
        aggregate["headline"]["benchmark_index_margin_vs_baseline"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(aggregate["headline"]["winner_beats_baseline"], true);
}

#[test]
fn benchmark_index_rewards_faster_equal_quality_even_with_process_pressure() {
    let codex = comparison_row("codex-cli", "one");
    let mut spark = comparison_row("spark-harness", "one");
    spark.duration_ms = 2_500;
    spark.input_tokens = 4_000;
    spark.tool_or_item_calls = 6;
    spark.process_score = 70.0;
    spark.harness_pressure_score = 70.0;

    let rows = indexed(vec![spark, codex]);
    let spark = rows
        .iter()
        .find(|row| row.runner == "spark-harness")
        .unwrap();

    assert!(spark.benchmark_index.unwrap() > 100.0);
}

#[test]
fn benchmark_index_reflects_quality_gated_throughput() {
    let codex = comparison_row("codex-cli", "one");
    let mut spark = comparison_row("spark-harness", "one");
    spark.duration_ms = 1_000;
    spark.input_tokens = 1_000;
    spark.tool_or_item_calls = 2;

    let rows = indexed(vec![spark, codex]);
    let spark = rows
        .iter()
        .find(|row| row.runner == "spark-harness")
        .unwrap();

    assert!(spark.benchmark_index.unwrap() >= 200.0);
}

#[test]
fn comparison_aggregate_uses_matched_rows_for_winner() {
    let mut matched_spark = comparison_row("spark-harness", "matched");
    matched_spark.duration_ms = 20_000;
    matched_spark.input_tokens = 20_000;
    matched_spark.tool_or_item_calls = 20;
    let rows = indexed(vec![
        matched_spark,
        comparison_row("codex-cli", "matched"),
        comparison_row("spark-harness", "spark-only-new-coverage"),
    ]);

    let aggregate = aggregate_comparison("real-world", &rows);

    assert_eq!(aggregate["winner"]["runner"], "codex-cli");
    assert_eq!(aggregate["matched_rows"], 2);
    assert_eq!(
        aggregate["unmatched_scenarios"][0]["scenario"],
        "spark-only-new-coverage"
    );
}

#[test]
fn failed_validation_caps_benchmark_index_below_successful_runs() {
    let codex = comparison_row("codex-cli", "one");
    let mut spark = comparison_row("spark-harness", "one");
    spark.success = false;
    spark.completion_score = 50.0;
    spark.quality_score = 50.0;
    spark.validation_exit_code = Some(1);
    spark.duration_ms = 100;
    spark.input_tokens = 100;
    spark.tool_or_item_calls = 1;

    let rows = indexed(vec![spark, codex]);
    let spark = rows
        .iter()
        .find(|row| row.runner == "spark-harness")
        .unwrap();
    let codex = rows.iter().find(|row| row.runner == "codex-cli").unwrap();

    assert!(spark.benchmark_index.unwrap() <= 50.0);
    assert!(spark.benchmark_index.unwrap() < codex.benchmark_index.unwrap());
}

#[test]
fn llm_judge_scores_steer_quality_and_process_when_present() {
    let mut rows = vec![
        comparison_row("codex-cli", "matched"),
        comparison_row("spark-harness", "matched"),
    ];
    let report = BenchmarkJudgeReport {
        suite: "real-world".to_string(),
        comparison_report: "comparison.json".to_string(),
        generated_at_unix_ms: 1,
        rows: vec![crate::benchmark::judge::BenchmarkJudgeScenario {
            scenario: "matched".to_string(),
            scores: vec![crate::benchmark::judge::BenchmarkJudgeRunnerScore {
                runner: "spark-harness".to_string(),
                solution_score: 60.0,
                process_score: 40.0,
                confidence: 80.0,
                notes: "passed but over-edited".to_string(),
            }],
            verdict: "codex-cli".to_string(),
            rationale: "Codex was cleaner.".to_string(),
            raw_response: String::new(),
        }],
    };

    apply_llm_judge_scores(&mut rows, &report);
    let spark = rows
        .iter()
        .find(|row| row.runner == "spark-harness")
        .expect("spark row");

    assert_eq!(spark.llm_solution_score, Some(60.0));
    assert_eq!(spark.quality_score, 60.0);
    assert_eq!(spark.task_quality_score, 60.0);
    assert_eq!(spark.process_score, 40.0);
    assert_eq!(spark.harness_pressure_score, 40.0);
    assert!(spark.llm_notes.contains("over-edited"));
}

#[test]
fn completion_score_penalizes_validation_failure() {
    let mut row = benchmark_row();
    row.success = false;
    row.validation_exit_code = Some(1);
    row.failure_points = "validation_failed".to_string();

    let completion = completion_score(&row);
    row.completion_score = completion;
    assert_eq!(completion, 30.0);
    assert!(quality_score(&row) <= completion);
}

#[test]
fn completion_score_penalizes_browser_validation_failure() {
    let mut row = benchmark_row();
    row.success = false;
    row.browser_validation_present = true;
    row.browser_validation_exit_code = Some(1);
    row.browser_screenshot = ".spark-profile/browser/react-calculator.png".to_string();
    row.failure_points = "browser_validation_failed".to_string();

    let completion = completion_score(&row);
    row.completion_score = completion;
    assert!(completion < 20.0);
    assert!(quality_score(&row) < 35.0);
}

#[test]
fn output_quality_does_not_double_penalize_process_pressure() {
    let mut clean = benchmark_row();
    clean.completion_score = 100.0;
    clean.process_score = 100.0;

    let mut pressured = clean.clone();
    pressured.process_score = 45.0;
    pressured.truncated_tool_results = 3;
    pressured.compactions = 2;
    pressured.repeated_tool_calls = 4;
    pressured.recovered_tool_failures = 2;
    pressured.max_tool_only_streak = 9;

    assert_eq!(quality_score(&pressured), quality_score(&clean));
    assert!(process_score(&pressured) < process_score(&clean));
}

#[test]
fn comparison_csv_keeps_legacy_prefix_and_appends_index_columns() {
    let rows = indexed(vec![
        comparison_row("codex-cli", "one"),
        comparison_row("spark-harness", "one"),
    ]);
    let csv = comparison_rows_to_csv(&rows);

    assert!(csv.starts_with(
        "runner,suite,scenario,model,reasoning_effort,command_path,command_version,score,task_quality_score,efficiency_score,harness_pressure_score"
    ));
    assert!(csv.contains(
        ",failure_points,source,completion_score,quality_score,process_score,llm_solution_score,llm_process_score,llm_confidence,llm_notes,efficiency_index,benchmark_index\n"
    ));
    assert!(!csv.lines().next().unwrap().contains("requests_or_turns"));
}

#[test]
fn comparison_html_uses_index_language_with_separate_completion_and_quality() {
    let rows = indexed(vec![
        comparison_row("codex-cli", "one"),
        comparison_row("spark-harness", "one"),
        comparison_row("opencode", "one"),
        comparison_row("spark-harness", "shell-recovery"),
    ]);
    let aggregate = aggregate_comparison("real-world", &rows);
    let html = comparison_rows_to_html("real-world", &rows, &aggregate, &empty_comparison_inputs());

    assert!(html.contains("Benchmark Index"));
    assert!(html.contains(">Completion<"));
    assert!(html.contains(">Quality<"));
    assert!(html.contains(">OpenCode<"));
    assert!(html.contains("model-strip"));
    assert!(html.contains("spark (medium)"));
    assert!(html.contains("<th>Model</th>"));
    assert!(html.contains("runner-opencode"));
    assert!(html.contains("Coverage Expansion"));
    assert!(html.contains("Per-Scenario Deltas"));
    assert!(html.contains("Raw request and turn counts are intentionally excluded"));
    assert!(html.contains("Both runners used about the same number of action items"));
    assert!(!html.contains("Turns/Requests"));
}

#[test]
fn process_score_tracks_successful_runs_with_process_pressure() {
    let mut row = benchmark_row();
    row.success = true;
    row.completion_score = 100.0;
    row.extra_calls_after_satisfied = 4;
    row.extra_turns_after_satisfied = 2;
    row.truncated_tool_results = 2;
    row.compactions = 1;
    row.max_tool_only_streak = 7;
    row.total_duration_ms = 55_000;

    let quality = quality_score(&row);
    let process = process_score(&row);

    assert!(quality > 95.0);
    assert!(process < 85.0);
}

#[test]
fn headline_score_includes_successful_process_pressure() {
    let mut clean = benchmark_row();
    clean.completion_score = 100.0;
    clean.quality_score = 100.0;
    clean.process_score = 100.0;
    clean.efficiency_score = 100.0;

    let mut pressured = clean.clone();
    pressured.process_score = 70.0;

    assert_eq!(benchmark_score(&clean), 100.0);
    assert_eq!(benchmark_score(&pressured), 97.0);
}

#[test]
fn benchmark_index_uses_process_as_quality_gate() {
    let codex = comparison_row("codex-cli", "one");
    let mut clean = comparison_row("spark-clean", "one");
    clean.duration_ms = 2_500;
    clean.input_tokens = 4_000;
    clean.tool_or_item_calls = 6;

    let mut pressured = clean.clone();
    pressured.runner = "spark-pressured".to_string();
    pressured.process_score = 70.0;
    pressured.harness_pressure_score = 70.0;

    let rows = indexed(vec![codex, clean, pressured]);
    let clean = rows.iter().find(|row| row.runner == "spark-clean").unwrap();
    let pressured = rows
        .iter()
        .find(|row| row.runner == "spark-pressured")
        .unwrap();

    assert!(clean.benchmark_index.unwrap() > pressured.benchmark_index.unwrap());
}

#[test]
fn external_process_tracks_long_noisy_successes() {
    let row = CodexCliBenchmarkRow {
        runner: "opencode".to_string(),
        suite: "real-world".to_string(),
        scenario: "technical-essay".to_string(),
        repeat_index: 1,
        model: "opencode-default".to_string(),
        command_path: String::new(),
        command_version: String::new(),
        reasoning_effort: "medium".to_string(),
        score: 0.0,
        success: true,
        exit_code: Some(0),
        timed_out: false,
        duration_ms: 95_000,
        json_events: 40,
        non_json_stdout_lines: 0,
        stderr_lines: 0,
        actionable_stderr_lines: 0,
        turns: 8,
        completed_items: 40,
        agent_messages: 5,
        tool_items: 15,
        input_tokens: 12_000,
        cached_input_tokens: 0,
        output_tokens: 2_000,
        reasoning_output_tokens: 0,
        expected_artifacts: 1,
        present_artifacts: 1,
        validation_exit_code: Some(0),
        validation_timed_out: false,
        browser_validation_present: false,
        browser_validation_exit_code: None,
        browser_validation_timed_out: false,
        browser_screenshot: String::new(),
        source_files: 1,
        source_bytes: 500,
        final_message_chars: 400,
        run_dir: "run".to_string(),
        provider_retry_hint: String::new(),
        failure_points: String::new(),
    };

    let quality = codex_quality_score(&row, 100.0);
    let process = codex_process_score(&row);

    assert!(quality >= 96.0);
    assert!(process < 100.0);
}

#[test]
fn external_headline_score_includes_process_pressure() {
    let row = CodexCliBenchmarkRow {
        runner: "opencode".to_string(),
        suite: "real-world".to_string(),
        scenario: "technical-essay".to_string(),
        repeat_index: 1,
        model: "opencode-default".to_string(),
        command_path: String::new(),
        command_version: String::new(),
        reasoning_effort: "medium".to_string(),
        score: 0.0,
        success: true,
        exit_code: Some(0),
        timed_out: false,
        duration_ms: 10_000,
        json_events: 20,
        non_json_stdout_lines: 0,
        stderr_lines: 0,
        actionable_stderr_lines: 0,
        turns: 4,
        completed_items: 20,
        agent_messages: 4,
        tool_items: 6,
        input_tokens: 4_000,
        cached_input_tokens: 0,
        output_tokens: 1_000,
        reasoning_output_tokens: 0,
        expected_artifacts: 1,
        present_artifacts: 1,
        validation_exit_code: Some(0),
        validation_timed_out: false,
        browser_validation_present: false,
        browser_validation_exit_code: None,
        browser_validation_timed_out: false,
        browser_screenshot: String::new(),
        source_files: 1,
        source_bytes: 500,
        final_message_chars: 400,
        run_dir: "run".to_string(),
        provider_retry_hint: String::new(),
        failure_points: String::new(),
    };

    let clean = codex_score_from_components(&row, 100.0, 100.0, 100.0, 100.0);
    let pressured = codex_score_from_components(&row, 100.0, 100.0, 70.0, 100.0);

    assert_eq!(clean, 100.0);
    assert_eq!(pressured, 97.0);
}

#[test]
fn benchmark_index_chart_excludes_coverage_only_rows() {
    let rows = indexed(vec![
        comparison_row("codex-cli", "one"),
        comparison_row("opencode", "one"),
        comparison_row("spark-harness", "one"),
        comparison_row("spark-harness", "shell-recovery"),
    ]);
    let svg = comparison_score_svg(&rows);

    assert!(svg.contains("Benchmark index comparison grouped by scenario"));
    assert!(svg.contains(">Spark<"));
    assert!(svg.contains(">Codex CLI<"));
    assert!(svg.contains(">OpenCode<"));
    assert!(svg.contains("one / spark-harness"));
    assert!(!svg.contains("shell-recovery / spark-harness"));
}

#[test]
fn benchmark_index_chart_renders_grouped_runner_variants() {
    let mut rows = vec![
        comparison_row("codex-cli/low", "one"),
        comparison_row("opencode/low", "one"),
        comparison_row("spark-harness/high", "one"),
        comparison_row("spark-harness/low", "one"),
        comparison_row("spark-harness/medium", "one"),
    ];
    for row in &mut rows {
        row.benchmark_index = Some(100.0);
    }
    let svg = comparison_score_svg(&rows);

    assert!(svg.contains(">Spark low<"));
    assert!(svg.contains(">Spark medium<"));
    assert!(svg.contains(">Spark high<"));
    assert!(svg.contains(">Codex CLI low<"));
    assert!(svg.contains(">OpenCode low<"));
    assert!(svg.contains("one / spark-harness/medium"));
}
