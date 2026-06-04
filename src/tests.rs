use crate::chat::command_args;
use crate::cli::{ProfileScenarioKind, TraceSort};
use crate::profile_scenarios::{
    prepare_profile_scenario, profile_scenario_expected_skills,
    profile_scenario_expected_tool_calls, profile_scenario_expected_tool_groups,
    profile_scenario_prompts, validate_scenario_repeat,
};
use crate::sessions::{is_active_session, session_name_for_display};
use crate::skill_commands::{contains_skill_mention, mentioned_skill_names};
use crate::trace_commands::{
    TraceListRecord, latest_trace_dir, list_trace_dirs, resolve_char_threshold, sort_trace_records,
    trace_export_record, trace_filter_label, trace_has_all_diagnostics,
    trace_matches_metric_filters, trace_runs_root, trace_sort_metric, trace_sort_name,
};
use crate::{APPROX_CHARS_PER_TOKEN, DEFAULT_COMPACT_AFTER_CHARS};
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
        trace_filter_label(Some("tool-recovery"), &[], None, None, None),
        "tool-recovery"
    );
    assert_eq!(
        trace_filter_label(None, &["tool_failures".to_string()], None, None, None),
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
        ),
        "skill-use diagnostics=tool_only_turn_streak min_tool_only_streak=3 min_overrun_turns=2 min_overrun_context_chars=10000"
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
        }
    });

    assert!(trace_matches_metric_filters(
        &summary,
        Some(8),
        Some(6),
        Some(101_846)
    ));
    assert!(!trace_matches_metric_filters(
        &summary,
        Some(9),
        Some(6),
        Some(101_846)
    ));
    assert!(!trace_matches_metric_filters(
        &summary,
        Some(8),
        Some(7),
        Some(101_846)
    ));
    assert!(!trace_matches_metric_filters(
        &summary,
        Some(8),
        Some(6),
        Some(101_847)
    ));
    assert!(!trace_matches_metric_filters(
        &json!({}),
        Some(1),
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
    let path = PathBuf::from("session-a.json");
    let active = Some(path.clone());

    assert!(is_active_session(&active, &path));
    assert!(!is_active_session(
        &active,
        &PathBuf::from("session-b.json")
    ));
    assert!(!is_active_session(&None, &path));
}

#[test]
fn session_display_name_uses_file_stem() {
    assert_eq!(
        session_name_for_display(&PathBuf::from("demo.session.json")),
        "demo.session"
    );
}

#[test]
fn compaction_pressure_scenario_targets_prompt_size() {
    let prompts = profile_scenario_prompts(ProfileScenarioKind::CompactionPressure, 45_000)
        .expect("scenario prompt");
    let prompt = prompts.first().expect("prompt");

    assert!(prompt.contains("Profile scenario: compaction-pressure"));
    assert!(prompt.contains("Synthetic payload follows"));
    assert!(prompt.len() >= 45_000 * APPROX_CHARS_PER_TOKEN);
    assert!(prompt.len() < 46_000 * APPROX_CHARS_PER_TOKEN);
}

#[test]
fn compaction_pressure_scenario_caps_below_context_window() {
    let error = profile_scenario_prompts(ProfileScenarioKind::CompactionPressure, 120_001)
        .expect_err("scenario should reject oversized target");

    assert!(
        error
            .to_string()
            .contains("below Spark's 128k context window")
    );
}

#[test]
fn repo_survey_scenario_is_small_and_tool_directed() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::RepoSurvey, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");

    assert!(prompt.contains("Profile scenario: repo-survey"));
    assert!(prompt.contains("Use targeted native tools"));
    assert!(prompt.len() < 1_000);
}

#[test]
fn file_edit_scenario_is_scoped_to_scratch_files() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::FileEdit, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");

    assert!(prompt.contains("Profile scenario: file-edit"));
    assert!(prompt.contains("Work only under .spark-scenarios/file-edit"));
    assert!(prompt.contains("Use fs.edit or fs.replace"));
    assert!(prompt.contains("Use fs.write"));
}

#[test]
fn file_edit_scenario_prepares_scratch_fixture() {
    let dir = tempfile::tempdir().expect("tempdir");

    prepare_profile_scenario(dir.path(), ProfileScenarioKind::FileEdit).expect("prepare scenario");

    let notes = std::fs::read_to_string(
        dir.path()
            .join(".spark-scenarios")
            .join("file-edit")
            .join("notes.md"),
    )
    .expect("read notes");
    let config = std::fs::read_to_string(
        dir.path()
            .join(".spark-scenarios")
            .join("file-edit")
            .join("config.toml"),
    )
    .expect("read config");

    assert!(notes.contains("TODO: replace this line"));
    assert!(config.contains("mode = \"draft\""));
}

