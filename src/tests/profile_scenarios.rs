use crate::cli::{ProfileBenchmarkSuiteKind, ProfileScenarioKind};
use crate::profile::scenarios::{
    benchmark_profile_prompts, benchmark_task_prompt, codex_cli_benchmark_prompt,
    prepare_profile_scenario, profile_scenario_expected_skills,
    profile_scenario_expected_tool_calls, profile_scenario_expected_tool_groups,
    profile_scenario_prompts, profile_scenario_validation_command,
};
use crate::{APPROX_CHARS_PER_TOKEN, DEFAULT_COMPACT_AFTER_CHARS};
use serde_json::json;

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
    assert!(prompt.contains("Recover by using fs.read"));
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

    assert_eq!(groups, vec![vec!["fs.read"]]);
}

#[test]
fn tool_recovery_scenario_declares_expected_exact_tool_calls() {
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::ToolRecovery);

    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["tool"], "fs.read");
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/tool-recovery/source/missing-note.md"
    );
    assert_eq!(calls[0]["ok"], false);
    assert_eq!(calls[1]["tool"], "fs.read");
    assert_eq!(
        calls[1]["path"],
        ".spark-scenarios/tool-recovery/source/note.md"
    );
}

#[test]
fn shell_recovery_scenario_exercises_terminal_error_recovery() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::ShellRecovery, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::ShellRecovery);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::ShellRecovery);

    assert!(prompt.contains("Profile scenario: shell-recovery"));
    assert!(prompt.contains("intentionally wrong command"));
    assert!(prompt.contains("inspect stdout/stderr"));
    assert_eq!(
        groups,
        vec![
            vec!["cmd.exec"],
            vec!["fs.list", "fs.search"],
            vec!["fs.read"]
        ]
    );
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0]["tool"], "cmd.exec");
    assert_eq!(calls[0]["ok"], false);
    assert_eq!(calls[1]["tool"], "cmd.exec");
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/shell-recovery/summary.txt"
    );
    assert!(profile_scenario_validation_command(ProfileScenarioKind::ShellRecovery).is_some());
}

#[test]
fn shell_recovery_scenario_prepares_script_and_events_fixture() {
    let dir = tempfile::tempdir().expect("tempdir");

    prepare_profile_scenario(dir.path(), ProfileScenarioKind::ShellRecovery)
        .expect("prepare scenario");

    let root = dir.path().join(".spark-scenarios").join("shell-recovery");
    let events =
        std::fs::read_to_string(root.join("data").join("events.csv")).expect("read events fixture");
    let script = std::fs::read_to_string(root.join("tools").join("analyze-events.ps1"))
        .expect("read script fixture");

    assert!(events.contains("payments,failed"));
    assert!(script.contains("top_service"));
    assert!(!root.join("scripts").join("analyze-events.ps1").exists());
}

#[test]
fn precise_patch_scenario_exercises_minimal_code_edit() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::PrecisePatch, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::PrecisePatch);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::PrecisePatch);

    assert!(prompt.contains("Profile scenario: precise-patch"));
    assert!(prompt.contains("without over-editing"));
    assert!(prompt.contains("default branch still returns Unknown"));
    assert_eq!(
        groups,
        vec![
            vec!["fs.read"],
            vec!["fs.edit", "fs.replace"],
            vec!["fs.search"]
        ]
    );
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/precise-patch/tests/status_map.spec.md"
    );
    assert_eq!(
        calls[1]["path"],
        ".spark-scenarios/precise-patch/src/status_map.ts"
    );
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/precise-patch/src/status_map.ts"
    );
    assert_eq!(calls[2]["tools"], json!(["fs.edit", "fs.replace"]));
    assert_eq!(calls[3]["path"], ".spark-scenarios/precise-patch/src");
    assert!(profile_scenario_validation_command(ProfileScenarioKind::PrecisePatch).is_some());
}

#[test]
fn precise_patch_scenario_prepares_code_fixture() {
    let dir = tempfile::tempdir().expect("tempdir");

    prepare_profile_scenario(dir.path(), ProfileScenarioKind::PrecisePatch)
        .expect("prepare scenario");

    let root = dir.path().join(".spark-scenarios").join("precise-patch");
    let source =
        std::fs::read_to_string(root.join("src").join("status_map.ts")).expect("read source");
    let spec =
        std::fs::read_to_string(root.join("tests").join("status_map.spec.md")).expect("read spec");

    assert!(source.contains("case 'queued'"));
    assert!(source.contains("return 'Unknown';"));
    assert!(spec.contains("`queued` must render as `Queued`"));
}

