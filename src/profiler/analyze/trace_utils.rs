use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Map, Value, json};

pub(super) fn timeline_turn(
    timeline: &mut BTreeMap<usize, Map<String, Value>>,
    turn: usize,
) -> &mut Map<String, Value> {
    timeline.entry(turn).or_insert_with(|| {
        let mut entry = Map::new();
        entry.insert("turn".to_string(), json!(turn));
        entry
    })
}

pub(super) fn push_timeline_array(
    timeline: &mut BTreeMap<usize, Map<String, Value>>,
    turn: usize,
    key: &str,
    value: Value,
) {
    let entry = timeline_turn(timeline, turn);
    match entry.get_mut(key) {
        Some(Value::Array(items)) => items.push(value),
        _ => {
            entry.insert(key.to_string(), Value::Array(vec![value]));
        }
    }
}

pub(super) fn is_tool_result_trace_file(name: &str) -> bool {
    name.ends_with("-tool-result.json") || name.contains("-tool-result-")
}

pub(in crate::profiler) fn trace_file_sort_key(path: &Path) -> (usize, String, usize, String) {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let stem = name.strip_suffix(".json").unwrap_or(name);
    let (turn, rest) = stem
        .split_once('-')
        .and_then(|(turn, rest)| turn.parse::<usize>().ok().map(|turn| (turn, rest)))
        .unwrap_or((usize::MAX, stem));
    let (kind, sequence) = rest
        .rsplit_once('-')
        .and_then(|(kind, suffix)| {
            suffix
                .parse::<usize>()
                .ok()
                .map(|sequence| (kind, sequence))
        })
        .unwrap_or((rest, 1));
    (turn, kind.to_string(), sequence, name.to_string())
}

pub(in crate::profiler) fn summarize_compaction_report(report: &Value) -> Value {
    let mut summary = Map::new();
    copy_field(report, &mut summary, "method");
    copy_field(report, &mut summary, "forced");
    copy_field(report, &mut summary, "trigger");
    copy_field(report, &mut summary, "duration_ms");
    copy_field(report, &mut summary, "before_chars");
    copy_field(report, &mut summary, "compact_request_chars");
    copy_field(report, &mut summary, "after_chars");
    copy_field(report, &mut summary, "threshold_chars");
    copy_field(report, &mut summary, "compacted_outputs");
    copy_field(report, &mut summary, "compacted_tool_outputs");
    copy_field(report, &mut summary, "compacted_messages");
    copy_field(report, &mut summary, "remote_error");
    copy_field(report, &mut summary, "fallback");
    copy_field(report, &mut summary, "local_pressure");
    add_compaction_retention_metrics(report, &mut summary);

    if let Some(raw) = report.get("raw") {
        let mut raw_summary = Map::new();
        copy_field(raw, &mut raw_summary, "object");
        copy_field(raw, &mut raw_summary, "id");
        copy_field(raw, &mut raw_summary, "created_at");
        copy_field(raw, &mut raw_summary, "usage");
        if let Some(output) = raw.get("output").and_then(Value::as_array) {
            raw_summary.insert("output_items".to_string(), json!(output.len()));
            raw_summary.insert(
                "output_types".to_string(),
                Value::Array(
                    output
                        .iter()
                        .filter_map(|item| item.get("type").and_then(Value::as_str))
                        .map(|kind| Value::String(kind.to_string()))
                        .collect(),
                ),
            );
        }
        if !raw_summary.is_empty() {
            summary.insert("raw_summary".to_string(), Value::Object(raw_summary));
        }
    }

    Value::Object(summary)
}

fn add_compaction_retention_metrics(report: &Value, summary: &mut Map<String, Value>) {
    let before = report.get("before_chars").and_then(Value::as_u64);
    let after = report.get("after_chars").and_then(Value::as_u64);
    if let (Some(before), Some(after)) = (before, after)
        && before > 0
    {
        summary.insert(
            "final_retained_pct".to_string(),
            json!(percent(after, before)),
        );
    }

    let remote_after = report
        .pointer("/local_pressure/remote_after_chars")
        .and_then(Value::as_u64);
    if let Some(remote_after) = remote_after {
        summary.insert("remote_after_chars".to_string(), json!(remote_after));
        if let Some(before) = before
            && before > 0
        {
            summary.insert(
                "remote_retained_pct".to_string(),
                json!(percent(remote_after, before)),
            );
        }
    }

    let final_chars = report
        .pointer("/local_pressure/final_chars")
        .and_then(Value::as_u64);
    if let Some(final_chars) = final_chars {
        summary.insert("local_pressure_final_chars".to_string(), json!(final_chars));
    }

    if let (Some(remote_after), Some(final_chars)) = (remote_after, final_chars)
        && remote_after > 0
    {
        let reduced = remote_after.saturating_sub(final_chars);
        summary.insert(
            "local_pressure_reduction_pct".to_string(),
            json!(percent(reduced, remote_after)),
        );
    }
}

fn percent(part: u64, whole: u64) -> f64 {
    (part as f64 / whole as f64) * 100.0
}

pub(super) fn sanitize_profile_summary(mut summary: Value) -> Value {
    let Some(object) = summary.as_object_mut() else {
        return summary;
    };
    if let Some(reports) = object
        .get_mut("compaction_reports")
        .and_then(Value::as_array_mut)
    {
        for report in reports {
            *report = summarize_compaction_report(report);
        }
    }
    summary
}

fn copy_field(source: &Value, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key) {
        target.insert(key.to_string(), value.clone());
    }
}

pub(super) fn is_profile_summary_trace_file(name: &str) -> bool {
    name.ends_with("-profile-summary.json") || name.contains("-profile-summary-")
}

pub(super) fn profile_summary_rank(name: &str, turn: usize) -> usize {
    let Some(stem) = name.strip_suffix(".json") else {
        return turn.saturating_mul(10_000);
    };
    let duplicate = stem
        .split_once("-profile-summary")
        .and_then(|(_, suffix)| suffix.strip_prefix('-'))
        .and_then(|suffix| suffix.parse::<usize>().ok())
        .unwrap_or(1);
    turn.saturating_mul(10_000).saturating_add(duplicate)
}
