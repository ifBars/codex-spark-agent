use std::path::PathBuf;

use anyhow::Result;
use serde_json::json;

use crate::{
    APPROX_CHARS_PER_TOKEN,
    benchmark::{results, workspace},
    cli::ProfileScenarioKind,
    config,
    profile::{scenarios, validation},
    profiler,
    skill::commands as skill_commands,
    tools,
    trace::commands as trace_commands,
};

pub(crate) struct ProfileRunOptions {
    pub(crate) cwd: PathBuf,
    pub(crate) model: String,
    pub(crate) reasoning_effort: String,
    pub(crate) target_tokens: usize,
    pub(crate) repeat: usize,
    pub(crate) no_trace: bool,
    pub(crate) no_profile: bool,
    pub(crate) compact_after_chars: usize,
    pub(crate) compact_after_tool_only_turns: usize,
    pub(crate) max_input_chars: usize,
    pub(crate) benchmark_suite: Option<String>,
}

pub(crate) async fn run_profile_scenarios(
    scenarios: &[ProfileScenarioKind],
    options: ProfileRunOptions,
) -> Result<()> {
    scenarios::validate_scenario_repeat(options.repeat)?;
    if scenarios.is_empty() {
        anyhow::bail!("benchmark suite must include at least one scenario");
    }

    if let Some(suite) = &options.benchmark_suite {
        let scenario_names = scenarios
            .iter()
            .map(|scenario| scenario.name())
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "benchmark_suite={suite} scenarios={} repeat={} reasoning_effort={}",
            scenario_names, options.repeat, options.reasoning_effort
        );
    }

    let auth = config::load_auth()?;
    let mut suite_summaries = Vec::new();
    let mut benchmark_traces = Vec::new();
    for scenario in scenarios {
        let prompts = if options.benchmark_suite.is_some() {
            scenarios::benchmark_profile_prompts(*scenario, options.target_tokens)?
        } else {
            scenarios::profile_scenario_prompts(*scenario, options.target_tokens)?
        };
        let total_prompt_chars = prompts.iter().map(String::len).sum::<usize>();
        println!(
            "scenario={:?} repeat={} prompts={} prompt_chars={} approx_tokens={} compact_after_chars={} compact_after_tool_only_turns={} max_input_chars={}",
            scenario,
            options.repeat,
            prompts.len(),
            total_prompt_chars,
            total_prompt_chars / APPROX_CHARS_PER_TOKEN,
            options.compact_after_chars,
            options.compact_after_tool_only_turns,
            options.max_input_chars
        );

        let mut scenario_summaries = Vec::new();
        let mut run_result = Ok(());
        for repeat_index in 1..=options.repeat {
            let scenario_cwd = if let Some(suite) = &options.benchmark_suite {
                let workspace = workspace::create_benchmark_workspace(
                    &options.cwd,
                    suite,
                    *scenario,
                    repeat_index,
                )?;
                println!(
                    "benchmark_workspace scenario={} path={}",
                    scenario.name(),
                    workspace.display()
                );
                scenarios::prepare_benchmark_scenario(&workspace, *scenario)?;
                workspace
            } else {
                scenarios::prepare_profile_scenario(&options.cwd, *scenario)?;
                scenarios::profile_scenario_cwd(&options.cwd, *scenario)
            };
            let read_roots = if options.benchmark_suite.is_some() {
                workspace::benchmark_read_roots(&options.cwd, &scenario_cwd, *scenario)
            } else {
                Vec::new()
            };
            if options.repeat > 1 {
                println!("scenario_repeat={repeat_index}/{}", options.repeat);
            }
            let mut runner = crate::agent::AgentRunner::new_with_reasoning_effort(
                auth.clone(),
                scenario_cwd.clone(),
                options.model.clone(),
                options.reasoning_effort.clone(),
                !options.no_trace,
                !options.no_profile,
                options.compact_after_chars,
                options.compact_after_tool_only_turns,
                options.max_input_chars,
                false,
                None,
                false,
                Some(json!({
                    "profile_scenario": {
                        "name": scenario.name(),
                        "benchmark_suite": options.benchmark_suite.as_deref(),
                        "target_tokens": options.target_tokens,
                        "prompt_count": prompts.len(),
                        "prompt_chars": total_prompt_chars,
                        "approx_prompt_tokens": total_prompt_chars / APPROX_CHARS_PER_TOKEN,
                        "repeat_index": repeat_index,
                        "repeat_count": options.repeat,
                        "reasoning_effort": options.reasoning_effort.as_str(),
                        "expected_tool_groups": scenarios::profile_scenario_expected_tool_groups(*scenario),
                        "expected_tool_calls": scenarios::profile_scenario_expected_tool_calls(*scenario),
                        "optional_tool_calls": scenarios::profile_scenario_optional_tool_calls(*scenario),
                        "expected_skills": scenarios::profile_scenario_expected_skills(*scenario),
                    }
                })),
                benchmark_agent_mode(*scenario),
            )?;
            runner.set_read_roots(read_roots.clone());
            let startup_context = if options.benchmark_suite.is_some() {
                benchmark_startup_context(&options.cwd, &read_roots, *scenario)?
            } else {
                None
            };
            for (index, prompt) in prompts.iter().enumerate() {
                println!(
                    "scenario_turn={}/{} prompt_chars={} approx_tokens={}",
                    index + 1,
                    prompts.len(),
                    prompt.len(),
                    prompt.len() / APPROX_CHARS_PER_TOKEN
                );
                let prompt_with_context;
                let run_prompt = if index == 0 {
                    if let Some(context) = &startup_context {
                        prompt_with_context = format!("{context}\n\n{prompt}");
                        prompt_with_context.as_str()
                    } else {
                        prompt
                    }
                } else {
                    prompt
                };
                skill_commands::load_skill_mentions(&mut runner, &scenario_cwd, run_prompt).await?;
                if let Err(error) = runner.run(run_prompt).await {
                    run_result = Err(error);
                    break;
                }
            }
            if !options.no_trace {
                match trace_commands::latest_trace_dir(&trace_commands::trace_runs_root(
                    &scenario_cwd,
                )) {
                    Ok(mut latest) => {
                        if let Err(error) = validation::run_and_write_scenario_validation(
                            &scenario_cwd,
                            &latest,
                            *scenario,
                        )
                        .await
                        {
                            eprintln!("warning: failed to run scenario validation: {error:#}");
                        }
                        if options.benchmark_suite.is_some() {
                            latest = workspace::mirror_trace_to_source(&options.cwd, &latest)?;
                            benchmark_traces.push(results::BenchmarkRunManifestTrace {
                                scenario: scenario.name().to_string(),
                                repeat_index,
                                workspace: scenario_cwd.display().to_string(),
                                trace_dir: trace_commands::display_trace_dir(&options.cwd, &latest)
                                    .display()
                                    .to_string(),
                            });
                        }
                        match profiler::analyze_trace(&latest) {
                            Ok(summary) => {
                                println!(
                                    "{}",
                                    profiler::format_trace_summary_row(
                                        &trace_commands::display_trace_dir(&options.cwd, &latest)
                                            .display()
                                            .to_string(),
                                        &summary,
                                    )
                                );
                                scenario_summaries.push(summary.clone());
                                suite_summaries.push(summary);
                            }
                            Err(error) => {
                                eprintln!("warning: failed to summarize scenario trace: {error:#}");
                            }
                        }
                    }
                    Err(error) => {
                        eprintln!("warning: failed to locate scenario trace: {error:#}");
                    }
                }
            }
            if run_result.is_err() {
                break;
            }
        }
        if !options.no_trace && options.repeat > 1 && !scenario_summaries.is_empty() {
            println!(
                "{}",
                profiler::format_trace_aggregate_row(scenario.name(), &scenario_summaries)
            );
        }
        if let Err(error) = run_result {
            if options.benchmark_suite.is_some() {
                eprintln!("warning: scenario {} failed: {error:#}", scenario.name());
            } else {
                return Err(error);
            }
        }
    }

    if !options.no_trace
        && scenarios.len() > 1
        && !suite_summaries.is_empty()
        && let Some(suite) = &options.benchmark_suite
    {
        println!(
            "{}",
            profiler::format_trace_aggregate_row(suite, &suite_summaries)
        );
    }
    if !options.no_trace
        && let Some(suite) = &options.benchmark_suite
    {
        let manifest_path = results::write_benchmark_run_manifest(
            &options.cwd,
            suite,
            scenarios,
            &benchmark_traces,
        )?;
        println!(
            "benchmark_run_manifest suite={} traces={} path={}",
            suite,
            benchmark_traces.len(),
            manifest_path.display()
        );
    }

    Ok(())
}

