mod account_usage;
mod agent;
mod auth;
mod benchmark;
mod chat;
mod cli;
mod client;
mod codex_integration;
mod config;
mod desktop_server;
mod mcp;
mod mcp_server;
mod memory;
mod model_context;
mod profile;
mod profiler;
mod prompt_commands;
mod repo_brief;
mod session;
mod setup;
mod skill;
mod spinner_preview;
mod spinners;
mod telemetry;
mod tools;
mod trace;
mod usage_history;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use anyhow::Result;
use cli::Command;

const DEFAULT_MODEL: &str = "gpt-5.3-codex-spark";
const DEFAULT_COMPACT_AFTER_CHARS: usize =
    model_context::default_compact_after_chars(model_context::DEFAULT_CONTEXT_WINDOW_TOKENS);
const DEFAULT_MAX_INPUT_CHARS: usize =
    model_context::default_max_input_chars(model_context::DEFAULT_CONTEXT_WINDOW_TOKENS);
const APPROX_CHARS_PER_TOKEN: usize = 4;
const DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS: usize = 0;
const DEFAULT_SCENARIO_TARGET_TOKENS: usize = 45_000;
const MAX_SCENARIO_TARGET_TOKENS: usize = 120_000;
const MAX_SCENARIO_REPEAT: usize = 50;

fn main() -> Result<()> {
    telemetry::init();
    let cli = cli::parse_with_stack()?;

    if matches!(&cli.command, Command::SpinnerPreview) {
        return spinner_preview::run();
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run_command(cli.command))
}

