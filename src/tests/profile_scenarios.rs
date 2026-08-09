use crate::benchmark::expected_scenario_artifacts;
use crate::cli::{ProfileBenchmarkSuiteKind, ProfileScenarioKind};
use crate::profile::scenarios::{
    benchmark_profile_prompts, benchmark_task_prompt, codex_cli_benchmark_prompt,
    prepare_benchmark_scenario, prepare_profile_scenario, profile_scenario_expected_skills,
    profile_scenario_expected_tool_calls, profile_scenario_expected_tool_groups,
    profile_scenario_optional_tool_calls, profile_scenario_prompts,
    profile_scenario_validation_checks, profile_scenario_validation_command,
};
use crate::{APPROX_CHARS_PER_TOKEN, DEFAULT_COMPACT_AFTER_CHARS};
use serde_json::json;

fn expected_powershell_program() -> &'static str {
    if cfg!(windows) { "powershell" } else { "pwsh" }
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
    assert!(!prompt.contains("Work only under"));
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
    assert!(!prompt.contains("Work only under"));
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
    assert_eq!(calls[0]["path"], "drafts/report-draft.md");
    assert_eq!(calls[1]["tool"], "fs.rename");
    assert_eq!(calls[1]["from"], "drafts/report-draft.md");
    assert_eq!(calls[1]["to"], "final/report.md");
    assert_eq!(calls[2]["tool"], "fs.stat");
    assert_eq!(calls[2]["path"], "final/report.md");
    assert_eq!(calls[3]["tool"], "fs.read");
    assert_eq!(calls[3]["path"], "final/report.md");
    assert_eq!(calls[4]["tool"], "fs.search");
    assert_eq!(calls[4]["path"], ".");
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
    assert_eq!(calls[0]["path"], "source/missing-note.md");
    assert_eq!(calls[0]["ok"], false);
    assert_eq!(calls[1]["tool"], "fs.read");
    assert_eq!(calls[1]["path"], "source/note.md");
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
    assert_eq!(calls[2]["path"], "summary.txt");
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
    assert_eq!(calls[0]["path"], "tests/status_map.spec.md");
    assert_eq!(calls[1]["path"], "src/status_map.ts");
    assert_eq!(calls[2]["path"], "src/status_map.ts");
    assert_eq!(calls[2]["tools"], json!(["fs.edit", "fs.replace"]));
    assert_eq!(calls[3]["path"], "src");
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
    assert_eq!(calls[0]["path"], "src/routes.ts");
    assert_eq!(calls[1]["path"], "src/navigation.ts");
    assert_eq!(calls[2]["path"], "docs/routes.md");
    assert_eq!(calls[3]["path"], "src/routes.ts");
    assert_eq!(
        calls[3]["tools"],
        json!(["fs.edit", "fs.replace", "fs.write"])
    );
    assert_eq!(calls[4]["path"], "src/navigation.ts");
    assert_eq!(calls[5]["path"], "docs/routes.md");
    assert_eq!(calls[6]["path"], ".");
    assert_eq!(calls[7]["path"], ".");
    assert!(profile_scenario_validation_command(ProfileScenarioKind::MultiFilePatch).is_some());
}

#[test]
fn multi_file_patch_allows_optional_final_artifact_verification() {
    let calls = profile_scenario_optional_tool_calls(ProfileScenarioKind::MultiFilePatch);

    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0]["path"], "src/routes.ts");
    assert_eq!(calls[1]["path"], "src/navigation.ts");
    assert_eq!(calls[2]["path"], "docs/routes.md");
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
fn write_and_scoped_patch_scenarios_have_executable_contracts() {
    let write_prompt = profile_scenario_prompts(ProfileScenarioKind::ManifestContractWrite, 45_000)
        .expect("write scenario");
    assert!(write_prompt[0].contains("two exact, mutually consistent release artifacts"));
    assert!(write_prompt[0].contains("do not call cmd.exec"));
    assert_eq!(
        profile_scenario_expected_tool_calls(ProfileScenarioKind::ManifestContractWrite).len(),
        6
    );

    let patch_prompt = profile_scenario_prompts(ProfileScenarioKind::ScopedPolicyPatch, 45_000)
        .expect("patch scenario");
    assert!(patch_prompt[0].contains("lookalike safety branch"));
    assert!(patch_prompt[0].contains("do not call cmd.exec"));
    assert!(profile_scenario_validation_command(ProfileScenarioKind::ScopedPolicyPatch).is_some());

    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::ManifestContractWrite)
        .expect("prepare write fixture");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::ScopedPolicyPatch)
        .expect("prepare patch fixture");
    assert!(
        dir.path()
            .join(".spark-scenarios/manifest-contract-write/data/releases.json")
            .is_file()
    );
    let source = std::fs::read_to_string(
        dir.path()
            .join(".spark-scenarios/scoped-policy-patch/src/rate_limit.ts"),
    )
    .expect("read patch fixture");
    assert!(source.contains("isRetryLimitExceeded"));
}

#[test]
fn benchmark_scenarios_use_the_passed_cwd_as_the_fixture_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    prepare_benchmark_scenario(dir.path(), ProfileScenarioKind::PrecisePatch)
        .expect("prepare benchmark fixture");

    assert!(dir.path().join("src/status_map.ts").is_file());
    assert!(dir.path().join("tests/status_map.spec.md").is_file());
    assert!(!dir.path().join(".spark-scenarios").exists());

    let prompt = profile_scenario_prompts(ProfileScenarioKind::PrecisePatch, 45_000)
        .expect("scenario prompt");
    assert!(prompt[0].contains("Read src/status_map.ts"));
    assert!(!prompt[0].contains("Work only under"));
    assert!(!prompt[0].contains(".spark-scenarios"));
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
    assert_eq!(calls[0]["path"], "src/profile/scenarios/prompts.rs");
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
    assert_eq!(calls[0]["path"], "brief.md");
    assert_eq!(calls[1]["path"], "package.json");
    assert_eq!(calls[2]["path"], "index.html");
    assert_eq!(calls[3]["path"], "src/main.tsx");
    assert_eq!(calls[4]["path"], "src/App.tsx");
    assert_eq!(calls[5]["path"], "src/App.test.tsx");
    assert_eq!(calls[6]["path"], "src/styles.css");
    assert_eq!(calls[7]["tool"], "cmd.exec");
    assert_eq!(calls[7]["command"], "bun test");
}

#[test]
fn codex_cli_prompt_uses_cli_neutral_actions_for_scaffolding() {
    let prompt = codex_cli_benchmark_prompt(ProfileScenarioKind::ReactCalculatorScaffold);

    assert!(prompt.contains("Benchmark scenario: react-calculator-scaffold"));
    assert!(prompt.contains("Use bun for JavaScript package management"));
    assert!(prompt.contains("Create index.html"));
    assert!(prompt.contains("Create src/App.tsx"));
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
    assert_eq!(validation.workdir, ".");
    assert_eq!(validation.program, expected_powershell_program());
    assert!(validation.args.join(" ").contains("cargo test"));
    assert!(validation.args.join(" ").contains("cargo run"));
    assert!(validation.args.join(" ").contains("Top error code"));
    assert!(validation.args.join(" ").contains("E42"));
    assert_eq!(
        groups,
        vec![vec!["fs.read"], vec!["fs.write"], vec!["cmd.exec"]]
    );
    assert_eq!(calls.len(), 6);
    assert_eq!(calls[0]["path"], "brief.md");
    assert_eq!(calls[1]["path"], "sample.log");
    assert_eq!(calls[2]["path"], "Cargo.toml");
    assert_eq!(calls[3]["path"], "src/lib.rs");
    assert_eq!(calls[4]["path"], "src/main.rs");
    assert_eq!(calls[5]["tool"], "cmd.exec");
    assert_eq!(calls[5]["command"], "cargo test");
}

