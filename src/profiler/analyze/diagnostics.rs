use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::actions::RequiredActionReport;
use super::trace_utils::sanitize_profile_summary;

pub(super) struct AnalysisReports<'a> {
    pub(super) timeline: BTreeMap<usize, Map<String, Value>>,
    pub(super) trace_metadata: Option<Value>,
    pub(super) embedded_profile_summary: Option<Value>,
    pub(super) required_action_report: &'a RequiredActionReport,
    pub(super) loaded_skill_contexts: &'a BTreeSet<String>,
    pub(super) tool_only_turn_report: &'a Value,
    pub(super) compaction_regrowth_report: &'a Value,
    pub(super) scenario_tool_expectation_report: &'a Option<Value>,
    pub(super) scenario_call_expectation_report: &'a Option<Value>,
    pub(super) scenario_skill_expectation_report: &'a Option<Value>,
    pub(super) tool_failure_recovery_report: &'a Option<Value>,
    pub(super) cmd_exec_scope_report: &'a Option<Value>,
}

pub(super) fn insert_analysis_reports(
    object: &mut Map<String, Value>,
    reports: AnalysisReports<'_>,
) {
    insert_report_fields(object, &reports);
    append_diagnostics(object, &reports);
    object.insert(
        "timeline".to_string(),
        Value::Array(reports.timeline.into_values().map(Value::Object).collect()),
    );
    if let Some(metadata) = reports.trace_metadata {
        object.insert("trace_metadata".to_string(), metadata);
    }
    if let Some(embedded) = reports.embedded_profile_summary {
        object.insert(
            "embedded_profile_summary".to_string(),
            sanitize_profile_summary(embedded),
        );
    }
}

fn insert_report_fields(object: &mut Map<String, Value>, reports: &AnalysisReports<'_>) {
    object.insert(
        "retained_required_actions".to_string(),
        json!(&reports.required_action_report.actions),
    );
    object.insert(
        "retained_required_actions_executed".to_string(),
        json!(&reports.required_action_report.executed),
    );
    object.insert(
        "retained_required_actions_missing".to_string(),
        json!(&reports.required_action_report.missing),
    );
    object.insert(
        "tool_calls_before_first_required_action".to_string(),
        json!(
            reports
                .required_action_report
                .calls_before_first_required_action
        ),
    );
    object.insert(
        "loaded_skill_contexts".to_string(),
        json!(reports.loaded_skill_contexts.iter().collect::<Vec<_>>()),
    );
    object.insert(
        "tool_only_turns".to_string(),
        reports.tool_only_turn_report.clone(),
    );
    if reports
        .compaction_regrowth_report
        .get("count")
        .and_then(Value::as_u64)
        .is_some_and(|count| count > 0)
    {
        object.insert(
            "compaction_regrowth".to_string(),
            reports.compaction_regrowth_report.clone(),
        );
    }
    insert_optional_report(
        object,
        "profile_scenario_tool_expectations",
        reports.scenario_tool_expectation_report,
    );
    insert_optional_report(
        object,
        "profile_scenario_call_expectations",
        reports.scenario_call_expectation_report,
    );
    insert_optional_report(
        object,
        "profile_scenario_skill_expectations",
        reports.scenario_skill_expectation_report,
    );
    insert_optional_report(
        object,
        "tool_failure_recovery",
        reports.tool_failure_recovery_report,
    );
    insert_optional_report(
        object,
        "cmd_exec_scope",
        reports.cmd_exec_scope_report,
    );
}

fn insert_optional_report(object: &mut Map<String, Value>, key: &str, report: &Option<Value>) {
    if let Some(report) = report {
        object.insert(key.to_string(), report.clone());
    }
}

fn append_diagnostics(object: &mut Map<String, Value>, reports: &AnalysisReports<'_>) {
    let response_text_chars = object
        .get("response_text_chars")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let compactions = object
        .get("compactions")
        .cloned()
        .unwrap_or_else(|| json!(0));
    let remote_compactions = object
        .get("remote_compactions")
        .cloned()
        .unwrap_or_else(|| json!(0));
    let fallback_compactions = object
        .get("fallback_compactions")
        .cloned()
        .unwrap_or_else(|| json!(0));

    let Some(diagnostics) = object.get_mut("diagnostics").and_then(Value::as_array_mut) else {
        return;
    };
    append_required_action_diagnostics(diagnostics, reports.required_action_report);
    append_scenario_diagnostics(diagnostics, reports);
    append_compaction_regrowth_diagnostic(diagnostics, reports.compaction_regrowth_report);
    append_tool_only_diagnostics(
        diagnostics,
        reports.tool_only_turn_report,
        response_text_chars,
        compactions,
        remote_compactions,
        fallback_compactions,
    );
    append_tool_failure_recovery_diagnostics(diagnostics, reports.tool_failure_recovery_report);
    append_cmd_exec_scope_diagnostics(diagnostics, reports.cmd_exec_scope_report);
}

