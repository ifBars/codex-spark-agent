use std::path::PathBuf;

use anyhow::Result;
use serde_json::json;

use crate::cli::TraceSort;
use crate::{profiler, trace_commands};

pub(crate) fn handle_traces(
    limit: usize,
    summary: bool,
    scenario: Option<String>,
    diagnostics: Vec<String>,
    aggregate: bool,
    sort: TraceSort,
    min_tool_only_streak: Option<u64>,
    min_overrun_turns: Option<u64>,
    min_overrun_context_chars: Option<u64>,
    min_compaction_regrowth_chars: Option<u64>,
    json: bool,
    jsonl: bool,
) -> Result<()> {
    if json && jsonl {
        anyhow::bail!("pass either --json or --jsonl, not both");
    }
    let cwd = std::fs::canonicalize(".").unwrap_or_else(|_| PathBuf::from("."));
    let mut matching_summaries = Vec::new();
    let mut json_records = Vec::new();
    let mut matching_records = Vec::new();
    let analyze = summary
        || scenario.is_some()
        || !diagnostics.is_empty()
        || aggregate
        || sort != TraceSort::Newest
        || min_tool_only_streak.is_some()
        || min_overrun_turns.is_some()
        || min_overrun_context_chars.is_some()
        || min_compaction_regrowth_chars.is_some()
        || json
        || jsonl;
    for run in trace_commands::list_trace_dirs(&trace_commands::trace_runs_root(&cwd), limit)? {
        let display = trace_commands::display_trace_dir(&cwd, &run);
        let trace_summary = if analyze {
            Some(profiler::analyze_trace(&run)?)
        } else {
            None
        };
        if let Some(scenario) = scenario.as_deref()
            && trace_summary
                .as_ref()
                .and_then(profiler::trace_profile_scenario_name)
                != Some(scenario)
        {
            continue;
        }
        if !diagnostics.is_empty()
            && !trace_commands::trace_has_all_diagnostics(
                trace_summary.as_ref().expect("summary loaded"),
                &diagnostics,
            )
        {
            continue;
        }
        if !trace_commands::trace_matches_metric_filters(
            trace_summary.as_ref().expect("summary loaded"),
            min_tool_only_streak,
            min_overrun_turns,
            min_overrun_context_chars,
            min_compaction_regrowth_chars,
        ) {
            continue;
        }
        if let Some(trace_summary) = &trace_summary {
            matching_summaries.push(trace_summary.clone());
        }
        matching_records.push(trace_commands::TraceListRecord {
            run,
            display,
            summary: trace_summary,
        });
    }
    trace_commands::sort_trace_records(&mut matching_records, sort);
    print_trace_records(
        &cwd,
        summary,
        json,
        jsonl,
        &mut json_records,
        &matching_records,
    )?;
    print_trace_aggregate(
        TraceOutputFilter {
            scenario,
            diagnostics,
            limit,
            aggregate,
            sort,
            min_tool_only_streak,
            min_overrun_turns,
            min_overrun_context_chars,
            min_compaction_regrowth_chars,
            json,
            jsonl,
        },
        json_records,
        &matching_summaries,
    )
}

fn print_trace_records(
    cwd: &std::path::Path,
    summary: bool,
    json: bool,
    jsonl: bool,
    json_records: &mut Vec<serde_json::Value>,
    matching_records: &[trace_commands::TraceListRecord],
) -> Result<()> {
    for record in matching_records {
        if json || jsonl {
            let record = trace_commands::trace_export_record(
                cwd,
                &record.run,
                &record.display,
                record.summary.as_ref(),
            );
            if jsonl {
                println!("{}", serde_json::to_string(&record)?);
            } else {
                json_records.push(record);
            }
            continue;
        }
        if summary {
            let trace_summary = record.summary.as_ref().expect("summary loaded");
            println!(
                "{}",
                profiler::format_trace_summary_row(
                    &record.display.display().to_string(),
                    trace_summary
                )
            );
        } else {
            println!("{}", record.display.display());
        }
    }
    Ok(())
}

struct TraceOutputFilter {
    scenario: Option<String>,
    diagnostics: Vec<String>,
    limit: usize,
    aggregate: bool,
    sort: TraceSort,
    min_tool_only_streak: Option<u64>,
    min_overrun_turns: Option<u64>,
    min_overrun_context_chars: Option<u64>,
    min_compaction_regrowth_chars: Option<u64>,
    json: bool,
    jsonl: bool,
}

fn print_trace_aggregate(
    filter: TraceOutputFilter,
    json_records: Vec<serde_json::Value>,
    matching_summaries: &[serde_json::Value],
) -> Result<()> {
    if filter.json {
        let output = json!({
            "filter": {
                "scenario": filter.scenario,
                "diagnostics": filter.diagnostics,
                "limit": filter.limit,
                "sort": trace_commands::trace_sort_name(filter.sort),
                "min_tool_only_streak": filter.min_tool_only_streak,
                "min_overrun_turns": filter.min_overrun_turns,
                "min_overrun_context_chars": filter.min_overrun_context_chars,
                "min_compaction_regrowth_chars": filter.min_compaction_regrowth_chars,
            },
            "runs": json_records,
            "aggregate": filter.aggregate.then(|| {
                profiler::trace_aggregate_json(
                    trace_filter_label(&filter).as_str(),
                    matching_summaries,
                )
            }),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    }
    if filter.aggregate {
        if filter.jsonl {
            let record = json!({
                "type": "aggregate",
                "aggregate": profiler::trace_aggregate_json(
                    trace_filter_label(&filter).as_str(),
                    matching_summaries,
                ),
            });
            println!("{}", serde_json::to_string(&record)?);
        } else if !filter.json {
            println!(
                "{}",
                profiler::format_trace_aggregate_row(
                    trace_filter_label(&filter).as_str(),
                    matching_summaries,
                )
            );
        }
    }
    Ok(())
}

fn trace_filter_label(filter: &TraceOutputFilter) -> String {
    trace_commands::trace_filter_label(
        filter.scenario.as_deref(),
        &filter.diagnostics,
        filter.min_tool_only_streak,
        filter.min_overrun_turns,
        filter.min_overrun_context_chars,
        filter.min_compaction_regrowth_chars,
    )
}