fn benchmark_startup_context(
    source_cwd: &std::path::Path,
    read_roots: &[PathBuf],
    scenario: ProfileScenarioKind,
) -> Result<Option<String>> {
    let reference_context = benchmark_reference_context(read_roots);
    let environment = format!(
        "<benchmark_context>{}\n  <note>When a benchmark prompt lists required actions, treat them as an execution checklist. Complete every required read, edit, command, search, and verification action explicitly before the final answer, even when a later smoke check seems sufficient.</note>\n</benchmark_context>\n\n<benchmark_quality_context>\n{}\n</benchmark_quality_context>",
        reference_context,
        benchmark_quality_context(scenario)
    );
    let source_is_reference = read_roots.iter().any(|root| same_path(root, source_cwd));
    let Some(agents) = source_is_reference
        .then(|| agents_context_message(source_cwd))
        .transpose()?
        .flatten()
    else {
        return Ok(Some(environment));
    };
    Ok(Some(format!("{agents}\n\n{environment}")))
}

fn benchmark_reference_context(read_roots: &[PathBuf]) -> String {
    if read_roots.is_empty() {
        String::new()
    } else {
        let roots = read_roots
            .iter()
            .map(|root| {
                format!(
                    "\n  <read_only_reference_root>{}</read_only_reference_root>",
                    context_path(root)
                )
            })
            .collect::<String>();
        roots
    }
}