fn append_required_action_diagnostics(diagnostics: &mut Vec<Value>, report: &RequiredActionReport) {
    if !report.missing.is_empty() {
        diagnostics.push(json!({
            "level": "warning",
            "kind": "retained_required_actions_missing",
            "message": "One or more required actions retained by local compaction were not observed in the trace tool calls.",
            "missing": &report.missing,
        }));
    }
    if report.calls_before_first_required_action > 0 {
        diagnostics.push(json!({
            "level": "info",
            "kind": "retained_required_action_detour",
            "message": "Spark made tool calls before executing the first required action retained by local compaction.",
            "calls_before_first_required_action": report.calls_before_first_required_action,
        }));
    }
}

fn append_scenario_diagnostics(diagnostics: &mut Vec<Value>, reports: &AnalysisReports<'_>) {
    if let Some(report) = reports.scenario_tool_expectation_report
        && report
            .get("missing_groups")
            .and_then(Value::as_array)
            .is_some_and(|missing| !missing.is_empty())
    {
        diagnostics.push(json!({
            "level": "warning",
            "kind": "profile_scenario_expected_tools_missing",
            "message": "The trace did not include all native tool groups expected for this profiling scenario.",
            "missing_groups": report.get("missing_groups").cloned().unwrap_or_else(|| json!([])),
        }));
    }
    if let Some(report) = reports.scenario_call_expectation_report
        && report
            .get("missing_calls")
            .and_then(Value::as_array)
            .is_some_and(|missing| !missing.is_empty())
    {
        diagnostics.push(json!({
            "level": "warning",
            "kind": "profile_scenario_expected_calls_missing",
            "message": "The trace did not include all exact native tool calls expected for this profiling scenario.",
            "missing_calls": report.get("missing_calls").cloned().unwrap_or_else(|| json!([])),
        }));
    }
    append_scenario_overrun_diagnostic(diagnostics, reports.scenario_call_expectation_report);
    if let Some(report) = reports.scenario_skill_expectation_report
        && report
            .get("missing_skills")
            .and_then(Value::as_array)
            .is_some_and(|missing| !missing.is_empty())
    {
        diagnostics.push(json!({
            "level": "warning",
            "kind": "profile_scenario_expected_skills_missing",
            "message": "The trace did not include all loaded skill contexts expected for this profiling scenario.",
            "missing_skills": report.get("missing_skills").cloned().unwrap_or_else(|| json!([])),
        }));
    }
}

fn append_scenario_overrun_diagnostic(diagnostics: &mut Vec<Value>, report: &Option<Value>) {
    if let Some(report) = report
        && report
            .get("extra_calls_after_satisfied")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0)
    {
        diagnostics.push(json!({
            "level": "info",
            "kind": "profile_scenario_extra_calls_after_expected",
            "message": "Spark satisfied all exact native tool calls expected for this profiling scenario, then made additional tool calls before completing.",
            "extra_calls_after_satisfied": report.get("extra_calls_after_satisfied").cloned().unwrap_or_else(|| json!(0)),
            "extra_turns_after_satisfied": report.get("extra_turns_after_satisfied").cloned().unwrap_or_else(|| json!(0)),
            "context_growth_after_satisfied_chars": report.get("context_growth_after_satisfied_chars").cloned().unwrap_or_else(|| json!(0)),
            "first_satisfied_call_index": report.get("first_satisfied_call_index").cloned().unwrap_or(Value::Null),
            "first_satisfied_turn": report.get("first_satisfied_turn").cloned().unwrap_or(Value::Null),
        }));
    }
}