#[test]
fn benchmark_bugfix_prompts_use_exact_files_without_listing() {
    let cases = [
        (
            ProfileScenarioKind::GithubIssueBugfix,
            "src/quote.ts",
            "tests/quote.test.ts",
        ),
        (
            ProfileScenarioKind::RustFailingTestBugfix,
            "src/lib.rs",
            "tests/retry_scheduler.rs",
        ),
        (
            ProfileScenarioKind::TypeScriptReducerBugfix,
            "src/cart.ts",
            "tests/cart.test.ts",
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
    assert_eq!(validation.workdir, ".");
    assert_eq!(validation.program, expected_powershell_program());
    assert_eq!(
        validation.args,
        &["-NoProfile", "-File", "validate-notes.ps1"]
    );
    assert_eq!(
        groups,
        vec![vec!["fs.read"], vec!["fs.write"], vec!["cmd.exec"]]
    );
    assert_eq!(calls.len(), 5);
    assert_eq!(calls[0]["path"], "brief.md");
    assert_eq!(calls[1]["path"], "Cargo.toml");
    assert_eq!(calls[2]["path"], "src/lib.rs");
    assert_eq!(calls[3]["path"], "src/main.rs");
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
    assert_eq!(calls[0]["path"], "issue.md");
    assert_eq!(calls[3]["path"], "src/quote.ts");
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
        (ProfileScenarioKind::GithubIssueBugfix, "src/quote.ts"),
        (ProfileScenarioKind::RustFailingTestBugfix, "src/lib.rs"),
        (ProfileScenarioKind::TypeScriptReducerBugfix, "src/cart.ts"),
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
    assert_eq!(calls[0]["path"], ".");
    assert_eq!(calls[1]["tool"], "fs.search");
    assert_eq!(calls[1]["path"], ".");
    assert_eq!(calls[2]["tool"], "fs.read");
    assert_eq!(calls[2]["path"], "config/app.json");
    assert_eq!(calls[3]["path"], "src/config.ts");
    assert_eq!(calls[4]["path"], "docs/config.md");
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
    assert_eq!(validation.workdir, ".");
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
    assert_eq!(calls[0]["path"], "issue.md");
    assert_eq!(calls[1]["path"], "src/lib.rs");
    assert_eq!(calls[2]["path"], "tests/retry_scheduler.rs");
    assert_eq!(calls[3]["path"], "src/lib.rs");
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
    assert_eq!(validation.workdir, ".");
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
    assert_eq!(calls[0]["path"], "issue.md");
    assert_eq!(calls[1]["path"], "src/cart.ts");
    assert_eq!(calls[2]["path"], "tests/cart.test.ts");
    assert_eq!(calls[3]["path"], "src/cart.ts");
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
    assert_eq!(validation.workdir, ".");
    assert_eq!(validation.program, expected_powershell_program());
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
    assert_eq!(calls[0]["path"], "issue.md");
    assert_eq!(calls[1]["path"], "src/featureFlags.ts");
    assert_eq!(calls[2]["path"], "tests/featureFlags.test.ts");
    assert_eq!(calls[3]["path"], "src/featureFlags.ts");
    assert_eq!(
        calls[3]["tools"],
        json!(["fs.edit", "fs.replace", "fs.write"])
    );
    assert_eq!(calls[4]["tool"], "cmd.exec");
    assert_eq!(calls[4]["command"], "bun test");
    assert_eq!(calls[5]["path"], "src/featureFlags.ts");
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
    assert_eq!(validation.program, expected_powershell_program());
    assert_eq!(
        groups,
        vec![
            vec!["fs.read"],
            vec!["fs.edit", "fs.replace", "fs.write"],
            vec!["cmd.exec", "fs.search"]
        ]
    );
    assert_eq!(calls.len(), 8);
    assert_eq!(calls[0]["path"], "migration.md");
    assert_eq!(calls[4]["path"], "config/app.json");
    assert_eq!(
        calls[4]["tools"],
        json!(["fs.edit", "fs.replace", "fs.write"])
    );
    assert_eq!(calls[5]["path"], "src/config.ts");
    assert_eq!(calls[6]["path"], "docs/config.md");
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
    assert_eq!(calls[0]["path"], "brief.md");
    assert_eq!(calls[1]["path"], "essay.md");
    assert_eq!(calls[2]["path"], "essay.md");
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
            ProfileScenarioKind::ManifestContractWrite,
            ProfileScenarioKind::ScopedPolicyPatch,
            ProfileScenarioKind::GithubIssueBugfix,
            ProfileScenarioKind::RustFailingTestBugfix,
            ProfileScenarioKind::TypeScriptReducerBugfix,
            ProfileScenarioKind::MergeConflictResolution,
            ProfileScenarioKind::ConfigMigration,
            ProfileScenarioKind::MultiModuleBugfix,
            ProfileScenarioKind::StatefulReconciliationBugfix,
            ProfileScenarioKind::FeatureRolloutConsistencyBugfix,
        ]
    );
    assert_eq!(
        ProfileBenchmarkSuiteKind::Reasoning.scenarios(),
        &[
            ProfileScenarioKind::TechnicalEssay,
            ProfileScenarioKind::ConfigMigration,
            ProfileScenarioKind::OpsReport,
            ProfileScenarioKind::InventoryRebalancePlan,
            ProfileScenarioKind::ExperimentRolloutAudit,
            ProfileScenarioKind::MultiModuleBugfix,
            ProfileScenarioKind::TerminalRepair,
            ProfileScenarioKind::MultiHopAnalysis,
            ProfileScenarioKind::PolicySupportAgent,
            ProfileScenarioKind::RustNotesTuiScaffold,
            ProfileScenarioKind::StatefulReconciliationBugfix,
            ProfileScenarioKind::FeatureRolloutConsistencyBugfix,
            ProfileScenarioKind::FrontierRuleTransfer,
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
        ProfileBenchmarkSuiteKind::Quantitative
            .scenarios()
            .contains(&ProfileScenarioKind::InventoryRebalancePlan)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::InventoryRebalancePlan)
    );
    assert!(
        ProfileBenchmarkSuiteKind::Quantitative
            .scenarios()
            .contains(&ProfileScenarioKind::ExperimentRolloutAudit)
    );
    assert!(
        ProfileBenchmarkSuiteKind::Analysis
            .scenarios()
            .contains(&ProfileScenarioKind::ExperimentRolloutAudit)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::ExperimentRolloutAudit)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::MultiModuleBugfix)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::StatefulReconciliationBugfix)
    );
    assert_eq!(
        ProfileBenchmarkSuiteKind::Frontier.scenarios(),
        &[
            ProfileScenarioKind::FrontierRuleTransfer,
            ProfileScenarioKind::FeatureRolloutConsistencyBugfix,
        ]
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::FrontierRuleTransfer)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::TerminalRepair)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::MultiHopAnalysis)
    );
    assert!(
        ProfileBenchmarkSuiteKind::RealWorld
            .scenarios()
            .contains(&ProfileScenarioKind::PolicySupportAgent)
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
            "issue.md",
            "annual quotes are undercharged",
        ),
        (
            ProfileScenarioKind::RustFailingTestBugfix,
            "tests/retry_scheduler.rs",
            "returns_highest_priority_jobs_first",
        ),
        (
            ProfileScenarioKind::TypeScriptReducerBugfix,
            "tests/cart.test.ts",
            "subtotal ignores inactive restored lines",
        ),
        (
            ProfileScenarioKind::MergeConflictResolution,
            "src/featureFlags.ts",
            "<<<<<<< HEAD",
        ),
        (
            ProfileScenarioKind::GithubIssueTriage,
            "src/cachePolicy.ts",
            "stale-while-revalidate=30",
        ),
        (
            ProfileScenarioKind::CiFailureTriage,
            "logs/frontend-tests.log",
            "Expected: 80",
        ),
        (
            ProfileScenarioKind::PullRequestReview,
            "src/checkout.ts",
            "includes('admin')",
        ),
        (
            ProfileScenarioKind::DependencyUpgradeTriage,
            "docs/time-utils-2.0.md",
            "parseBusinessDate(input, { zone: 'utc' })",
        ),
        (
            ProfileScenarioKind::TechnicalEssay,
            "brief.md",
            "Operational Visibility Is a Product Feature",
        ),
        (
            ProfileScenarioKind::ConfigMigration,
            "migration.md",
            "schema version 2",
        ),
        (
            ProfileScenarioKind::OpsReport,
            "data/tickets.csv",
            "billing,P1,open,95",
        ),
        (
            ProfileScenarioKind::InventoryRebalancePlan,
            "data/transfer_options.csv",
            "T14,Atlas,NORTH,EAST,10,1,15,5",
        ),
        (
            ProfileScenarioKind::ExperimentRolloutAudit,
            "data/events.csv",
            "ET06,T06,checkout,2026-07-04T00:00:00Z",
        ),
        (
            ProfileScenarioKind::MultiModuleBugfix,
            "tests/invoice.test.ts",
            "applies discount before tax",
        ),
        (
            ProfileScenarioKind::TerminalRepair,
            "config/settings.json",
            "data/summary.json",
        ),
        (
            ProfileScenarioKind::MultiHopAnalysis,
            "data/orders.csv",
            "A6,Atlas,EMEA,1,80.00,returned",
        ),
        (
            ProfileScenarioKind::PolicySupportAgent,
            "policy.md",
            "Damaged-on-arrival",
        ),
    ];

    for (scenario, path, expected) in scenarios {
        let dir = tempfile::tempdir().expect("tempdir");
        prepare_benchmark_scenario(dir.path(), scenario).expect("prepare scenario");
        let content = std::fs::read_to_string(dir.path().join(path)).expect("fixture content");
        assert!(
            content.contains(expected),
            "{scenario:?} fixture missing {expected}"
        );
        if scenario == ProfileScenarioKind::OpsReport {
            let ops_brief =
                std::fs::read_to_string(dir.path().join("brief.md")).expect("ops brief");
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
    assert_eq!(validation.workdir, ".");
    assert_eq!(validation.program, expected_powershell_program());
    assert!(command.contains("bun test"));
    assert!(command.contains("\\bExpected\\b[^\\r\\n]*\\b80\\b"));
    assert!(command.contains("\\bReceived\\b[^\\r\\n]*\\b100\\b"));
    assert_eq!(groups, vec![vec!["fs.read"], vec!["fs.write"]]);
    assert_eq!(calls.len(), 6);
    assert_eq!(calls[0]["path"], "issue.md");
    assert_eq!(calls[1]["path"], ".github/workflows/frontend.yml");
    assert_eq!(calls[2]["path"], "logs/frontend-tests.log");
    assert_eq!(calls[5]["path"], "ci-triage.md");
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
    assert_eq!(validation.workdir, ".");
    assert_eq!(validation.program, expected_powershell_program());
    assert!(command.contains("read-only-admin"));
    assert!(command.contains("includes\\s*\\("));
    assert!(command.contains("strict equality"));
    assert_eq!(groups, vec![vec!["fs.read"], vec!["fs.write"]]);
    assert_eq!(calls.len(), 5);
    assert_eq!(calls[0]["path"], "pr.md");
    assert_eq!(calls[1]["path"], "diff.patch");
    assert_eq!(calls[2]["path"], "src/checkout.ts");
    assert_eq!(calls[3]["path"], "tests/checkout.test.ts");
    assert_eq!(calls[4]["path"], "review.md");
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
    assert_eq!(validation.workdir, ".");
    assert_eq!(validation.program, expected_powershell_program());
    assert!(command.contains("@acme/time-utils"));
    assert!(command.contains("zone\\s*:\\s*"));
    assert!(command.contains("missing test gap recommendation"));
    assert_eq!(groups, vec![vec!["fs.read"], vec!["fs.write"]]);
    assert_eq!(calls.len(), 7);
    assert_eq!(calls[0]["path"], "upgrade.md");
    assert_eq!(calls[1]["path"], "package.json");
    assert_eq!(calls[3]["path"], "docs/time-utils-2.0.md");
    assert_eq!(calls[4]["path"], "src/billingWindow.ts");
    assert_eq!(calls[6]["path"], "upgrade-triage.md");
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
fn multi_module_bugfix_declares_cross_module_expectations() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::MultiModuleBugfix, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let validation = profile_scenario_validation_command(ProfileScenarioKind::MultiModuleBugfix)
        .expect("validation");
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::MultiModuleBugfix);
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::MultiModuleBugfix);
    let optional = profile_scenario_optional_tool_calls(ProfileScenarioKind::MultiModuleBugfix);

    assert!(prompt.contains("Profile scenario: multi-module-bugfix"));
    assert!(prompt.contains("src/invoice.ts"));
    assert!(prompt.contains("src/total.ts"));
    assert!(prompt.contains("Keep src/tax.ts unchanged"));
    assert!(
        benchmark_task_prompt(ProfileScenarioKind::MultiModuleBugfix)
            .contains("discounts apply before tax")
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
    assert_eq!(calls.len(), 7);
    assert_eq!(calls[0]["path"], "issue.md");
    assert_eq!(calls[4]["path"], "src/invoice.ts");
    assert_eq!(calls[5]["path"], "src/total.ts");
    assert_eq!(calls[6]["command"], "bun test");
    assert_eq!(optional[0]["path"], "src/tax.ts");
}