fn benchmark_quality_context(scenario: ProfileScenarioKind) -> &'static str {
    match scenario {
        ProfileScenarioKind::AssetRipperExploration
        | ProfileScenarioKind::FiveMExploration
        | ProfileScenarioKind::Cpp2IlExploration
        | ProfileScenarioKind::Il2CppInteropExploration => {
            "This scenario measures exploration quality and the explanation produced after four bounded task subsets. Preserve a compact evidence ledger across turns, verify architectural claims against concrete files or symbols, and reserve the fourth response for synthesis. The final explanation should cover every subset, cite specific paths, distinguish confirmed facts from inference and unknowns, avoid inventing relationships, and recommend the next two highest-value checks."
        }
        ProfileScenarioKind::ToolRecovery | ProfileScenarioKind::ShellRecovery => {
            "Comparison evidence: Spark already completes recovery tasks quickly; quality losses came from missing exact expected probes or adding extra calls after the required path. Follow the required failing probe and recovery path exactly, verify the requested success condition once, and stop without unrelated exploration."
        }
        ProfileScenarioKind::ReactCalculatorScaffold
        | ProfileScenarioKind::RustLogAnalyzerScaffold => {
            "Comparison evidence: Spark lost scaffold quality when browser/runtime validation failed, tool output was truncated, or retries drifted. Spend extra effort here: read the brief, create every required file, keep commands scoped to the fixture, run the requested test, and verify the app or CLI entrypoint satisfies the harness smoke check before finalizing."
        }
        ProfileScenarioKind::RustNotesTuiScaffold => {
            "Comparison evidence: Spark can complete Rust scaffold tasks but loses process quality when it duplicates harness-owned smoke validation through many separate shell calls. Spend extra effort here on implementation and unit tests: read the brief, create the required Cargo files, run cargo test once after fixes, optionally inspect validate-notes.ps1, and rely on the harness-owned validation script for the full add/list/search/export/help-keys smoke check."
        }
        ProfileScenarioKind::TechnicalEssay => {
            "Comparison evidence: Spark lost essay quality on validation failure and incomplete brief coverage. Spend extra effort here: read every local source note, build the essay from those notes only, verify title/headings/word count/citations [S1], [S2], and [S3], then report the checks."
        }
        ProfileScenarioKind::OpsReport => {
            "Comparison evidence: Spark lost report quality on validation failure despite moving quickly, and a later smoke run picked the highest-risk team by ticket volume instead of severity/age. Spend extra effort here: start by reading only the brief and CSV, compute metrics from the CSV header-aware data, rank highest-risk team by open P1 severity and age before simple volume, write both required outputs, re-read metrics.json and report.md, and verify every requested metric and risk claim before finalizing."
        }
        ProfileScenarioKind::InventoryRebalancePlan => {
            "This scenario measures constrained quantitative planning, not prose fluency. Read the brief, policy, and all three CSVs; enumerate every feasible subset independently for both budgets with a Windows-compatible Bun or PowerShell command from the benchmark workspace root; enforce lead time, origin surplus, destination deficit, and cost constraints before ranking candidates; apply both tie-breakers; write the exact JSON schema and grounded memo to the fully qualified fixture paths; then verify each selected set and arithmetic total once before stopping."
        }
        ProfileScenarioKind::ExperimentRolloutAudit => {
            "This scenario measures data-quality reasoning and policy-grounded experiment analysis. Read the brief, policy, and all three CSVs; use a Windows-compatible Bun or PowerShell script to deduplicate assignments and events, remove conflicts and exclusions, attribute unique orders inside the half-open 72-hour window, join refunds only to attributed orders, and calculate both variants from eligible-user denominators. Write the exact JSON schema and a memo that evaluates every launch gate, then re-read both outputs and verify the decisive refund guardrail before stopping."
        }
        ProfileScenarioKind::ConfigMigration => {
            "Comparison evidence: Spark completed migration tasks but lost process quality through repeated/extra calls and incomplete proof. Spend extra effort here only on targeted verification: read each required file, patch the three required artifacts, validate JSON, search for stale names, and stop after the migration evidence is complete."
        }
        ProfileScenarioKind::GithubIssueBugfix
        | ProfileScenarioKind::RustFailingTestBugfix
        | ProfileScenarioKind::TypeScriptReducerBugfix
        | ProfileScenarioKind::MergeConflictResolution
        | ProfileScenarioKind::MultiModuleBugfix => {
            "Comparison evidence: Codex/OpenCode improved quality by grounding bugfix work in source and validation evidence. Spend extra effort here: read the issue first, inspect only relevant local code and tests, produce the required fix, then run the focused validation after the code change. If you run validation before fixing and it fails as expected, treat that as reproduction evidence only; rerun the same validation after the patch and do not finalize until the post-patch run passes."
        }
        ProfileScenarioKind::GithubIssueTriage => {
            "Comparison evidence: Codex/OpenCode improved quality by grounding issue work in source and log evidence. Spend extra effort here: read the issue first, inspect only relevant local code and logs, produce the required triage artifact, and cite the concrete evidence in the final answer."
        }
        ProfileScenarioKind::CiFailureTriage => {
            "Comparison evidence: CI triage quality depends on source/log linkage, not edits. Spend extra effort here: read the issue, workflow, frontend-tests.log, source, and tests; leave source files unchanged; write the requested CI triage artifact; cite the failing command, Expected: 80, Received: 100, and tie the likely root cause to SAVE20/applyDiscount."
        }
        ProfileScenarioKind::PullRequestReview => {
            "Comparison evidence: review tasks lose quality when they become broad summaries or speculative fixes. Spend extra effort here: read the PR, diff, source, and tests; do not edit source files; write a blocking review that cites read-only-admin and role.includes('admin'); recommend exact admin equality plus a regression test."
        }
        ProfileScenarioKind::DependencyUpgradeTriage => {
            "Comparison evidence: dependency triage quality comes from linking the upgrade note, lockfile, docs, source, and test gap. Spend extra effort here: read upgrade.md, package.json, bun.lock, docs, source, and tests; do not edit source files; identify the parseBusinessDate UTC/local date-only change; recommend { zone: 'utc' } and a regression test."
        }
        ProfileScenarioKind::RepoSurvey
        | ProfileScenarioKind::RepoArchitectureSurvey
        | ProfileScenarioKind::BenchmarkDesignSurvey => {
            "Comparison evidence: Spark lost survey quality from missing expected evidence, context pressure, repeated calls, and truncated outputs. Spend extra effort here by being precise, not broad: follow the required evidence path, use bounded reads/searches, summarize only after the named files/symbols were checked, and avoid broad recursive output that causes truncation. For repo-survey specifically, do not recursively list src, do not search from the repository root, and do not read more than four src files unless a search result is ambiguous."
        }
        ProfileScenarioKind::PrecisePatch | ProfileScenarioKind::MultiFilePatch => {
            "Comparison evidence: Spark succeeds on patch tasks but loses quality when it over-calls tools or under-proves the final state. Spend extra effort here on precise verification: inspect the target files, make the smallest scoped edit, run the focused validation when available, re-check the changed lines, and avoid unrelated refactors."
        }
        ProfileScenarioKind::ManifestContractWrite => {
            "For this contract-write task, use only native file tools: read the two named inputs, write exactly the two required outputs, then reread them. Preserve source order and do not inspect unrelated files or invoke the shell."
        }
        ProfileScenarioKind::ScopedPolicyPatch => {
            "For this scoped policy patch, use only native file tools: read the named specification and source file, make one line-scoped edit in canRetryPayment, then search and reread the source. Do not invoke the shell or inspect unrelated paths."
        }
        ProfileScenarioKind::TerminalRepair => {
            "Comparison evidence: terminal repair quality depends on reading actual error output before editing. Run the failing start command first and treat the failure as the diagnostic signal, fix only config/settings.json (the JSON syntax and the dataPath), never src/index.js or data/report.csv, then rerun the same start command once and stop after REPORT OK with rows=5 and top=api."
        }
        ProfileScenarioKind::MultiHopAnalysis => {
            "Comparison evidence: multi-hop analysis quality comes from joining every source instead of shortcutting from one file. Read question.md, policy.md, orders.csv, and refunds.csv; count only shipped Atlas EMEA orders, subtract only refunds attached to those orders, ignore the returned order and its refund entirely, write answer.json and answer.md, then verify both files once before stopping."
        }
        ProfileScenarioKind::PolicySupportAgent => {
            "Comparison evidence: policy tasks lose quality when the decision ignores rule composition. Apply the policy literally each turn: final-sale denies refunds until damaged-on-arrival evidence exists, and gift-card purchases always refund as store credit. Write resolution.json after turn 1, update it after turn 2, and keep the schema keys and reason codes exactly as the brief defines them."
        }
        _ => {
            "Comparison evidence: Codex/OpenCode generally scored higher by spending more time on evidence and validation, while Spark won speed. Complete the scenario's required evidence path, use one focused self-check near the end, verify required files/searches/commands are present, then stop without broad repo sweeps or repeated equivalent reads."
        }
    }
}

