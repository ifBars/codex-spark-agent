use std::path::PathBuf;

use anyhow::Result;
use serde_json::json;

use crate::{
    APPROX_CHARS_PER_TOKEN, benchmark_results, benchmark_workspace, cli::ProfileScenarioKind,
    config, profile_scenarios, profiler, scenario_validation, skill_commands, tools,
    trace_commands,
};

pub(crate) struct ProfileRunOptions {
    pub(crate) cwd: PathBuf,
    pub(crate) model: String,
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
    profile_scenarios::validate_scenario_repeat(options.repeat)?;
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
            "benchmark_suite={suite} scenarios={} repeat={}",
            scenario_names, options.repeat
        );
    }

    let auth = config::load_auth()?;
    let mut suite_summaries = Vec::new();
    let mut benchmark_traces = Vec::new();
    for scenario in scenarios {
        let prompts = if options.benchmark_suite.is_some() {
            profile_scenarios::benchmark_profile_prompts(*scenario, options.target_tokens)?
        } else {
            profile_scenarios::profile_scenario_prompts(*scenario, options.target_tokens)?
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
                let workspace = benchmark_workspace::create_benchmark_workspace(
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
            profile_scenarios::prepare_profile_scenario(&scenario_cwd, *scenario)?;
            if options.repeat > 1 {
                println!("scenario_repeat={repeat_index}/{}", options.repeat);
            }
            let mut runner = crate::agent::AgentRunner::new(
                auth.clone(),
                scenario_cwd.clone(),
                options.model.clone(),
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
                        "expected_tool_groups": profile_scenarios::profile_scenario_expected_tool_groups(*scenario),
                        "expected_tool_calls": profile_scenarios::profile_scenario_expected_tool_calls(*scenario),
                        "expected_skills": profile_scenarios::profile_scenario_expected_skills(*scenario),
                    }
                })),
                tools::AgentMode::Work,
            )?;
            let startup_context = if options.benchmark_suite.is_some() {
                benchmark_startup_context(&scenario_cwd)?
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
                        if let Err(error) = scenario_validation::run_and_write_scenario_validation(
                            &scenario_cwd,
                            &latest,
                            *scenario,
                        )
                        .await
                        {
                            eprintln!("warning: failed to run scenario validation: {error:#}");
                        }
                        if options.benchmark_suite.is_some() {
                            latest =
                                benchmark_workspace::mirror_trace_to_source(&options.cwd, &latest)?;
                            benchmark_traces.push(benchmark_results::BenchmarkRunManifestTrace {
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
        let manifest_path = benchmark_results::write_benchmark_run_manifest(
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

fn benchmark_startup_context(cwd: &std::path::Path) -> Result<Option<String>> {
    let Some(agents) = agents_context_message(cwd)? else {
        return Ok(None);
    };
    Ok(Some(format!(
        "{agents}\n\n<environment_context>\n  <cwd>{}</cwd>\n</environment_context>",
        cwd.display()
    )))
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
    use super::{agents_context_message, benchmark_startup_context};

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

        let context = benchmark_startup_context(dir.path())
            .expect("startup context")
            .expect("context should exist");

        assert!(context.contains("# AGENTS.md instructions for "));
        assert!(context.contains("<environment_context>"));
        assert!(context.contains("<cwd>"));
    }
}