#[test]
fn multi_file_patch_scenario_exercises_coordinated_updates() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::MultiFilePatch, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::MultiFilePatch);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::MultiFilePatch);

    assert!(prompt.contains("Profile scenario: multi-file-patch"));
    assert!(prompt.contains("coordinate a small feature across multiple files"));
    assert!(prompt.contains("Reports navigation item"));
    assert_eq!(
        groups,
        vec![
            vec!["fs.read"],
            vec!["fs.edit", "fs.replace", "fs.write"],
            vec!["fs.search"]
        ]
    );
    assert_eq!(calls.len(), 7);
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/multi-file-patch/src/routes.ts"
    );
    assert_eq!(
        calls[1]["path"],
        ".spark-scenarios/multi-file-patch/src/navigation.ts"
    );
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/multi-file-patch/docs/routes.md"
    );
    assert_eq!(
        calls[3]["path"],
        ".spark-scenarios/multi-file-patch/src/routes.ts"
    );
    assert_eq!(
        calls[3]["tools"],
        json!(["fs.edit", "fs.replace", "fs.write"])
    );
    assert_eq!(
        calls[4]["path"],
        ".spark-scenarios/multi-file-patch/src/navigation.ts"
    );
    assert_eq!(
        calls[5]["path"],
        ".spark-scenarios/multi-file-patch/docs/routes.md"
    );
    assert_eq!(calls[6]["path"], ".spark-scenarios/multi-file-patch");
    assert!(profile_scenario_validation_command(ProfileScenarioKind::MultiFilePatch).is_some());
}

#[test]
fn multi_file_patch_scenario_prepares_code_and_docs_fixture() {
    let dir = tempfile::tempdir().expect("tempdir");

    prepare_profile_scenario(dir.path(), ProfileScenarioKind::MultiFilePatch)
        .expect("prepare scenario");

    let root = dir.path().join(".spark-scenarios").join("multi-file-patch");
    let routes = std::fs::read_to_string(root.join("src").join("routes.ts")).expect("routes");
    let nav = std::fs::read_to_string(root.join("src").join("navigation.ts")).expect("navigation");
    let docs = std::fs::read_to_string(root.join("docs").join("routes.md")).expect("docs");

    assert!(routes.contains("settings"));
    assert!(nav.contains("Settings"));
    assert!(docs.contains("/settings"));
    assert!(!routes.contains("reports"));
    assert!(!nav.contains("Reports"));
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
    assert_eq!(calls[0]["path"], "src/profile/scenarios.rs");
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
    assert!(brief.contains("harness-owned Playwright browser smoke check"));
    assert!(brief.contains("do not install Playwright"));
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
    assert!(prompt.contains("harness-owned Playwright browser smoke check"));
    assert!(prompt.contains("Do not install Playwright"));
    assert!(prompt.contains("the harness will run that browser smoke check externally"));
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
    assert_eq!(calls[7]["command"], "bun test");
}

#[test]
fn codex_cli_prompt_uses_cli_neutral_actions_for_scaffolding() {
    let prompt = codex_cli_benchmark_prompt(ProfileScenarioKind::ReactCalculatorScaffold);

    assert!(prompt.contains("Benchmark scenario: react-calculator-scaffold"));
    assert!(prompt.contains("Use bun for JavaScript package management"));
    assert!(prompt.contains("Create .spark-scenarios/react-calculator/index.html"));
    assert!(prompt.contains("Create .spark-scenarios/react-calculator/src/App.tsx"));
    assert!(prompt.contains("Do not install Playwright"));
    assert!(prompt.contains("the harness owns that browser smoke check"));
    assert!(!prompt.contains("fs.write"));
    assert!(!prompt.contains("cmd.exec"));
}