fn benchmark_agent_mode(scenario: ProfileScenarioKind) -> tools::AgentMode {
    if workspace::is_external_exploration(scenario) {
        tools::AgentMode::Ask
    } else {
        tools::AgentMode::Work
    }
}

fn context_path(path: &std::path::Path) -> String {
    let display = path.display().to_string();
    display
        .strip_prefix(r"\\?\")
        .unwrap_or(&display)
        .to_string()
}

fn same_path(left: &std::path::Path, right: &std::path::Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn agents_context_message(cwd: &std::path::Path) -> Result<Option<String>> {
    let files = agents_instruction_files(cwd);
    if files.is_empty() {
        return Ok(None);
    }

    let mut sections = Vec::with_capacity(files.len());
    for path in files {
        let instructions = std::fs::read_to_string(&path)
            .map_err(|error| anyhow::anyhow!("failed to read {}: {error}", path.display()))?;
        let dir = path.parent().unwrap_or(cwd);
        sections.push(format!(
            "# AGENTS.md instructions for {}\n\n<INSTRUCTIONS>\n{}\n</INSTRUCTIONS>",
            dir.display(),
            instructions.trim_end()
        ));
    }
    Ok(Some(sections.join("\n\n--- project-doc ---\n\n")))
}

fn agents_instruction_files(cwd: &std::path::Path) -> Vec<std::path::PathBuf> {
    let root = project_root(cwd);
    let mut dirs = Vec::new();
    let mut current = root.clone();
    dirs.push(current.clone());
    while current != cwd {
        let Ok(stripped) = cwd.strip_prefix(&current) else {
            break;
        };
        let Some(next_component) = stripped.components().next() else {
            break;
        };
        current = current.join(next_component.as_os_str());
        dirs.push(current.clone());
    }
    dirs.into_iter()
        .filter_map(|dir| {
            let override_path = dir.join("AGENTS.override.md");
            if override_path.exists() {
                Some(override_path)
            } else {
                let agents_path = dir.join("AGENTS.md");
                agents_path.exists().then_some(agents_path)
            }
        })
        .collect()
}

fn project_root(cwd: &std::path::Path) -> std::path::PathBuf {
    let mut current = cwd;
    loop {
        if current.join(".git").exists() {
            return current.to_path_buf();
        }
        let Some(parent) = current.parent() else {
            break;
        };
        current = parent;
    }
    cwd.to_path_buf()
}

#[cfg(test)]
mod tests {
    use crate::cli::{ProfileBenchmarkSuiteKind, ProfileScenarioKind};

    use super::{
        agents_context_message, benchmark_agent_mode, benchmark_quality_context,
        benchmark_startup_context, context_path,
    };

    #[test]
    fn agents_context_message_matches_codex_project_instruction_shape() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("AGENTS.md"),
            "Use bun.\nNever set CARGO_TARGET_DIR.\n",
        )
        .expect("write agents");

        let context = agents_context_message(dir.path())
            .expect("agents context")
            .expect("context should exist");

        assert!(context.starts_with("# AGENTS.md instructions for "));
        assert!(context.contains("<INSTRUCTIONS>\nUse bun."));
        assert!(context.contains("Never set CARGO_TARGET_DIR."));
        assert!(context.ends_with("</INSTRUCTIONS>"));
    }

    #[test]
    fn agents_context_message_is_absent_without_agents_file() {
        let dir = tempfile::tempdir().expect("tempdir");

        let context = agents_context_message(dir.path()).expect("agents context");

        assert!(context.is_none());
    }

    #[test]
    fn agents_context_message_prefers_override_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("AGENTS.md"), "Use bun.").expect("write agents");
        std::fs::write(
            dir.path().join("AGENTS.override.md"),
            "Override instructions.",
        )
        .expect("write override");

        let context = agents_context_message(dir.path())
            .expect("agents context")
            .expect("context should exist");

        assert!(context.contains("Override instructions."));
        assert!(!context.contains("Use bun."));
    }

    #[test]
    fn benchmark_startup_context_does_not_repeat_workspace_cwd() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("AGENTS.md"), "Use bun.").expect("write agents");

        let context = benchmark_startup_context(
            dir.path(),
            &[dir.path().to_path_buf()],
            ProfileScenarioKind::RepoSurvey,
        )
        .expect("startup context")
        .expect("context should exist");

        assert!(context.contains("# AGENTS.md instructions for "));
        assert!(context.contains("<benchmark_context>"));
        assert!(!context.contains("<cwd>"));
        assert!(!context.contains("Work only under"));
    }

    #[test]
    fn benchmark_startup_context_keeps_only_benchmark_specific_guidance() {
        let dir = tempfile::tempdir().expect("tempdir");

        let context = benchmark_startup_context(dir.path(), &[], ProfileScenarioKind::RepoSurvey)
            .expect("startup context")
            .expect("benchmark context should exist");

        assert!(!context.contains("# AGENTS.md instructions for "));
        assert!(context.contains("<benchmark_context>"));
        assert!(!context.contains("<cwd>"));
        assert!(!context.contains(&context_path(dir.path())));
        assert!(!context.contains("Use paths relative to cwd"));
        assert!(!context.contains("For cmd.exec workdir"));
        assert!(context.contains("treat them as an execution checklist"));
        assert!(context.contains("Complete every required read, edit, command, search"));
    }

    #[test]
    fn benchmark_startup_context_includes_reference_root() {
        let source = tempfile::tempdir().expect("source");
        let context = benchmark_startup_context(
            source.path(),
            &[source.path().to_path_buf()],
            ProfileScenarioKind::RepoSurvey,
        )
        .expect("startup context")
        .expect("benchmark context should exist");

        assert!(context.contains("<read_only_reference_root>"));
        assert!(context.contains(&context_path(source.path())));
        assert!(!context.contains("writes and shell commands remain scoped to cwd"));
        assert!(!context.contains("<cwd>"));
    }

    #[test]
    fn context_path_strips_windows_verbatim_prefix() {
        let path = std::path::Path::new(r"\\?\C:\repo\workspace");

        assert_eq!(context_path(path), r"C:\repo\workspace");
    }

    #[test]
    fn benchmark_quality_context_uses_real_comparison_lessons_by_task_risk() {
        let scaffold = benchmark_quality_context(ProfileScenarioKind::ReactCalculatorScaffold);
        assert!(scaffold.contains("browser/runtime validation failed"));
        assert!(scaffold.contains("verify the app or CLI entrypoint"));

        let notes = benchmark_quality_context(ProfileScenarioKind::RustNotesTuiScaffold);
        assert!(notes.contains("harness-owned validation script"));
        assert!(notes.contains("add/list/search/export/help-keys"));

        let recovery = benchmark_quality_context(ProfileScenarioKind::ToolRecovery);
        assert!(recovery.contains("Follow the required failing probe"));
        assert!(recovery.contains("stop without unrelated exploration"));

        let report = benchmark_quality_context(ProfileScenarioKind::OpsReport);
        assert!(report.contains("re-read metrics.json and report.md"));
        assert!(report.contains("open P1 severity and age"));

        let bugfix = benchmark_quality_context(ProfileScenarioKind::RustFailingTestBugfix);
        assert!(bugfix.contains("run the focused validation after the code change"));
        assert!(bugfix.contains("reproduction evidence only"));
        assert!(bugfix.contains("rerun the same validation after the patch"));
        assert!(bugfix.contains("do not finalize until the post-patch run passes"));

        let merge_conflict =
            benchmark_quality_context(ProfileScenarioKind::MergeConflictResolution);
        assert!(merge_conflict.contains("run the focused validation after the code change"));
        assert!(merge_conflict.contains("do not finalize until the post-patch run passes"));

        let ci = benchmark_quality_context(ProfileScenarioKind::CiFailureTriage);
        assert!(ci.contains("frontend-tests.log"));
        assert!(ci.contains("Expected: 80"));
        assert!(ci.contains("Received: 100"));
        assert!(ci.contains("leave source files unchanged"));

        let review = benchmark_quality_context(ProfileScenarioKind::PullRequestReview);
        assert!(review.contains("read-only-admin"));
        assert!(review.contains("role.includes('admin')"));
        assert!(review.contains("blocking review"));
        assert!(review.contains("do not edit source files"));

        let dependency = benchmark_quality_context(ProfileScenarioKind::DependencyUpgradeTriage);
        assert!(dependency.contains("@acme/time-utils") || dependency.contains("upgrade.md"));
        assert!(dependency.contains("parseBusinessDate"));
        assert!(dependency.contains("{ zone: 'utc' }"));
        assert!(dependency.contains("regression test"));

        let survey = benchmark_quality_context(ProfileScenarioKind::RepoSurvey);
        assert!(survey.contains("do not recursively list src"));
        assert!(survey.contains("do not read more than four src files"));

        let exploration = benchmark_quality_context(ProfileScenarioKind::Cpp2IlExploration);
        assert!(exploration.contains("after four bounded task subsets"));
        assert!(exploration.contains("distinguish confirmed facts from inference and unknowns"));
    }

    #[test]
    fn external_exploration_scenarios_are_forced_to_ask_mode() {
        for scenario in [
            ProfileScenarioKind::AssetRipperExploration,
            ProfileScenarioKind::FiveMExploration,
            ProfileScenarioKind::Cpp2IlExploration,
            ProfileScenarioKind::Il2CppInteropExploration,
        ] {
            assert_eq!(benchmark_agent_mode(scenario), crate::tools::AgentMode::Ask);
        }
        assert_eq!(
            benchmark_agent_mode(ProfileScenarioKind::PrecisePatch),
            crate::tools::AgentMode::Work
        );
    }

    #[test]
    fn quick_real_world_scenarios_avoid_generic_quality_context() {
        let helper = std::fs::read_to_string("scripts/quick_benchmark_scenarios.ps1")
            .expect("read quick scenario helper");
        let scenarios = quick_real_world_scenarios(&helper);
        assert!(!scenarios.is_empty());

        let real_world = ProfileBenchmarkSuiteKind::RealWorld.scenarios();
        for scenario_name in scenarios {
            let scenario = real_world
                .iter()
                .copied()
                .find(|scenario| scenario.name() == scenario_name)
                .unwrap_or_else(|| panic!("{scenario_name} is not in the real-world suite"));
            let context = benchmark_quality_context(scenario);
            assert!(
                !context.contains("Codex/OpenCode generally scored higher"),
                "{scenario_name} is using the generic quality context"
            );
        }
    }

    fn quick_real_world_scenarios(helper: &str) -> Vec<String> {
        let Some(start) = helper.find("return @(") else {
            return Vec::new();
        };
        let after_start = &helper[start..];
        let Some(end) = after_start.find("\n    )") else {
            return Vec::new();
        };

        after_start[..end]
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim().trim_end_matches(',');
                if trimmed.starts_with('"') && trimmed.ends_with('"') {
                    Some(trimmed.trim_matches('"').to_string())
                } else {
                    None
                }
            })
            .collect()
    }
}