#[test]
fn file_ops_scenario_exercises_native_rename_flow() {
    let prompts = profile_scenario_prompts(ProfileScenarioKind::FileOps, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");

    assert!(prompt.contains("Profile scenario: file-ops"));
    assert!(prompt.contains("Work only under .spark-scenarios/file-ops"));
    assert!(prompt.contains("Use fs.write"));
    assert!(prompt.contains("Use fs.rename"));
    assert!(prompt.contains("Use fs.stat"));
    assert!(prompt.contains("Use fs.search"));
    assert!(prompt.contains("not cmd.exec"));
}

#[test]
fn file_ops_scenario_prepares_scratch_fixture() {
    let dir = tempfile::tempdir().expect("tempdir");

    prepare_profile_scenario(dir.path(), ProfileScenarioKind::FileOps).expect("prepare scenario");

    let root = dir.path().join(".spark-scenarios").join("file-ops");
    let manifest = std::fs::read_to_string(root.join("manifest.txt")).expect("read manifest");

    assert!(root.join("drafts").is_dir());
    assert!(manifest.contains("expected_final=final/report.md"));
}

#[test]
fn file_ops_scenario_declares_expected_tool_groups() {
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::FileOps);

    assert_eq!(
        groups,
        vec![
            vec!["fs.write"],
            vec!["fs.rename"],
            vec!["fs.stat"],
            vec!["fs.read"],
            vec!["fs.search"]
        ]
    );
}

#[test]
fn file_ops_scenario_declares_expected_exact_tool_calls() {
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::FileOps);

    assert_eq!(calls.len(), 5);
    assert_eq!(calls[0]["tool"], "fs.write");
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/file-ops/drafts/report-draft.md"
    );
    assert_eq!(calls[1]["tool"], "fs.rename");
    assert_eq!(
        calls[1]["from"],
        ".spark-scenarios/file-ops/drafts/report-draft.md"
    );
    assert_eq!(calls[1]["to"], ".spark-scenarios/file-ops/final/report.md");
    assert_eq!(calls[2]["tool"], "fs.stat");
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/file-ops/final/report.md"
    );
    assert_eq!(calls[3]["tool"], "fs.read");
    assert_eq!(
        calls[3]["path"],
        ".spark-scenarios/file-ops/final/report.md"
    );
    assert_eq!(calls[4]["tool"], "fs.search");
    assert_eq!(calls[4]["path"], ".spark-scenarios/file-ops");
}

#[test]
fn tool_recovery_scenario_exercises_failed_probe_then_recovery() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::ToolRecovery, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");

    assert!(prompt.contains("Profile scenario: tool-recovery"));
    assert!(prompt.contains("missing-note.md"));
    assert!(prompt.contains("This path is intentionally missing"));
    assert!(prompt.contains("Use fs.stat"));
    assert!(prompt.contains("Use fs.write"));
    assert!(prompt.contains("not cmd.exec"));
}

#[test]
fn tool_recovery_scenario_prepares_scratch_fixture() {
    let dir = tempfile::tempdir().expect("tempdir");

    prepare_profile_scenario(dir.path(), ProfileScenarioKind::ToolRecovery)
        .expect("prepare scenario");

    let root = dir.path().join(".spark-scenarios").join("tool-recovery");
    let note = std::fs::read_to_string(root.join("source").join("note.md")).expect("read note");

    assert!(root.join("source").is_dir());
    assert!(note.contains("Spark recovery path verified."));
    assert!(!root.join("source").join("missing-note.md").exists());
}

#[test]
fn tool_recovery_scenario_declares_recovery_tool_groups() {
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::ToolRecovery);

    assert_eq!(
        groups,
        vec![vec!["fs.read"], vec!["fs.stat"], vec!["fs.write"]]
    );
}

#[test]
fn tool_recovery_scenario_declares_expected_exact_tool_calls() {
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::ToolRecovery);

    assert_eq!(calls.len(), 4);
    assert_eq!(calls[0]["tool"], "fs.read");
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/tool-recovery/source/missing-note.md"
    );
    assert_eq!(calls[1]["tool"], "fs.stat");
    assert_eq!(
        calls[1]["path"],
        ".spark-scenarios/tool-recovery/source/note.md"
    );
    assert_eq!(calls[2]["tool"], "fs.read");
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/tool-recovery/source/note.md"
    );
    assert_eq!(calls[3]["tool"], "fs.write");
    assert_eq!(
        calls[3]["path"],
        ".spark-scenarios/tool-recovery/recovery-summary.txt"
    );
}

#[test]
fn skill_use_scenario_exercises_skill_mention_and_native_tools() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::SkillUse, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");

    assert!(prompt.contains("Profile scenario: skill-use"));
    assert!(prompt.contains("@rust-patterns"));
    assert!(prompt.contains("Use fs.read"));
    assert!(prompt.contains("Use fs.search"));
    assert!(prompt.contains("not cmd.exec"));
}

#[test]
fn skill_use_scenario_declares_expected_tools_and_skill() {
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::SkillUse);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::SkillUse);
    let skills = profile_scenario_expected_skills(ProfileScenarioKind::SkillUse);

    assert_eq!(groups, vec![vec!["fs.read"], vec!["fs.search"]]);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["tool"], "fs.read");
    assert_eq!(calls[0]["path"], "src/main.rs");
    assert_eq!(calls[1]["tool"], "fs.search");
    assert_eq!(calls[1]["path"], "src");
    assert_eq!(skills, vec!["rust-patterns"]);
}

#[test]
fn natural_compaction_scenario_uses_multiple_chat_turns() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::NaturalCompaction, 45_000).expect("scenario");
    let total_chars = prompts.iter().map(String::len).sum::<usize>();

    assert_eq!(prompts.len(), 3);
    assert!(total_chars >= DEFAULT_COMPACT_AFTER_CHARS);
    assert!(total_chars / APPROX_CHARS_PER_TOKEN < 120_000);
    assert!(prompts[0].contains("turn 1/3"));
    assert!(prompts[1].contains("turn 2/3"));
    assert!(prompts[2].contains("fs.list on src with recursive=false"));
}
