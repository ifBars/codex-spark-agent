use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};

use serde_json::Value;

use super::{
    AverageIterator, ComparisonRow, compare_comparison_rows, html_escape, ratio_or_zero, round1,
    scenarios::{scenario_family, scenario_question},
};

fn skipped_infrastructure_scenario_summary(aggregate: &Value, runner: &str) -> String {
    let Some(scenarios) = aggregate
        .pointer(&format!(
            "/diagnostics/skipped_infrastructure_scenarios/{runner}"
        ))
        .and_then(Value::as_object)
    else {
        return String::new();
    };
    if scenarios.is_empty() {
        return String::new();
    }
    let mut parts = scenarios
        .iter()
        .filter_map(|(scenario, count)| {
            let count = count.as_u64()?;
            (count > 0).then(|| {
                if count == 1 {
                    html_escape(scenario)
                } else {
                    format!("{} x{}", html_escape(scenario), count)
                }
            })
        })
        .collect::<Vec<_>>();
    parts.sort();
    if parts.is_empty() {
        String::new()
    } else {
        format!(" Skipped scenarios: {}.", parts.join(", "))
    }
}

fn skipped_infrastructure_retry_hint_summary(aggregate: &Value, runner: &str) -> String {
    let Some(hints) = aggregate
        .pointer(&format!(
            "/diagnostics/skipped_infrastructure_retry_hints/{runner}"
        ))
        .and_then(Value::as_object)
    else {
        return String::new();
    };
    if hints.is_empty() {
        return String::new();
    }
    let mut scenarios_by_hint = BTreeMap::<String, Vec<String>>::new();
    for (scenario, hint) in hints {
        let Some(hint) = hint.as_str().map(str::trim) else {
            continue;
        };
        if hint.is_empty() {
            continue;
        }
        scenarios_by_hint
            .entry(hint.to_string())
            .or_default()
            .push(scenario.clone());
    }
    if scenarios_by_hint.is_empty() {
        String::new()
    } else if scenarios_by_hint.len() == 1 {
        let hint = scenarios_by_hint
            .keys()
            .next()
            .expect("non-empty retry hints");
        format!(" Retry hint for skipped scenarios: {}.", html_escape(hint))
    } else {
        let mut parts = scenarios_by_hint
            .into_iter()
            .map(|(hint, mut scenarios)| {
                scenarios.sort();
                format!(
                    "{}: {}",
                    html_escape(&scenarios.join(", ")),
                    html_escape(&hint)
                )
            })
            .collect::<Vec<_>>();
        parts.sort();
        format!(" Retry hints: {}.", parts.join(", "))
    }
}

