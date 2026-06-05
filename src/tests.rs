use crate::chat::{
    command_args, matching_slash_commands, parse_mode, slash_command_token,
    unknown_slash_command_warning,
};
mod profile_scenarios;

use crate::DEFAULT_COMPACT_AFTER_CHARS;
use crate::cli::TraceSort;
use crate::client::output_text_delta;
use crate::profile::scenarios::validate_scenario_repeat;
use crate::session::{is_active_session, timestamp_session_name};
use crate::skill::commands::{contains_skill_mention, mentioned_skill_names};
use crate::trace::commands::{
    TraceListRecord, latest_trace_dir, list_trace_dirs, resolve_char_threshold, sort_trace_records,
    trace_export_record, trace_filter_label, trace_has_all_diagnostics,
    trace_matches_metric_filters, trace_runs_root, trace_sort_metric, trace_sort_name,
};
use serde_json::json;
use std::path::PathBuf;

#[test]
fn slash_commands_match_exactly_or_with_whitespace() {
    assert_eq!(command_args("/skill", "/skill"), Some(""));
    assert_eq!(
        command_args("/skill load rust", "/skill"),
        Some("load rust")
    );
    assert_eq!(command_args("/compact", "/compact"), Some(""));
    assert_eq!(command_args("/compact now", "/compact"), Some("now"));
    assert_eq!(command_args("/compaction", "/compact"), None);
    assert_eq!(command_args("/profile", "/profile"), Some(""));
    assert_eq!(command_args("/profiles", "/profile"), None);
    assert_eq!(command_args("/skills", "/skill"), None);
    assert_eq!(command_args("/sessions", "/session"), None);
}

#[test]
fn slash_command_helpers_match_menu_and_unknown_warning() {
    assert_eq!(slash_command_token("/sk load rust"), Some("/sk"));
    assert_eq!(slash_command_token("hello /sk"), None);

    let matches = matching_slash_commands("/sk");
    assert!(matches.iter().any(|command| command.name == "/skill"));
    assert!(matches.iter().any(|command| command.name == "/skills"));
    assert!(unknown_slash_command_warning("/wat now").contains("unknown command: /wat"));
}

#[test]
fn parse_mode_accepts_ask_work_and_agent_alias() {
    assert_eq!(parse_mode("ask"), Some(crate::tools::AgentMode::Ask));
    assert_eq!(parse_mode("work"), Some(crate::tools::AgentMode::Work));
    assert_eq!(parse_mode("agent"), Some(crate::tools::AgentMode::Work));
    assert_eq!(parse_mode(""), None);
}

#[test]
fn output_text_delta_reads_streaming_response_events() {
    let event = serde_json::json!({
        "type": "response.output_text.delta",
        "delta": "hello"
    });

    assert_eq!(output_text_delta(&event), Some("hello"));
    assert_eq!(
        output_text_delta(&serde_json::json!({"type": "response.output_text.done"})),
        None
    );
}

#[test]
fn detects_repo_local_skill_mentions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let skill_dir = dir.path().join(".agents").join("skills").join("demo-skill");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: demo-skill\ndescription: Demo\n---\n\n# Demo\n",
    )
    .expect("write skill");

    let mentions = mentioned_skill_names(
        &dir.path().to_path_buf(),
        "Please use @demo-skill for this.",
    )
    .expect("mentions");

    assert_eq!(mentions, vec!["demo-skill"]);
}

#[test]
fn skill_mentions_require_boundaries() {
    assert!(contains_skill_mention(
        "Please use @demo-skill.",
        "@demo-skill"
    ));
    assert!(!contains_skill_mention(
        "Please use @demo-skill-extra.",
        "@demo-skill"
    ));
}

#[test]
fn trace_dirs_are_listed_newest_first() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = trace_runs_root(dir.path());
    std::fs::create_dir_all(root.join("run-100")).expect("create old trace");
    std::fs::create_dir_all(root.join("run-300")).expect("create new trace");
    std::fs::create_dir_all(root.join("run-200")).expect("create middle trace");
    std::fs::create_dir_all(root.join("other")).expect("create ignored dir");
    std::fs::write(root.join("run-400"), "{}").expect("create ignored file");

    let runs = list_trace_dirs(&root, 2).expect("list trace dirs");
    let names = runs
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["run-300", "run-200"]);
}

