use crate::cli::{ProfileBenchmarkSuiteKind, ProfileScenarioKind};
use crate::profile::scenarios::{
    benchmark_profile_prompts, benchmark_task_prompt, codex_cli_benchmark_prompt,
    prepare_profile_scenario, profile_scenario_expected_skills,
    profile_scenario_expected_tool_calls, profile_scenario_expected_tool_groups,
    profile_scenario_optional_tool_calls, profile_scenario_prompts,
    profile_scenario_validation_command,
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
    let benchmark_prompt = benchmark_task_prompt(ProfileScenarioKind::PrecisePatch);
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::PrecisePatch);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::PrecisePatch);

    assert!(prompt.contains("Profile scenario: precise-patch"));
    assert!(prompt.contains("without over-editing"));
    assert!(prompt.contains("default branch still returns Unknown"));
    assert!(prompt.contains("appears in more than one branch"));
    assert!(prompt.contains("do not replace that bare line globally"));
    assert!(prompt.contains("branch label is preserved"));
    assert!(benchmark_prompt.contains("do not replace that bare line globally"));
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
    assert_eq!(calls.len(), 8);
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
    assert_eq!(calls[7]["path"], ".spark-scenarios/multi-file-patch");
    assert!(profile_scenario_validation_command(ProfileScenarioKind::MultiFilePatch).is_some());
}

#[test]
fn multi_file_patch_allows_optional_final_artifact_verification() {
    let calls = profile_scenario_optional_tool_calls(ProfileScenarioKind::MultiFilePatch);

    assert_eq!(calls.len(), 3);
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
    let benchmark_prompt = benchmark_task_prompt(ProfileScenarioKind::RustLogAnalyzerScaffold);
    let groups =
        profile_scenario_expected_tool_groups(ProfileScenarioKind::RustLogAnalyzerScaffold);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::RustLogAnalyzerScaffold);
    let validation =
        profile_scenario_validation_command(ProfileScenarioKind::RustLogAnalyzerScaffold)
            .expect("validation");

    assert!(prompt.contains("Profile scenario: rust-log-analyzer-scaffold"));
    assert!(benchmark_prompt.contains("Benchmark scenario: rust-log-analyzer-scaffold"));
    assert!(prompt.contains("Do not set CARGO_TARGET_DIR"));
    assert!(benchmark_prompt.contains("Do not list the scenario directory"));
    assert!(benchmark_prompt.contains("harness will run the CLI sample-log smoke check"));
    assert!(benchmark_prompt.contains("Do not run cargo run manually"));
    assert!(prompt.contains("cargo test"));
    assert_eq!(validation.workdir, ".spark-scenarios/rust-log-analyzer");
    assert_eq!(validation.program, "powershell");
    assert!(validation.args.join(" ").contains("cargo test"));
    assert!(validation.args.join(" ").contains("cargo run"));
    assert!(validation.args.join(" ").contains("Top error code"));
    assert!(validation.args.join(" ").contains("E42"));
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
fn benchmark_bugfix_prompts_use_exact_files_without_listing() {
    let cases = [
        (
            ProfileScenarioKind::GithubIssueBugfix,
            ".spark-scenarios/github-issue-bugfix/src/quote.ts",
            ".spark-scenarios/github-issue-bugfix/tests/quote.test.ts",
        ),
        (
            ProfileScenarioKind::RustFailingTestBugfix,
            ".spark-scenarios/rust-failing-test-bugfix/src/lib.rs",
            ".spark-scenarios/rust-failing-test-bugfix/tests/retry_scheduler.rs",
        ),
        (
            ProfileScenarioKind::TypeScriptReducerBugfix,
            ".spark-scenarios/typescript-reducer-bugfix/src/cart.ts",
            ".spark-scenarios/typescript-reducer-bugfix/tests/cart.test.ts",
        ),
    ];

    for (scenario, source, test) in cases {
        let prompt = benchmark_task_prompt(scenario);

        assert!(prompt.contains(source));
        assert!(prompt.contains(test));
        assert!(prompt.contains("Do not list the scenario directory"));
        assert!(prompt.contains("the paths above are the complete evidence set"));
    }
}

