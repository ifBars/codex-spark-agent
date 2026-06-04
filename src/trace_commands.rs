use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::{Value, json};

use crate::{APPROX_CHARS_PER_TOKEN, TraceSort};

pub(crate) fn trace_runs_root(cwd: &Path) -> PathBuf {
    cwd.join(".spark-runs")
}

pub(crate) fn display_trace_dir(cwd: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(cwd)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf())
}

pub(crate) fn trace_export_record(
    cwd: &Path,
    path: &Path,
    display: &Path,
    summary: Option<&Value>,
) -> Value {
    let absolute_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    json!({
        "type": "trace",
        "trace_dir": display.display().to_string(),
        "trace_dir_abs": absolute_path.display().to_string(),
        "workspace": cwd.display().to_string(),
        "summary": summary.cloned().unwrap_or(Value::Null),
    })
}

pub(crate) struct TraceListRecord {
    pub(crate) run: PathBuf,
    pub(crate) display: PathBuf,
    pub(crate) summary: Option<Value>,
}

pub(crate) fn sort_trace_records(records: &mut [TraceListRecord], sort: TraceSort) {
    match sort {
        TraceSort::Newest => {}
        TraceSort::OverrunContext
        | TraceSort::OverrunTurns
        | TraceSort::ToolOnlyStreak
        | TraceSort::Context
        | TraceSort::RequestMs => {
            records.sort_by(|left, right| {
                trace_sort_metric(right.summary.as_ref(), sort)
                    .cmp(&trace_sort_metric(left.summary.as_ref(), sort))
                    .then_with(|| right.run.file_name().cmp(&left.run.file_name()))
            });
        }
    }
}

pub(crate) fn trace_sort_metric(summary: Option<&Value>, sort: TraceSort) -> u64 {
    let Some(summary) = summary else {
        return 0;
    };
    let pointer = match sort {
        TraceSort::Newest => return 0,
        TraceSort::OverrunContext => {
            "/profile_scenario_call_expectations/context_growth_after_satisfied_chars"
        }
        TraceSort::OverrunTurns => {
            "/profile_scenario_call_expectations/extra_turns_after_satisfied"
        }
        TraceSort::ToolOnlyStreak => "/tool_only_turns/max_consecutive",
        TraceSort::Context => "/max_approx_input_tokens",
        TraceSort::RequestMs => "/max_request_duration_ms",
    };
    summary
        .pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

pub(crate) fn trace_sort_name(sort: TraceSort) -> &'static str {
    match sort {
        TraceSort::Newest => "newest",
        TraceSort::OverrunContext => "overrun-context",
        TraceSort::OverrunTurns => "overrun-turns",
        TraceSort::ToolOnlyStreak => "tool-only-streak",
        TraceSort::Context => "context",
        TraceSort::RequestMs => "request-ms",
    }
}

pub(crate) fn resolve_char_threshold(
    name: &str,
    chars: Option<usize>,
    tokens: Option<usize>,
    default_chars: usize,
) -> Result<usize> {
    match (chars, tokens) {
        (Some(_), Some(_)) => {
            anyhow::bail!("pass either --{name}-chars or --{name}-tokens, not both")
        }
        (Some(chars), None) => Ok(chars),
        (None, Some(tokens)) => Ok(tokens.saturating_mul(APPROX_CHARS_PER_TOKEN)),
        (None, None) => Ok(default_chars),
    }
}

pub(crate) fn trace_has_all_diagnostics(summary: &Value, required: &[String]) -> bool {
    if required.is_empty() {
        return true;
    }
    let Some(diagnostics) = summary.get("diagnostics").and_then(Value::as_array) else {
        return false;
    };
    required.iter().all(|required_kind| {
        diagnostics.iter().any(|diagnostic| {
            diagnostic.get("kind").and_then(Value::as_str) == Some(required_kind.as_str())
        })
    })
}

pub(crate) fn trace_matches_metric_filters(
    summary: &Value,
    min_tool_only_streak: Option<u64>,
    min_overrun_turns: Option<u64>,
    min_overrun_context_chars: Option<u64>,
) -> bool {
    metric_at_least(
        summary,
        "/tool_only_turns/max_consecutive",
        min_tool_only_streak,
    ) && metric_at_least(
        summary,
        "/profile_scenario_call_expectations/extra_turns_after_satisfied",
        min_overrun_turns,
    ) && metric_at_least(
        summary,
        "/profile_scenario_call_expectations/context_growth_after_satisfied_chars",
        min_overrun_context_chars,
    )
}

fn metric_at_least(summary: &Value, pointer: &str, minimum: Option<u64>) -> bool {
    let Some(minimum) = minimum else {
        return true;
    };
    summary
        .pointer(pointer)
        .and_then(Value::as_u64)
        .is_some_and(|value| value >= minimum)
}

pub(crate) fn trace_filter_label(
    scenario: Option<&str>,
    diagnostics: &[String],
    min_tool_only_streak: Option<u64>,
    min_overrun_turns: Option<u64>,
    min_overrun_context_chars: Option<u64>,
) -> String {
    let mut label = scenario.unwrap_or("all").to_string();
    if !diagnostics.is_empty() {
        label.push_str(" diagnostics=");
        label.push_str(&diagnostics.join(","));
    }
    if let Some(minimum) = min_tool_only_streak {
        label.push_str(&format!(" min_tool_only_streak={minimum}"));
    }
    if let Some(minimum) = min_overrun_turns {
        label.push_str(&format!(" min_overrun_turns={minimum}"));
    }
    if let Some(minimum) = min_overrun_context_chars {
        label.push_str(&format!(" min_overrun_context_chars={minimum}"));
    }
    label
}

pub(crate) fn latest_trace_dir(root: &Path) -> Result<PathBuf> {
    list_trace_dirs(root, 1)?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no trace directories found under {}", root.display()))
}

pub(crate) fn list_trace_dirs(root: &Path, limit: usize) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut runs = Vec::new();
    for entry in std::fs::read_dir(root)
        .map_err(|error| anyhow::anyhow!("failed to read {}: {error}", root.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(suffix) = name.strip_prefix("run-") else {
            continue;
        };
        let order = suffix.parse::<u128>().unwrap_or(0);
        runs.push((order, entry.path()));
    }

    runs.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| right.1.file_name().cmp(&left.1.file_name()))
    });
    runs.truncate(limit);
    Ok(runs.into_iter().map(|(_, path)| path).collect())
}
