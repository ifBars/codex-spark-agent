mod agent;
mod auth;
mod benchmark_judge;
mod benchmark_results;
mod benchmark_workspace;
mod chat;
mod chat_markdown;
mod chat_tui;
mod cli;
mod client;
mod codex_cli_benchmark;
mod config;
mod opencode_benchmark;
mod profile_runner;
mod profile_scenarios;
mod profiler;
mod scenario_validation;
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
            reasoning_effort,
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
            profile_runner::run_profile_scenarios(
                &[scenario],
                profile_runner::ProfileRunOptions {
                    cwd,
                    model,
                    reasoning_effort: reasoning_effort.wire_value().to_string(),
                    max_turns,
                    target_tokens,
                    repeat,
                    no_trace,
                    no_profile,
                    compact_after_chars,
                    compact_after_tool_only_turns,
                    max_input_chars,
                    benchmark_suite: None,
                },
            )
            .await?;
        }
        Command::ProfileBenchmark {
            suite,
            cwd,
            model,
            reasoning_effort,
            max_turns,
            target_tokens,
            repeat,
            scenarios,
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
            let scenarios = selected_benchmark_scenarios(suite, &scenarios)?;
            profile_runner::run_profile_scenarios(
                &scenarios,
                profile_runner::ProfileRunOptions {
                    cwd,
                    model,
                    reasoning_effort: reasoning_effort.wire_value().to_string(),
                    max_turns,
                    target_tokens,
                    repeat,
                    no_trace,
                    no_profile,
                    compact_after_chars,
                    compact_after_tool_only_turns,
                    max_input_chars,
                    benchmark_suite: Some(suite.name().to_string()),
                },
            )
            .await?;
        }
        Command::ProfileBenchmarkReport {
            suite,
            cwd,
            limit,
            all_runs,
            output_dir,
        } => {
            let cwd = std::fs::canonicalize(&cwd)
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or(cwd));
            let output_dir = if output_dir.is_absolute() {
                output_dir
            } else {
                cwd.join(output_dir)
            };
            let report = benchmark_results::write_benchmark_report(
                benchmark_results::BenchmarkReportOptions {
                    cwd,
                    suite,
                    limit,
                    all_runs,
                    output_dir,
                },
            )?;
            println!(
                "benchmark_report suite={} rows={} avg_completion={} avg_quality={} avg_process={} json={} csv={} html={}",
                suite.name(),
                report.rows,
                report
                    .aggregate
                    .get("average_completion_score")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0),
                report
                    .aggregate
                    .get("average_quality_score")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0),
                report
                    .aggregate
                    .get("average_process_score")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0),
                report.json_path.display(),
                report.csv_path.display(),
                report.html_path.display()
            );
        }
        Command::CodexCliBenchmark {
            suite,
            cwd,
            codex_bin,
            model,
            reasoning_effort,
            repeat,
            scenarios,
            timeout_seconds,
            ignore_user_config,
            isolated_codex_home,
            output_dir,
        } => {
            let cwd = std::fs::canonicalize(&cwd)
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or(cwd));
            let output_dir = if output_dir.is_absolute() {
                output_dir
            } else {
                cwd.join(output_dir)
            };
            let report = codex_cli_benchmark::run_codex_cli_benchmark(
                codex_cli_benchmark::CodexCliBenchmarkOptions {
                    cwd,
                    suite,
                    model,
                    reasoning_effort: reasoning_effort.wire_value().to_string(),
                    repeat,
                    scenarios,
                    timeout_seconds,
                    ignore_user_config,
                    isolated_codex_home,
                    codex_bin,
                    output_dir,
                },
            )
            .await?;
            println!(
                "codex_cli_benchmark suite={} rows={} legacy_avg_score={} json={}",
                suite.name(),
                report.rows,
                report
                    .aggregate
                    .get("average_score")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0),
                report.json_path.display()
            );
        }
        Command::OpencodeBenchmark {
            suite,
            cwd,
            opencode_bin,
            model,
            reasoning_effort,
            repeat,
            scenarios,
            timeout_seconds,
            pure,
            output_dir,
        } => {
            let cwd = std::fs::canonicalize(&cwd)
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or(cwd));
            let output_dir = if output_dir.is_absolute() {
                output_dir
            } else {
                cwd.join(output_dir)
            };
            let report = opencode_benchmark::run_opencode_benchmark(
                opencode_benchmark::OpencodeBenchmarkOptions {
                    cwd,
                    suite,
                    model,
                    reasoning_effort: reasoning_effort.wire_value().to_string(),
                    repeat,
                    scenarios,
                    timeout_seconds,
                    opencode_bin,
                    pure,
                    output_dir,
                },
            )
            .await?;
            println!(
                "opencode_benchmark suite={} rows={} legacy_avg_score={} json={}",
                suite.name(),
                report.rows,
                report
                    .aggregate
                    .get("average_score")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0),
                report.json_path.display()
            );
        }
        Command::BenchmarkCompare {
            suite,
            cwd,
            limit,
            all_runs,
            codex_cli_report,
            opencode_report,
            llm_judge_report,
            group_by_reasoning,
            output_dir,
        } => {
            let cwd = std::fs::canonicalize(&cwd)
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or(cwd));
            let output_dir = if output_dir.is_absolute() {
                output_dir
            } else {
                cwd.join(output_dir)
            };
            let report = benchmark_results::write_benchmark_comparison(
                benchmark_results::BenchmarkComparisonOptions {
                    cwd,
                    suite,
                    limit,
                    all_runs,
                    codex_cli_reports: codex_cli_report,
                    opencode_reports: opencode_report,
                    llm_judge_report,
                    group_by_reasoning,
                    output_dir,
                },
            )?;
            println!(
                "benchmark_comparison suite={} rows={} winner={} benchmark_indices={} json={} csv={} html={}",
                suite.name(),
                report.rows,
                report
                    .aggregate
                    .pointer("/winner/runner")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("undetermined"),
                report
                    .aggregate
                    .get("matched_runner_benchmark_index_averages")
                    .map(serde_json::Value::to_string)
                    .unwrap_or_else(|| "{}".to_string()),
                report.json_path.display(),
                report.csv_path.display(),
                report.html_path.display()
            );
        }
        Command::BenchmarkJudge {
            comparison_report,
            cwd,
            model,
            reasoning_effort,
            output_dir,
            limit,
        } => {
            let cwd = std::fs::canonicalize(&cwd)
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or(cwd));
            let output_dir = if output_dir.is_absolute() {
                output_dir
            } else {
                cwd.join(output_dir)
            };
            let report =
                benchmark_judge::write_llm_judge_report(benchmark_judge::BenchmarkJudgeOptions {
                    cwd,
                    comparison_report,
                    model,
                    reasoning_effort: reasoning_effort.wire_value().to_string(),
                    output_dir,
                    limit,
                })
                .await?;
            println!(
                "benchmark_judge rows={} json={}",
                report.rows,
                report.json_path.display()
            );
        }
    }

    Ok(())
}

fn selected_benchmark_scenarios(
    suite: cli::ProfileBenchmarkSuiteKind,
    requested: &[cli::ProfileScenarioKind],
) -> Result<Vec<cli::ProfileScenarioKind>> {
    if requested.is_empty() {
        return Ok(suite.scenarios().to_vec());
    }
    let allowed = suite.scenarios();
    let invalid = requested
        .iter()
        .copied()
        .filter(|scenario| !allowed.contains(scenario))
        .map(cli::ProfileScenarioKind::name)
        .collect::<Vec<_>>();
    if !invalid.is_empty() {
        anyhow::bail!(
            "scenario(s) not in suite '{}': {}",
            suite.name(),
            invalid.join(",")
        );
    }
    Ok(requested.to_vec())
}