#[test]
fn benchmark_profile_prompt_matches_codex_cli_prompt_for_fairness() {
    let prompts = benchmark_profile_prompts(ProfileScenarioKind::ReactCalculatorScaffold, 45_000)
        .expect("benchmark prompt");

    assert_eq!(
        prompts,
        vec![codex_cli_benchmark_prompt(
            ProfileScenarioKind::ReactCalculatorScaffold
        )]
    );
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
    assert_eq!(calls[5]["command"], "cargo test");
}

#[test]
fn rust_notes_tui_scaffold_prepares_brief_and_validation_script() {
    let dir = tempfile::tempdir().expect("tempdir");

    prepare_profile_scenario(dir.path(), ProfileScenarioKind::RustNotesTuiScaffold)
        .expect("prepare scenario");

    let root = dir.path().join(".spark-scenarios").join("rust-notes-tui");
    let brief = std::fs::read_to_string(root.join("brief.md")).expect("read brief");
    let validation =
        std::fs::read_to_string(root.join("validate-notes.ps1")).expect("read validation");

    assert!(brief.contains("notevim"));
    assert!(brief.contains("vim-style notes tool"));
    assert!(brief.contains("do not set CARGO_TARGET_DIR"));
    assert!(validation.contains("cargo test"));
    assert!(validation.contains("help-keys"));
    assert!(validation.contains("export missing"));
}

#[test]
fn rust_notes_tui_scaffold_declares_project_file_expectations() {
    let prompts = profile_scenario_prompts(ProfileScenarioKind::RustNotesTuiScaffold, 45_000)
        .expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let benchmark_prompt = benchmark_task_prompt(ProfileScenarioKind::RustNotesTuiScaffold);
    let validation = profile_scenario_validation_command(ProfileScenarioKind::RustNotesTuiScaffold)
        .expect("validation");
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::RustNotesTuiScaffold);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::RustNotesTuiScaffold);

    assert!(prompt.contains("Profile scenario: rust-notes-tui-scaffold"));
    assert!(benchmark_prompt.contains("Benchmark scenario: rust-notes-tui-scaffold"));
    assert!(benchmark_prompt.contains("help-keys"));
    assert!(benchmark_prompt.contains("validate-notes.ps1"));
    assert!(
        benchmark_prompt
            .contains("Do not manually run the full add/list/search/export/help-keys smoke path")
    );
    assert!(benchmark_prompt.contains("Do not set CARGO_TARGET_DIR"));
    assert_eq!(validation.workdir, ".spark-scenarios/rust-notes-tui");
    assert_eq!(validation.program, "powershell");
    assert_eq!(
        validation.args,
        &["-NoProfile", "-File", "validate-notes.ps1"]
    );
    assert_eq!(
        groups,
        vec![vec!["fs.read"], vec!["fs.write"], vec!["cmd.exec"]]
    );
    assert_eq!(calls.len(), 5);
    assert_eq!(calls[0]["path"], ".spark-scenarios/rust-notes-tui/brief.md");
    assert_eq!(
        calls[1]["path"],
        ".spark-scenarios/rust-notes-tui/Cargo.toml"
    );
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/rust-notes-tui/src/lib.rs"
    );
    assert_eq!(
        calls[3]["path"],
        ".spark-scenarios/rust-notes-tui/src/main.rs"
    );
    assert_eq!(calls[4]["tool"], "cmd.exec");
    assert_eq!(calls[4]["command"], "cargo test");
}

#[test]
fn rust_failing_test_bugfix_declares_cargo_validation_expectations() {
    let prompts = profile_scenario_prompts(ProfileScenarioKind::RustFailingTestBugfix, 45_000)
        .expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let benchmark_prompt = benchmark_task_prompt(ProfileScenarioKind::RustFailingTestBugfix);
    let validation =
        profile_scenario_validation_command(ProfileScenarioKind::RustFailingTestBugfix)
            .expect("validation");
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::RustFailingTestBugfix);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::RustFailingTestBugfix);

    assert!(prompt.contains("Profile scenario: rust-failing-test-bugfix"));
    assert!(benchmark_prompt.contains("Do not set CARGO_TARGET_DIR"));
    assert_eq!(
        validation.workdir,
        ".spark-scenarios/rust-failing-test-bugfix"
    );
    assert_eq!(validation.program, "cargo");
    assert_eq!(validation.args, &["test"]);
    assert_eq!(
        groups,
        vec![
            vec!["fs.read"],
            vec!["fs.edit", "fs.replace"],
            vec!["cmd.exec"]
        ]
    );
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/rust-failing-test-bugfix/issue.md"
    );
    assert_eq!(
        calls[1]["path"],
        ".spark-scenarios/rust-failing-test-bugfix/src/lib.rs"
    );
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/rust-failing-test-bugfix/tests/retry_scheduler.rs"
    );
    assert_eq!(
        calls[3]["path"],
        ".spark-scenarios/rust-failing-test-bugfix/src/lib.rs"
    );
    assert_eq!(
        calls[3]["tools"],
        json!(["fs.edit", "fs.replace", "fs.write"])
    );
    assert_eq!(calls[4]["tool"], "cmd.exec");
    assert_eq!(calls[4]["command"], "cargo test");
}

