use crate::cli::{ProfileBenchmarkSuiteKind, ProfileScenarioKind};
use crate::profile_scenarios::{
    codex_cli_benchmark_prompt, prepare_profile_scenario, profile_scenario_expected_skills,
    profile_scenario_expected_tool_calls, profile_scenario_expected_tool_groups,
    profile_scenario_prompts,
};
use crate::{APPROX_CHARS_PER_TOKEN, DEFAULT_COMPACT_AFTER_CHARS};

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
fn steamnetworklib_survey_scenario_matches_observed_question_shape() {
    let prompts = profile_scenario_prompts(ProfileScenarioKind::SteamNetworkLibSurvey, 45_000)
        .expect("scenario");
    let prompt = prompts.first().expect("prompt");

    assert!(prompt.contains("Profile scenario: steamnetworklib-survey"));
    assert!(prompt.contains("What is SteamNetworkLib"));
    assert!(prompt.contains("what does it do"));
    assert!(prompt.contains("how does it work"));
    assert!(prompt.contains("targeted native tools"));
}

#[test]
fn steamnetworklib_survey_declares_enough_evidence_calls() {
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::SteamNetworkLibSurvey);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::SteamNetworkLibSurvey);

    assert_eq!(
        groups,
        vec![vec!["fs.list"], vec!["fs.read"], vec!["fs.search"]]
    );
    assert_eq!(calls.len(), 4);
    assert_eq!(calls[0]["tool"], "fs.list");
    assert_eq!(calls[0]["path"], ".");
    assert_eq!(calls[1]["path"], "README.md");
    assert_eq!(calls[2]["path"], "SteamNetworkClient.cs");
    assert_eq!(calls[3]["tool"], "fs.search");
}

#[test]
fn s1api_survey_scenario_matches_observed_question_shape() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::S1ApiSurvey, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");

    assert!(prompt.contains("Profile scenario: s1api-survey"));
    assert!(prompt.contains("What is S1API"));
    assert!(prompt.contains("what does it do"));
    assert!(prompt.contains("how does it work"));
    assert!(prompt.contains("index.md"));
    assert!(prompt.contains("generated api/_site"));
}

#[test]
fn s1api_survey_declares_enough_evidence_calls() {
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::S1ApiSurvey);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::S1ApiSurvey);

    assert_eq!(groups, vec![vec!["fs.list"], vec!["fs.read"]]);
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0]["tool"], "fs.list");
    assert_eq!(calls[0]["path"], ".");
    assert_eq!(calls[1]["path"], "index.md");
    assert_eq!(calls[2]["path"], "S1API.cs");
}

#[test]
fn repo_architecture_survey_exercises_harness_understanding() {
    let prompts = profile_scenario_prompts(ProfileScenarioKind::RepoArchitectureSurvey, 45_000)
        .expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::RepoArchitectureSurvey);

    assert!(prompt.contains("Profile scenario: repo-architecture-survey"));
    assert!(prompt.contains("architecture map"));
    assert!(prompt.contains("ProfileScenarioKind"));
    assert!(prompt.contains("AgentRunner"));
    assert_eq!(calls.len(), 4);
    assert_eq!(calls[0]["tool"], "fs.list");
    assert_eq!(calls[0]["path"], ".");
    assert_eq!(calls[0]["recursive"], false);
    assert_eq!(calls[1]["path"], "AGENTS.md");
    assert_eq!(calls[2]["path"], "README.md");
    assert_eq!(calls[3]["path"], "src");
}

#[test]
fn benchmark_design_survey_targets_existing_scenario_taxonomy() {
    let prompts = profile_scenario_prompts(ProfileScenarioKind::BenchmarkDesignSurvey, 45_000)
        .expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::BenchmarkDesignSurvey);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::BenchmarkDesignSurvey);

    assert!(prompt.contains("Profile scenario: benchmark-design-survey"));
    assert!(prompt.contains("propose benchmark coverage gaps"));
    assert!(prompt.contains("three concrete new task prompts"));
    assert_eq!(groups, vec![vec!["fs.read"], vec!["fs.search"]]);
    assert_eq!(calls.len(), 4);
    assert_eq!(calls[0]["path"], "src/profile_scenarios.rs");
    assert_eq!(calls[1]["path"], "src/profiler/analyze/expectations.rs");
    assert_eq!(calls[2]["path"], "README.md");
    assert_eq!(calls[3]["path"], "src");
}

#[test]
fn react_calculator_scaffold_prepares_gitignored_project_brief() {
    let dir = tempfile::tempdir().expect("tempdir");

    prepare_profile_scenario(dir.path(), ProfileScenarioKind::ReactCalculatorScaffold)
        .expect("prepare scenario");

    let brief = std::fs::read_to_string(
        dir.path()
            .join(".spark-scenarios")
            .join("react-calculator")
            .join("brief.md"),
    )
    .expect("read brief");

    assert!(brief.contains("React + TypeScript calculator"));
    assert!(brief.contains("Use bun"));
}