#[test]
fn ci_failure_triage_prompt_delegates_final_artifact_check_to_harness() {
    let prompt = benchmark_task_prompt(ProfileScenarioKind::CiFailureTriage);

    assert!(prompt.contains("issue.md"));
    assert!(prompt.contains("logs/frontend-tests.log"));
    assert!(prompt.contains("src/discount.ts"));
    assert!(prompt.contains("tests/discount.test.ts"));
    assert!(prompt.contains("Do not list the scenario directory"));
    assert!(prompt.contains("Do not re-read ci-triage.md solely to verify terms"));
    assert!(prompt.contains("the harness validates those required terms after your run"));
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
fn github_issue_bugfix_declares_validation_expectations() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::GithubIssueBugfix, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::GithubIssueBugfix);

    assert!(prompt.contains("only finalize after the post-patch run passes"));
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/github-issue-bugfix/issue.md"
    );
    assert_eq!(
        calls[3]["path"],
        ".spark-scenarios/github-issue-bugfix/src/quote.ts"
    );
    assert_eq!(
        calls[3]["tools"],
        json!(["fs.edit", "fs.replace", "fs.write"])
    );
    assert_eq!(calls[4]["tool"], "cmd.exec");
    assert_eq!(calls[4]["command"], "bun test");
}

#[test]
fn bugfix_scenarios_allow_optional_final_source_verification() {
    let cases = [
        (
            ProfileScenarioKind::GithubIssueBugfix,
            ".spark-scenarios/github-issue-bugfix/src/quote.ts",
        ),
        (
            ProfileScenarioKind::RustFailingTestBugfix,
            ".spark-scenarios/rust-failing-test-bugfix/src/lib.rs",
        ),
        (
            ProfileScenarioKind::TypeScriptReducerBugfix,
            ".spark-scenarios/typescript-reducer-bugfix/src/cart.ts",
        ),
    ];

    for (scenario, path) in cases {
        let calls = profile_scenario_optional_tool_calls(scenario);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["tool"], "fs.read");
        assert_eq!(calls[0]["path"], path);
    }
}

#[test]
fn config_migration_allows_optional_final_artifact_verification() {
    let calls = profile_scenario_optional_tool_calls(ProfileScenarioKind::ConfigMigration);

    assert_eq!(calls.len(), 5);
    assert_eq!(calls[0]["tool"], "fs.search");
    assert_eq!(calls[0]["path"], ".spark-scenarios/config-migration");
    assert_eq!(calls[1]["tool"], "fs.search");
    assert_eq!(calls[1]["path"], ".spark-scenarios/config-migration");
    assert_eq!(calls[2]["tool"], "fs.read");
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/config-migration/config/app.json"
    );
    assert_eq!(
        calls[3]["path"],
        ".spark-scenarios/config-migration/src/config.ts"
    );
    assert_eq!(
        calls[4]["path"],
        ".spark-scenarios/config-migration/docs/config.md"
    );
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
    assert!(prompt.contains("only finalize after the post-patch run passes"));
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
    assert!(prompt.contains("only finalize after the post-patch run passes"));
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
fn merge_conflict_resolution_declares_conflict_and_validation_expectations() {
    let prompts = profile_scenario_prompts(ProfileScenarioKind::MergeConflictResolution, 45_000)
        .expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let benchmark_prompt = benchmark_task_prompt(ProfileScenarioKind::MergeConflictResolution);
    let validation =
        profile_scenario_validation_command(ProfileScenarioKind::MergeConflictResolution)
            .expect("validation");
    let groups =
        profile_scenario_expected_tool_groups(ProfileScenarioKind::MergeConflictResolution);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::MergeConflictResolution);
    let command = validation.args.join(" ");

    assert!(prompt.contains("Profile scenario: merge-conflict-resolution"));
    assert!(prompt.contains("conflict markers"));
    assert!(prompt.contains("dashboard-v2 and data-residency"));
    assert!(benchmark_prompt.contains("Preserve dashboard-v2"));
    assert!(benchmark_prompt.contains("Run bun test"));
    assert_eq!(
        validation.workdir,
        ".spark-scenarios/merge-conflict-resolution"
    );
    assert_eq!(validation.program, "powershell");
    assert!(command.contains("unresolved conflict marker"));
    assert!(command.contains("dashboard-v2"));
    assert!(command.contains("data-residency"));
    assert!(command.contains("bun test"));
    assert_eq!(
        groups,
        vec![
            vec!["fs.read"],
            vec!["fs.edit", "fs.replace"],
            vec!["cmd.exec"],
            vec!["fs.search", "fs.read"]
        ]
    );
    assert_eq!(calls.len(), 6);
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/merge-conflict-resolution/issue.md"
    );
    assert_eq!(
        calls[1]["path"],
        ".spark-scenarios/merge-conflict-resolution/src/featureFlags.ts"
    );
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/merge-conflict-resolution/tests/featureFlags.test.ts"
    );
    assert_eq!(
        calls[3]["path"],
        ".spark-scenarios/merge-conflict-resolution/src/featureFlags.ts"
    );
    assert_eq!(
        calls[3]["tools"],
        json!(["fs.edit", "fs.replace", "fs.write"])
    );
    assert_eq!(calls[4]["tool"], "cmd.exec");
    assert_eq!(calls[4]["command"], "bun test");
    assert_eq!(
        calls[5]["path"],
        ".spark-scenarios/merge-conflict-resolution/src/featureFlags.ts"
    );
}