#[test]
fn typescript_reducer_bugfix_declares_bun_validation_expectations() {
    let prompts = profile_scenario_prompts(ProfileScenarioKind::TypeScriptReducerBugfix, 45_000)
        .expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let benchmark_prompt = benchmark_task_prompt(ProfileScenarioKind::TypeScriptReducerBugfix);
    let validation =
        profile_scenario_validation_command(ProfileScenarioKind::TypeScriptReducerBugfix)
            .expect("validation");
    let groups =
        profile_scenario_expected_tool_groups(ProfileScenarioKind::TypeScriptReducerBugfix);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::TypeScriptReducerBugfix);

    assert!(prompt.contains("Profile scenario: typescript-reducer-bugfix"));
    assert!(benchmark_prompt.contains("Use bun for JavaScript package management"));
    assert_eq!(
        validation.workdir,
        ".spark-scenarios/typescript-reducer-bugfix"
    );
    assert_eq!(validation.program, "bun");
    assert_eq!(validation.args, &["test"]);
    assert_eq!(
        groups,
        vec![
            vec!["fs.read"],
            vec!["fs.edit", "fs.replace"],
            vec!["cmd.exec"]
        ]
    );
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/typescript-reducer-bugfix/issue.md"
    );
    assert_eq!(
        calls[1]["path"],
        ".spark-scenarios/typescript-reducer-bugfix/src/cart.ts"
    );
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/typescript-reducer-bugfix/tests/cart.test.ts"
    );
    assert_eq!(
        calls[3]["path"],
        ".spark-scenarios/typescript-reducer-bugfix/src/cart.ts"
    );
    assert_eq!(
        calls[3]["tools"],
        json!(["fs.edit", "fs.replace", "fs.write"])
    );
    assert_eq!(calls[4]["tool"], "cmd.exec");
    assert_eq!(calls[4]["command"], "bun test");
}

#[test]
fn config_migration_declares_ordered_mutation_and_validation_expectations() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::ConfigMigration, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let validation = profile_scenario_validation_command(ProfileScenarioKind::ConfigMigration)
        .expect("validation");
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::ConfigMigration);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::ConfigMigration);

    assert!(prompt.contains("Profile scenario: config-migration"));
    assert!(prompt.contains("Use cmd.exec or fs.search"));
    assert_eq!(validation.program, "powershell");
    assert_eq!(
        groups,
        vec![
            vec!["fs.read"],
            vec!["fs.edit", "fs.replace", "fs.write"],
            vec!["cmd.exec", "fs.search"]
        ]
    );
    assert_eq!(calls.len(), 8);
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/config-migration/migration.md"
    );
    assert_eq!(
        calls[4]["path"],
        ".spark-scenarios/config-migration/config/app.json"
    );
    assert_eq!(
        calls[4]["tools"],
        json!(["fs.edit", "fs.replace", "fs.write"])
    );
    assert_eq!(
        calls[5]["path"],
        ".spark-scenarios/config-migration/src/config.ts"
    );
    assert_eq!(
        calls[6]["path"],
        ".spark-scenarios/config-migration/docs/config.md"
    );
    assert_eq!(calls[7]["tools"], json!(["cmd.exec", "fs.search"]));
}