async fn run_command(command: Command) -> Result<()> {
    match command {
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
        Command::Setup {
            non_interactive,
            skip_login,
            skip_skill_migration,
            skill_source,
            cwd,
            codex,
            force_codex,
        } => {
            setup::run(setup::SetupOptions {
                cwd,
                non_interactive,
                skip_login,
                skip_skill_migration,
                skill_source,
                codex,
                force_codex,
            })
            .await?;
        }
        Command::AuthStatus => {
            let tokens = config::load_auth()?;
            println!(
                "Logged in. Account: {}. Expires at unix: {}",
                tokens.account_id.as_deref().unwrap_or("unknown"),
                tokens.expires_at
            );
        }
        Command::Usage {
            json,
            history,
            codex_home,
            since_days,
            max_files,
            output,
        } => {
            if history {
                let report = usage_history::scan_history(usage_history::HistoryOptions {
                    codex_home,
                    since_days,
                    max_files,
                })?;
                let rendered_json = serde_json::to_string_pretty(&report)?;
                if let Some(output) = output {
                    if let Some(parent) =
                        output.parent().filter(|path| !path.as_os_str().is_empty())
                    {
                        std::fs::create_dir_all(parent).map_err(|error| {
                            anyhow::anyhow!(
                                "could not create usage history output directory: {error}"
                            )
                        })?;
                    }
                    std::fs::write(&output, format!("{rendered_json}\n")).map_err(|error| {
                        anyhow::anyhow!("could not write usage history output: {error}")
                    })?;
                }
                if json {
                    println!("{rendered_json}");
                } else {
                    print!("{}", usage_history::render_human(&report));
                }
            } else {
                let auth = config::load_auth()?;
                let usage = account_usage::fetch_usage(&auth).await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&usage)?);
                } else {
                    print!("{}", account_usage::render_human(&usage));
                }
            }
        }
        Command::Chat {
            prompt,
            prompt_file,
            cwd,
            model,
            reasoning_effort,
            system_prompt,
            goal,
            goal_checkpoints,
            mode,
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
            let interactive = prompt_file.is_none() && prompt.is_empty() && goal.is_none();
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
            let auth = config::load_auth()?;
            let limits = model_context::resolve_input_limits(
                &auth,
                &model,
                compact_after_chars,
                compact_after_tokens,
                max_input_chars,
                max_input_tokens,
            )
            .await?;
            let explicit_session = session.is_some();
            let session_name =
                session.or_else(|| interactive.then(session::timestamp_session_name));
            let start_new_session = new_session || (interactive && !explicit_session);
            session::prepare_default_session_store(session_name.as_deref())?;
            let mut runner = agent::AgentRunner::new_with_reasoning_effort(
                auth,
                cwd.clone(),
                model,
                reasoning_effort,
                trace,
                profile,
                limits.compact_after_chars,
                compact_after_tool_only_turns,
                limits.max_input_chars,
                interactive,
                session_name.clone(),
                start_new_session,
                None,
                mode.into(),
            )?;
            runner.set_system_prompt(system_prompt);
            if let Some(name) = &session_name {
                if start_new_session {
                    runner.save_session_named(name)?;
                    println!("Started new session: {name}");
                } else if runner.load_session_named(name)? {
                    println!("Resumed session: {name}");
                }
            }
            for skill_name in requested_skills {
                skill::commands::load_skill_into_runner(&mut runner, &cwd, &skill_name, false)
                    .await?;
            }
            let goal_requested = goal.is_some();
            if let Some(goal) = goal {
                runner.set_goal(&goal)?;
                if let Some(name) = &session_name {
                    runner.save_session_named(name)?;
                }
            }
            if interactive {
                chat::run_interactive_chat(&mut runner, session_name, cwd).await?;
            } else {
                let prompt = prompt.unwrap_or_default();
                let has_prompt = !prompt.trim().is_empty();
                if !has_prompt && runner.goal().is_none() {
                    anyhow::bail!("prompt is required");
                }
                if has_prompt {
                    let prompt = if prompt.starts_with('/') {
                        prompt_commands::expand_slash_command(&cwd, &prompt)?.unwrap_or(prompt)
                    } else {
                        prompt
                    };
                    skill::commands::load_skill_mentions(&mut runner, &cwd, &prompt).await?;
                    runner.run(&prompt).await?;
                }
                if runner.goal().is_some() && (goal_requested || !has_prompt) {
                    let report = runner
                        .run_goal_checkpoints(
                            goal_checkpoints,
                            tokio_util::sync::CancellationToken::new(),
                        )
                        .await?;
                    println!(
                        "Goal checkpoints: {} status={}",
                        report.checkpoints_run,
                        report.status.name()
                    );
                }
                if let Some(name) = &session_name {
                    runner.save_session_named(name)?;
                    println!("Saved session: {name}");
                }
            }
        }
        Command::Tools => {
            println!("{}", serde_json::to_string_pretty(&tools::builtin_tools())?);
        }
        Command::McpServer => {
            mcp_server::run().await?;
        }
        Command::DesktopServer { stdio: true } => {
            desktop_server::run_stdio().await?;
        }
        Command::DesktopServer { stdio: false } => {
            anyhow::bail!("desktop-server requires --stdio");
        }
        Command::Brief {
            question,
            cwd,
            paths,
            format,
            reasoning_effort,
            trace,
            timeout_seconds,
        } => {
            let request = repo_brief::RepoBriefRequest {
                question,
                cwd: Some(cwd),
                paths,
                context: None,
                reasoning_effort: Some(reasoning_effort),
                trace,
            };
            let report = match repo_brief::run_standalone(request.clone(), timeout_seconds).await {
                Ok(report) => report,
                Err(error) => repo_brief::standalone_error_report(&request, &error),
            };
            match format {
                cli::RepoBriefFormat::Text => print!("{}", report.answer_markdown),
                cli::RepoBriefFormat::Json => println!("{}", serde_json::to_string(&report)?),
            }
            if report.status != repo_brief::RepoBriefStatus::Completed {
                if let Some(error) = &report.error {
                    eprintln!(
                        "repo brief {}: {error}",
                        serde_json::to_string(&report.status)?
                    );
                } else {
                    eprintln!("repo brief contract incomplete; see contract_diagnostic");
                }
                anyhow::bail!("repo brief completed with exit code {}", report.exit_code());
            }
        }
        Command::Sessions => {
            session::prepare_default_session_store(None)?;
            for session in session::store::SessionStore::open_default()?.list_names()? {
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
                for source in skill::registry::discover_sources(&cwd)? {
                    let skill =
                        skill::commands::compile_skill_cached(&runner, &cwd, &source.name, true)
                            .await?;
                    println!(
                        "{} - {} ({}, {} chars)",
                        skill.name, skill.description, skill.source_path, skill.full_text_chars
                    );
                }
            } else {
                for skill in skill::registry::list_status(&cwd)? {
                    println!(
                        "{} [{}] - {} ({})",
                        skill.name, skill.cache_status, skill.description, skill.source_path
                    );
                }
            }
        }
        Command::Commands {
            cwd,
            json,
            name,
            args,
        } => {
            let cwd = std::fs::canonicalize(&cwd)
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or(cwd));
            if let Some(name) = name {
                let prompt = prompt_commands::expand_command(&cwd, &name, &args.join(" "))?;
                println!("{prompt}");
            } else {
                let commands = prompt_commands::discover_commands(&cwd)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&commands)?);
                } else {
                    for command in commands {
                        if command.description.is_empty() {
                            println!("{} ({})", command.name, command.source_path);
                        } else {
                            println!(
                                "{} - {} ({})",
                                command.name, command.description, command.source_path
                            );
                        }
                    }
                }
            }
        }
        Command::SpinnerPreview => {
            spinner_preview::run()?;
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
            trace::cli::handle_traces(
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
        Command::TraceRetention {
            cwd,
            older_than_days,
            purge,
            confirm,
        } => {
            trace::cli::handle_trace_retention(cwd, older_than_days, purge, confirm)?;
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
                None => trace::commands::latest_trace_dir(&trace::commands::trace_runs_root(&cwd))?,
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
            let auth = config::load_auth()?;
            let limits = model_context::resolve_input_limits(
                &auth,
                &model,
                compact_after_chars,
                compact_after_tokens,
                max_input_chars,
                max_input_tokens,
            )
            .await?;
            profile::runner::run_profile_scenarios(
                &[scenario],
                profile::runner::ProfileRunOptions {
                    cwd,
                    model,
                    reasoning_effort: reasoning_effort.wire_value().to_string(),
                    target_tokens,
                    repeat,
                    no_trace,
                    no_profile,
                    compact_after_chars: limits.compact_after_chars,
                    compact_after_tool_only_turns,
                    max_input_chars: limits.max_input_chars,
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
            let auth = config::load_auth()?;
            let limits = model_context::resolve_input_limits(
                &auth,
                &model,
                compact_after_chars,
                compact_after_tokens,
                max_input_chars,
                max_input_tokens,
            )
            .await?;
            let scenarios = selected_benchmark_scenarios(suite, &scenarios)?;
            profile::runner::run_profile_scenarios(
                &scenarios,
                profile::runner::ProfileRunOptions {
                    cwd,
                    model,
                    reasoning_effort: reasoning_effort.wire_value().to_string(),
                    target_tokens,
                    repeat,
                    no_trace,
                    no_profile,
                    compact_after_chars: limits.compact_after_chars,
                    compact_after_tool_only_turns,
                    max_input_chars: limits.max_input_chars,
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
            run_manifests,
            output_dir,
        } => {
            let cwd = std::fs::canonicalize(&cwd)
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or(cwd));
            let output_dir = if output_dir.is_absolute() {
                output_dir
            } else {
                cwd.join(output_dir)
            };
            let report = benchmark::results::write_benchmark_report(
                benchmark::results::BenchmarkReportOptions {
                    cwd,
                    suite,
                    limit,
                    all_runs,
                    run_manifests,
                    output_dir,
                },
            )?;
            println!(
                "benchmark_report suite={} rows={} avg_completion={} avg_quality={} avg_execution_hygiene={} json={} csv={} html={}",
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
                    .get("average_execution_hygiene_score")
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
            let report = benchmark::codex_cli::run_codex_cli_benchmark(
                benchmark::codex_cli::CodexCliBenchmarkOptions {
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
            let average_score = report
                .aggregate
                .get("average_score")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);
            let comparable_runs = report
                .aggregate
                .get("comparable_runs")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(report.rows as u64);
            let request_failures = report
                .aggregate
                .pointer("/diagnostics/request_failure")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let request_failure_scenarios = json_count_map_summary(
                report
                    .aggregate
                    .pointer("/diagnostics/request_failure_scenarios"),
            )
            .unwrap_or_else(|| "n/a".to_string());
            let comparable_average_score = report
                .aggregate
                .get("comparable_average_score")
                .and_then(serde_json::Value::as_f64)
                .map(|score| format!("{score:.1}"))
                .unwrap_or_else(|| "n/a".to_string());
            println!(
                "codex_cli_benchmark suite={} rows={} comparable_runs={} request_failures={} request_failure_scenarios={} legacy_avg_score={average_score:.1} comparable_avg_score={} json={}",
                suite.name(),
                report.rows,
                comparable_runs,
                request_failures,
                request_failure_scenarios,
                comparable_average_score,
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
            let report = benchmark::opencode::run_opencode_benchmark(
                benchmark::opencode::OpencodeBenchmarkOptions {
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
            harness_report,
            harness_variant,
            baseline_runner,
            codex_cli_report,
            opencode_report,
            usage_history_report,
            llm_judge_report,
            group_by_reasoning,
            group_by_model,
            successful_only,
            fail_on_directional_comparison,
            output_dir,
        } => {
            ensure_benchmark_comparison_inputs(
                &harness_report,
                &harness_variant,
                &codex_cli_report,
                &opencode_report,
                &usage_history_report,
            )?;
            let harness_variants = parse_harness_variant_reports(&harness_variant)?;
            let cwd = std::fs::canonicalize(&cwd)
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or(cwd));
            let output_dir = if output_dir.is_absolute() {
                output_dir
            } else {
                cwd.join(output_dir)
            };
            let report = benchmark::results::write_benchmark_comparison(
                benchmark::results::BenchmarkComparisonOptions {
                    cwd,
                    suite,
                    limit,
                    all_runs,
                    harness_reports: harness_report,
                    harness_variants,
                    baseline_runner,
                    codex_cli_reports: codex_cli_report,
                    opencode_reports: opencode_report,
                    usage_history_reports: usage_history_report,
                    llm_judge_report,
                    group_by_reasoning,
                    group_by_model,
                    successful_only,
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
            if fail_on_directional_comparison {
                if let Some(message) =
                    benchmark::results::comparison_directional_failure_message(&report.aggregate)
                {
                    anyhow::bail!(
                        "{}; artifacts: json={} csv={} html={}",
                        message,
                        report.json_path.display(),
                        report.csv_path.display(),
                        report.html_path.display()
                    );
                }
            }
        }
        Command::BenchmarkJudge {
            comparison_report,
            cwd,
            model,
            codex_bin,
            reasoning_effort,
            timeout_seconds,
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
                benchmark::judge::write_llm_judge_report(benchmark::judge::BenchmarkJudgeOptions {
                    cwd,
                    comparison_report,
                    model,
                    codex_bin,
                    reasoning_effort: reasoning_effort.wire_value().to_string(),
                    timeout_seconds,
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

fn ensure_benchmark_comparison_inputs(
    harness_reports: &[std::path::PathBuf],
    harness_variants: &[String],
    codex_cli_reports: &[std::path::PathBuf],
    opencode_reports: &[std::path::PathBuf],
    usage_history_reports: &[std::path::PathBuf],
) -> Result<()> {
    if harness_reports.is_empty()
        && harness_variants.is_empty()
        && codex_cli_reports.is_empty()
        && opencode_reports.is_empty()
        && usage_history_reports.is_empty()
    {
        anyhow::bail!(
            "benchmark-compare requires at least one of --harness-report, --harness-variant, --codex-cli-report, --opencode-report, or --usage-history-report"
        );
    }
    Ok(())
}

fn parse_harness_variant_reports(
    values: &[String],
) -> Result<Vec<benchmark::results::HarnessVariantReport>> {
    values
        .iter()
        .map(|value| {
            let (label, path) = value.split_once('=').ok_or_else(|| {
                anyhow::anyhow!("invalid --harness-variant '{value}'; expected LABEL=REPORT")
            })?;
            let label = label.trim();
            let path = path.trim();
            if label.is_empty() || path.is_empty() {
                anyhow::bail!(
                    "invalid --harness-variant '{value}'; label and report path must be non-empty"
                );
            }
            Ok(benchmark::results::HarnessVariantReport {
                label: label.to_string(),
                path: std::path::PathBuf::from(path),
            })
        })
        .collect()
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

pub(crate) fn json_count_map_summary(value: Option<&serde_json::Value>) -> Option<String> {
    let object = value?.as_object()?;
    let mut parts = object
        .iter()
        .filter_map(|(key, value)| {
            let count = value.as_u64()?;
            (count > 0).then(|| format!("{key}:{count}"))
        })
        .collect::<Vec<_>>();
    parts.sort();
    (!parts.is_empty()).then(|| parts.join(","))
}