#[test]
#[cfg(windows)]
fn merge_conflict_resolution_validation_accepts_resolved_both_sides() {
    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::MergeConflictResolution)
        .expect("prepare merge conflict");
    let root = dir
        .path()
        .join(".spark-scenarios/merge-conflict-resolution");
    std::fs::write(
        root.join("src").join("featureFlags.ts"),
        "export type Account = {\n  plan: 'free' | 'team' | 'enterprise';\n  tenant: string;\n  region: 'us' | 'eu';\n};\n\nexport function enabledFlags(account: Account): string[] {\n  const flags = ['core'];\n  if (account.plan === 'enterprise' || account.tenant.startsWith('beta-')) {\n    flags.push('dashboard-v2');\n  }\n  if (account.region === 'eu') {\n    flags.push('data-residency');\n  }\n  return flags;\n}\n",
    )
    .expect("write resolved source");
    let validation =
        profile_scenario_validation_command(ProfileScenarioKind::MergeConflictResolution)
            .expect("validation");

    let good = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        good.status.success(),
        "expected resolved merge conflict to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&good.stdout),
        String::from_utf8_lossy(&good.stderr)
    );

    std::fs::write(
        root.join("src").join("featureFlags.ts"),
        "export type Account = {\n  plan: 'free' | 'team' | 'enterprise';\n  tenant: string;\n  region: 'us' | 'eu';\n};\n\nexport function enabledFlags(account: Account): string[] {\n  const flags = ['core'];\n  if (account.plan === 'enterprise' || account.tenant.startsWith('beta-')) {\n    flags.push('dashboard-v2');\n  }\n  return flags;\n}\n",
    )
    .expect("write incomplete source");
    let bad = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        !bad.status.success(),
        "expected missing incoming branch to fail"
    );
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
    assert!(prompt.contains("Do not keep the old key names in rewritten docs or code"));
    assert!(prompt.contains("make an actual cmd.exec or fs.search tool call"));
    assert!(prompt.contains("Do not replace this tool call with a prose claim"));
    assert!(prompt.contains("verify stale authMode/retry.retries references are gone"));
    assert!(prompt.contains("rerun validation after the final edit before answering"));
    assert!(prompt.contains("use paths like config/app.json, src/config.ts, and docs/config.md"));
    assert!(prompt.contains("do not combine these terms into one -SimpleMatch alternation"));
    assert!(
        benchmark_task_prompt(ProfileScenarioKind::ConfigMigration)
            .contains("Do not keep the old key names in rewritten docs or code")
    );
    assert!(
        benchmark_task_prompt(ProfileScenarioKind::ConfigMigration)
            .contains("make an actual cmd.exec or fs.search tool call")
    );
    assert!(
        benchmark_task_prompt(ProfileScenarioKind::ConfigMigration)
            .contains("rerun validation after the final edit before answering")
    );
    assert!(
        benchmark_task_prompt(ProfileScenarioKind::ConfigMigration)
            .contains("do not combine these terms into one -SimpleMatch alternation")
    );
    assert!(
        benchmark_task_prompt(ProfileScenarioKind::ConfigMigration)
            .contains("use paths like config/app.json, src/config.ts, and docs/config.md")
    );
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
            ProfileScenarioKind::MergeConflictResolution,
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
            .contains(&ProfileScenarioKind::MergeConflictResolution)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::TechnicalEssay)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::CiFailureTriage)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::PullRequestReview)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::DependencyUpgradeTriage)
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
            ProfileScenarioKind::MergeConflictResolution,
            ".spark-scenarios/merge-conflict-resolution/src/featureFlags.ts",
            "<<<<<<< HEAD",
        ),
        (
            ProfileScenarioKind::GithubIssueTriage,
            ".spark-scenarios/github-issue-triage/src/cachePolicy.ts",
            "stale-while-revalidate=30",
        ),
        (
            ProfileScenarioKind::CiFailureTriage,
            ".spark-scenarios/ci-failure-triage/logs/frontend-tests.log",
            "Expected: 80",
        ),
        (
            ProfileScenarioKind::PullRequestReview,
            ".spark-scenarios/pull-request-review/src/checkout.ts",
            "includes('admin')",
        ),
        (
            ProfileScenarioKind::DependencyUpgradeTriage,
            ".spark-scenarios/dependency-upgrade-triage/docs/time-utils-2.0.md",
            "parseBusinessDate(input, { zone: 'utc' })",
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
            let ops_brief =
                std::fs::read_to_string(dir.path().join(".spark-scenarios/ops-report/brief.md"))
                    .expect("ops brief");
            assert!(
                content.contains("Treat the first CSV line as the header, not a ticket")
                    || ops_brief.contains("Treat the first CSV line as the header, not a ticket"),
                "ops-report fixture should make header handling explicit"
            );
            assert!(
                ops_brief.contains("Do not count P2 tickets as P1 tickets"),
                "ops-report fixture should make risk ranking deterministic"
            );
        }
        assert!(
            profile_scenario_validation_command(scenario).is_some(),
            "{scenario:?} should have deterministic validation"
        );
    }
}