#[test]
fn multi_module_bugfix_fixture_fails_then_passes_after_fix() {
    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::MultiModuleBugfix)
        .expect("prepare multi-module bugfix");
    let root = dir.path().join(".spark-scenarios/multi-module-bugfix");
    let validation = profile_scenario_validation_command(ProfileScenarioKind::MultiModuleBugfix)
        .expect("validation");

    let failing = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        !failing.status.success(),
        "expected fixture tests to fail before the fix"
    );

    std::fs::write(
        root.join("src").join("invoice.ts"),
        "export type OrderItem = {\n  sku: string;\n  quantity: number;\n  unitPriceCents: number;\n};\n\nexport type InvoiceLine = {\n  sku: string;\n  amountCents: number;\n};\n\nexport function buildInvoiceLines(items: OrderItem[]): InvoiceLine[] {\n  return items.map((item) => ({\n    sku: item.sku,\n    amountCents: item.unitPriceCents * item.quantity,\n  }));\n}\n",
    )
    .expect("write fixed invoice.ts");
    std::fs::write(
        root.join("src").join("total.ts"),
        "import type { InvoiceLine } from './invoice';\nimport { taxCentsFor } from './tax';\n\nexport function invoiceTotalCents(\n  lines: InvoiceLine[],\n  discountCents: number,\n  taxRateBps: number,\n): number {\n  const subtotalCents = lines.reduce((sum, line) => sum + line.amountCents, 0);\n  const discountedCents = subtotalCents - discountCents;\n  return Math.round(discountedCents + taxCentsFor(discountedCents, taxRateBps));\n}\n",
    )
    .expect("write fixed total.ts");
    let fixed = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        fixed.status.success(),
        "expected fixed modules to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&fixed.stdout),
        String::from_utf8_lossy(&fixed.stderr)
    );
}

#[test]
fn terminal_repair_declares_terminal_first_expectations() {
    let prompts =
        profile_scenario_prompts(ProfileScenarioKind::TerminalRepair, 45_000).expect("scenario");
    let prompt = prompts.first().expect("prompt");
    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::TerminalRepair);
    let groups = profile_scenario_expected_tool_groups(ProfileScenarioKind::TerminalRepair);
    let validation = profile_scenario_validation_command(ProfileScenarioKind::TerminalRepair)
        .expect("validation");

    assert!(prompt.contains("Profile scenario: terminal-repair"));
    assert!(prompt.contains("bun run start"));
    assert!(prompt.contains("Do not modify src/index.js or data/report.csv"));
    assert!(
        benchmark_task_prompt(ProfileScenarioKind::TerminalRepair)
            .contains("capture the current failure")
    );
    assert_eq!(calls.len(), 4);
    assert_eq!(calls[0]["tool"], "cmd.exec");
    assert_eq!(calls[0]["command"], "bun run start");
    assert_eq!(calls[0]["ok"], false);
    assert_eq!(calls[1]["path"], "config/settings.json");
    assert_eq!(calls[3]["command"], "bun run start");
    assert!(calls[3].get("ok").is_none());
    assert_eq!(groups[0], vec!["cmd.exec"]);
    assert_eq!(validation.workdir, ".");
}