#[test]
fn technical_essay_uses_read_metadata_for_final_word_count_check() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::TechnicalEssay, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::TechnicalEssay);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::TechnicalEssay);

    assert!(prompt.contains("Profile scenario: technical-essay"));
    assert!(prompt.contains("fs.read total_words"));
    assert!(prompt.contains("do not use cmd.exec just to count words"));
    assert_eq!(groups, vec![vec!["fs.read"], vec!["fs.write"]]);
    assert_eq!(calls.len(), 3);
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/technical-essay/brief.md"
    );
    assert_eq!(
        calls[1]["path"],
        ".spark-scenarios/technical-essay/essay.md"
    );
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/technical-essay/essay.md"
    );
    assert_eq!(calls[2]["tool"], "fs.read");
}

#[test]
fn benchmark_suites_group_existing_and_real_world_scenarios() {
    assert_eq!(
        ProfileBenchmarkSuiteKind::Scaffolding.scenarios(),
        &[
            ProfileScenarioKind::ReactCalculatorScaffold,
            ProfileScenarioKind::RustLogAnalyzerScaffold,
            ProfileScenarioKind::RustNotesTuiScaffold
        ]
    );
    assert_eq!(
        ProfileBenchmarkSuiteKind::Editing.scenarios(),
        &[
            ProfileScenarioKind::PrecisePatch,
            ProfileScenarioKind::MultiFilePatch,
            ProfileScenarioKind::GithubIssueBugfix,
            ProfileScenarioKind::RustFailingTestBugfix,
            ProfileScenarioKind::TypeScriptReducerBugfix,
            ProfileScenarioKind::ConfigMigration
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
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::RustNotesTuiScaffold)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::ShellRecovery)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::MultiFilePatch)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::GithubIssueBugfix)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::RustFailingTestBugfix)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::TypeScriptReducerBugfix)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::TechnicalEssay)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::OpsReport)
    );
    assert!(
        ProfileBenchmarkSuiteKind::Core
            .scenarios()
            .contains(&ProfileScenarioKind::ToolRecovery)
    );
    assert!(
        ProfileBenchmarkSuiteKind::Core
            .scenarios()
            .contains(&ProfileScenarioKind::ShellRecovery)
    );
}

#[test]
fn real_world_issue_writing_and_reporting_scenarios_prepare_fixtures() {
    let scenarios = [
        (
            ProfileScenarioKind::GithubIssueBugfix,
            ".spark-scenarios/github-issue-bugfix/issue.md",
            "annual quotes are undercharged",
        ),
        (
            ProfileScenarioKind::RustFailingTestBugfix,
            ".spark-scenarios/rust-failing-test-bugfix/tests/retry_scheduler.rs",
            "returns_highest_priority_jobs_first",
        ),
        (
            ProfileScenarioKind::TypeScriptReducerBugfix,
            ".spark-scenarios/typescript-reducer-bugfix/tests/cart.test.ts",
            "subtotal ignores inactive restored lines",
        ),
        (
            ProfileScenarioKind::GithubIssueTriage,
            ".spark-scenarios/github-issue-triage/src/cachePolicy.ts",
            "stale-while-revalidate=30",
        ),
        (
            ProfileScenarioKind::TechnicalEssay,
            ".spark-scenarios/technical-essay/brief.md",
            "Operational Visibility Is a Product Feature",
        ),
        (
            ProfileScenarioKind::ConfigMigration,
            ".spark-scenarios/config-migration/migration.md",
            "schema version 2",
        ),
        (
            ProfileScenarioKind::OpsReport,
            ".spark-scenarios/ops-report/data/tickets.csv",
            "billing,P1,open,95",
        ),
    ];

    for (scenario, path, expected) in scenarios {
        let dir = tempfile::tempdir().expect("tempdir");
        prepare_profile_scenario(dir.path(), scenario).expect("prepare scenario");
        let content = std::fs::read_to_string(dir.path().join(path)).expect("fixture content");
        assert!(
            content.contains(expected),
            "{scenario:?} fixture missing {expected}"
        );
        if scenario == ProfileScenarioKind::OpsReport {
            assert!(
                content.contains("Treat the first CSV line as the header, not a ticket")
                    || std::fs::read_to_string(
                        dir.path().join(".spark-scenarios/ops-report/brief.md")
                    )
                    .expect("ops brief")
                    .contains("Treat the first CSV line as the header, not a ticket"),
                "ops-report fixture should make header handling explicit"
            );
        }
        assert!(
            profile_scenario_validation_command(scenario).is_some(),
            "{scenario:?} should have deterministic validation"
        );
    }
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