#[test]
fn react_calculator_scaffold_declares_project_file_expectations() {
    let prompts = profile_scenario_prompts(ProfileScenarioKind::ReactCalculatorScaffold, 45_000)
        .expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let groups =
        profile_scenario_expected_tool_groups(ProfileScenarioKind::ReactCalculatorScaffold);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::ReactCalculatorScaffold);

    assert!(prompt.contains("Profile scenario: react-calculator-scaffold"));
    assert!(prompt.contains("Use bun for JavaScript package management"));
    assert!(prompt.contains("Do not create files outside this ignored fixture folder"));
    assert!(prompt.contains("Playwright browser smoke check"));
    assert_eq!(
        groups,
        vec![vec!["fs.read"], vec!["fs.write"], vec!["cmd.exec"]]
    );
    assert_eq!(calls.len(), 8);
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/react-calculator/brief.md"
    );
    assert_eq!(
        calls[1]["path"],
        ".spark-scenarios/react-calculator/package.json"
    );
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/react-calculator/index.html"
    );
    assert_eq!(
        calls[3]["path"],
        ".spark-scenarios/react-calculator/src/main.tsx"
    );
    assert_eq!(
        calls[4]["path"],
        ".spark-scenarios/react-calculator/src/App.tsx"
    );
    assert_eq!(
        calls[5]["path"],
        ".spark-scenarios/react-calculator/src/App.test.tsx"
    );
    assert_eq!(
        calls[6]["path"],
        ".spark-scenarios/react-calculator/src/styles.css"
    );
    assert_eq!(calls[7]["tool"], "cmd.exec");
}

#[test]
fn codex_cli_prompt_uses_cli_neutral_actions_for_scaffolding() {
    let prompt = codex_cli_benchmark_prompt(ProfileScenarioKind::ReactCalculatorScaffold);

    assert!(prompt.contains("Codex CLI benchmark scenario: react-calculator-scaffold"));
    assert!(prompt.contains("Use bun for JavaScript package management"));
    assert!(prompt.contains("Create .spark-scenarios/react-calculator/index.html"));
    assert!(prompt.contains("Create .spark-scenarios/react-calculator/src/App.tsx"));
    assert!(!prompt.contains("fs.write"));
    assert!(!prompt.contains("cmd.exec"));
}

#[test]
fn rust_log_analyzer_scaffold_prepares_brief_and_sample_log() {
    let dir = tempfile::tempdir().expect("tempdir");

    prepare_profile_scenario(dir.path(), ProfileScenarioKind::RustLogAnalyzerScaffold)
        .expect("prepare scenario");

    let root = dir
        .path()
        .join(".spark-scenarios")
        .join("rust-log-analyzer");
    let brief = std::fs::read_to_string(root.join("brief.md")).expect("read brief");
    let sample = std::fs::read_to_string(root.join("sample.log")).expect("read sample");

    assert!(brief.contains("Rust CLI project"));
    assert!(brief.contains("do not set CARGO_TARGET_DIR"));
    assert!(sample.contains("ERROR code=E42"));
}

#[test]
fn rust_log_analyzer_scaffold_declares_project_file_expectations() {
    let prompts = profile_scenario_prompts(ProfileScenarioKind::RustLogAnalyzerScaffold, 45_000)
        .expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let groups =
        profile_scenario_expected_tool_groups(ProfileScenarioKind::RustLogAnalyzerScaffold);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::RustLogAnalyzerScaffold);

    assert!(prompt.contains("Profile scenario: rust-log-analyzer-scaffold"));
    assert!(prompt.contains("Do not set CARGO_TARGET_DIR"));
    assert!(prompt.contains("cargo test"));
    assert_eq!(
        groups,
        vec![vec!["fs.read"], vec!["fs.write"], vec!["cmd.exec"]]
    );
    assert_eq!(calls.len(), 6);
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/rust-log-analyzer/brief.md"
    );
    assert_eq!(
        calls[1]["path"],
        ".spark-scenarios/rust-log-analyzer/sample.log"
    );
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/rust-log-analyzer/Cargo.toml"
    );
    assert_eq!(
        calls[3]["path"],
        ".spark-scenarios/rust-log-analyzer/src/lib.rs"
    );
    assert_eq!(
        calls[4]["path"],
        ".spark-scenarios/rust-log-analyzer/src/main.rs"
    );
    assert_eq!(calls[5]["tool"], "cmd.exec");
}

#[test]
fn benchmark_suites_group_existing_and_real_world_scenarios() {
    assert_eq!(
        ProfileBenchmarkSuiteKind::Scaffolding.scenarios(),
        &[
            ProfileScenarioKind::ReactCalculatorScaffold,
            ProfileScenarioKind::RustLogAnalyzerScaffold
        ]
    );
    assert!(
        ProfileBenchmarkSuiteKind::Survey
            .scenarios()
            .contains(&ProfileScenarioKind::BenchmarkDesignSurvey)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::ReactCalculatorScaffold)
    );
    assert!(
        ProfileBenchmarkSuiteKind::Core
            .scenarios()
            .contains(&ProfileScenarioKind::ToolRecovery)
    );
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