#[test]
fn terminal_repair_validation_fails_broken_then_accepts_repaired_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::TerminalRepair)
        .expect("prepare terminal repair");
    let root = dir.path().join(".spark-scenarios/terminal-repair");
    let validation = profile_scenario_validation_command(ProfileScenarioKind::TerminalRepair)
        .expect("validation");

    let broken = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        !broken.status.success(),
        "expected broken fixture to fail validation"
    );

    std::fs::write(
        root.join("config").join("settings.json"),
        "{\n  \"dataPath\": \"data/report.csv\"\n}\n",
    )
    .expect("write repaired settings.json");
    let repaired = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        repaired.status.success(),
        "expected repaired config to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&repaired.stdout),
        String::from_utf8_lossy(&repaired.stderr)
    );
}

#[test]
fn multi_hop_analysis_validation_accepts_exact_answer_and_rejects_distractors() {
    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::MultiHopAnalysis)
        .expect("prepare multi-hop analysis");
    let root = dir.path().join(".spark-scenarios/multi-hop-analysis");
    let validation = profile_scenario_validation_command(ProfileScenarioKind::MultiHopAnalysis)
        .expect("validation");

    std::fs::write(
        root.join("answer.json"),
        "{\n  \"product\": \"Atlas\",\n  \"region\": \"EMEA\",\n  \"netRevenue\": 180.00\n}\n",
    )
    .expect("write answer.json");
    std::fs::write(
        root.join("answer.md"),
        "# Answer\n\nIncluded shipped Atlas EMEA orders A1 and A4, then subtracted the A4 refund for a net revenue of 180.00.\n",
    )
    .expect("write answer.md");
    let good = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        good.status.success(),
        "expected exact answer to pass: {}",
        String::from_utf8_lossy(&good.stderr)
    );

    std::fs::write(
        root.join("answer.json"),
        "{\n  \"product\": \"Atlas\",\n  \"region\": \"EMEA\",\n  \"netRevenue\": 200.00\n}\n",
    )
    .expect("write distractor answer.json");
    let bad = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        !bad.status.success(),
        "expected returned-order distractor answer to fail"
    );

    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::MultiHopAnalysis);
    assert_eq!(calls.len(), 6);
    assert_eq!(calls[3]["path"], "data/refunds.csv");
    assert_eq!(calls[4]["path"], "answer.json");
    assert!(
        benchmark_task_prompt(ProfileScenarioKind::MultiHopAnalysis)
            .contains("joining the policy rules with both data files")
    );
}

#[test]
#[cfg(windows)]
fn inventory_rebalance_plan_validation_is_exact_and_granular() {
    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::InventoryRebalancePlan)
        .expect("prepare inventory rebalance plan");
    let root = dir.path().join(".spark-scenarios/inventory-rebalance-plan");
    let validation =
        profile_scenario_validation_command(ProfileScenarioKind::InventoryRebalancePlan)
            .expect("validation");
    let perfect_plan = r#"{
  "basePlan": {
    "budget": 325,
    "selectedOptionIds": ["T05", "T07", "T08", "T11", "T12"],
    "totalUnits": 72,
    "totalCost": 307,
    "grossAvoidedPenalty": 2950,
    "netBenefit": 2643,
    "remainingBudget": 18
  },
  "contingencyPlan": {
    "budget": 250,
    "selectedOptionIds": ["T02", "T03", "T11", "T12"],
    "totalUnits": 52,
    "totalCost": 247,
    "grossAvoidedPenalty": 2470,
    "netBenefit": 2223,
    "remainingBudget": 3
  },
  "incrementalNetBenefit": 420
}
"#;
    std::fs::write(root.join("plan.json"), perfect_plan).expect("write perfect plan");
    std::fs::write(
        root.join("memo.md"),
        "# Recommendation\n\nThe base budget adds 420 net benefit over the contingency plan. T14 is ineligible because its lead time is 5 days. Both plans stay within budget, origin surplus, and destination deficit constraints.\n",
    )
    .expect("write grounded memo");

    let good = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        good.status.success(),
        "expected exact optimum to pass: {}",
        String::from_utf8_lossy(&good.stderr)
    );

    let checks = profile_scenario_validation_checks(ProfileScenarioKind::InventoryRebalancePlan);
    assert_eq!(checks.iter().map(|check| check.weight).sum::<u32>(), 100);
    let perfect_score = checks
        .iter()
        .filter(|check| {
            std::process::Command::new(check.program)
                .args(check.args)
                .current_dir(&root)
                .output()
                .expect("run granular validation check")
                .status
                .success()
        })
        .map(|check| check.weight)
        .sum::<u32>();
    assert_eq!(perfect_score, 100);

    std::fs::write(
        root.join("plan.json"),
        perfect_plan.replace(
            r#""selectedOptionIds": ["T02", "T03", "T11", "T12"]"#,
            r#""selectedOptionIds": ["T01", "T03", "T11", "T12"]"#,
        ),
    )
    .expect("write suboptimal contingency selection");
    let bad = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run invalid validation");
    assert!(
        !bad.status.success(),
        "suboptimal contingency plan should fail"
    );
    let partial_score = checks
        .iter()
        .filter(|check| {
            std::process::Command::new(check.program)
                .args(check.args)
                .current_dir(&root)
                .output()
                .expect("run granular validation check")
                .status
                .success()
        })
        .map(|check| check.weight)
        .sum::<u32>();
    assert_eq!(partial_score, 80);

    let prompt = benchmark_task_prompt(ProfileScenarioKind::InventoryRebalancePlan);
    assert!(prompt.contains("enumerate every feasible all-or-nothing option subset"));
    assert!(prompt.contains("do not use a greedy shortcut"));
}

