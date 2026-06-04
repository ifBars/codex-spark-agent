mod agent;
mod auth;
mod chat;
mod chat_markdown;
mod chat_tui;
mod cli;
mod client;
mod config;
mod profile_scenarios;
mod profiler;
mod session_store;
mod sessions;
mod skill_commands;
mod skills;
mod tools;
mod trace_cli;
mod trace_commands;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use serde_json::json;

use cli::{Cli, Command};

const DEFAULT_MODEL: &str = "gpt-5.3-codex-spark";
const DEFAULT_COMPACT_AFTER_CHARS: usize = 160_000;
const DEFAULT_MAX_INPUT_CHARS: usize = 500_000;
const APPROX_CHARS_PER_TOKEN: usize = 4;
const DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS: usize = 12;
const DEFAULT_SCENARIO_TARGET_TOKENS: usize = 45_000;
const MAX_SCENARIO_TARGET_TOKENS: usize = 120_000;
const MAX_SCENARIO_REPEAT: usize = 50;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Login { no_browser, device } => {
            let tokens = if device {
                auth::login_device_code().await?
            } else {
                auth::login(!no_browser).await?
            };
            config::save_auth(&tokens)?;
            println!(
                "Logged in. Account: {}",
                tokens.account_id.as_deref().unwrap_or("unknown")
            );
        }
        Command::AuthStatus => {
            let tokens = config::load_auth()?;
            println!(
                "Logged in. Account: {}. Expires at unix: {}",
                tokens.account_id.as_deref().unwrap_or("unknown"),
                tokens.expires_at
            );
        }
        Command::Chat {
            prompt,
            prompt_file,
            cwd,
            model,
            mode,
            max_turns,
            trace,
            profile,
            session,
            skills: requested_skills,
            new_session,
            compact_after_chars,
            compact_after_tokens,
            compact_after_tool_only_turns,
            max_input_chars,
            max_input_tokens,
        } => {
            let interactive = prompt_file.is_none() && prompt.is_empty();
            let prompt = if let Some(path) = prompt_file {
                Some(std::fs::read_to_string(&path).map_err(|error| {
                    anyhow::anyhow!("failed to read {}: {error}", path.display())
                })?)
            } else if prompt.is_empty() {
                None
            } else {
                Some(prompt.join(" "))
            };
            let cwd = std::fs::canonicalize(&cwd)
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or(cwd));
            let compact_after_chars = trace_commands::resolve_char_threshold(
                "compact-after",
                compact_after_chars,
                compact_after_tokens,
                DEFAULT_COMPACT_AFTER_CHARS,
            )?;
            let max_input_chars = trace_commands::resolve_char_threshold(
                "max-input",
                max_input_chars,
                max_input_tokens,
                DEFAULT_MAX_INPUT_CHARS,
            )?;
            let explicit_session = session.is_some();
            let session_name =
                session.or_else(|| interactive.then(sessions::timestamp_session_name));
            let start_new_session = new_session || (interactive && !explicit_session);
            sessions::prepare_default_session_store(session_name.as_deref())?;
            let auth = config::load_auth()?;
            let mut runner = agent::AgentRunner::new(
                auth,
                cwd.clone(),
                model,
                max_turns,
                trace,
                profile,
                compact_after_chars,
                compact_after_tool_only_turns,
                max_input_chars,
                interactive,
                session_name.clone(),
                start_new_session,
                None,
                mode.into(),
            )?;
            if let Some(name) = &session_name {
                if start_new_session {
                    runner.save_session_named(name)?;
                    println!("Started new session: {name}");
                } else if runner.load_session_named(name)? {
                    println!("Resumed session: {name}");
                }
            }
            for skill_name in requested_skills {
                skill_commands::load_skill_into_runner(&mut runner, &cwd, &skill_name, false)
                    .await?;
            }
            if interactive {
                chat::run_interactive_chat(&mut runner, session_name, cwd).await?;
            } else {
                let prompt = prompt.unwrap_or_default();
                if prompt.trim().is_empty() {
                    anyhow::bail!("prompt is required");
                }
                skill_commands::load_skill_mentions(&mut runner, &cwd, &prompt).await?;
                runner.run(&prompt).await?;
                if let Some(name) = &session_name {
                    runner.save_session_named(name)?;
                    println!("Saved session: {name}");
                }
            }
        }
        Command::Tools => {
            println!("{}", serde_json::to_string_pretty(&tools::builtin_tools())?);
        }
        Command::Sessions => {
            sessions::prepare_default_session_store(None)?;
            for session in session_store::SessionStore::open_default()?.list_names()? {
                println!("{session}");
            }
        }
        Command::Skills { refresh } => {
            let cwd = std::fs::canonicalize(".").unwrap_or_else(|_| PathBuf::from("."));
            if refresh {
                let auth = config::load_auth()?;
                let runner = agent::AgentRunner::new(
                    auth,
                    cwd.clone(),
                    DEFAULT_MODEL.to_string(),
                    None,
                    false,
                    false,
                    DEFAULT_COMPACT_AFTER_CHARS,
                    DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS,
                    DEFAULT_MAX_INPUT_CHARS,
                    false,
                    None,
                    false,
                    None,
                    tools::AgentMode::Work,
                )?;
                for source in skills::discover_sources(&cwd)? {
                    let skill =
                        skill_commands::compile_skill_cached(&runner, &cwd, &source.name, true)
                            .await?;
                    println!(
                        "{} - {} ({}, {} chars)",
                        skill.name, skill.description, skill.source_path, skill.full_text_chars
                    );
                }
            } else {
                for skill in skills::list_status(&cwd)? {
                    println!(
                        "{} [{}] - {} ({})",
                        skill.name, skill.cache_status, skill.description, skill.source_path
                    );
                }
            }
        }
        Command::Traces {
            limit,
            summary,
            scenario,
            diagnostics,
            aggregate,
            sort,
            min_tool_only_streak,
            min_overrun_turns,
            min_overrun_context_chars,
            min_compaction_regrowth_chars,
            json,
            jsonl,
        } => {
            trace_cli::handle_traces(
                limit,
                summary,
                scenario,
                diagnostics,
                aggregate,
                sort,
                min_tool_only_streak,
                min_overrun_turns,
                min_overrun_context_chars,
                min_compaction_regrowth_chars,
                json,
                jsonl,
            )?;
        }
        Command::AnalyzeTrace {
            dir,
            latest,
            timeline,
        } => {
            if latest && dir.is_some() {
                anyhow::bail!("pass either a trace directory or --latest, not both");
            }
            let cwd = std::fs::canonicalize(".").unwrap_or_else(|_| PathBuf::from("."));
            let dir = match dir {
                Some(dir) => dir,
                None => trace_commands::latest_trace_dir(&trace_commands::trace_runs_root(&cwd))?,
            };
            let summary = profiler::analyze_trace(&dir)?;
            if timeline {
                print!("{}", profiler::format_trace_timeline(&summary));
            } else {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            }
        }
        Command::ProfileScenario {
            scenario,
            cwd,
            model,
            max_turns,
            target_tokens,
            repeat,
            no_trace,
            no_profile,
            compact_after_chars,
            compact_after_tokens,
            compact_after_tool_only_turns,
            max_input_chars,
            max_input_tokens,
        } => {
            let cwd = std::fs::canonicalize(&cwd)
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or(cwd));
            let compact_after_chars = trace_commands::resolve_char_threshold(
                "compact-after",
                compact_after_chars,
                compact_after_tokens,
                DEFAULT_COMPACT_AFTER_CHARS,
            )?;
            let max_input_chars = trace_commands::resolve_char_threshold(
                "max-input",
                max_input_chars,
                max_input_tokens,
                DEFAULT_MAX_INPUT_CHARS,
            )?;
            profile_scenarios::validate_scenario_repeat(repeat)?;
            let prompts = profile_scenarios::profile_scenario_prompts(scenario, target_tokens)?;
            let total_prompt_chars = prompts.iter().map(String::len).sum::<usize>();
            println!(
                "scenario={:?} repeat={} prompts={} prompt_chars={} approx_tokens={} compact_after_chars={} compact_after_tool_only_turns={} max_input_chars={}",
                scenario,
                repeat,
                prompts.len(),
                total_prompt_chars,
                total_prompt_chars / APPROX_CHARS_PER_TOKEN,
                compact_after_chars,
                compact_after_tool_only_turns,
                max_input_chars
            );
            let auth = config::load_auth()?;
            let mut summaries = Vec::new();
            let mut run_result = Ok(());
            for repeat_index in 1..=repeat {
                profile_scenarios::prepare_profile_scenario(&cwd, scenario)?;
                if repeat > 1 {
                    println!("scenario_repeat={repeat_index}/{repeat}");
                }
                let mut runner = agent::AgentRunner::new(
                    auth.clone(),
                    cwd.clone(),
                    model.clone(),
                    max_turns,
                    !no_trace,
                    !no_profile,
                    compact_after_chars,
                    compact_after_tool_only_turns,
                    max_input_chars,
                    false,
                    None,
                    false,
                    Some(json!({
                        "profile_scenario": {
                            "name": scenario.name(),
                            "target_tokens": target_tokens,
                            "prompt_count": prompts.len(),
                            "prompt_chars": total_prompt_chars,
                            "approx_prompt_tokens": total_prompt_chars / APPROX_CHARS_PER_TOKEN,
                            "repeat_index": repeat_index,
                            "repeat_count": repeat,
                            "expected_tool_groups": profile_scenarios::profile_scenario_expected_tool_groups(scenario),
                            "expected_tool_calls": profile_scenarios::profile_scenario_expected_tool_calls(scenario),
                            "expected_skills": profile_scenarios::profile_scenario_expected_skills(scenario),
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
                    skill_commands::load_skill_mentions(&mut runner, &cwd, prompt).await?;
                    if let Err(error) = runner.run(prompt).await {
                        run_result = Err(error);
                        break;
                    }
                }
                if !no_trace {
                    match trace_commands::latest_trace_dir(&trace_commands::trace_runs_root(&cwd))
                        .and_then(|latest| {
                            let summary = profiler::analyze_trace(&latest)?;
                            Ok((latest, summary))
                        }) {
                        Ok((latest, summary)) => {
                            println!(
                                "{}",
                                profiler::format_trace_summary_row(
                                    &trace_commands::display_trace_dir(&cwd, &latest)
                                        .display()
                                        .to_string(),
                                    &summary,
                                )
                            );
                            summaries.push(summary);
                        }
                        Err(error) => {
                            eprintln!("warning: failed to summarize scenario trace: {error:#}");
                        }
                    }
                }
                if run_result.is_err() {
                    break;
                }
            }
            if !no_trace && repeat > 1 && !summaries.is_empty() {
                println!(
                    "{}",
                    profiler::format_trace_aggregate_row(scenario.name(), &summaries)
                );
            }
            run_result?;
        }
    }

    Ok(())
}