pub(super) fn comparison_rows_to_html(
    suite: &str,
    rows: &[ComparisonRow],
    aggregate: &Value,
    inputs: &Value,
) -> String {
    let winner = aggregate
        .pointer("/winner/runner")
        .and_then(Value::as_str)
        .unwrap_or("undetermined");
    let matched_rows = aggregate
        .get("matched_rows")
        .and_then(Value::as_u64)
        .unwrap_or(rows.len() as u64);
    let unmatched_rows = rows.len().saturating_sub(matched_rows as usize);
    let spark_index = aggregate_metric(
        aggregate,
        "matched_runner_benchmark_index_averages",
        "spark-harness",
    );
    let codex_index = aggregate_metric(
        aggregate,
        "matched_runner_benchmark_index_averages",
        "codex-cli",
    );
    let opencode_index = aggregate_metric(
        aggregate,
        "matched_runner_benchmark_index_averages",
        "opencode",
    );
    let spark_completion = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_completion_score_averages",
        "runner_completion_score_averages",
        "spark-harness",
    );
    let codex_completion = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_completion_score_averages",
        "runner_completion_score_averages",
        "codex-cli",
    );
    let opencode_completion = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_completion_score_averages",
        "runner_completion_score_averages",
        "opencode",
    );
    let spark_duration = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_duration_ms_averages",
        "runner_duration_ms_averages",
        "spark-harness",
    );
    let codex_duration = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_duration_ms_averages",
        "runner_duration_ms_averages",
        "codex-cli",
    );
    let opencode_duration = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_duration_ms_averages",
        "runner_duration_ms_averages",
        "opencode",
    );
    let spark_calls = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_tool_or_item_call_averages",
        "runner_tool_or_item_call_averages",
        "spark-harness",
    );
    let codex_calls = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_tool_or_item_call_averages",
        "runner_tool_or_item_call_averages",
        "codex-cli",
    );
    let opencode_calls = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_tool_or_item_call_averages",
        "runner_tool_or_item_call_averages",
        "opencode",
    );
    let spark_tokens = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_input_token_averages",
        "runner_input_token_averages",
        "spark-harness",
    );
    let codex_tokens = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_input_token_averages",
        "runner_input_token_averages",
        "codex-cli",
    );
    let opencode_tokens = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_input_token_averages",
        "runner_input_token_averages",
        "opencode",
    );
    let spark_pressure = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_process_score_averages",
        "runner_process_score_averages",
        "spark-harness",
    );
    let codex_pressure = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_process_score_averages",
        "runner_process_score_averages",
        "codex-cli",
    );
    let opencode_pressure = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_process_score_averages",
        "runner_process_score_averages",
        "opencode",
    );
    let spark_quality = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_quality_score_averages",
        "runner_quality_score_averages",
        "spark-harness",
    );
    let codex_quality = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_quality_score_averages",
        "runner_quality_score_averages",
        "codex-cli",
    );
    let opencode_quality = aggregate_metric_with_fallback(
        aggregate,
        "matched_runner_quality_score_averages",
        "runner_quality_score_averages",
        "opencode",
    );
    let spark_present = runner_family_present(rows, "spark-harness");
    let codex_present = runner_family_present(rows, "codex-cli");
    let opencode_present = runner_family_present(rows, "opencode");
    let spark_skipped_infrastructure = aggregate
        .pointer("/diagnostics/skipped_infrastructure_failures/spark-harness")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let codex_skipped_infrastructure = aggregate
        .pointer("/diagnostics/skipped_infrastructure_failures/codex-cli")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let opencode_skipped_infrastructure = aggregate
        .pointer("/diagnostics/skipped_infrastructure_failures/opencode")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let spark_skipped_scenarios =
        skipped_infrastructure_scenario_summary(aggregate, "spark-harness");
    let codex_skipped_scenarios = skipped_infrastructure_scenario_summary(aggregate, "codex-cli");
    let opencode_skipped_scenarios = skipped_infrastructure_scenario_summary(aggregate, "opencode");
    let spark_skipped_retry_hints =
        skipped_infrastructure_retry_hint_summary(aggregate, "spark-harness");
    let codex_skipped_retry_hints =
        skipped_infrastructure_retry_hint_summary(aggregate, "codex-cli");
    let opencode_skipped_retry_hints =
        skipped_infrastructure_retry_hint_summary(aggregate, "opencode");
    let spark_index_text = metric_display(spark_index, spark_present);
    let codex_index_text = metric_display(codex_index, codex_present);
    let opencode_index_text = metric_display(opencode_index, opencode_present);
    let spark_completion_text = metric_display(spark_completion, spark_present);
    let codex_completion_text = metric_display(codex_completion, codex_present);
    let opencode_completion_text = metric_display(opencode_completion, opencode_present);
    let spark_quality_text = metric_display(spark_quality, spark_present);
    let codex_quality_text = metric_display(codex_quality, codex_present);
    let opencode_quality_text = metric_display(opencode_quality, opencode_present);
    let spark_duration_text = duration_display(spark_duration, spark_present);
    let codex_duration_text = duration_display(codex_duration, codex_present);
    let opencode_duration_text = duration_display(opencode_duration, opencode_present);
    let spark_calls_text = metric_display(spark_calls, spark_present);
    let codex_calls_text = metric_display(codex_calls, codex_present);
    let opencode_calls_text = metric_display(opencode_calls, opencode_present);
    let spark_tokens_text = metric_display(spark_tokens, spark_present);
    let codex_tokens_text = metric_display(codex_tokens, codex_present);
    let opencode_tokens_text = metric_display(opencode_tokens, opencode_present);
    let spark_pressure_text = metric_display(spark_pressure, spark_present);
    let codex_pressure_text = metric_display(codex_pressure, codex_present);
    let opencode_pressure_text = metric_display(opencode_pressure, opencode_present);
    let mut missing_runner_notes = Vec::<String>::new();
    if !spark_present {
        missing_runner_notes.push("<p class=\"note\">Spark harness has no valid row in this comparison. Provider/API infrastructure failures such as usage-limit or rate-limit errors are excluded instead of being scored as agent output.</p>".to_string());
    } else if spark_skipped_infrastructure > 0 {
        missing_runner_notes.push(format!("<p class=\"note\">Spark harness skipped {} provider/API infrastructure failure row(s). Valid Spark rows remain in the comparison; skipped rows are excluded instead of being scored as agent output.{}{}</p>", spark_skipped_infrastructure, spark_skipped_scenarios, spark_skipped_retry_hints));
    }
    if !codex_present {
        missing_runner_notes.push("<p class=\"note\">Codex CLI has no valid row in this comparison. Provider/API infrastructure failures such as quota, usage-limit, or rate-limit errors are excluded instead of being scored as agent output.</p>".to_string());
    } else if codex_skipped_infrastructure > 0 {
        missing_runner_notes.push(format!("<p class=\"note\">Codex CLI skipped {} provider/API infrastructure failure row(s). Valid native rows remain in the comparison; skipped rows are excluded instead of being scored as agent output.{}{}</p>", codex_skipped_infrastructure, codex_skipped_scenarios, codex_skipped_retry_hints));
    }
    if !opencode_present {
        missing_runner_notes.push("<p class=\"note\">OpenCode has no valid row in this comparison. Provider/API infrastructure failures such as insufficient balance, quota, usage-limit, or rate-limit errors are excluded instead of being scored as agent output.</p>".to_string());
    } else if opencode_skipped_infrastructure > 0 {
        missing_runner_notes.push(format!("<p class=\"note\">OpenCode skipped {} provider/API infrastructure failure row(s). Valid OpenCode rows remain in the comparison; skipped rows are excluded instead of being scored as agent output.{}{}</p>", opencode_skipped_infrastructure, opencode_skipped_scenarios, opencode_skipped_retry_hints));
    }
    let missing_runner_notes = missing_runner_notes.join("");
    let mut html = format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Agent Benchmark Comparison - {}</title>