#[test]
fn ci_failure_triage_declares_log_source_and_validation_expectations() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::CiFailureTriage, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let benchmark_prompt = benchmark_task_prompt(ProfileScenarioKind::CiFailureTriage);
    let validation = profile_scenario_validation_command(ProfileScenarioKind::CiFailureTriage)
        .expect("validation");
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::CiFailureTriage);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::CiFailureTriage);
    let command = validation.args.join(" ");

    assert!(prompt.contains("Profile scenario: ci-failure-triage"));
    assert!(prompt.contains("logs/frontend-tests.log"));
    assert!(prompt.contains("do not modify source files"));
    assert!(benchmark_prompt.contains("SAVE20 path in applyDiscount"));
    assert!(benchmark_prompt.contains("Expected 80 / Received 100 evidence"));
    assert_eq!(validation.workdir, ".spark-scenarios/ci-failure-triage");
    assert_eq!(validation.program, "powershell");
    assert!(command.contains("bun test"));
    assert!(command.contains("\\bExpected\\b[^\\r\\n]*\\b80\\b"));
    assert!(command.contains("\\bReceived\\b[^\\r\\n]*\\b100\\b"));
    assert_eq!(groups, vec![vec!["fs.read"], vec!["fs.write"]]);
    assert_eq!(calls.len(), 6);
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/ci-failure-triage/issue.md"
    );
    assert_eq!(
        calls[1]["path"],
        ".spark-scenarios/ci-failure-triage/.github/workflows/frontend.yml"
    );
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/ci-failure-triage/logs/frontend-tests.log"
    );
    assert_eq!(
        calls[5]["path"],
        ".spark-scenarios/ci-failure-triage/ci-triage.md"
    );
}

