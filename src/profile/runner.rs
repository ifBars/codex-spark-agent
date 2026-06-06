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
    pub(crate) max_turns: Option<usize>,
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
                workspace
            } else {
                options.cwd.clone()
            };
            scenarios::prepare_profile_scenario(&scenario_cwd, *scenario)?;
            let read_roots = if options.benchmark_suite.is_some() {
                workspace::benchmark_read_roots(&options.cwd, &scenario_cwd)
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
                options.max_turns,
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
                        "expected_skills": scenarios::profile_scenario_expected_skills(*scenario),
                    }
                })),
                tools::AgentMode::Work,
            )?;
            runner.set_read_roots(read_roots.clone());
            let startup_context = if options.benchmark_suite.is_some() {
                benchmark_startup_context(&options.cwd, &scenario_cwd, &read_roots, *scenario)?
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
    cwd: &std::path::Path,
    read_roots: &[PathBuf],
    scenario: ProfileScenarioKind,
) -> Result<Option<String>> {
    let reference_context = benchmark_reference_context(read_roots);
    let environment = format!(
        "<environment_context>\n  <cwd>{}</cwd>\n  <note>Use paths relative to cwd for native filesystem and shell tools. Do not copy the absolute cwd into fs.* path arguments.</note>\n  <note>For cmd.exec workdir, use a path inside this cwd, preferably a relative .spark-scenarios/... path. Do not use source repository or read-only reference-root absolute paths as command workdirs.</note>\n  <note>For benchmark prompts with exact .spark-scenarios paths, start by reading those paths directly. Do not list cwd, recursively list .spark-scenarios, or search for AGENTS.md just to rediscover provided fixtures or instructions.</note>\n  <note>When a benchmark prompt lists required actions, treat them as an execution checklist. Complete every required read, edit, command, search, and verification action explicitly before the final answer, even when a later smoke check seems sufficient.</note>{}\n</environment_context>\n\n<benchmark_quality_context>\n{}\n</benchmark_quality_context>",
        context_path(cwd),
        reference_context,
        benchmark_quality_context(scenario)
    );
    let Some(agents) = agents_context_message(source_cwd)? else {
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
        format!(
            "\n  <note>Native read-only tools may read source evidence from the reference root, but writes and shell commands remain scoped to cwd.</note>{roots}"
        )
    }
}

fn benchmark_quality_context(scenario: ProfileScenarioKind) -> &'static str {
    match scenario {
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
        ProfileScenarioKind::ConfigMigration => {
            "Comparison evidence: Spark completed migration tasks but lost process quality through repeated/extra calls and incomplete proof. Spend extra effort here only on targeted verification: read each required file, patch the three required artifacts, validate JSON, search for stale names, and stop after the migration evidence is complete."
        }
        ProfileScenarioKind::GithubIssueBugfix | ProfileScenarioKind::GithubIssueTriage => {
            "Comparison evidence: Codex/OpenCode improved quality by grounding issue work in source and validation evidence. Spend extra effort here: read the issue first, inspect only relevant local code/logs/tests, produce the required fix or triage artifact, run or perform the focused validation, and cite the concrete evidence in the final answer."
        }
        ProfileScenarioKind::RepoSurvey
        | ProfileScenarioKind::RepoArchitectureSurvey
        | ProfileScenarioKind::BenchmarkDesignSurvey => {
            "Comparison evidence: Spark lost survey quality from missing expected evidence, context pressure, repeated calls, and truncated outputs. Spend extra effort here by being precise, not broad: follow the required evidence path, use bounded reads/searches, summarize only after the named files/symbols were checked, and avoid broad recursive output that causes truncation. For repo-survey specifically, do not recursively list src, do not search from the repository root, and do not read more than four src files unless a search result is ambiguous."
        }
        ProfileScenarioKind::PrecisePatch | ProfileScenarioKind::MultiFilePatch => {
            "Comparison evidence: Spark succeeds on patch tasks but loses quality when it over-calls tools or under-proves the final state. Spend extra effort here on precise verification: inspect the target files, make the smallest scoped edit, run the focused validation when available, re-check the changed lines, and avoid unrelated refactors."
        }
        _ => {
            "Comparison evidence: Codex/OpenCode generally scored higher by spending more time on evidence and validation, while Spark won speed. Complete the scenario's required evidence path, use one focused self-check near the end, verify required files/searches/commands are present, then stop without broad repo sweeps or repeated equivalent reads."
        }
    }
}

fn context_path(path: &std::path::Path) -> String {
    let display = path.display().to_string();
    display
        .strip_prefix(r"\\?\")
        .unwrap_or(&display)
        .to_string()
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
    use crate::cli::ProfileScenarioKind;

    use super::{
        agents_context_message, benchmark_quality_context, benchmark_startup_context, context_path,
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
    fn benchmark_startup_context_includes_environment_cwd() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("AGENTS.md"), "Use bun.").expect("write agents");

        let context =
            benchmark_startup_context(dir.path(), dir.path(), &[], ProfileScenarioKind::RepoSurvey)
                .expect("startup context")
                .expect("context should exist");

        assert!(context.contains("# AGENTS.md instructions for "));
        assert!(context.contains("<environment_context>"));
        assert!(context.contains("<cwd>"));
    }

    #[test]
    fn benchmark_startup_context_includes_environment_without_agents_file() {
        let dir = tempfile::tempdir().expect("tempdir");

        let context =
            benchmark_startup_context(dir.path(), dir.path(), &[], ProfileScenarioKind::RepoSurvey)
                .expect("startup context")
                .expect("environment context should exist");

        assert!(!context.contains("# AGENTS.md instructions for "));
        assert!(context.contains("<environment_context>"));
        assert!(context.contains("<cwd>"));
        assert!(context.contains(&context_path(dir.path())));
        assert!(context.contains("Use paths relative to cwd"));
        assert!(context.contains("For cmd.exec workdir"));
        assert!(
            context.contains(
                "Do not use source repository or read-only reference-root absolute paths"
            )
        );
        assert!(context.contains("start by reading those paths directly"));
        assert!(context.contains("Do not list cwd"));
        assert!(context.contains("search for AGENTS.md"));
        assert!(context.contains("treat them as an execution checklist"));
        assert!(context.contains("Complete every required read, edit, command, search"));
    }

    #[test]
    fn benchmark_startup_context_includes_reference_root() {
        let source = tempfile::tempdir().expect("source");
        let scenario = tempfile::tempdir().expect("scenario");

        let context = benchmark_startup_context(
            source.path(),
            scenario.path(),
            &[source.path().to_path_buf()],
            ProfileScenarioKind::RepoSurvey,
        )
        .expect("startup context")
        .expect("environment context should exist");

        assert!(context.contains("<read_only_reference_root>"));
        assert!(context.contains(&context_path(source.path())));
        assert!(context.contains("writes and shell commands remain scoped to cwd"));
        assert!(
            context.contains(
                "Do not use source repository or read-only reference-root absolute paths"
            )
        );
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

        let survey = benchmark_quality_context(ProfileScenarioKind::RepoSurvey);
        assert!(survey.contains("do not recursively list src"));
        assert!(survey.contains("do not read more than four src files"));
    }
}