<style>
:root {{ color-scheme: light; --ink: #18212f; --muted: #596579; --line: #d8dee8; --soft: #eef2f6; --spark: #146c78; --codex: #6a527d; --opencode: #9a641d; --paper: #ffffff; --warn: #9a641d; --bad: #9c3d3d; }}
* {{ box-sizing: border-box; }}
body {{ margin: 0; font-family: Segoe UI, Arial, sans-serif; color: var(--ink); background: #f6f7f9; }}
main {{ max-width: 1240px; margin: 0 auto; padding: 34px 24px 54px; }}
.eyebrow {{ color: var(--muted); font-size: 12px; font-weight: 700; letter-spacing: 0; text-transform: uppercase; }}
h1 {{ margin: 4px 0 10px; font-size: 30px; line-height: 1.15; letter-spacing: 0; }}
h2 {{ margin: 30px 0 12px; font-size: 18px; letter-spacing: 0; }}
p {{ line-height: 1.55; color: var(--muted); max-width: 920px; }}
.hero {{ display: grid; grid-template-columns: minmax(0, 1.45fr) minmax(280px, .75fr); gap: 24px; align-items: end; border-bottom: 1px solid var(--line); padding-bottom: 22px; }}
.verdict {{ border-top: 3px solid var(--ink); border-bottom: 1px solid var(--line); padding: 16px 0 18px; margin: 22px 0 24px; display: grid; gap: 8px; }}
.verdict strong {{ color: var(--ink); }}
.metric-grid {{ display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 10px; margin: 18px 0 24px; }}
.metric {{ background: var(--paper); border: 1px solid var(--line); border-radius: 6px; padding: 13px; min-height: 94px; }}
.metric strong {{ display: block; font-size: 24px; line-height: 1.1; font-variant-numeric: tabular-nums; }}
.metric span {{ display: block; margin-top: 6px; color: var(--muted); font-size: 12px; line-height: 1.35; }}
.note {{ background: #fffaf0; border: 1px solid #ead2a8; border-radius: 6px; padding: 12px 14px; color: #63430e; }}
.ledger {{ background: var(--paper); border: 1px solid var(--line); border-radius: 6px; overflow: auto; margin: 14px 0 24px; }}
table {{ width: 100%; border-collapse: collapse; font-size: 13px; }}
th, td {{ padding: 9px 10px; border-bottom: 1px solid #e4e8ef; text-align: left; vertical-align: top; }}
th {{ color: #344154; font-weight: 700; background: var(--soft); }}
tbody tr:last-child td {{ border-bottom: 0; }}
td.num {{ text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }}
.runner-spark-harness {{ border-left: 3px solid var(--spark); }}
.runner-codex-cli {{ border-left: 3px solid var(--codex); }}
.runner-opencode {{ border-left: 3px solid var(--opencode); }}
.pill {{ display: inline-block; border: 1px solid var(--line); border-radius: 999px; padding: 2px 7px; font-size: 12px; color: var(--muted); white-space: nowrap; }}
.surface {{ color: var(--ink); font-weight: 650; }}
.chart {{ background: var(--paper); border: 1px solid var(--line); border-radius: 6px; padding: 16px; overflow-x: auto; }}
.model-strip {{ display: flex; flex-wrap: wrap; gap: 8px; margin: 0 0 12px; }}
.model-chip {{ border: 1px solid var(--line); border-radius: 6px; padding: 7px 9px; background: #f8fafc; color: #344154; font-size: 12px; line-height: 1.35; }}
.model-chip strong {{ color: var(--ink); }}
.readout {{ border-left: 3px solid var(--ink); padding-left: 14px; margin: 22px 0 8px; }}
svg {{ width: 100%; height: auto; display: block; min-width: 720px; }}
@media (max-width: 860px) {{ .hero {{ grid-template-columns: 1fr; }} .metric-grid {{ grid-template-columns: repeat(2, minmax(0, 1fr)); }} }}
</style>
</head>
<body><main>
<section class="hero">
<div>
<div class="eyebrow">Benchmark Comparison</div>
<h1>Agent Benchmark Comparison: {}</h1>
</div>
<p>Verified real-world tasks are gated by completion first, then ranked with a Codex-baselined benchmark index. The verdict uses matched scenarios only; runner-only new scenarios are shown as coverage expansion.</p>
</section>
<section class="verdict">
<div>Outcome on matched scenarios: <strong>{}</strong> wins this comparison.</div>
<div>Matched rows: <strong>{}</strong>. New coverage rows outside the shared Codex CLI set: <strong>{}</strong>.</div>
<div>Spark Benchmark Index: <strong>{}</strong>. Codex CLI baseline: <strong>{}</strong>. OpenCode: <strong>{}</strong>. Completion: <strong>{}</strong> / <strong>{}</strong> / <strong>{}</strong>.</div>
</section>
<section class="metric-grid">
<div class="metric"><strong>{}</strong><span>Spark Benchmark Index on matched scenarios</span></div>
<div class="metric"><strong>{}</strong><span>Codex CLI baseline index on matched scenarios</span></div>
<div class="metric"><strong>{}</strong><span>OpenCode Benchmark Index on matched scenarios</span></div>
<div class="metric"><strong>{}</strong><span>Spark process score on matched scenarios</span></div>
</section>
"#,
        html_escape(suite),
        html_escape(suite),
        html_escape(winner),
        matched_rows,
        unmatched_rows,
        spark_index_text,
        codex_index_text,
        opencode_index_text,
        spark_completion_text,
        codex_completion_text,
        opencode_completion_text,
        spark_index_text,
        codex_index_text,
        opencode_index_text,
        spark_pressure_text
    );
    if let Some(caveat) = comparison_freshness_caveat(inputs, aggregate) {
        let _ = write!(html, "<p class=\"note\">{}</p>", html_escape(&caveat));
    }
    html.push_str(&missing_runner_notes);
    let _ = write!(
        html,
        "<h2>Evidence Ledger</h2><div class=\"ledger\"><table><thead><tr><th>Metric</th><th>Spark Harness</th><th>Codex CLI</th><th>OpenCode</th><th>Readout</th></tr></thead><tbody>\
         <tr><td>Benchmark Index</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>Codex CLI matched scenarios normalize to 100; other runners can rank above or below that baseline.</td></tr>\
         <tr><td>Completion</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>Completion is bounded and gates failed runs below successful runs.</td></tr>\
         <tr><td>Quality</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>Quality reflects artifact, validation, and footprint checks available for the scenario.</td></tr>\
         <tr><td>Average duration</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td></tr>\
         <tr><td>Average tool/items</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td></tr>\
         <tr><td>Average input tokens</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>Token counts are reported when the runner exposes them.</td></tr>\
         <tr><td>Process score</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>Process penalties are applied after the efficiency index so retries and pressure remain visible.</td></tr>\
         </tbody></table></div>",
        spark_index_text,
        codex_index_text,
        opencode_index_text,
        spark_completion_text,
        codex_completion_text,
        opencode_completion_text,
        spark_quality_text,
        codex_quality_text,
        opencode_quality_text,
        spark_duration_text,
        codex_duration_text,
        opencode_duration_text,
        duration_readout(spark_duration, codex_duration),
        spark_calls_text,
        codex_calls_text,
        opencode_calls_text,
        tool_items_readout(spark_calls, codex_calls),
        spark_tokens_text,
        codex_tokens_text,
        opencode_tokens_text,
        spark_pressure_text,
        codex_pressure_text,
        opencode_pressure_text
    );
    if unmatched_rows > 0 {
        let _ = write!(
            html,
            "<p class=\"note\">{} comparison row(s) are coverage-only because the other runner has no matching scenario in the selected report. They are visible below, but not used for the headline winner or matched averages.</p>",
            unmatched_rows
        );
    }
    html.push_str(
        "<p class=\"note\">Raw request and turn counts are intentionally excluded from scoring and comparison tables because a Spark harness request and a Codex CLI exec turn are not equivalent protocol units. The report compares validation, LLM review, duration, tokens, tool/items, and source footprint instead.</p>",
    );
    html.push_str(
        "<section class=\"readout\"><h2>Professional Readout</h2>\
         <p>Spark performs better when the harness keeps the visible workspace narrow, passes project instructions and environment context in the initial task shape, and validates the produced artifact with real commands or browser checks. The comparison now covers survey/exploration, terminal recovery, precise patching, coordinated multi-file edits, and app scaffolding rather than only broad completion.</p>\
         <p>Codex CLI and OpenCode are kept as external runner evidence: their rows use command output, artifact validation, and source footprint rather than Spark trace internals. The harness now exposes pressure details such as failed tool recovery, extra calls after success, and tool-only streaks without letting those diagnostics override verified task completion.</p></section>",
    );
    html.push_str("<h2>Task Surface Matrix</h2>");
    html.push_str(&comparison_surface_table(rows));
    html.push_str("<h2>Coverage Expansion</h2>");
    html.push_str(&comparison_coverage_table(aggregate));
    html.push_str("<h2>Report Inputs</h2>");
    html.push_str(&comparison_input_table(inputs));
    html.push_str("<h2>Per-Scenario Deltas</h2>");
    html.push_str(&comparison_delta_table(rows));
    html.push_str("<h2>Benchmark Index Comparison</h2><div class=\"chart\">");
    html.push_str(&comparison_model_strip(rows));
    html.push_str(&comparison_score_svg(rows));
    html.push_str("</div><h2>Run Rows</h2><div class=\"ledger\"><table><thead><tr><th>Runner</th><th>Model</th><th>Command</th><th>Scenario</th><th>Attempts</th><th>Benchmark Index</th><th>Completion</th><th>Quality</th><th>Process</th><th>LLM review</th><th>Legacy score</th><th>Validation</th><th>Success</th><th>Duration</th><th>Source footprint</th><th>Items/Tools</th><th>Failure points</th></tr></thead><tbody>");
    for row in rows {
        let validation = row
            .validation_exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| {
                if row.validation_timed_out {
                    "timeout".to_string()
                } else {
                    "n/a".to_string()
                }
            });
        let _ = write!(
            html,
            "<tr class=\"runner-{}\"><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"num\">{}/{}</td><td class=\"num\">{}</td><td class=\"num\">{:.1}</td><td class=\"num\">{:.1}</td><td class=\"num\">{:.1}</td><td>{}</td><td class=\"num\">{:.1}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{} files / {} bytes</td><td class=\"num\">{}</td><td>{}</td></tr>",
            html_escape(&row.runner),
            html_escape(&row.runner),
            html_escape(&model_label(row)),
            html_escape(&command_label(row)),
            html_escape(&row.scenario),
            row.successful_attempts,
            row.attempts,
            optional_metric(row.benchmark_index),
            row.completion_score,
            row.quality_score,
            row.process_score,
            html_escape(&llm_review_cell(row)),
            row.score,
            html_escape(&validation),
            row.success,
            duration_display(
                row.duration_ms
                    .map(|value| value as f64)
                    .unwrap_or_default(),
                row.duration_ms.is_some(),
            ),
            row.source_files,
            row.source_bytes,
            row.tool_or_item_calls
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            html_escape(&row.failure_points)
        );
    }
    html.push_str("</tbody></table></div></main></body></html>");
    html
}

fn comparison_freshness_caveat(inputs: &Value, aggregate: &Value) -> Option<String> {
    let validity = aggregate.pointer("/diagnostics/comparison_validity");
    let warning = validity
        .and_then(|validity| validity.get("mixed_input_warning"))
        .or_else(|| inputs.pointer("/freshness/mixed_input_warning"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !warning {
        return None;
    }
    let span = validity
        .and_then(|validity| validity.get("input_modified_span_label"))
        .or_else(|| inputs.pointer("/freshness/modified_span_label"))
        .and_then(Value::as_str)
        .unwrap_or("n/a");
    let skipped = validity
        .and_then(|validity| validity.get("excluded_provider_api_rows"))
        .or_else(|| aggregate.pointer("/diagnostics/total_skipped_infrastructure_failures"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let directional = validity
        .and_then(|validity| validity.get("directional_until_fresh_paired_run"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            aggregate
                .pointer("/diagnostics/total_skipped_infrastructure_failures")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                > 0
                || warning
        });
    if !directional {
        return None;
    }
    if skipped > 0 {
        Some(format!(
            "Comparison freshness caveat: selected input reports span {span}, and {skipped} provider/API failure row(s) were excluded. Treat headline indices as directional until a fresh paired run completes for each runner."
        ))
    } else {
        Some(format!(
            "Comparison freshness caveat: selected input reports span {span}. Treat headline indices as directional until a fresh paired run completes for each runner."
        ))
    }
}

fn comparison_model_strip(rows: &[ComparisonRow]) -> String {
    let mut runner_models = BTreeMap::<String, BTreeSet<String>>::new();
    for row in rows {
        runner_models
            .entry(row.runner.clone())
            .or_default()
            .insert(model_label(row));
    }
    let mut runners = runner_models.keys().cloned().collect::<Vec<_>>();
    runners.sort_by(|left, right| compare_runner_labels(left, right));
    let mut html = String::from("<div class=\"model-strip\" aria-label=\"Runner models\">");
    for runner in runners {
        let models = runner_models
            .get(&runner)
            .map(|models| models.iter().cloned().collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        let _ = write!(
            html,
            "<div class=\"model-chip\"><strong>{}</strong><br>{}</div>",
            html_escape(&runner_label(&runner)),
            html_escape(&models)
        );
    }
    html.push_str("</div>");
    html
}

pub(super) fn comparison_input_table(inputs: &Value) -> String {
    let mut html = String::new();
    if let Some(text) = input_freshness_readout(inputs) {
        let _ = write!(html, "<p class=\"note\">{}</p>", html_escape(&text));
    }
    html.push_str(
        "<div class=\"ledger\"><table><thead><tr><th>Input</th><th>Path</th><th>Rows</th><th>Scenarios</th><th>Generated</th><th>Modified</th><th>Status</th></tr></thead><tbody>",
    );
    let latest_modified = inputs
        .pointer("/freshness/latest_modified_unix_ms")
        .and_then(Value::as_u64);
    for (label, pointer) in [
        ("Spark harness", "/harness_reports"),
        ("Codex CLI", "/codex_cli_reports"),
        ("OpenCode", "/opencode_reports"),
        ("Spark usage history", "/usage_history_reports"),
    ] {
        let Some(reports) = inputs.pointer(pointer).and_then(Value::as_array) else {
            continue;
        };
        if reports.is_empty() {
            let _ = write!(
                html,
                "<tr><td>{}</td><td>latest trace scan</td><td class=\"num\">n/a</td><td>n/a</td><td>n/a</td><td>n/a</td><td>implicit</td></tr>",
                html_escape(label)
            );
            continue;
        }
        for report in reports {
            let path = report
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let row_count = report
                .get("row_count")
                .and_then(Value::as_u64)
                .map(|count| count.to_string())
                .unwrap_or_else(|| "n/a".to_string());
            let scenarios = report
                .get("scenarios")
                .and_then(Value::as_array)
                .map(|scenarios| {
                    scenarios
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .filter(|scenarios| !scenarios.is_empty())
                .unwrap_or_else(|| "n/a".to_string());
            let generated = report
                .get("generated_at_unix_ms")
                .and_then(Value::as_u64)
                .map(|value| input_timestamp_label(value, latest_modified))
                .unwrap_or_else(|| "n/a".to_string());
            let modified = report
                .get("modified_unix_ms")
                .and_then(Value::as_u64)
                .map(|value| input_timestamp_label(value, latest_modified))
                .unwrap_or_else(|| "n/a".to_string());
            let status = report.get("error").and_then(Value::as_str).unwrap_or("ok");
            let _ = write!(
                html,
                "<tr><td>{}</td><td>{}</td><td class=\"num\">{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td></tr>",
                html_escape(label),
                html_escape(path),
                html_escape(&row_count),
                html_escape(&scenarios),
                html_escape(&generated),
                html_escape(&modified),
                html_escape(status)
            );
        }
    }
    html.push_str("</tbody></table></div>");
    html
}

fn input_freshness_readout(inputs: &Value) -> Option<String> {
    let freshness = inputs.get("freshness")?;
    let count = freshness.get("input_count").and_then(Value::as_u64)?;
    if count == 0 {
        return None;
    }
    let span = freshness
        .get("modified_span_label")
        .and_then(Value::as_str)
        .unwrap_or("n/a");
    let warning = freshness
        .get("mixed_input_warning")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if warning {
        Some(format!(
            "Input freshness warning: {count} input reports span {span} by modified time. Treat mixed fresh/stale comparisons as directional until a single fresh paired run is available."
        ))
    } else {
        Some(format!(
            "Input freshness: {count} input reports span {span} by modified time."
        ))
    }
}

fn input_timestamp_label(value: u64, latest: Option<u64>) -> String {
    let Some(latest) = latest else {
        return value.to_string();
    };
    if latest == value {
        format!("{value} (newest)")
    } else if latest > value {
        format!("{value} ({} older)", format_duration_ms(latest - value))
    } else {
        value.to_string()
    }
}

pub(super) fn format_duration_ms(value: u64) -> String {
    if value < 1_000 {
        return format!("{value}ms");
    }
    let seconds = value as f64 / 1_000.0;
    if seconds < 60.0 {
        return format!("{seconds:.1}s");
    }
    let minutes = seconds / 60.0;
    if minutes < 60.0 {
        return format!("{minutes:.1}m");
    }
    let hours = minutes / 60.0;
    if hours < 48.0 {
        return format!("{hours:.1}h");
    }
    format!("{:.1}d", hours / 24.0)
}

fn model_label(row: &ComparisonRow) -> String {
    if row.reasoning_effort.is_empty() {
        row.model.clone()
    } else {
        format!("{} ({})", row.model, row.reasoning_effort)
    }
}

fn command_label(row: &ComparisonRow) -> String {
    match (
        row.command_version.trim().is_empty(),
        row.command_path.trim().is_empty(),
    ) {
        (true, true) => "n/a".to_string(),
        (false, true) => row.command_version.clone(),
        (true, false) => row.command_path.clone(),
        (false, false) => format!("{} - {}", row.command_version, row.command_path),
    }
}

fn aggregate_metric_with_fallback(
    aggregate: &Value,
    preferred_field: &str,
    fallback_field: &str,
    runner: &str,
) -> f64 {
    let preferred = aggregate_metric(aggregate, preferred_field, runner);
    if preferred > 0.0 {
        preferred
    } else {
        aggregate_metric(aggregate, fallback_field, runner)
    }
}

fn comparison_surface_table(rows: &[ComparisonRow]) -> String {
    let mut by_scenario = BTreeMap::<String, Vec<&ComparisonRow>>::new();
    for row in rows {
        by_scenario
            .entry(row.scenario.clone())
            .or_default()
            .push(row);
    }
    let mut html = String::from(
        "<div class=\"ledger\"><table><thead><tr><th>Scenario</th><th>Surface</th><th>Question answered</th><th>Runners present</th><th>Validation</th><th>Pressure signal</th></tr></thead><tbody>",
    );
    for (scenario, rows) in by_scenario {
        let runners = rows
            .iter()
            .map(|row| row.runner.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let validation = if rows.iter().any(|row| row.validation_exit_code == Some(0)) {
            "artifact validation passed"
        } else if rows.iter().any(|row| row.validation_timed_out) {
            "validation timeout"
        } else if rows.iter().any(|row| row.validation_exit_code.is_some()) {
            "validation failed"
        } else {
            "not applicable"
        };
        let pressure = rows
            .iter()
            .filter_map(|row| {
                (!row.failure_points.is_empty()).then_some(row.failure_points.as_str())
            })
            .collect::<Vec<_>>()
            .join("; ");
        let _ = write!(
            html,
            "<tr><td>{}</td><td><span class=\"pill\">{}</span></td><td class=\"surface\">{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            html_escape(&scenario),
            html_escape(scenario_family(&scenario)),
            html_escape(scenario_question(&scenario)),
            html_escape(&runners),
            html_escape(validation),
            html_escape(if pressure.is_empty() {
                "none"
            } else {
                &pressure
            })
        );
    }
    html.push_str("</tbody></table></div>");
    html
}

fn comparison_coverage_table(aggregate: &Value) -> String {
    let Some(unmatched) = aggregate
        .get("unmatched_scenarios")
        .and_then(Value::as_array)
    else {
        return "<p class=\"note\">No coverage-only scenarios in this comparison.</p>".to_string();
    };
    if unmatched.is_empty() {
        return "<p class=\"note\">Every scenario in this comparison has matching runner evidence.</p>".to_string();
    }
    let mut html = String::from(
        "<div class=\"ledger\"><table><thead><tr><th>Scenario</th><th>Surface</th><th>Runner coverage</th><th>Interpretation</th></tr></thead><tbody>",
    );
    for entry in unmatched {
        let scenario = entry
            .get("scenario")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let runners = entry
            .get("runners")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        let _ = write!(
            html,
            "<tr><td>{}</td><td><span class=\"pill\">{}</span></td><td>{}</td><td>Coverage expansion only; excluded from matched-run verdict until another runner has this scenario.</td></tr>",
            html_escape(scenario),
            html_escape(scenario_family(scenario)),
            html_escape(&runners)
        );
    }
    html.push_str("</tbody></table></div>");
    html
}

fn comparison_delta_table(rows: &[ComparisonRow]) -> String {
    let mut by_scenario = BTreeMap::<String, Vec<&ComparisonRow>>::new();
    for row in rows {
        by_scenario
            .entry(row.scenario.clone())
            .or_default()
            .push(row);
    }
    let mut html = String::from(
        "<div class=\"ledger\"><table><thead><tr><th>Scenario</th><th>Spark vs Codex duration</th><th>Token ratio</th><th>Tool ratio</th><th>Index delta</th></tr></thead><tbody>",
    );
    for (scenario, rows) in by_scenario {
        let Some(spark) = comparison_row_for_runner(&rows, "spark-harness") else {
            continue;
        };
        let Some(codex) = comparison_row_for_runner(&rows, "codex-cli") else {
            continue;
        };
        let duration_delta = spark
            .duration_ms
            .zip(codex.duration_ms)
            .map(|(spark, codex)| signed_ms(spark as i128 - codex as i128))
            .unwrap_or_else(|| "unavailable".to_string());
        let token_ratio = spark
            .input_tokens
            .zip(codex.input_tokens)
            .map(|(spark, codex)| format!("{:.1}x", ratio_or_zero(spark as f64, codex as f64)))
            .unwrap_or_else(|| "unavailable".to_string());
        let tool_ratio = spark
            .tool_or_item_calls
            .zip(codex.tool_or_item_calls)
            .map(|(spark, codex)| format!("{:.1}x", ratio_or_zero(spark as f64, codex as f64)))
            .unwrap_or_else(|| "unavailable".to_string());
        let index_delta =
            spark.benchmark_index.unwrap_or(0.0) - codex.benchmark_index.unwrap_or(0.0);
        let _ = write!(
            html,
            "<tr><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{:+.1}</td></tr>",
            html_escape(&scenario),
            duration_delta,
            token_ratio,
            tool_ratio,
            index_delta
        );
    }
    html.push_str("</tbody></table></div>");
    html
}

fn comparison_row_for_runner<'a>(
    rows: &'a [&ComparisonRow],
    runner: &str,
) -> Option<&'a ComparisonRow> {
    rows.iter()
        .copied()
        .find(|row| row.runner == runner)
        .or_else(|| {
            let prefix = format!("{runner}/");
            rows.iter()
                .copied()
                .filter(|row| row.runner.starts_with(&prefix))
                .max_by(|left, right| compare_comparison_rows(left, right))
        })
}

fn aggregate_metric(aggregate: &Value, field: &str, runner: &str) -> f64 {
    if let Some(value) = aggregate
        .get(field)
        .and_then(|value| value.get(runner))
        .and_then(Value::as_f64)
    {
        return value;
    }
    let Some(object) = aggregate.get(field).and_then(Value::as_object) else {
        return 0.0;
    };
    let prefix = format!("{runner}/");
    object
        .iter()
        .filter_map(|(key, value)| key.starts_with(&prefix).then(|| value.as_f64()).flatten())
        .pipe_average()
        .map(round1)
        .unwrap_or(0.0)
}

fn runner_family_present(rows: &[ComparisonRow], family: &str) -> bool {
    let prefix = format!("{family}/");
    rows.iter()
        .any(|row| row.runner == family || row.runner.starts_with(&prefix))
}

fn metric_display(value: f64, present: bool) -> String {
    if present {
        format!("{value:.1}")
    } else {
        "n/a".to_string()
    }
}

fn duration_display(value: f64, present: bool) -> String {
    if present {
        format_ms(value)
    } else {
        "n/a".to_string()
    }
}

fn format_ms(value: f64) -> String {
    if value >= 1000.0 {
        format!("{:.1}s", value / 1000.0)
    } else {
        format!("{value:.0}ms")
    }
}

fn signed_ms(value: i128) -> String {
    let sign = if value >= 0 { "+" } else { "-" };
    format!("{sign}{}", format_ms(value.unsigned_abs() as f64))
}

fn optional_metric(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn llm_review_cell(row: &ComparisonRow) -> String {
    match (row.llm_solution_score, row.llm_process_score) {
        (Some(solution), Some(process)) => {
            if row.llm_notes.is_empty() {
                format!("solution {solution:.1}, process {process:.1}")
            } else {
                format!(
                    "solution {solution:.1}, process {process:.1}; {}",
                    row.llm_notes
                )
            }
        }
        _ => "n/a".to_string(),
    }
}

fn duration_readout(spark_duration: f64, codex_duration: f64) -> String {
    if spark_duration > 0.0 && spark_duration < codex_duration {
        format!(
            "Spark was {} faster on average.",
            format_ms(codex_duration - spark_duration)
        )
    } else if codex_duration > 0.0 && codex_duration < spark_duration {
        format!(
            "Codex CLI was {} faster on average.",
            format_ms(spark_duration - codex_duration)
        )
    } else {
        "No average duration advantage.".to_string()
    }
}

fn tool_items_readout(spark_calls: f64, codex_calls: f64) -> String {
    if spark_calls <= 0.0 || codex_calls <= 0.0 {
        "Action-item counts are incomplete for at least one runner.".to_string()
    } else if (spark_calls - codex_calls).abs() < 0.1 {
        "Both runners used about the same number of action items.".to_string()
    } else if spark_calls < codex_calls {
        format!(
            "Spark used {:.1} fewer action items on average.",
            codex_calls - spark_calls
        )
    } else {
        format!(
            "Codex CLI used {:.1} fewer action items on average.",
            spark_calls - codex_calls
        )
    }
}

pub(super) fn comparison_score_svg(rows: &[ComparisonRow]) -> String {
    let indexed_rows = rows
        .iter()
        .filter(|row| row.benchmark_index.is_some())
        .collect::<Vec<_>>();
    if indexed_rows.is_empty() {
        return "<p class=\"note\">No matched benchmark-index rows are available for this comparison.</p>".to_string();
    }
    let mut scenarios = Vec::<String>::new();
    let mut rows_by_scenario = BTreeMap::<String, Vec<&ComparisonRow>>::new();
    for row in &indexed_rows {
        if !rows_by_scenario.contains_key(&row.scenario) {
            scenarios.push(row.scenario.clone());
        }
        rows_by_scenario
            .entry(row.scenario.clone())
            .or_default()
            .push(*row);
    }
    let mut runners = indexed_rows
        .iter()
        .map(|row| row.runner.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    runners.sort_by(|left, right| compare_runner_labels(left, right));
    let label_width = 250usize;
    let chart_width = 650usize;
    let right_padding = 96usize;
    let legend_columns = 3usize;
    let legend_rows = runners.len().div_ceil(legend_columns).max(1);
    let legend_height = 34usize + legend_rows * 24usize;
    let bar_height = 13usize;
    let bar_gap = 7usize;
    let group_height = 38usize + runners.len() * (bar_height + bar_gap);
    let height = legend_height + 28 + scenarios.len() * group_height;
    let max_index = rows_by_scenario
        .values()
        .flatten()
        .filter_map(|row| row.benchmark_index)
        .fold(100.0, f64::max);
    let axis_max = ((max_index / 25.0).ceil() * 25.0).max(100.0);
    let mut svg = format!(
        r#"<svg viewBox="0 0 {} {}" role="img" aria-label="Benchmark index comparison grouped by scenario">"#,
        label_width + chart_width + right_padding,
        height
    );
    svg.push_str(
        r##"<text x="0" y="16" font-size="12" font-weight="700" fill="#344154">Runner</text>"##,
    );
    for (index, runner) in runners.iter().enumerate() {
        let column = index % legend_columns;
        let row = index / legend_columns;
        let x = 74 + column * 185;
        let y = 5 + row * 24;
        let _ = write!(
            svg,
            r##"<rect x="{}" y="{}" width="12" height="12" rx="3" fill="{}"/><text x="{}" y="{}" font-size="12" fill="#344154">{}</text>"##,
            x,
            y,
            runner_color(runner),
            x + 18,
            y + 11,
            html_escape(&runner_label(runner))
        );
    }
    for tick in [
        0.0,
        axis_max * 0.25,
        axis_max * 0.5,
        axis_max * 0.75,
        axis_max,
    ] {
        let x = label_width + (tick / axis_max * chart_width as f64).round() as usize;
        let _ = write!(
            svg,
            r##"<line x1="{x}" y1="30" x2="{x}" y2="{}" stroke="#e4e8ef" stroke-width="1"/><text x="{x}" y="42" font-size="10" text-anchor="middle" fill="#6b7280">{:.0}</text>"##,
            height - 12,
            tick
        );
    }
    for (scenario_index, scenario) in scenarios.iter().enumerate() {
        let group_y = legend_height + scenario_index * group_height;
        if scenario_index % 2 == 0 {
            let _ = write!(
                svg,
                r##"<rect x="0" y="{}" width="{}" height="{}" rx="4" fill="#f8fafc"/>"##,
                group_y - 4,
                label_width + chart_width + right_padding - 4,
                group_height - 8
            );
        }
        let _ = write!(
            svg,
            r##"<text x="0" y="{}" font-size="12" font-weight="700" fill="#263244">{}</text><text x="0" y="{}" font-size="10" fill="#6b7280">{}</text>"##,
            group_y + 20,
            html_escape(scenario),
            group_y + 36,
            html_escape(scenario_family(scenario))
        );
        let Some(scenario_rows) = rows_by_scenario.get(scenario) else {
            continue;
        };
        for (runner_index, runner) in runners.iter().enumerate() {
            let Some(row) = scenario_rows.iter().find(|row| row.runner == *runner) else {
                continue;
            };
            let value = row.benchmark_index.unwrap_or(0.0);
            let width = (value / axis_max * chart_width as f64).round() as usize;
            let y = group_y + 8 + runner_index * (bar_height + bar_gap);
            let label_x = label_width + width + 8;
            let label_anchor = if label_x + 48 > label_width + chart_width + right_padding {
                "end"
            } else {
                "start"
            };
            let label_x = if label_anchor == "end" {
                label_width + chart_width + 86
            } else {
                label_x
            };
            let _ = write!(
                svg,
                r##"<text x="{}" y="{}" font-size="10" text-anchor="end" fill="#596579">{}</text><rect x="{}" y="{}" width="{}" height="{}" rx="4" fill="{}"><title>{} / {} / {} benchmark index {:.1}</title></rect><text x="{}" y="{}" font-size="11" text-anchor="{}" fill="#263244">{:.1}</text>"##,
                label_width - 12,
                y + 10,
                html_escape(&runner_label(runner)),
                label_width,
                y,
                width,
                bar_height,
                runner_color(runner),
                html_escape(scenario),
                html_escape(runner),
                html_escape(&model_label(row)),
                value,
                label_x,
                y + 10,
                label_anchor,
                value
            );
        }
    }
    svg.push_str("</svg>");
    svg
}

fn runner_color(runner: &str) -> &'static str {
    match runner_family(runner) {
        "spark-harness" => "#2f7da1",
        "opencode" => "#b8832f",
        "codex-cli" => "#7b6bb7",
        _ => "#697386",
    }
}

fn runner_label(runner: &str) -> String {
    let family = match runner_family(runner) {
        "spark-harness" => "Spark",
        "codex-cli" => "Codex CLI",
        "opencode" => "OpenCode",
        _ => "Runner",
    };
    if let Some((_, variant)) = runner.split_once('/') {
        format!("{family} {variant}")
    } else {
        family.to_string()
    }
}

fn runner_family(runner: &str) -> &str {
    runner
        .split_once('/')
        .map(|(family, _)| family)
        .unwrap_or(runner)
}

fn compare_runner_labels(left: &str, right: &str) -> Ordering {
    runner_family_order(left)
        .cmp(&runner_family_order(right))
        .then_with(|| runner_variant_order(left).cmp(&runner_variant_order(right)))
        .then_with(|| left.cmp(right))
}

fn runner_family_order(runner: &str) -> usize {
    match runner_family(runner) {
        "spark-harness" => 0,
        "codex-cli" => 1,
        "opencode" => 2,
        _ => 3,
    }
}

fn runner_variant_order(runner: &str) -> usize {
    match runner.split_once('/').map(|(_, variant)| variant) {
        Some("minimal") => 0,
        Some("low") => 1,
        Some("medium") => 2,
        Some("high") => 3,
        Some("xhigh") | Some("max") => 4,
        Some(_) => 5,
        None => 0,
    }
}