#[test]
#[cfg(windows)]
fn ci_failure_triage_validation_accepts_grounded_report() {
    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::CiFailureTriage)
        .expect("prepare ci triage");
    let root = dir.path().join(".spark-scenarios/ci-failure-triage");
    std::fs::write(
        root.join("ci-triage.md"),
        "# CI Triage\n\nThe failing command is `bun test` from `.github/workflows/frontend.yml`.\n\nThe failure is in `tests/discount.test.ts`: SAVE20 expects **Expected: 80** but got **Received: 100**. The likely root cause is that `src/discount.ts` `applyDiscount` handles SAVE10 but not SAVE20. Minimal fix plan: add the SAVE20 branch and rerun bun test.\n",
    )
    .expect("write triage");
    let validation = profile_scenario_validation_command(ProfileScenarioKind::CiFailureTriage)
        .expect("validation");

    let good = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        good.status.success(),
        "expected grounded CI triage to pass: {}",
        String::from_utf8_lossy(&good.stderr)
    );

    std::fs::write(
        root.join("ci-triage.md"),
        "# CI Triage\n\n## Failing command\n`bun test` (from `.github/workflows/frontend.yml`)\n\n## Failing test / assertion\n`tests/discount.test.ts` -> `applies SAVE20 campaign to checkout totals`:\n- Expected: `applyDiscount(100, 'SAVE20')` to be `80`\n- Received: `100`\n\n## Likely root cause\n`src/discount.ts` only handles `SAVE10`; the `SAVE20` campaign path is missing in `applyDiscount`.\n",
    )
    .expect("write report-style triage");
    let report_style = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        report_style.status.success(),
        "expected report-style assertion evidence to pass: {}",
        String::from_utf8_lossy(&report_style.stderr)
    );

    std::fs::write(
        root.join("ci-triage.md"),
        "# CI Triage\n\n- **Failing command:** `bun test`\n- **Failing assertion:** `tests/discount.test.ts` applies SAVE20 campaign to checkout totals: Expected `80`, Received `100`.\n- **Likely root cause:** `applyDiscount` in `src/discount.ts` only handles SAVE10, so the SAVE20 path falls through unchanged.\n",
    )
    .expect("write markdown assertion triage");
    let markdown_assertion = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        markdown_assertion.status.success(),
        "expected markdown assertion evidence to pass: {}",
        String::from_utf8_lossy(&markdown_assertion.stderr)
    );

    std::fs::write(
        root.join("ci-triage.md"),
        "# CI Triage\n\nThe job failed in tests, but the cause is unclear.\n",
    )
    .expect("write incomplete triage");
    let bad = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(!bad.status.success(), "expected incomplete triage to fail");
}

#[test]
fn pull_request_review_declares_diff_source_and_validation_expectations() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::PullRequestReview, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let benchmark_prompt = benchmark_task_prompt(ProfileScenarioKind::PullRequestReview);
    let validation = profile_scenario_validation_command(ProfileScenarioKind::PullRequestReview)
        .expect("validation");
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::PullRequestReview);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::PullRequestReview);
    let command = validation.args.join(" ");

    assert!(prompt.contains("Profile scenario: pull-request-review"));
    assert!(prompt.contains("diff.patch"));
    assert!(prompt.contains("do not modify source files"));
    assert!(benchmark_prompt.contains("role.includes('admin')"));
    assert!(benchmark_prompt.contains("read-only-admin users"));
    assert_eq!(validation.workdir, ".spark-scenarios/pull-request-review");
    assert_eq!(validation.program, "powershell");
    assert!(command.contains("read-only-admin"));
    assert!(command.contains("includes\\s*\\("));
    assert!(command.contains("strict equality"));
    assert_eq!(groups, vec![vec!["fs.read"], vec!["fs.write"]]);
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/pull-request-review/pr.md"
    );
    assert_eq!(
        calls[1]["path"],
        ".spark-scenarios/pull-request-review/diff.patch"
    );
    assert_eq!(
        calls[2]["path"],
        ".spark-scenarios/pull-request-review/src/checkout.ts"
    );
    assert_eq!(
        calls[3]["path"],
        ".spark-scenarios/pull-request-review/tests/checkout.test.ts"
    );
    assert_eq!(
        calls[4]["path"],
        ".spark-scenarios/pull-request-review/review.md"
    );
}