#[test]
fn inventory_rebalance_fixture_optima_are_derived_from_the_published_inputs() {
    #[derive(Clone)]
    struct TransferOption {
        id: String,
        sku: String,
        origin: String,
        destination: String,
        units: i64,
        cost: i64,
        lead_days: i64,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct Optimum {
        ids: String,
        total_units: i64,
        total_cost: i64,
        gross_avoided_penalty: i64,
        net_benefit: i64,
        remaining_budget: i64,
    }

    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::InventoryRebalancePlan)
        .expect("prepare inventory rebalance plan");
    let data = dir
        .path()
        .join(".spark-scenarios/inventory-rebalance-plan/data");

    let products = std::fs::read_to_string(data.join("products.csv")).expect("products");
    let penalties = products
        .lines()
        .skip(1)
        .map(|line| {
            let fields = line.split(',').collect::<Vec<_>>();
            (
                fields[0].to_string(),
                fields[1].parse::<i64>().expect("penalty"),
            )
        })
        .collect::<std::collections::HashMap<_, _>>();

    let warehouses = std::fs::read_to_string(data.join("warehouses.csv")).expect("warehouses");
    let mut surplus = std::collections::HashMap::new();
    let mut deficit = std::collections::HashMap::new();
    for line in warehouses.lines().skip(1) {
        let fields = line.split(',').collect::<Vec<_>>();
        let on_hand = fields[2].parse::<i64>().expect("on hand");
        let forecast = fields[3].parse::<i64>().expect("forecast");
        let safety = fields[4].parse::<i64>().expect("safety");
        let available = on_hand - forecast - safety;
        let key = (fields[1].to_string(), fields[0].to_string());
        if available > 0 {
            surplus.insert(key, available);
        } else if available < 0 {
            deficit.insert(key, -available);
        }
    }

    let options_csv = std::fs::read_to_string(data.join("transfer_options.csv")).expect("options");
    let options = options_csv
        .lines()
        .skip(1)
        .map(|line| {
            let fields = line.split(',').collect::<Vec<_>>();
            let units = fields[4].parse::<i64>().expect("units");
            let variable_cost = fields[5].parse::<i64>().expect("variable cost");
            let fixed_cost = fields[6].parse::<i64>().expect("fixed cost");
            TransferOption {
                id: fields[0].to_string(),
                sku: fields[1].to_string(),
                origin: fields[2].to_string(),
                destination: fields[3].to_string(),
                units,
                cost: units * variable_cost + fixed_cost,
                lead_days: fields[7].parse::<i64>().expect("lead days"),
            }
        })
        .collect::<Vec<_>>();

    let solve = |budget: i64| {
        let mut best: Option<Optimum> = None;
        for mask in 0usize..(1usize << options.len()) {
            let mut origin_units = std::collections::HashMap::new();
            let mut destination_units = std::collections::HashMap::new();
            let mut ids = Vec::new();
            let mut total_units = 0;
            let mut total_cost = 0;
            let mut gross = 0;
            let mut feasible = true;
            for (index, option) in options.iter().enumerate() {
                if mask & (1usize << index) == 0 {
                    continue;
                }
                if option.lead_days > 3 {
                    feasible = false;
                    break;
                }
                total_cost += option.cost;
                if total_cost > budget {
                    feasible = false;
                    break;
                }
                total_units += option.units;
                gross += penalties[&option.sku] * option.units;
                ids.push(option.id.clone());
                let origin_key = (option.sku.clone(), option.origin.clone());
                let destination_key = (option.sku.clone(), option.destination.clone());
                let used_origin = origin_units.entry(origin_key.clone()).or_insert(0);
                *used_origin += option.units;
                let used_destination = destination_units
                    .entry(destination_key.clone())
                    .or_insert(0);
                *used_destination += option.units;
                if *used_origin > surplus[&origin_key]
                    || *used_destination > deficit[&destination_key]
                {
                    feasible = false;
                    break;
                }
            }
            if !feasible {
                continue;
            }
            ids.sort();
            let ids = ids.join(",");
            let net_benefit = gross - total_cost;
            let candidate = Optimum {
                ids,
                total_units,
                total_cost,
                gross_avoided_penalty: gross,
                net_benefit,
                remaining_budget: budget - total_cost,
            };
            let should_replace = best.as_ref().is_none_or(|current| {
                candidate.net_benefit > current.net_benefit
                    || (candidate.net_benefit == current.net_benefit
                        && (candidate.total_cost < current.total_cost
                            || (candidate.total_cost == current.total_cost
                                && candidate.ids < current.ids)))
            });
            if should_replace {
                best = Some(candidate);
            }
        }
        best.expect("at least the empty plan is feasible")
    };

    assert_eq!(
        solve(325),
        Optimum {
            ids: "T05,T07,T08,T11,T12".to_string(),
            total_units: 72,
            total_cost: 307,
            gross_avoided_penalty: 2950,
            net_benefit: 2643,
            remaining_budget: 18,
        }
    );
    assert_eq!(
        solve(250),
        Optimum {
            ids: "T02,T03,T11,T12".to_string(),
            total_units: 52,
            total_cost: 247,
            gross_avoided_penalty: 2470,
            net_benefit: 2223,
            remaining_budget: 3,
        }
    );
}

