use std::path::PathBuf;

use anyhow::Result;
use serde_json::json;

use crate::{
    APPROX_CHARS_PER_TOKEN, benchmark_workspace, cli::ProfileScenarioKind, config,
    profile_scenarios, profiler, scenario_validation, skill_commands, tools, trace_commands,
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
    for scenario in scenarios {
        let prompts =
            profile_scenarios::profile_scenario_prompts(*scenario, options.target_tokens)?;
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
            for (index, prompt) in prompts.iter().enumerate() {
                println!(
                    "scenario_turn={}/{} prompt_chars={} approx_tokens={}",
                    index + 1,
                    prompts.len(),
                    prompt.len(),
                    prompt.len() / APPROX_CHARS_PER_TOKEN
                );
                skill_commands::load_skill_mentions(&mut runner, &scenario_cwd, prompt).await?;
                if let Err(error) = runner.run(prompt).await {
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
                        } else if options.benchmark_suite.is_some()
                            && let Some(validation) =
                                scenario_validation::read_scenario_validation(&latest)
                            && validation_failed(&validation)
                            && let Some(repair_prompt) =
                                validation_repair_prompt(*scenario, &validation)
                        {
                            println!(
                                "scenario_validation_repair scenario={} prompt_chars={}",
                                scenario.name(),
                                repair_prompt.len()
                            );
                            skill_commands::load_skill_mentions(
                                &mut runner,
                                &scenario_cwd,
                                &repair_prompt,
                            )
                            .await?;
                            if let Err(error) = runner.run(&repair_prompt).await {
                                run_result = Err(error);
                                break;
                            }
                            if let Err(error) =
                                scenario_validation::run_and_write_scenario_validation(
                                    &scenario_cwd,
                                    &latest,
                                    *scenario,
                                )
                                .await
                            {
                                eprintln!(
                                    "warning: failed to rerun scenario validation after repair: {error:#}"
                                );
                            }
                        }
                        if options.benchmark_suite.is_some() {
                            latest =
                                benchmark_workspace::mirror_trace_to_source(&options.cwd, &latest)?;
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

    Ok(())
}

fn validation_failed(validation: &scenario_validation::ScenarioValidationResult) -> bool {
    validation.timed_out
        || validation.exit_code != Some(0)
        || validation
            .browser
            .as_ref()
            .is_some_and(|browser| browser.timed_out || browser.exit_code != Some(0))
}

fn validation_repair_prompt(
    scenario: ProfileScenarioKind,
    validation: &scenario_validation::ScenarioValidationResult,
) -> Option<String> {
    let scope = match scenario {
        ProfileScenarioKind::ReactCalculatorScaffold => ".spark-scenarios/react-calculator",
        ProfileScenarioKind::RustLogAnalyzerScaffold => ".spark-scenarios/rust-log-analyzer",
        _ => return None,
    };
    let mut details = String::new();
    details.push_str("Primary validation stdout:\n");
    details.push_str(&bounded_excerpt(&validation.stdout, 2_000));
    details.push_str("\nPrimary validation stderr:\n");
    details.push_str(&bounded_excerpt(&validation.stderr, 3_000));
    if let Some(browser) = &validation.browser {
        details.push_str("\nBrowser validation stdout:\n");
        details.push_str(&bounded_excerpt(&browser.stdout, 2_000));
        details.push_str("\nBrowser validation stderr:\n");
        details.push_str(&bounded_excerpt(&browser.stderr, 4_000));
    }

    Some(format!(
        "The external scenario validation failed after your implementation.\n\
         Work only under {scope}. Inspect the relevant files, fix the concrete failure below, and rerun the relevant validation command if possible.\n\
         For React browser failures, the app must render in Vite and the browser smoke must be able to click 1 + 2 = and see display result 3.\n\
         Do not rewrite the project from scratch unless the smallest fix is unclear.\n\n\
         Validation evidence:\n{details}\n\n\
         Finish with what you changed and whether validation now passes."
    ))
}

fn bounded_excerpt(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut excerpt = trimmed.chars().take(max_chars).collect::<String>();
    excerpt.push_str("\n[truncated]");
    excerpt
}