#[test]
#[cfg(windows)]
fn pull_request_review_validation_accepts_blocking_finding() {
    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::PullRequestReview)
        .expect("prepare pr review");
    let root = dir.path().join(".spark-scenarios/pull-request-review");
    std::fs::write(
        root.join("review.md"),
        "# Review\n\n**Blocking**: `src/checkout.ts` changes `discountFor` to use `role.includes('admin')`, so a `read-only-admin` user receives the full cart comp even though the PR rule says only a role exactly admin can discount. The missing coverage is in `tests/checkout.test.ts`. Add a read-only-admin regression test and change the check to strict equality against 'admin'.\n",
    )
    .expect("write review");
    let validation = profile_scenario_validation_command(ProfileScenarioKind::PullRequestReview)
        .expect("validation");

    let good = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        good.status.success(),
        "expected grounded PR review to pass: {}",
        String::from_utf8_lossy(&good.stderr)
    );

    std::fs::write(
        root.join("review.md"),
        "# Review\n\nThe PR needs more tests before merging.\n",
    )
    .expect("write incomplete review");
    let bad = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(!bad.status.success(), "expected incomplete review to fail");
}

#[test]
fn dependency_upgrade_triage_declares_migration_source_and_validation_expectations() {
    let prompts = profile_scenario_prompts(ProfileScenarioKind::DependencyUpgradeTriage, 45_000)
        .expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let benchmark_prompt = benchmark_task_prompt(ProfileScenarioKind::DependencyUpgradeTriage);
    let validation =
        profile_scenario_validation_command(ProfileScenarioKind::DependencyUpgradeTriage)
            .expect("validation");
    let groups =
        profile_scenario_expected_tool_groups(ProfileScenarioKind::DependencyUpgradeTriage);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::DependencyUpgradeTriage);
    let command = validation.args.join(" ");

    assert!(prompt.contains("Profile scenario: dependency-upgrade-triage"));
    assert!(prompt.contains("docs/time-utils-2.0.md"));
    assert!(prompt.contains("do not modify source files"));
    assert!(benchmark_prompt.contains("@acme/time-utils 2.0.0"));
    assert!(benchmark_prompt.contains("date-only defaults from UTC to local time"));
    assert_eq!(
        validation.workdir,
        ".spark-scenarios/dependency-upgrade-triage"
    );
    assert_eq!(validation.program, "powershell");
    assert!(command.contains("@acme/time-utils"));
    assert!(command.contains("zone\\s*:\\s*"));
    assert!(command.contains("missing test gap recommendation"));
    assert_eq!(groups, vec![vec!["fs.read"], vec!["fs.write"]]);
    assert_eq!(calls.len(), 7);
    assert_eq!(
        calls[0]["path"],
        ".spark-scenarios/dependency-upgrade-triage/upgrade.md"
    );
    assert_eq!(
        calls[1]["path"],
        ".spark-scenarios/dependency-upgrade-triage/package.json"
    );
    assert_eq!(
        calls[3]["path"],
        ".spark-scenarios/dependency-upgrade-triage/docs/time-utils-2.0.md"
    );
    assert_eq!(
        calls[4]["path"],
        ".spark-scenarios/dependency-upgrade-triage/src/billingWindow.ts"
    );
    assert_eq!(
        calls[6]["path"],
        ".spark-scenarios/dependency-upgrade-triage/upgrade-triage.md"
    );
}