#[test]
fn trace_sort_metrics_read_expected_summary_fields() {
    let summary = json!({
        "max_approx_input_tokens": 42,
        "max_request_duration_ms": 1234,
        "tool_only_turns": {
            "max_consecutive": 8
        },
        "compaction_regrowth": {
            "max_next_request_growth_chars": 65536
        },
        "profile_scenario_call_expectations": {
            "extra_turns_after_satisfied": 6,
            "context_growth_after_satisfied_chars": 101846
        }
    });

    assert_eq!(
        trace_sort_metric(Some(&summary), TraceSort::OverrunContext),
        101_846
    );
    assert_eq!(
        trace_sort_metric(Some(&summary), TraceSort::OverrunTurns),
        6
    );
    assert_eq!(
        trace_sort_metric(Some(&summary), TraceSort::ToolOnlyStreak),
        8
    );
    assert_eq!(
        trace_sort_metric(Some(&summary), TraceSort::CompactionRegrowth),
        65_536
    );
    assert_eq!(trace_sort_metric(Some(&summary), TraceSort::Context), 42);
    assert_eq!(
        trace_sort_metric(Some(&summary), TraceSort::RequestMs),
        1_234
    );
    assert_eq!(trace_sort_metric(None, TraceSort::RequestMs), 0);
    assert_eq!(
        trace_sort_name(TraceSort::OverrunContext),
        "overrun-context"
    );
    assert_eq!(
        trace_sort_name(TraceSort::CompactionRegrowth),
        "compaction-regrowth"
    );
}

#[test]
fn trace_records_sort_by_worst_metric_then_newest_name() {
    let mut records = vec![
        TraceListRecord {
            run: PathBuf::from("run-100"),
            display: PathBuf::from("run-100"),
            summary: Some(json!({
                "tool_only_turns": {"max_consecutive": 2}
            })),
        },
        TraceListRecord {
            run: PathBuf::from("run-300"),
            display: PathBuf::from("run-300"),
            summary: Some(json!({
                "tool_only_turns": {"max_consecutive": 8}
            })),
        },
        TraceListRecord {
            run: PathBuf::from("run-200"),
            display: PathBuf::from("run-200"),
            summary: Some(json!({
                "tool_only_turns": {"max_consecutive": 8}
            })),
        },
    ];

    sort_trace_records(&mut records, TraceSort::ToolOnlyStreak);
    let names = records
        .iter()
        .map(|record| record.run.display().to_string())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["run-300", "run-200", "run-100"]);
}

#[test]
fn latest_trace_dir_uses_highest_run_suffix() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = trace_runs_root(dir.path());
    std::fs::create_dir_all(root.join("run-1")).expect("create old trace");
    std::fs::create_dir_all(root.join("run-2")).expect("create latest trace");

    let latest = latest_trace_dir(&root).expect("latest trace");

    assert_eq!(latest.file_name().unwrap(), "run-2");
}

#[test]
fn token_thresholds_resolve_to_estimated_chars() {
    let chars = resolve_char_threshold(
        "compact-after",
        None,
        Some(32_000),
        DEFAULT_COMPACT_AFTER_CHARS,
    )
    .expect("resolve threshold");

    assert_eq!(chars, 128_000);
}

#[test]
fn char_thresholds_conflict_with_token_thresholds() {
    let error = resolve_char_threshold("max-input", Some(1), Some(1), 10)
        .expect_err("conflicting thresholds");

    assert!(
        error
            .to_string()
            .contains("pass either --max-input-chars or --max-input-tokens")
    );
}

#[test]
fn scenario_repeat_must_be_in_supported_range() {
    validate_scenario_repeat(1).expect("repeat 1");
    validate_scenario_repeat(50).expect("max repeat");

    let zero = validate_scenario_repeat(0).expect_err("zero repeat");
    assert!(zero.to_string().contains("greater than 0"));

    let too_many = validate_scenario_repeat(51).expect_err("too many repeats");
    assert!(too_many.to_string().contains("<= 50"));
}