#[test]
#[cfg(windows)]
fn experiment_rollout_audit_validation_is_exact_and_granular() {
    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::ExperimentRolloutAudit)
        .expect("prepare experiment rollout audit");
    let root = dir.path().join(".spark-scenarios/experiment-rollout-audit");
    let validation =
        profile_scenario_validation_command(ProfileScenarioKind::ExperimentRolloutAudit)
            .expect("validation");
    let perfect_audit = r#"{
  "control": {
    "eligibleUsers": 10,
    "converters": 5,
    "conversionRatePct": 50,
    "orders": 5,
    "grossRevenueCents": 47000,
    "refundedOrders": 1,
    "refundRatePct": 20,
    "refundCents": 8000,
    "netRevenueCents": 39000,
    "netRevenuePerEligibleCents": 3900
  },
  "treatment": {
    "eligibleUsers": 10,
    "converters": 7,
    "conversionRatePct": 70,
    "orders": 8,
    "grossRevenueCents": 65000,
    "refundedOrders": 2,
    "refundRatePct": 25,
    "refundCents": 24000,
    "netRevenueCents": 41000,
    "netRevenuePerEligibleCents": 4100
  },
  "uplifts": {
    "conversionUpliftPercentagePoints": 20,
    "relativeConversionUpliftPct": 40,
    "netRevenuePerEligibleUpliftPct": 5.13,
    "refundRateDeltaPercentagePoints": 5
  },
  "dataQuality": {
    "assignmentRows": 25,
    "duplicateAssignmentRows": 1,
    "conflictedUsers": 1,
    "excludedUsers": 2,
    "eventRows": 27,
    "duplicateEventRows": 1,
    "orphanEvents": 4,
    "outOfWindowCheckouts": 2,
    "duplicateOrderEvents": 1
  },
  "decision": "hold"
}
"#;
    let perfect_memo = "# Hold recommendation\n\nConversion passes with 40% relative uplift and revenue per eligible passes at 5.13%, but the refund-rate delta is 5 percentage points versus the 3 point guardrail. Hold the rollout. The audit removed duplicate rows and tracked conflicted, excluded, and orphan events before attribution.\n";
    std::fs::write(root.join("audit.json"), perfect_audit).expect("write perfect audit");
    std::fs::write(root.join("memo.md"), perfect_memo).expect("write grounded memo");

    let good = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        good.status.success(),
        "expected exact audit to pass: {}",
        String::from_utf8_lossy(&good.stderr)
    );

    let checks = profile_scenario_validation_checks(ProfileScenarioKind::ExperimentRolloutAudit);
    assert_eq!(checks.iter().map(|check| check.weight).sum::<u32>(), 100);
    let score = || {
        checks
            .iter()
            .filter(|check| {
                std::process::Command::new(check.program)
                    .args(check.args)
                    .current_dir(&root)
                    .output()
                    .expect("run granular validation check")
                    .status
                    .success()
            })
            .map(|check| check.weight)
            .sum::<u32>()
    };
    assert_eq!(score(), 100);

    let mutations = [
        (
            "unexpected schema field",
            perfect_audit.replacen(
                r#""decision": "hold""#,
                "\"decision\": \"hold\",\n  \"unexpected\": true",
                1,
            ),
            perfect_memo.to_string(),
            90,
        ),
        (
            "data-quality count",
            perfect_audit.replacen(r#""assignmentRows": 25"#, r#""assignmentRows": 24"#, 1),
            perfect_memo.to_string(),
            80,
        ),
        (
            "control attribution",
            perfect_audit.replacen(r#""eligibleUsers": 10"#, r#""eligibleUsers": 11"#, 1),
            perfect_memo.to_string(),
            85,
        ),
        (
            "treatment attribution",
            perfect_audit.replacen(
                "\"treatment\": {\n    \"eligibleUsers\": 10",
                "\"treatment\": {\n    \"eligibleUsers\": 11",
                1,
            ),
            perfect_memo.to_string(),
            80,
        ),
        (
            "uplift calculation",
            perfect_audit.replacen(
                r#""relativeConversionUpliftPct": 40"#,
                r#""relativeConversionUpliftPct": 39"#,
                1,
            ),
            perfect_memo.to_string(),
            80,
        ),
        (
            "rollout decision",
            perfect_audit.replacen(r#""decision": "hold""#, r#""decision": "launch""#, 1),
            perfect_memo.to_string(),
            85,
        ),
        (
            "memo evidence",
            perfect_audit.to_string(),
            perfect_memo.replace("orphan events", "unattributed events"),
            85,
        ),
    ];
    for (name, audit, memo, expected_score) in mutations {
        std::fs::write(root.join("audit.json"), audit)
            .unwrap_or_else(|error| panic!("write {name} mutation: {error}"));
        std::fs::write(root.join("memo.md"), memo)
            .unwrap_or_else(|error| panic!("write {name} memo: {error}"));
        let result = std::process::Command::new(validation.program)
            .args(validation.args)
            .current_dir(&root)
            .output()
            .unwrap_or_else(|error| panic!("run {name} validation: {error}"));
        assert!(
            !result.status.success(),
            "{name} mutation should fail exact validation"
        );
        assert_eq!(
            score(),
            expected_score,
            "{name} mutation should lose only its intended score dimension"
        );
    }

    let prompt = benchmark_task_prompt(ProfileScenarioKind::ExperimentRolloutAudit);
    assert!(prompt.contains("deduplicate rows"));
    assert!(prompt.contains("half-open 72-hour window") || prompt.contains("72-hour"));
    assert!(prompt.contains("Do not hand-count"));
}

#[test]
fn experiment_rollout_fixture_metrics_are_derived_from_the_published_inputs() {
    #[derive(Clone)]
    struct Order {
        variant: String,
        user: String,
        amount: i64,
        occurred_minute: i64,
    }

    #[derive(Debug, PartialEq)]
    struct VariantMetrics {
        eligible_users: usize,
        converters: usize,
        conversion_rate_pct: f64,
        orders: usize,
        gross_revenue_cents: i64,
        refunded_orders: usize,
        refund_rate_pct: f64,
        refund_cents: i64,
        net_revenue_cents: i64,
        net_revenue_per_eligible_cents: i64,
    }

    fn timestamp_minute(value: &str) -> i64 {
        let fields = value
            .split(['-', 'T', ':', 'Z'])
            .filter(|part| !part.is_empty())
            .map(|part| part.parse::<i64>().expect("timestamp field"))
            .collect::<Vec<_>>();
        fields[2] * 24 * 60 + fields[3] * 60 + fields[4]
    }

    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::ExperimentRolloutAudit)
        .expect("prepare experiment rollout audit");
    let data = dir
        .path()
        .join(".spark-scenarios/experiment-rollout-audit/data");

    let assignments = std::fs::read_to_string(data.join("assignments.csv")).expect("assignments");
    let assignment_rows = assignments.lines().skip(1).count();
    let mut seen_assignment_rows = std::collections::HashSet::new();
    let mut duplicate_assignment_rows = 0;
    let mut assignments_by_user: std::collections::HashMap<String, Vec<(String, i64)>> =
        std::collections::HashMap::new();
    for line in assignments.lines().skip(1) {
        if !seen_assignment_rows.insert(line.to_string()) {
            duplicate_assignment_rows += 1;
            continue;
        }
        let fields = line.split(',').collect::<Vec<_>>();
        assignments_by_user
            .entry(fields[0].to_string())
            .or_default()
            .push((fields[1].to_string(), timestamp_minute(fields[2])));
    }
    let exclusions = std::fs::read_to_string(data.join("exclusions.csv")).expect("exclusions");
    let excluded_users = exclusions
        .lines()
        .skip(1)
        .map(|line| line.split(',').next().expect("excluded user").to_string())
        .collect::<std::collections::HashSet<_>>();
    let conflicted_users = assignments_by_user
        .values()
        .filter(|rows| {
            rows.iter()
                .map(|(variant, _)| variant)
                .collect::<std::collections::HashSet<_>>()
                .len()
                > 1
        })
        .count();
    let counted_excluded_users = excluded_users
        .iter()
        .filter(|user| {
            assignments_by_user.get(*user).is_some_and(|rows| {
                rows.iter()
                    .map(|(variant, _)| variant)
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    == 1
            })
        })
        .count();
    let eligible = assignments_by_user
        .iter()
        .filter_map(|(user, rows)| {
            let variants = rows
                .iter()
                .map(|(variant, _)| variant)
                .collect::<std::collections::HashSet<_>>();
            (variants.len() == 1 && !excluded_users.contains(user))
                .then(|| (user.clone(), (rows[0].0.clone(), rows[0].1)))
        })
        .collect::<std::collections::HashMap<_, _>>();

    let events = std::fs::read_to_string(data.join("events.csv")).expect("events");
    let event_rows = events.lines().skip(1).count();
    let mut seen_event_ids = std::collections::HashSet::new();
    let mut duplicate_event_rows = 0;
    let mut orphan_events = 0;
    let mut out_of_window_checkouts = 0;
    let mut duplicate_order_events = 0;
    let mut orders: std::collections::HashMap<String, Order> = std::collections::HashMap::new();
    let mut refunds = Vec::new();
    for line in events.lines().skip(1) {
        let fields = line.split(',').collect::<Vec<_>>();
        if !seen_event_ids.insert(fields[0].to_string()) {
            duplicate_event_rows += 1;
            continue;
        }
        let Some((variant, assigned_minute)) = eligible.get(fields[1]) else {
            orphan_events += 1;
            continue;
        };
        let occurred_minute = timestamp_minute(fields[3]);
        match fields[2] {
            "checkout" => {
                if occurred_minute < *assigned_minute
                    || occurred_minute >= *assigned_minute + 72 * 60
                {
                    out_of_window_checkouts += 1;
                    continue;
                }
                let order = Order {
                    variant: variant.clone(),
                    user: fields[1].to_string(),
                    amount: fields[5].parse::<i64>().expect("checkout amount"),
                    occurred_minute,
                };
                match orders.get(fields[4]) {
                    Some(current) => {
                        duplicate_order_events += 1;
                        if order.occurred_minute < current.occurred_minute {
                            orders.insert(fields[4].to_string(), order);
                        }
                    }
                    None => {
                        orders.insert(fields[4].to_string(), order);
                    }
                }
            }
            "refund" => refunds.push((
                fields[4].to_string(),
                fields[5].parse::<i64>().expect("refund amount"),
                occurred_minute,
            )),
            _ => {}
        }
    }
    let refund_cutoff = timestamp_minute("2026-07-08T00:00:00Z");
    let mut refunded_orders = std::collections::HashMap::new();
    for (order_id, amount, occurred_minute) in refunds {
        if occurred_minute <= refund_cutoff && orders.contains_key(&order_id) {
            refunded_orders.entry(order_id).or_insert(amount);
        }
    }

    let metrics = |variant: &str| {
        let eligible_users = eligible
            .values()
            .filter(|(row_variant, _)| row_variant == variant)
            .count();
        let variant_orders = orders
            .iter()
            .filter(|(_, order)| order.variant == variant)
            .collect::<Vec<_>>();
        let converters = variant_orders
            .iter()
            .map(|(_, order)| order.user.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len();
        let gross_revenue_cents = variant_orders
            .iter()
            .map(|(_, order)| order.amount)
            .sum::<i64>();
        let variant_refunds = variant_orders
            .iter()
            .filter_map(|(order_id, _)| refunded_orders.get(*order_id))
            .copied()
            .collect::<Vec<_>>();
        let refund_cents = variant_refunds.iter().sum::<i64>();
        let orders = variant_orders.len();
        let net_revenue_cents = gross_revenue_cents - refund_cents;
        VariantMetrics {
            eligible_users,
            converters,
            conversion_rate_pct: converters as f64 / eligible_users as f64 * 100.0,
            orders,
            gross_revenue_cents,
            refunded_orders: variant_refunds.len(),
            refund_rate_pct: variant_refunds.len() as f64 / orders as f64 * 100.0,
            refund_cents,
            net_revenue_cents,
            net_revenue_per_eligible_cents: (net_revenue_cents as f64 / eligible_users as f64)
                .round() as i64,
        }
    };

    assert_eq!(assignment_rows, 25);
    assert_eq!(duplicate_assignment_rows, 1);
    assert_eq!(conflicted_users, 1);
    assert_eq!(counted_excluded_users, 2);
    assert_eq!(event_rows, 27);
    assert_eq!(duplicate_event_rows, 1);
    assert_eq!(orphan_events, 4);
    assert_eq!(out_of_window_checkouts, 2);
    assert_eq!(duplicate_order_events, 1);

    let control = metrics("control");
    let treatment = metrics("treatment");
    assert_eq!(
        control,
        VariantMetrics {
            eligible_users: 10,
            converters: 5,
            conversion_rate_pct: 50.0,
            orders: 5,
            gross_revenue_cents: 47_000,
            refunded_orders: 1,
            refund_rate_pct: 20.0,
            refund_cents: 8_000,
            net_revenue_cents: 39_000,
            net_revenue_per_eligible_cents: 3_900,
        }
    );
    assert_eq!(
        treatment,
        VariantMetrics {
            eligible_users: 10,
            converters: 7,
            conversion_rate_pct: 70.0,
            orders: 8,
            gross_revenue_cents: 65_000,
            refunded_orders: 2,
            refund_rate_pct: 25.0,
            refund_cents: 24_000,
            net_revenue_cents: 41_000,
            net_revenue_per_eligible_cents: 4_100,
        }
    );
    let relative_conversion_uplift = (treatment.conversion_rate_pct - control.conversion_rate_pct)
        / control.conversion_rate_pct
        * 100.0;
    let revenue_uplift = (treatment.net_revenue_per_eligible_cents
        - control.net_revenue_per_eligible_cents) as f64
        / control.net_revenue_per_eligible_cents as f64
        * 100.0;
    let refund_delta = treatment.refund_rate_pct - control.refund_rate_pct;
    assert!((relative_conversion_uplift - 40.0).abs() < 0.001);
    assert!((revenue_uplift - 5.128205).abs() < 0.001);
    assert!((refund_delta - 5.0).abs() < 0.001);
    assert!(
        relative_conversion_uplift >= 20.0 && revenue_uplift >= 5.0 && refund_delta > 3.0,
        "fixture should pass the upside gates but fail the refund guardrail"
    );
}

#[test]
fn stateful_reconciliation_fixture_requires_cross_module_invariants() {
    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(
        dir.path(),
        ProfileScenarioKind::StatefulReconciliationBugfix,
    )
    .expect("prepare stateful reconciliation");
    let root = dir
        .path()
        .join(".spark-scenarios/stateful-reconciliation-bugfix");
    let validation =
        profile_scenario_validation_command(ProfileScenarioKind::StatefulReconciliationBugfix)
            .expect("validation");

    let initial = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run initial validation");
    assert!(
        !initial.status.success(),
        "broken fixture should fail before the agent repairs it"
    );

    std::fs::write(
        root.join("src/normalize.ts"),
        r#"import type { ReservationEvent } from "./types";

export function normalizeEvents(events: ReservationEvent[]): ReservationEvent[] {
  const unique = new Map<string, ReservationEvent>();
  for (const event of events) {
    const current = unique.get(event.eventId);
    if (!current || Date.parse(event.receivedAt) > Date.parse(current.receivedAt)) {
      unique.set(event.eventId, event);
    }
  }
  return [...unique.values()].sort((left, right) =>
    Date.parse(left.occurredAt) - Date.parse(right.occurredAt)
    || left.sequence - right.sequence
    || left.eventId.localeCompare(right.eventId)
  );
}
"#,
    )
    .expect("write repaired normalizer");
    std::fs::write(
        root.join("src/project.ts"),
        r#"import { normalizeEvents } from "./normalize";
import type { ReservationEvent, ReservationState } from "./types";

export function projectReservations(events: ReservationEvent[]): ReservationState[] {
  const states = new Map<string, ReservationState>();
  for (const event of normalizeEvents(events)) {
    const key = `${event.orderId}\u{0}${event.sku}`;
    const state = states.get(key) ?? {
      orderId: event.orderId, sku: event.sku, reserved: 0, shipped: 0, terminal: false,
    };
    if (state.terminal || !Number.isFinite(event.quantity) || event.quantity <= 0) continue;
    if (event.kind === "reserve") state.reserved += event.quantity;
    if (event.kind === "release") state.reserved = Math.max(0, state.reserved - event.quantity);
    if (event.kind === "ship") {
      const consumed = Math.min(state.reserved, event.quantity);
      if (consumed > 0) {
        state.reserved -= consumed;
        state.shipped += consumed;
        state.terminal = true;
      }
    }
    states.set(key, state);
  }
  return [...states.values()];
}
"#,
    )
    .expect("write repaired projector");
    let repaired = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run repaired validation");
    assert!(
        repaired.status.success(),
        "coherent repair should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&repaired.stdout),
        String::from_utf8_lossy(&repaired.stderr)
    );

    let prompt = benchmark_task_prompt(ProfileScenarioKind::StatefulReconciliationBugfix);
    assert!(prompt.contains("documented invariants hold as a coherent state machine"));
    assert!(!prompt.contains("latest receivedAt is authoritative"));

    let calls =
        profile_scenario_expected_tool_calls(ProfileScenarioKind::StatefulReconciliationBugfix);
    assert_eq!(calls.len(), 8);
    assert_eq!(calls[5]["path"], "src/normalize.ts");
    assert_eq!(calls[6]["path"], "src/project.ts");
    assert_eq!(calls[7]["command"], "bun test");

    let artifacts = expected_scenario_artifacts(ProfileScenarioKind::StatefulReconciliationBugfix);
    assert_eq!(artifacts.len(), 2);
}

#[test]
fn feature_rollout_fixture_requires_all_cross_module_invariants() {
    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(
        dir.path(),
        ProfileScenarioKind::FeatureRolloutConsistencyBugfix,
    )
    .expect("prepare rollout fixture");
    let root = dir
        .path()
        .join(".spark-scenarios/feature-rollout-consistency-bugfix");
    let validation =
        profile_scenario_validation_command(ProfileScenarioKind::FeatureRolloutConsistencyBugfix)
            .expect("validation");

    assert!(
        !std::process::Command::new(validation.program)
            .args(validation.args)
            .current_dir(&root)
            .status()
            .expect("run broken validation")
            .success(),
        "broken rollout fixture should fail"
    );

    std::fs::write(
        root.join("src/store.ts"),
        r#"import type { FlagConfig } from "./types";

const keyFor = (tenantId: string, flagKey: string) => JSON.stringify([tenantId, flagKey]);

export class FlagConfigStore {
  private readonly configs = new Map<string, FlagConfig>();

  upsert(config: FlagConfig): boolean {
    const key = keyFor(config.tenantId, config.flagKey);
    const current = this.configs.get(key);
    if (current && config.revision <= current.revision) return false;
    this.configs.set(key, config);
    return true;
  }

  get(tenantId: string, flagKey: string): FlagConfig | undefined {
    return this.configs.get(keyFor(tenantId, flagKey));
  }
}
"#,
    )
    .expect("write store oracle");
    std::fs::write(
        root.join("src/cache.ts"),
        r#"import type { Decision, FlagConfig, Subject } from "./types";

const keyFor = (config: FlagConfig, subject: Subject) =>
  JSON.stringify([config.tenantId, config.flagKey, config.revision, subject.subjectId]);

export class DecisionCache {
  private readonly decisions = new Map<string, Decision>();

  get(config: FlagConfig, subject: Subject): Decision | undefined {
    return this.decisions.get(keyFor(config, subject));
  }

  set(config: FlagConfig, subject: Subject, decision: Decision): void {
    this.decisions.set(keyFor(config, subject), decision);
  }
}
"#,
    )
    .expect("write cache oracle");
    std::fs::write(
        root.join("src/evaluate.ts"),
        r#"import { stableBucket } from "./hash";
import type { Decision, FlagConfig, Subject } from "./types";

export function evaluate(config: FlagConfig, subject: Subject): Decision {
  if (!config.enabled) return { allowed: false, reason: "disabled", bucket: null };
  if (config.tenantId !== subject.tenantId) {
    return { allowed: false, reason: "tenant_mismatch", bucket: null };
  }
  if (config.denySubjects.includes(subject.subjectId)) {
    return { allowed: false, reason: "denied", bucket: null };
  }
  if (config.allowSubjects.includes(subject.subjectId)) {
    return { allowed: true, reason: "allowed", bucket: null };
  }
  const bucket = stableBucket(`${config.tenantId}:${config.flagKey}:${subject.subjectId}`);
  const percent = Math.max(0, Math.min(100, config.rolloutPercent));
  return bucket < percent
    ? { allowed: true, reason: "rollout", bucket }
    : { allowed: false, reason: "outside_rollout", bucket };
}
"#,
    )
    .expect("write evaluation oracle");

    assert!(
        std::process::Command::new(validation.program)
            .args(validation.args)
            .current_dir(&root)
            .status()
            .expect("run repaired validation")
            .success(),
        "complete rollout oracle should pass"
    );
    let checks =
        profile_scenario_validation_checks(ProfileScenarioKind::FeatureRolloutConsistencyBugfix);
    assert_eq!(checks.len(), 6);
    assert_eq!(checks.iter().map(|check| check.weight).sum::<u32>(), 100);
}

#[test]
fn frontier_rule_transfer_fixture_is_hidden_case_scored() {
    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::FrontierRuleTransfer)
        .expect("prepare frontier fixture");
    let root = dir.path().join(".spark-scenarios/frontier-rule-transfer");
    let validation = profile_scenario_validation_command(ProfileScenarioKind::FrontierRuleTransfer)
        .expect("validation");

    assert!(
        !std::process::Command::new(validation.program)
            .args(validation.args)
            .current_dir(&root)
            .status()
            .expect("run broken validation")
            .success(),
        "stub frontier solver should fail"
    );

    std::fs::write(
        root.join("src/solver.ts"),
        r#"import type { FrontierAnswer, FrontierCase, FrontierEdge } from "./types";

export function solveFrontierCase(input: FrontierCase): FrontierAnswer {
  const nodes = new Map(input.nodes.map(node => [node.id, node]));
  const seed = input.nodes.find(node => node.role === "seed");
  if (!seed?.tone) throw new Error("missing seed");
  const labels = seed.tone === "amber"
    ? ["alpha", "beta", "alpha", "beta"] as const
    : ["beta", "alpha", "beta", "alpha"] as const;
  const visited = new Set([seed.id]);
  const path: string[] = [];
  let current = seed.id;
  for (const label of labels) {
    const candidates = input.edges
      .filter(edge => edge.from === current && edge.label === label && !visited.has(edge.to))
      .filter(edge => nodes.has(edge.to))
      .sort((left, right) => {
        const score = (edge: FrontierEdge) => nodes.get(edge.to)!.value + edge.bias;
        return score(right) - score(left) || left.to.localeCompare(right.to);
      });
    if (candidates.length === 0) break;
    current = candidates[0].to;
    visited.add(current);
    path.push(current);
  }
  return {
    path,
    selected: path.filter((id, index) => (nodes.get(id)!.value + index + 1) % 3 === 0),
    checksum: path.reduce(
      (total, id, index) => total + (index + 1) * nodes.get(id)!.value,
      0,
    ) % 97,
  };
}
"#,
    )
    .expect("write frontier oracle");

    assert!(
        std::process::Command::new(validation.program)
            .args(validation.args)
            .current_dir(&root)
            .status()
            .expect("run frontier oracle")
            .success(),
        "general frontier oracle should pass all public and hidden cases"
    );
    let checks = profile_scenario_validation_checks(ProfileScenarioKind::FrontierRuleTransfer);
    assert_eq!(checks.len(), 6);
    assert_eq!(checks.iter().map(|check| check.weight).sum::<u32>(), 100);
    assert!(
        benchmark_task_prompt(ProfileScenarioKind::FrontierRuleTransfer)
            .contains("Do not inspect or modify tests/.harness")
    );
}

#[test]
fn policy_support_agent_is_two_turn_and_validates_updated_resolution() {
    let prompts = profile_scenario_prompts(ProfileScenarioKind::PolicySupportAgent, 45_000)
        .expect("scenario");
    assert_eq!(prompts.len(), 2);
    assert!(prompts[0].contains("policy-support-agent turn 1/2"));
    assert!(prompts[0].contains("full refund on order 5591"));
    assert!(prompts[1].contains("policy-support-agent turn 2/2"));
    assert!(prompts[1].contains("arrived cracked"));
    let benchmark = benchmark_profile_prompts(ProfileScenarioKind::PolicySupportAgent, 45_000)
        .expect("benchmark prompts");
    assert_eq!(benchmark.len(), 2);

    let dir = tempfile::tempdir().expect("tempdir");
    prepare_profile_scenario(dir.path(), ProfileScenarioKind::PolicySupportAgent)
        .expect("prepare policy support agent");
    let root = dir.path().join(".spark-scenarios/policy-support-agent");
    let validation = profile_scenario_validation_command(ProfileScenarioKind::PolicySupportAgent)
        .expect("validation");

    std::fs::write(
        root.join("resolution.json"),
        "{\n  \"orderId\": \"5591\",\n  \"refundApproved\": true,\n  \"refundAmount\": 48.50,\n  \"refundMethod\": \"store_credit\",\n  \"reasonCode\": \"damaged_on_arrival\",\n  \"policyCitations\": [\"S3\", \"S4\"]\n}\n",
    )
    .expect("write updated resolution.json");
    let good = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        good.status.success(),
        "expected updated resolution to pass: {}",
        String::from_utf8_lossy(&good.stderr)
    );

    std::fs::write(
        root.join("resolution.json"),
        "{\n  \"orderId\": \"5591\",\n  \"refundApproved\": true,\n  \"refundAmount\": 48.50,\n  \"refundMethod\": \"original_payment\",\n  \"reasonCode\": \"damaged_on_arrival\",\n  \"policyCitations\": [\"S3\"]\n}\n",
    )
    .expect("write wrong-method resolution.json");
    let bad = std::process::Command::new(validation.program)
        .args(validation.args)
        .current_dir(&root)
        .output()
        .expect("run validation");
    assert!(
        !bad.status.success(),
        "expected original_payment resolution to fail the gift-card rule"
    );

    let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::PolicySupportAgent);
    assert_eq!(calls.len(), 5);
    assert_eq!(calls[0]["path"], "brief.md");
    assert_eq!(calls[4]["path"], "resolution.json");
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