#[test]
#[cfg(windows)]
fn dependency_upgrade_triage_validation_accepts_grounded_report() {
    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::DependencyUpgradeTriage)
        .expect("prepare dependency upgrade");
    let root = dir
        .path()
        .join(".spark-scenarios/dependency-upgrade-triage");
    std::fs::write(
        root.join("upgrade-triage.md"),
        "# Upgrade Triage\n\n`@acme/time-utils` is upgraded to **2.0.0**. Blocking risk: `parseBusinessDate` now parses date-only strings in the local timezone instead of UTC, while billing cutoffs must stay UTC. `src/billingWindow.ts` calls `parseBusinessDate(input)` without options, so use `parseBusinessDate(input, { zone: 'utc' })`. Test gap: `tests/billingWindow.test.ts` should add a regression test around a timezone boundary.\n",
    )
    .expect("write triage");
    let validation =
        profile_scenario_validation_command(ProfileScenarioKind::DependencyUpgradeTriage)
            .expect("validation");

    let good = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        good.status.success(),
        "expected grounded upgrade triage to pass: {}",
        String::from_utf8_lossy(&good.stderr)
    );

    std::fs::write(
        root.join("upgrade-triage.md"),
        "# Upgrade Triage\n\nThe package update looks safe after reading package.json.\n",
    )
    .expect("write incomplete triage");
    let bad = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(!bad.status.success(), "expected incomplete triage to fail");
}

#[test]
fn ops_report_validation_requires_billing_as_highest_risk_team() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::OpsReport, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let benchmark_prompt = benchmark_task_prompt(ProfileScenarioKind::OpsReport);
    let validation =
        profile_scenario_validation_command(ProfileScenarioKind::OpsReport).expect("validation");
    let command = validation.args.join(" ");

    assert!(prompt.contains("do not count P2 tickets as P1 tickets"));
    assert!(benchmark_prompt.contains("do not count P2 tickets as P1 tickets"));
    assert!(command.contains("report must identify billing as highest-risk team"));
    assert!(command.contains("report incorrectly identifies api as highest-risk team"));
    assert!(command.contains("95 minute open P1 age"));
}

#[test]
#[cfg(windows)]
fn ops_report_validation_accepts_billing_colon_form_and_rejects_api() {
    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::OpsReport).expect("prepare ops");
    let root = dir.path().join(".spark-scenarios/ops-report");
    std::fs::write(
        root.join("metrics.json"),
        "{\n  \"totalTickets\": 8,\n  \"openTickets\": 5,\n  \"p1Open\": 2,\n  \"averageOpenMinutes\": 51.4\n}\n",
    )
    .expect("write metrics");
    std::fs::write(
        root.join("report.md"),
        "# Ops Report\n\n## Highest-Risk Team\n- **billing** is the highest-risk team because its open P1 is **95** minutes old.\n",
    )
    .expect("write good report");
    let validation =
        profile_scenario_validation_command(ProfileScenarioKind::OpsReport).expect("validation");

    let good = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        good.status.success(),
        "expected good report to pass: {}",
        String::from_utf8_lossy(&good.stderr)
    );

    std::fs::write(
        root.join("report.md"),
        "# Ops Report\n\n## Highest-risk team\nThe **billing** team is highest risk because its open P1 is **95** minutes old.\n",
    )
    .expect("write natural report");
    let natural = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        natural.status.success(),
        "expected natural report to pass: {}",
        String::from_utf8_lossy(&natural.stderr)
    );

    std::fs::write(
        root.join("report.md"),
        "# Ops Report\n\n## Highest-risk team\n- **Team:** `billing`\n- **Why:** billing is tied on open P1 count but has the older open P1 at **95** minutes.\n",
    )
    .expect("write heading field report");
    let heading_field = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        heading_field.status.success(),
        "expected heading plus Team field report to pass: {}",
        String::from_utf8_lossy(&heading_field.stderr)
    );

    std::fs::write(
        root.join("report.md"),
        "# Ops Report\n\n## Highest-Risk Team\n- **api** is the highest-risk team. billing has a 95 minute open P1.\n",
    )
    .expect("write bad report");
    let bad = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(!bad.status.success(), "expected api report to fail");
    assert!(
        String::from_utf8_lossy(&bad.stderr).contains("report must identify billing")
            || String::from_utf8_lossy(&bad.stderr).contains("incorrectly identifies api"),
        "unexpected validation error: {}",
        String::from_utf8_lossy(&bad.stderr)
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