#[test]
fn trace_diagnostic_filter_requires_all_requested_kinds() {
    let summary = json!({
        "diagnostics": [
            {"kind": "tool_failures"},
            {"kind": "tool_failure_recovered"}
        ]
    });

    assert!(trace_has_all_diagnostics(
        &summary,
        &["tool_failures".to_string()]
    ));
    assert!(trace_has_all_diagnostics(
        &summary,
        &[
            "tool_failures".to_string(),
            "tool_failure_recovered".to_string()
        ]
    ));
    assert!(!trace_has_all_diagnostics(
        &summary,
        &[
            "tool_failures".to_string(),
            "weak_compaction_shrink".to_string()
        ]
    ));
    assert!(!trace_has_all_diagnostics(
        &json!({}),
        &["tool_failures".to_string()]
    ));
}

#[test]
fn trace_filter_label_includes_scenario_and_diagnostics() {
    assert_eq!(
        trace_filter_label(Some("tool-recovery"), &[], None, None, None, None),
        "tool-recovery"
    );
    assert_eq!(
        trace_filter_label(None, &["tool_failures".to_string()], None, None, None, None),
        "all diagnostics=tool_failures"
    );
    assert_eq!(
        trace_filter_label(
            Some("tool-recovery"),
            &[
                "tool_failures".to_string(),
                "tool_failure_recovered".to_string()
            ],
            None,
            None,
            None,
            None,
        ),
        "tool-recovery diagnostics=tool_failures,tool_failure_recovered"
    );
    assert_eq!(
        trace_filter_label(
            Some("skill-use"),
            &["tool_only_turn_streak".to_string()],
            Some(3),
            Some(2),
            Some(10_000),
            Some(64_000),
        ),
        "skill-use diagnostics=tool_only_turn_streak min_tool_only_streak=3 min_overrun_turns=2 min_overrun_context_chars=10000 min_compaction_regrowth_chars=64000"
    );
}

#[test]
fn trace_metric_filters_require_requested_thresholds() {
    let summary = json!({
        "tool_only_turns": {
            "max_consecutive": 8
        },
        "profile_scenario_call_expectations": {
            "extra_turns_after_satisfied": 6,
            "context_growth_after_satisfied_chars": 101846
        },
        "compaction_regrowth": {
            "max_next_request_growth_chars": 64000
        }
    });

    assert!(trace_matches_metric_filters(
        &summary,
        Some(8),
        Some(6),
        Some(101_846),
        Some(64_000)
    ));
    assert!(!trace_matches_metric_filters(
        &summary,
        Some(9),
        Some(6),
        Some(101_846),
        Some(64_000)
    ));
    assert!(!trace_matches_metric_filters(
        &summary,
        Some(8),
        Some(7),
        Some(101_846),
        Some(64_000)
    ));
    assert!(!trace_matches_metric_filters(
        &summary,
        Some(8),
        Some(6),
        Some(101_847),
        Some(64_000)
    ));
    assert!(!trace_matches_metric_filters(
        &summary,
        Some(8),
        Some(6),
        Some(101_846),
        Some(64_001)
    ));
    assert!(!trace_matches_metric_filters(
        &json!({}),
        Some(1),
        None,
        None,
        None
    ));
}

#[test]
fn trace_export_record_wraps_summary_with_paths() {
    let dir = tempfile::tempdir().expect("tempdir");
    let run = dir.path().join(".spark-runs").join("run-42");
    std::fs::create_dir_all(&run).expect("create trace dir");
    let display = PathBuf::from(".spark-runs").join("run-42");
    let summary = json!({
        "requests": 1,
        "tool_calls": 2,
    });

    let record = trace_export_record(dir.path(), &run, &display, Some(&summary));

    assert_eq!(record["type"], "trace");
    assert_eq!(
        record["trace_dir"],
        format!(".spark-runs{}run-42", std::path::MAIN_SEPARATOR)
    );
    assert!(
        record["trace_dir_abs"]
            .as_str()
            .unwrap()
            .ends_with("run-42")
    );
    assert_eq!(record["summary"]["requests"], 1);
    assert_eq!(record["summary"]["tool_calls"], 2);
}

#[test]
fn active_session_matching_handles_same_path() {
    let active = Some("session-a".to_string());

    assert!(is_active_session(&active, "session-a"));
    assert!(!is_active_session(&active, "session-b"));
    assert!(!is_active_session(&None, "session-a"));
}

#[test]
fn timestamp_session_name_is_filename_safe_and_not_workspace_scoped() {
    let name = timestamp_session_name();

    assert!(name.starts_with("chat-"));
    assert!(!name.starts_with("workspace-"));
    assert!(
        name.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
    );
}