fn append_compaction_regrowth_diagnostic(diagnostics: &mut Vec<Value>, report: &Value) {
    if report
        .get("max_next_request_growth_chars")
        .and_then(Value::as_u64)
        .is_some_and(|chars| chars >= 100_000)
    {
        diagnostics.push(json!({
            "level": "info",
            "kind": "post_compaction_context_regrowth",
            "message": "Request input grew substantially after a compaction boundary. Compare the compaction_regrowth report with subsequent tool calls before tuning thresholds.",
            "max_same_turn_growth_chars": report.get("max_same_turn_growth_chars").cloned().unwrap_or_else(|| json!(0)),
            "max_next_request_growth_chars": report.get("max_next_request_growth_chars").cloned().unwrap_or_else(|| json!(0)),
        }));
    }
}

fn append_tool_only_diagnostics(
    diagnostics: &mut Vec<Value>,
    report: &Value,
    response_text_chars: u64,
    compactions: Value,
    remote_compactions: Value,
    fallback_compactions: Value,
) {
    if report
        .get("max_consecutive")
        .and_then(Value::as_u64)
        .is_some_and(|count| count >= 3)
    {
        diagnostics.push(json!({
            "level": "info",
            "kind": "tool_only_turn_streak",
            "message": "Spark spent several consecutive turns calling tools without producing user-facing text. Compare this with scenario completion and context growth before changing harness defaults.",
            "count": report.get("count").cloned().unwrap_or_else(|| json!(0)),
            "max_consecutive": report.get("max_consecutive").cloned().unwrap_or_else(|| json!(0)),
            "turns": report.get("turns").cloned().unwrap_or_else(|| json!([])),
        }));
    }
    if report
        .get("max_consecutive")
        .and_then(Value::as_u64)
        .is_some_and(|count| count >= 8)
        && response_text_chars == 0
        && !diagnostics
            .iter()
            .any(|diagnostic| diagnostic["kind"] == "completion_starvation")
    {
        diagnostics.push(json!({
            "level": "warning",
            "kind": "completion_starvation",
            "message": "Spark kept calling tools across many turns without emitting any user-facing response text. Profile tool-call sequence, compaction timing, and context growth before adding stop conditions or changing defaults.",
            "tool_only_turns": report.get("count").cloned().unwrap_or_else(|| json!(0)),
            "max_consecutive": report.get("max_consecutive").cloned().unwrap_or_else(|| json!(0)),
            "compactions": compactions,
            "remote_compactions": remote_compactions,
            "fallback_compactions": fallback_compactions,
        }));
    }
}

fn append_cmd_exec_scope_diagnostics(diagnostics: &mut Vec<Value>, report: &Option<Value>) {
    let Some(report) = report else {
        return;
    };
    let probes = report.get("probes").and_then(Value::as_array).map_or(0, Vec::len);
    if probes == 0 {
        return;
    }
    diagnostics.push(json!({
        "level": "warning",
        "kind": "cmd_exec_out_of_scope",
        "message": "cmd.exec tool calls referenced paths outside the benchmark workspace (.spark-runs, .spark-profile, .spark-scenarios). These paths may indicate the agent probed trace dirs, benchmark reports, or scenario fixtures.",
        "probe_count": probes,
        "probes": report.get("probes").cloned().unwrap_or_else(|| json!([])),
    }));
}

fn append_tool_failure_recovery_diagnostics(diagnostics: &mut Vec<Value>, report: &Option<Value>) {
    let Some(report) = report else {
        return;
    };
    if report
        .get("recovered_failures")
        .and_then(Value::as_u64)
        .is_some_and(|count| count > 0)
    {
        diagnostics.push(json!({
            "level": "info",
            "kind": "tool_failure_recovered",
            "message": "Spark recovered from one or more failed native tool observations later in the trace.",
            "recovered_failures": report.get("recovered_failures").cloned().unwrap_or_else(|| json!(0)),
            "failed_tool_results": report.get("failed_tool_results").cloned().unwrap_or_else(|| json!(0)),
        }));
    }
    if report
        .get("unrecovered_failures")
        .and_then(Value::as_u64)
        .is_some_and(|count| count > 0)
    {
        diagnostics.push(json!({
            "level": "warning",
            "kind": "tool_failure_unrecovered",
            "message": "One or more failed native tool observations had no later successful observation from the same tool.",
            "unrecovered_failures": report.get("unrecovered_failures").cloned().unwrap_or_else(|| json!(0)),
            "failed_tool_results": report.get("failed_tool_results").cloned().unwrap_or_else(|| json!(0)),
        }));
    }
}