#[test]
fn external_exploration_scenarios_use_four_read_only_task_subsets() {
    let cases = [
        (
            ProfileScenarioKind::AssetRipperExploration,
            "ProductManager",
            "settings, scripts, and serialized assets",
        ),
        (
            ProfileScenarioKind::FiveMExploration,
            "FXServer",
            "the selected boundary",
        ),
        (
            ProfileScenarioKind::Cpp2IlExploration,
            "LibCpp2IL context initialization",
            "where Cpp2IL stops",
        ),
        (
            ProfileScenarioKind::Il2CppInteropExploration,
            "generate command",
            "generator versus runtime responsibilities",
        ),
    ];

    for (scenario, trace_term, synthesis_term) in cases {
        let prompts = profile_scenario_prompts(scenario, 45_000).expect("profile prompts");
        let benchmark = benchmark_profile_prompts(scenario, 45_000).expect("benchmark prompts");

        assert_eq!(prompts.len(), 4);
        assert_eq!(benchmark.len(), 4);
        for (index, prompt) in prompts.iter().enumerate() {
            assert!(prompt.contains(&format!("task subset {}/4", index + 1)));
            assert!(prompt.contains("single read-only reference root"));
            assert!(prompt.contains("Do not call cmd.exec"));
            assert!(prompt.contains("do not write, edit, rename, or delete"));
        }
        assert!(prompts[1].contains(trace_term));
        assert!(prompts[3].contains(synthesis_term));
        assert!(prompts[3].contains("Cite at least six specific paths"));
        assert!(prompts[3].contains("inference"));
        assert!(prompts[3].contains("unknown"));

        assert_eq!(
            profile_scenario_expected_tool_groups(scenario),
            vec![vec!["fs.list"], vec!["fs.read"], vec!["fs.search"]]
        );
        assert_eq!(profile_scenario_expected_tool_calls(scenario).len(), 4);
    }
}

#[test]
fn survey_suite_contains_all_external_exploration_scenarios() {
    let survey = ProfileBenchmarkSuiteKind::Survey.scenarios();

    for scenario in [
        ProfileScenarioKind::AssetRipperExploration,
        ProfileScenarioKind::FiveMExploration,
        ProfileScenarioKind::Cpp2IlExploration,
        ProfileScenarioKind::Il2CppInteropExploration,
    ] {
        assert!(
            survey.contains(&scenario),
            "{scenario:?} missing from survey suite"
        );
    }
}
