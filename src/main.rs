mod agent;
mod auth;
mod client;
mod config;
mod profiler;
mod skills;
mod tools;

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};

const DEFAULT_MODEL: &str = "gpt-5.3-codex-spark";
const DEFAULT_COMPACT_AFTER_CHARS: usize = 160_000;
const DEFAULT_MAX_INPUT_CHARS: usize = 500_000;
const APPROX_CHARS_PER_TOKEN: usize = 4;
const DEFAULT_SCENARIO_TARGET_TOKENS: usize = 45_000;
const MAX_SCENARIO_TARGET_TOKENS: usize = 120_000;
const MAX_SCENARIO_REPEAT: usize = 50;

#[derive(Debug, Parser)]
#[command(name = "spark")]
#[command(about = "A small GPT-5.3 Codex Spark agent harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Sign in with ChatGPT/Codex OAuth and save tokens locally.
    Login {
        /// Do not open the browser automatically.
        #[arg(long)]
        no_browser: bool,
        /// Use Codex device-code auth instead of local browser callback.
        #[arg(long)]
        device: bool,
    },
    /// Show saved auth status.
    AuthStatus,
    /// Send one instruction to the Spark agent loop.
    Chat {
        prompt: Vec<String>,
        /// Read the prompt from a file instead of command-line args.
        #[arg(long)]
        prompt_file: Option<PathBuf>,
        /// Workspace root for filesystem and command tools.
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// Model slug to use.
        #[arg(long, default_value = DEFAULT_MODEL)]
        model: String,
        /// Maximum agent/tool turns. Omit to let Spark run until it completes.
        #[arg(long)]
        max_turns: Option<usize>,
        /// Save raw request/response JSON under .spark-runs/.
        #[arg(long)]
        trace: bool,
        /// Print a compact profiling summary after each completed prompt.
        #[arg(long)]
        profile: bool,
        /// Named session to resume/save under ~/.spark-codex/sessions.
        #[arg(long)]
        session: Option<String>,
        /// Load a compiled skill into the conversation before the prompt.
        #[arg(long = "skill")]
        skills: Vec<String>,
        /// Start the named session from an empty history, replacing any saved state after the next save.
        #[arg(long)]
        new_session: bool,
        /// Compact older tool outputs once request JSON exceeds this many characters.
        #[arg(long)]
        compact_after_chars: Option<usize>,
        /// Compact older tool outputs once estimated input exceeds this many tokens.
        #[arg(long)]
        compact_after_tokens: Option<usize>,
        /// Refuse to send request JSON above this many characters.
        #[arg(long)]
        max_input_chars: Option<usize>,
        /// Refuse to send a request once estimated input exceeds this many tokens.
        #[arg(long)]
        max_input_tokens: Option<usize>,
    },
    /// Print available built-in tools as JSON.
    Tools,
    /// List saved chat sessions.
    Sessions,
    /// List or refresh repo-local Spark skill cache.
    Skills {
        /// Rebuild cached summaries from .agents/skills.
        #[arg(long)]
        refresh: bool,
    },
    /// List saved trace runs under .spark-runs/.
    Traces {
        /// Maximum number of trace directories to print.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Print one compact profile row per trace.
        #[arg(long)]
        summary: bool,
        /// Only include traces for a profile scenario name.
        #[arg(long)]
        scenario: Option<String>,
        /// Only include traces that contain this diagnostic kind. Repeat to require multiple kinds.
        #[arg(long = "diagnostic")]
        diagnostics: Vec<String>,
        /// Print an aggregate row for matching trace summaries.
        #[arg(long)]
        aggregate: bool,
        /// Print matching analyzed traces as one JSON object.
        #[arg(long)]
        json: bool,
        /// Print matching analyzed traces as one JSON object per line.
        #[arg(long)]
        jsonl: bool,
    },
    /// Summarize a .spark-runs/run-* trace for repeated tool calls and compaction behavior.
    AnalyzeTrace {
        /// Trace directory to analyze. Defaults to the latest .spark-runs/run-* directory.
        dir: Option<PathBuf>,
        /// Analyze the latest .spark-runs/run-* directory.
        #[arg(long)]
        latest: bool,
        /// Print a compact human-readable timeline instead of full JSON.
        #[arg(long)]
        timeline: bool,
    },
    /// Run a repeatable Spark profiling scenario through the real agent loop.
    ProfileScenario {
        /// Scenario to run.
        #[arg(value_enum)]
        scenario: ProfileScenarioKind,
        /// Workspace root for filesystem and command tools.
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// Model slug to use.
        #[arg(long, default_value = DEFAULT_MODEL)]
        model: String,
        /// Maximum agent/tool turns. Omit to let Spark run until it completes.
        #[arg(long)]
        max_turns: Option<usize>,
        /// Target prompt size for long-context scenarios, in approximate tokens.
        #[arg(long, default_value_t = DEFAULT_SCENARIO_TARGET_TOKENS)]
        target_tokens: usize,
        /// Run the scenario this many times and aggregate the traces from this invocation.
        #[arg(long, default_value_t = 1)]
        repeat: usize,
        /// Disable trace files for this scenario.
        #[arg(long)]
        no_trace: bool,
        /// Disable printed profile JSON for this scenario.
        #[arg(long)]
        no_profile: bool,
        /// Compact older context once request JSON exceeds this many characters.
        #[arg(long)]
        compact_after_chars: Option<usize>,
        /// Compact older context once estimated input exceeds this many tokens.
        #[arg(long)]
        compact_after_tokens: Option<usize>,
        /// Refuse to send request JSON above this many characters.
        #[arg(long)]
        max_input_chars: Option<usize>,
        /// Refuse to send a request once estimated input exceeds this many tokens.
        #[arg(long)]
        max_input_tokens: Option<usize>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProfileScenarioKind {
    /// Small repo survey that usually exercises read/list/search without edits.
    RepoSurvey,
    /// Multi-turn conversation that crosses auto-compaction pressure naturally.
    NaturalCompaction,
    /// Long prompt that crosses compaction pressure while staying below 128k tokens.
    CompactionPressure,
    /// Scratch-file coding task that exercises read, edit, write, and verification tools.
    FileEdit,
    /// Scratch-file workflow that exercises write, rename, search, and verification tools.
    FileOps,
    /// Scratch-file task that intentionally exercises native tool failure and recovery.
    ToolRecovery,
    /// Repo-local skill mention task that exercises automatic skill compile/load.
    SkillUse,
}

impl ProfileScenarioKind {
    fn name(self) -> &'static str {
        match self {
            Self::RepoSurvey => "repo-survey",
            Self::NaturalCompaction => "natural-compaction",
            Self::CompactionPressure => "compaction-pressure",
            Self::FileEdit => "file-edit",
            Self::FileOps => "file-ops",
            Self::ToolRecovery => "tool-recovery",
            Self::SkillUse => "skill-use",
        }
    }
}

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
            max_turns,
            trace,
            profile,
            session,
            skills: requested_skills,
            new_session,
            compact_after_chars,
            compact_after_tokens,
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
            let compact_after_chars = resolve_char_threshold(
                "compact-after",
                compact_after_chars,
                compact_after_tokens,
                DEFAULT_COMPACT_AFTER_CHARS,
            )?;
            let max_input_chars = resolve_char_threshold(
                "max-input",
                max_input_chars,
                max_input_tokens,
                DEFAULT_MAX_INPUT_CHARS,
            )?;
            let session_name = session.or_else(|| interactive.then(|| "default".to_string()));
            let auth = config::load_auth()?;
            let mut runner = agent::AgentRunner::new(
                auth,
                cwd.clone(),
                model,
                max_turns,
                trace,
                profile,
                compact_after_chars,
                max_input_chars,
                interactive,
                session_name.clone(),
                new_session,
                None,
            )?;
            let session_path = session_name
                .as_deref()
                .map(config::session_path)
                .transpose()?;
            if let Some(path) = &session_path {
                if new_session {
                    runner.save_session(path)?;
                    println!("Started new session: {}", path.display());
                } else if runner.load_session(path)? {
                    println!("Resumed session: {}", path.display());
                }
            }
            for skill_name in requested_skills {
                load_skill_into_runner(&mut runner, &cwd, &skill_name, false).await?;
            }
            if interactive {
                run_interactive_chat(&mut runner, session_path, cwd).await?;
            } else {
                let prompt = prompt.unwrap_or_default();
                if prompt.trim().is_empty() {
                    anyhow::bail!("prompt is required");
                }
                load_skill_mentions(&mut runner, &cwd, &prompt).await?;
                runner.run(&prompt).await?;
                if let Some(path) = &session_path {
                    runner.save_session(path)?;
                    println!("Saved session: {}", path.display());
                }
            }
        }
        Command::Tools => {
            println!("{}", serde_json::to_string_pretty(&tools::builtin_tools())?);
        }
        Command::Sessions => {
            for session in config::list_sessions()? {
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
                    DEFAULT_MAX_INPUT_CHARS,
                    false,
                    None,
                    false,
                    None,
                )?;
                for source in skills::discover_sources(&cwd)? {
                    let skill = compile_skill_cached(&runner, &cwd, &source.name, true).await?;
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
            json,
            jsonl,
        } => {
            if json && jsonl {
                anyhow::bail!("pass either --json or --jsonl, not both");
            }
            let cwd = std::fs::canonicalize(".").unwrap_or_else(|_| PathBuf::from("."));
            let mut matching_summaries = Vec::new();
            let mut json_records = Vec::new();
            let analyze = summary
                || scenario.is_some()
                || !diagnostics.is_empty()
                || aggregate
                || json
                || jsonl;
            for run in list_trace_dirs(&trace_runs_root(&cwd), limit)? {
                let display = display_trace_dir(&cwd, &run);
                let trace_summary = if analyze {
                    Some(profiler::analyze_trace(&run)?)
                } else {
                    None
                };
                if let Some(scenario) = scenario.as_deref()
                    && trace_summary
                        .as_ref()
                        .and_then(profiler::trace_profile_scenario_name)
                        != Some(scenario)
                {
                    continue;
                }
                if !diagnostics.is_empty()
                    && !trace_has_all_diagnostics(
                        trace_summary.as_ref().expect("summary loaded"),
                        &diagnostics,
                    )
                {
                    continue;
                }
                if let Some(trace_summary) = &trace_summary {
                    matching_summaries.push(trace_summary.clone());
                }
                if json || jsonl {
                    let record = trace_export_record(&cwd, &run, &display, trace_summary.as_ref());
                    if jsonl {
                        println!("{}", serde_json::to_string(&record)?);
                    } else {
                        json_records.push(record);
                    }
                    continue;
                }
                if summary {
                    let trace_summary = trace_summary.expect("summary loaded");
                    println!(
                        "{}",
                        profiler::format_trace_summary_row(
                            &display.display().to_string(),
                            &trace_summary
                        )
                    );
                } else {
                    println!("{}", display.display());
                }
            }
            if json {
                let output = json!({
                    "filter": {
                        "scenario": scenario,
                        "diagnostics": diagnostics,
                        "limit": limit,
                    },
                    "runs": json_records,
                    "aggregate": aggregate.then(|| {
                        profiler::trace_aggregate_json(
                            trace_filter_label(scenario.as_deref(), &diagnostics).as_str(),
                            &matching_summaries,
                        )
                    }),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            if aggregate {
                if jsonl {
                    let record = json!({
                        "type": "aggregate",
                        "aggregate": profiler::trace_aggregate_json(
                            trace_filter_label(scenario.as_deref(), &diagnostics).as_str(),
                            &matching_summaries,
                        ),
                    });
                    println!("{}", serde_json::to_string(&record)?);
                } else if !json {
                    println!(
                        "{}",
                        profiler::format_trace_aggregate_row(
                            trace_filter_label(scenario.as_deref(), &diagnostics).as_str(),
                            &matching_summaries,
                        )
                    );
                }
            }
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
                None => latest_trace_dir(&trace_runs_root(&cwd))?,
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
            max_input_chars,
            max_input_tokens,
        } => {
            let cwd = std::fs::canonicalize(&cwd)
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or(cwd));
            let compact_after_chars = resolve_char_threshold(
                "compact-after",
                compact_after_chars,
                compact_after_tokens,
                DEFAULT_COMPACT_AFTER_CHARS,
            )?;
            let max_input_chars = resolve_char_threshold(
                "max-input",
                max_input_chars,
                max_input_tokens,
                DEFAULT_MAX_INPUT_CHARS,
            )?;
            validate_scenario_repeat(repeat)?;
            let prompts = profile_scenario_prompts(scenario, target_tokens)?;
            let total_prompt_chars = prompts.iter().map(String::len).sum::<usize>();
            println!(
                "scenario={:?} repeat={} prompts={} prompt_chars={} approx_tokens={} compact_after_chars={} max_input_chars={}",
                scenario,
                repeat,
                prompts.len(),
                total_prompt_chars,
                total_prompt_chars / APPROX_CHARS_PER_TOKEN,
                compact_after_chars,
                max_input_chars
            );
            let auth = config::load_auth()?;
            let mut summaries = Vec::new();
            let mut run_result = Ok(());
            for repeat_index in 1..=repeat {
                prepare_profile_scenario(&cwd, scenario)?;
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
                            "expected_tool_groups": profile_scenario_expected_tool_groups(scenario),
                            "expected_tool_calls": profile_scenario_expected_tool_calls(scenario),
                            "expected_skills": profile_scenario_expected_skills(scenario),
                        }
                    })),
                )?;
                for (index, prompt) in prompts.iter().enumerate() {
                    println!(
                        "scenario_turn={}/{} prompt_chars={} approx_tokens={}",
                        index + 1,
                        prompts.len(),
                        prompt.len(),
                        prompt.len() / APPROX_CHARS_PER_TOKEN
                    );
                    load_skill_mentions(&mut runner, &cwd, prompt).await?;
                    if let Err(error) = runner.run(prompt).await {
                        run_result = Err(error);
                        break;
                    }
                }
                if !no_trace {
                    match latest_trace_dir(&trace_runs_root(&cwd)).and_then(|latest| {
                        let summary = profiler::analyze_trace(&latest)?;
                        Ok((latest, summary))
                    }) {
                        Ok((latest, summary)) => {
                            println!(
                                "{}",
                                profiler::format_trace_summary_row(
                                    &display_trace_dir(&cwd, &latest).display().to_string(),
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

async fn run_interactive_chat(
    runner: &mut agent::AgentRunner,
    mut session_path: Option<PathBuf>,
    cwd: PathBuf,
) -> Result<()> {
    if let Some(path) = &session_path {
        println!("Spark interactive chat. Session: {}", path.display());
    } else {
        println!("Spark interactive chat.");
    }
    println!("Type /help for commands, /exit to quit.");
    let stdin = std::io::stdin();
    loop {
        print!("spark> ");
        std::io::stdout().flush()?;
        let mut line = String::new();
        let bytes = stdin.read_line(&mut line)?;
        if bytes == 0 {
            println!();
            return Ok(());
        }
        let input = line.trim().trim_start_matches('\u{feff}');
        if input.is_empty() {
            continue;
        }

        if let Some(command) = command_args(input, "/session") {
            handle_session_command(runner, &mut session_path, command.trim())?;
            continue;
        }

        if let Some(command) = command_args(input, "/new") {
            handle_new_session_command(runner, &mut session_path, command.trim())?;
            continue;
        }

        if input == "/skills" {
            handle_skill_command(runner, &cwd, "list").await?;
            continue;
        }

        if let Some(command) = command_args(input, "/skill") {
            handle_skill_command(runner, &cwd, command.trim()).await?;
            if let Some(path) = &session_path {
                runner.save_session(path)?;
            }
            continue;
        }

        match input {
            "/exit" | "/quit" => return Ok(()),
            "/help" => {
                println!(
                    "Commands: /help, /status, /profile, /compact, /session, /new, /skill, /skills, /save, /clear, /exit"
                );
                println!(
                    "Session commands: /session, /session list, /session open <name>, /session new <name>, /session use <name>, /session rename [old] <new>, /session delete <name>"
                );
                println!(
                    "Skill commands: /skills, /skill load <name>, /skill refresh, /skill list"
                );
                continue;
            }
            "/status" => {
                println!("conversation input JSON chars: {}", runner.input_chars()?);
                println!("{}", runner.profile_status());
                continue;
            }
            "/profile" => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&runner.profile_summary())?
                );
                continue;
            }
            "/compact" => {
                match runner.compact_now().await {
                    Ok(Some(report)) => {
                        println!("{}", serde_json::to_string_pretty(&report)?);
                        println!("conversation input JSON chars: {}", runner.input_chars()?);
                        if let Some(path) = &session_path {
                            runner.save_session(path)?;
                        }
                    }
                    Ok(None) => {
                        println!("nothing to compact");
                    }
                    Err(error) => {
                        eprintln!("error: {error:#}");
                    }
                }
                continue;
            }
            "/clear" => {
                runner.clear_conversation();
                if let Some(path) = &session_path {
                    runner.save_session(path)?;
                }
                println!("conversation cleared");
                continue;
            }
            "/save" => {
                if let Some(path) = &session_path {
                    runner.save_session(path)?;
                    println!("saved session: {}", path.display());
                } else {
                    println!("no session configured; start with --session <name>");
                }
                continue;
            }
            _ => {}
        }

        let mut save_after_run = false;
        if let Err(error) = load_skill_mentions(runner, &cwd, input).await {
            eprintln!("error: {error:#}");
        } else if let Err(error) = runner.run(input).await {
            eprintln!("error: {error:#}");
            save_after_run = true;
        } else {
            save_after_run = true;
        }

        if save_after_run && let Some(path) = &session_path {
            runner.save_session(path)?;
        }
    }
}

fn command_args<'a>(input: &'a str, command: &str) -> Option<&'a str> {
    if input == command {
        return Some("");
    }
    input
        .strip_prefix(command)
        .and_then(|rest| rest.strip_prefix(char::is_whitespace))
}

fn handle_session_command(
    runner: &mut agent::AgentRunner,
    session_path: &mut Option<PathBuf>,
    command: &str,
) -> Result<()> {
    let mut parts = command.split_whitespace();
    let action = parts.next();
    match action {
        None => {
            print_session_status(runner, session_path)?;
        }
        Some("list") => {
            for session in config::list_sessions()? {
                println!("{session}");
            }
        }
        Some("save") => {
            save_current_session(runner, session_path)?;
        }
        Some("open" | "switch") => {
            let name = required_session_arg(parts.next(), "open")?;
            let target = config::session_path(name)?;
            if !target.exists() {
                anyhow::bail!("session `{name}` does not exist");
            }
            switch_session(runner, session_path, target, /*load_existing*/ true)?;
        }
        Some("new") => {
            let name = parts
                .next()
                .map(str::to_string)
                .unwrap_or_else(timestamp_session_name);
            let target = config::session_path(&name)?;
            switch_session(runner, session_path, target, /*load_existing*/ false)?;
        }
        Some("use") => {
            let name = required_session_arg(parts.next(), "use")?;
            let target = config::session_path(name)?;
            switch_session(
                runner,
                session_path,
                target.clone(),
                /*load_existing*/ target.exists(),
            )?;
        }
        Some("rename" | "mv") => {
            let first = required_session_arg(parts.next(), "rename")?;
            let second = parts.next();
            rename_session(runner, session_path, first, second)?;
        }
        Some("delete" | "rm") => {
            let name = required_session_arg(parts.next(), "delete")?;
            delete_session(session_path, name)?;
        }
        Some(name) => {
            let target = config::session_path(name)?;
            switch_session(
                runner,
                session_path,
                target.clone(),
                /*load_existing*/ target.exists(),
            )?;
        }
    }
    Ok(())
}

fn handle_new_session_command(
    runner: &mut agent::AgentRunner,
    session_path: &mut Option<PathBuf>,
    command: &str,
) -> Result<()> {
    let name = command
        .split_whitespace()
        .next()
        .map(str::to_string)
        .unwrap_or_else(timestamp_session_name);
    let target = config::session_path(&name)?;
    switch_session(runner, session_path, target, /*load_existing*/ false)
}

fn switch_session(
    runner: &mut agent::AgentRunner,
    session_path: &mut Option<PathBuf>,
    target: PathBuf,
    load_existing: bool,
) -> Result<()> {
    if let Some(current) = session_path.as_ref() {
        runner.save_session(current)?;
    }
    if load_existing {
        runner.load_session(&target)?;
        println!("opened session: {}", target.display());
    } else {
        runner.clear_conversation();
        runner.save_session(&target)?;
        println!("new session: {}", target.display());
    }
    *session_path = Some(target);
    Ok(())
}

fn rename_session(
    runner: &agent::AgentRunner,
    session_path: &mut Option<PathBuf>,
    first: &str,
    second: Option<&str>,
) -> Result<()> {
    let (source, new_name) = match second {
        Some(new_name) => (config::session_path(first)?, new_name),
        None => {
            let Some(current) = session_path.as_ref() else {
                anyhow::bail!("/session rename <new> requires an active session");
            };
            (current.clone(), first)
        }
    };
    if !source.exists() {
        anyhow::bail!(
            "session `{}` does not exist",
            session_name_for_display(&source)
        );
    }
    let target = config::session_path(new_name)?;
    if target.exists() {
        anyhow::bail!("session `{new_name}` already exists");
    }
    if is_active_session(session_path, &source) {
        runner.save_session(&source)?;
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| anyhow::anyhow!("failed to create {}: {error}", parent.display()))?;
    }
    std::fs::rename(&source, &target).map_err(|error| {
        anyhow::anyhow!(
            "failed to rename {} to {}: {error}",
            source.display(),
            target.display()
        )
    })?;
    if is_active_session(session_path, &source) {
        *session_path = Some(target.clone());
    }
    println!(
        "renamed session: {} -> {}",
        source.display(),
        target.display()
    );
    Ok(())
}

fn delete_session(session_path: &Option<PathBuf>, name: &str) -> Result<()> {
    let target = config::session_path(name)?;
    if is_active_session(session_path, &target) {
        anyhow::bail!("cannot delete the active session; switch or start /new first");
    }
    if !target.exists() {
        anyhow::bail!("session `{name}` does not exist");
    }
    std::fs::remove_file(&target)
        .map_err(|error| anyhow::anyhow!("failed to delete {}: {error}", target.display()))?;
    println!("deleted session: {}", target.display());
    Ok(())
}

fn is_active_session(session_path: &Option<PathBuf>, target: &Path) -> bool {
    session_path
        .as_ref()
        .is_some_and(|active| normalize_session_path(active) == normalize_session_path(target))
}

fn normalize_session_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn session_name_for_display(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("<unknown>")
        .to_string()
}

fn save_current_session(runner: &agent::AgentRunner, session_path: &Option<PathBuf>) -> Result<()> {
    let Some(path) = session_path else {
        println!("no session configured; use /session new <name>");
        return Ok(());
    };
    runner.save_session(path)?;
    println!("saved session: {}", path.display());
    Ok(())
}

fn print_session_status(runner: &agent::AgentRunner, session_path: &Option<PathBuf>) -> Result<()> {
    if let Some(path) = session_path {
        println!("session: {}", path.display());
    } else {
        println!("session: none");
    }
    println!("conversation input JSON chars: {}", runner.input_chars()?);
    println!("{}", runner.profile_status());
    Ok(())
}

fn required_session_arg<'a>(arg: Option<&'a str>, command: &str) -> Result<&'a str> {
    arg.ok_or_else(|| anyhow::anyhow!("/session {command} requires a session name"))
}

fn timestamp_session_name() -> String {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("chat-{now_secs}")
}

fn prepare_profile_scenario(cwd: &Path, scenario: ProfileScenarioKind) -> Result<()> {
    let Some(name) = (match scenario {
        ProfileScenarioKind::FileEdit => Some("file-edit"),
        ProfileScenarioKind::FileOps => Some("file-ops"),
        ProfileScenarioKind::ToolRecovery => Some("tool-recovery"),
        _ => None,
    }) else {
        return Ok(());
    };

    let dir = cwd.join(".spark-scenarios").join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|error| anyhow::anyhow!("failed to reset {}: {error}", dir.display()))?;
    }
    std::fs::create_dir_all(&dir)
        .map_err(|error| anyhow::anyhow!("failed to create {}: {error}", dir.display()))?;
    match scenario {
        ProfileScenarioKind::FileEdit => {
            std::fs::write(
                dir.join("notes.md"),
                "# Spark File Edit Fixture\n\n- status: draft\n- owner: spark\n\nTODO: replace this line with a concise final note.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture notes.md: {error}"))?;
            std::fs::write(
                dir.join("config.toml"),
                "name = \"spark-fixture\"\nmode = \"draft\"\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture config.toml: {error}"))?;
        }
        ProfileScenarioKind::FileOps => {
            std::fs::create_dir_all(dir.join("drafts"))
                .map_err(|error| anyhow::anyhow!("failed to create drafts fixture: {error}"))?;
            std::fs::write(
                dir.join("manifest.txt"),
                "file-ops fixture\nexpected_final=final/report.md\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture manifest.txt: {error}"))?;
        }
        ProfileScenarioKind::ToolRecovery => {
            std::fs::create_dir_all(dir.join("source"))
                .map_err(|error| anyhow::anyhow!("failed to create source fixture: {error}"))?;
            std::fs::write(
                dir.join("source").join("note.md"),
                "# Recovery Fixture\n\nSpark recovery path verified.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture source/note.md: {error}"))?;
        }
        _ => {}
    }
    Ok(())
}

fn profile_scenario_prompts(
    scenario: ProfileScenarioKind,
    target_tokens: usize,
) -> Result<Vec<String>> {
    if target_tokens == 0 {
        anyhow::bail!("--target-tokens must be greater than 0");
    }
    if target_tokens > MAX_SCENARIO_TARGET_TOKENS {
        anyhow::bail!(
            "--target-tokens must be <= {MAX_SCENARIO_TARGET_TOKENS} so the prompt stays below Spark's 128k context window with JSON overhead"
        );
    }

    match scenario {
        ProfileScenarioKind::RepoSurvey => Ok(vec![
            "Profile scenario: repo-survey.\n\
             Inspect this repository like a coding agent. Use targeted native tools, not broad command output.\n\
             1. List the repository root.\n\
             2. Read Cargo.toml and README.md with bounded windows.\n\
             3. Search src for tool and compaction surfaces.\n\
             4. Finish with a concise harness-risk summary and one next profiling recommendation."
                .to_string(),
        ]),
        ProfileScenarioKind::FileEdit => Ok(vec![
            "Profile scenario: file-edit.\n\
             Work only under .spark-scenarios/file-edit.\n\
             Use native file tools, not cmd.exec, unless verification cannot be done otherwise.\n\
             Required actions:\n\
             1. Use fs.read on .spark-scenarios/file-edit/notes.md.\n\
             2. Use fs.edit or fs.replace on .spark-scenarios/file-edit/notes.md to replace the TODO line with: Final note: Spark edited this fixture with native tools.\n\
             3. Use fs.write on .spark-scenarios/file-edit/summary.txt with a one-line summary of what changed.\n\
             4. Use fs.read on both changed files to verify the final contents.\n\
             Finish with the tools used, whether verification passed, and any harness behavior that made the task easier or harder."
                .to_string(),
        ]),
        ProfileScenarioKind::FileOps => Ok(vec![
            "Profile scenario: file-ops.\n\
             Work only under .spark-scenarios/file-ops.\n\
             Use native file tools, not cmd.exec, unless verification cannot be done otherwise.\n\
             Required actions:\n\
             1. Use fs.write on .spark-scenarios/file-ops/drafts/report-draft.md with a short markdown report containing the exact phrase: Spark rename path verified.\n\
             2. Use fs.rename to move .spark-scenarios/file-ops/drafts/report-draft.md to .spark-scenarios/file-ops/final/report.md.\n\
             3. Use fs.stat on .spark-scenarios/file-ops/final/report.md to verify the final path exists before reading it.\n\
             4. Use fs.read on .spark-scenarios/file-ops/final/report.md to verify the final contents.\n\
             5. Use fs.search under .spark-scenarios/file-ops for Spark rename path verified.\n\
             Finish with the native tools used, whether verification passed, and any harness behavior that made the workflow easier or harder."
                .to_string(),
        ]),
        ProfileScenarioKind::ToolRecovery => Ok(vec![
            "Profile scenario: tool-recovery.\n\
             Work only under .spark-scenarios/tool-recovery.\n\
             Use native file tools, not cmd.exec.\n\
             Required actions:\n\
             1. First use fs.read on .spark-scenarios/tool-recovery/source/missing-note.md. This path is intentionally missing; do not skip this failing probe.\n\
             2. Use fs.stat on .spark-scenarios/tool-recovery/source/note.md to verify the corrected path exists after the failed observation.\n\
             3. Use fs.read on .spark-scenarios/tool-recovery/source/note.md to verify it contains: Spark recovery path verified.\n\
             4. Use fs.write on .spark-scenarios/tool-recovery/recovery-summary.txt with one line naming whether native-tool recovery succeeded.\n\
             Finish with the native tools used, whether recovery passed, and whether the harness observation made the correction clear."
                .to_string(),
        ]),
        ProfileScenarioKind::SkillUse => Ok(vec![
            "Profile scenario: skill-use.\n\
             Load and apply @rust-patterns before answering.\n\
             Use native file tools, not cmd.exec, unless verification cannot be done otherwise.\n\
             Required actions:\n\
             1. Use fs.read on src/main.rs with a bounded window.\n\
             2. Use fs.search under src for load_skill_mentions.\n\
             Finish with two concise Rust harness risks or cleanup opportunities, and mention whether the loaded skill guidance affected your review."
                .to_string(),
        ]),
        ProfileScenarioKind::NaturalCompaction => natural_compaction_scenario_prompts(target_tokens),
        ProfileScenarioKind::CompactionPressure => {
            let target_chars = target_tokens.saturating_mul(APPROX_CHARS_PER_TOKEN);
            let mut prompt = String::from(
                "Profile scenario: compaction-pressure.\n\
                 This prompt intentionally creates long-context pressure below Spark's 128k context window.\n\
                 Let the harness compact automatically if its threshold is crossed.\n\
                 Do not restate the synthetic payload. After any compaction, use fs.list on src with recursive=false, then answer with:\n\
                 - whether the task remained understandable,\n\
                 - which tool you used,\n\
                 - any missing information caused by compaction,\n\
                 - one concrete harness change that would make this scenario more reliable.\n\n\
                 Synthetic payload follows. Preserve the high-level instruction above; payload rows are intentionally repetitive profiling filler.\n",
            );
            let mut row = 0usize;
            while prompt.len() < target_chars {
                row += 1;
                prompt.push_str(&format!(
                    "row {row:05}: spark compaction profiling filler; keep task intent, discard repetition, prefer native tools over shell floods, report uncertainty plainly.\n"
                ));
            }
            Ok(vec![prompt])
        }
    }
}

fn natural_compaction_scenario_prompts(target_tokens: usize) -> Result<Vec<String>> {
    let turn_count = 3usize;
    let target_chars = target_tokens.saturating_mul(APPROX_CHARS_PER_TOKEN);
    let target_chars_per_turn = target_chars.div_ceil(turn_count);
    let mut prompts = Vec::with_capacity(turn_count);

    for turn in 1..=turn_count {
        let mut prompt = format!(
            "Profile scenario: natural-compaction turn {turn}/{turn_count}.\n\
             This is a scripted multi-turn chat profiling run. Treat each message as normal conversation history and do not restate the filler rows.\n"
        );
        match turn {
            1 => prompt.push_str(
                "Answer with one sentence confirming you are tracking the harness context pressure.\n",
            ),
            2 => prompt.push_str(
                "Answer with one sentence naming one risk signal you would watch in the trace.\n",
            ),
            _ => prompt.push_str(
                "After the harness has a chance to compact naturally, use fs.list on src with recursive=false, then answer with whether compaction preserved the task intent and whether any required information was missing.\n",
            ),
        }

        let mut row = 0usize;
        while prompt.len() < target_chars_per_turn {
            row += 1;
            prompt.push_str(&format!(
                "turn {turn} row {row:05}: conversational long-context filler; retain the current turn objective, discard repetition, prefer native tools after compaction, and report uncertainty plainly.\n"
            ));
        }
        prompts.push(prompt);
    }

    Ok(prompts)
}

fn profile_scenario_expected_tool_groups(scenario: ProfileScenarioKind) -> Vec<Vec<&'static str>> {
    match scenario {
        ProfileScenarioKind::RepoSurvey => {
            vec![vec!["fs.list"], vec!["fs.read"], vec!["fs.search"]]
        }
        ProfileScenarioKind::NaturalCompaction | ProfileScenarioKind::CompactionPressure => {
            vec![vec!["fs.list"]]
        }
        ProfileScenarioKind::FileEdit => vec![
            vec!["fs.read"],
            vec!["fs.edit", "fs.replace"],
            vec!["fs.write"],
        ],
        ProfileScenarioKind::FileOps => {
            vec![
                vec!["fs.write"],
                vec!["fs.rename"],
                vec!["fs.stat"],
                vec!["fs.read"],
                vec!["fs.search"],
            ]
        }
        ProfileScenarioKind::ToolRecovery => {
            vec![vec!["fs.read"], vec!["fs.stat"], vec!["fs.write"]]
        }
        ProfileScenarioKind::SkillUse => vec![vec!["fs.read"], vec!["fs.search"]],
    }
}

fn profile_scenario_expected_tool_calls(scenario: ProfileScenarioKind) -> Vec<Value> {
    match scenario {
        ProfileScenarioKind::RepoSurvey => vec![],
        ProfileScenarioKind::NaturalCompaction | ProfileScenarioKind::CompactionPressure => {
            vec![json!({
                "tool": "fs.list",
                "path": "src",
                "recursive": false,
            })]
        }
        ProfileScenarioKind::FileEdit => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/file-edit/notes.md",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/file-edit/summary.txt",
            }),
        ],
        ProfileScenarioKind::FileOps => vec![
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/file-ops/drafts/report-draft.md",
            }),
            json!({
                "tool": "fs.rename",
                "from": ".spark-scenarios/file-ops/drafts/report-draft.md",
                "to": ".spark-scenarios/file-ops/final/report.md",
            }),
            json!({
                "tool": "fs.stat",
                "path": ".spark-scenarios/file-ops/final/report.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/file-ops/final/report.md",
            }),
            json!({
                "tool": "fs.search",
                "path": ".spark-scenarios/file-ops",
            }),
        ],
        ProfileScenarioKind::ToolRecovery => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/tool-recovery/source/missing-note.md",
            }),
            json!({
                "tool": "fs.stat",
                "path": ".spark-scenarios/tool-recovery/source/note.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/tool-recovery/source/note.md",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/tool-recovery/recovery-summary.txt",
            }),
        ],
        ProfileScenarioKind::SkillUse => vec![
            json!({
                "tool": "fs.read",
                "path": "src/main.rs",
            }),
            json!({
                "tool": "fs.search",
                "path": "src",
            }),
        ],
    }
}

fn profile_scenario_expected_skills(scenario: ProfileScenarioKind) -> Vec<&'static str> {
    match scenario {
        ProfileScenarioKind::SkillUse => vec!["rust-patterns"],
        _ => vec![],
    }
}

async fn handle_skill_command(
    runner: &mut agent::AgentRunner,
    cwd: &PathBuf,
    command: &str,
) -> Result<()> {
    let mut parts = command.split_whitespace();
    match parts.next() {
        None | Some("list") => {
            for skill in skills::list_status(cwd)? {
                let loaded = if runner.loaded_skills().contains(&skill.name) {
                    " loaded"
                } else {
                    ""
                };
                println!(
                    "{}{} [{}] - {}",
                    skill.name, loaded, skill.cache_status, skill.description
                );
            }
        }
        Some("refresh") => {
            let mut refreshed = 0usize;
            for source in skills::discover_sources(cwd)? {
                compile_skill_cached(runner, cwd, &source.name, true).await?;
                refreshed += 1;
            }
            println!("refreshed {refreshed} skill(s)");
        }
        Some("load") => {
            let name = parts
                .next()
                .ok_or_else(|| anyhow::anyhow!("/skill load requires a skill name"))?;
            load_skill_into_runner(runner, cwd, name, false).await?;
        }
        Some(name) => {
            load_skill_into_runner(runner, cwd, name, false).await?;
        }
    }
    Ok(())
}

async fn load_skill_into_runner(
    runner: &mut agent::AgentRunner,
    cwd: &PathBuf,
    name: &str,
    refresh: bool,
) -> Result<()> {
    let skill = compile_skill_cached(runner, cwd, name, refresh).await?;
    if runner.load_skill_context(&skill.name, &skill.summary) {
        println!("loaded skill: {}", skill.name);
    } else {
        println!("skill already loaded: {}", skill.name);
    }
    Ok(())
}

async fn load_skill_mentions(
    runner: &mut agent::AgentRunner,
    cwd: &PathBuf,
    text: &str,
) -> Result<Vec<String>> {
    let mut loaded = Vec::new();
    for name in mentioned_skill_names(cwd, text)? {
        let already_loaded = runner.loaded_skills().contains(&name);
        load_skill_into_runner(runner, cwd, &name, false).await?;
        if !already_loaded {
            loaded.push(name);
        }
    }
    Ok(loaded)
}

fn mentioned_skill_names(cwd: &PathBuf, text: &str) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for source in skills::discover_sources(cwd)? {
        let mention = format!("@{}", source.name);
        if contains_skill_mention(text, &mention) {
            names.push(source.name);
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

fn contains_skill_mention(text: &str, mention: &str) -> bool {
    let mut start = 0usize;
    while let Some(offset) = text[start..].find(mention) {
        let index = start + offset;
        let after = index + mention.len();
        let before_ok = text[..index]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_skill_name_boundary_continuation(ch, None));
        let after_slice = &text[after..];
        let mut after_chars = after_slice.chars();
        let after_first = after_chars.next();
        let after_second = after_chars.next();
        let after_ok =
            after_first.is_none_or(|ch| !is_skill_name_boundary_continuation(ch, after_second));
        if before_ok && after_ok {
            return true;
        }
        start = after;
    }
    false
}

fn is_skill_name_boundary_continuation(ch: char, next: Option<char>) -> bool {
    ch.is_ascii_alphanumeric()
        || matches!(ch, '-' | '_')
        || (ch == '.' && next.is_some_and(|next| next.is_ascii_alphanumeric()))
}

fn trace_runs_root(cwd: &Path) -> PathBuf {
    cwd.join(".spark-runs")
}

fn display_trace_dir(cwd: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(cwd)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf())
}

fn trace_export_record(cwd: &Path, path: &Path, display: &Path, summary: Option<&Value>) -> Value {
    let absolute_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    json!({
        "type": "trace",
        "trace_dir": display.display().to_string(),
        "trace_dir_abs": absolute_path.display().to_string(),
        "workspace": cwd.display().to_string(),
        "summary": summary.cloned().unwrap_or(Value::Null),
    })
}

fn resolve_char_threshold(
    name: &str,
    chars: Option<usize>,
    tokens: Option<usize>,
    default_chars: usize,
) -> Result<usize> {
    match (chars, tokens) {
        (Some(_), Some(_)) => {
            anyhow::bail!("pass either --{name}-chars or --{name}-tokens, not both")
        }
        (Some(chars), None) => Ok(chars),
        (None, Some(tokens)) => Ok(tokens.saturating_mul(APPROX_CHARS_PER_TOKEN)),
        (None, None) => Ok(default_chars),
    }
}

fn validate_scenario_repeat(repeat: usize) -> Result<()> {
    if repeat == 0 {
        anyhow::bail!("--repeat must be greater than 0");
    }
    if repeat > MAX_SCENARIO_REPEAT {
        anyhow::bail!("--repeat must be <= {MAX_SCENARIO_REPEAT}");
    }
    Ok(())
}

fn trace_has_all_diagnostics(summary: &Value, required: &[String]) -> bool {
    if required.is_empty() {
        return true;
    }
    let Some(diagnostics) = summary.get("diagnostics").and_then(Value::as_array) else {
        return false;
    };
    required.iter().all(|required_kind| {
        diagnostics.iter().any(|diagnostic| {
            diagnostic.get("kind").and_then(Value::as_str) == Some(required_kind.as_str())
        })
    })
}

fn trace_filter_label(scenario: Option<&str>, diagnostics: &[String]) -> String {
    let mut label = scenario.unwrap_or("all").to_string();
    if !diagnostics.is_empty() {
        label.push_str(" diagnostics=");
        label.push_str(&diagnostics.join(","));
    }
    label
}

fn latest_trace_dir(root: &Path) -> Result<PathBuf> {
    list_trace_dirs(root, 1)?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no trace directories found under {}", root.display()))
}

fn list_trace_dirs(root: &Path, limit: usize) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut runs = Vec::new();
    for entry in std::fs::read_dir(root)
        .map_err(|error| anyhow::anyhow!("failed to read {}: {error}", root.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(suffix) = name.strip_prefix("run-") else {
            continue;
        };
        let order = suffix.parse::<u128>().unwrap_or(0);
        runs.push((order, entry.path()));
    }

    runs.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| right.1.file_name().cmp(&left.1.file_name()))
    });
    runs.truncate(limit);
    Ok(runs.into_iter().map(|(_, path)| path).collect())
}

#[cfg(test)]
mod tests {
    use super::{
        APPROX_CHARS_PER_TOKEN, DEFAULT_COMPACT_AFTER_CHARS, ProfileScenarioKind, command_args,
        contains_skill_mention, is_active_session, latest_trace_dir, list_trace_dirs,
        mentioned_skill_names, prepare_profile_scenario, profile_scenario_expected_skills,
        profile_scenario_expected_tool_calls, profile_scenario_expected_tool_groups,
        profile_scenario_prompts, resolve_char_threshold, session_name_for_display,
        trace_export_record, trace_filter_label, trace_has_all_diagnostics, trace_runs_root,
        validate_scenario_repeat,
    };
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn slash_commands_match_exactly_or_with_whitespace() {
        assert_eq!(command_args("/skill", "/skill"), Some(""));
        assert_eq!(
            command_args("/skill load rust", "/skill"),
            Some("load rust")
        );
        assert_eq!(command_args("/compact", "/compact"), Some(""));
        assert_eq!(command_args("/compact now", "/compact"), Some("now"));
        assert_eq!(command_args("/compaction", "/compact"), None);
        assert_eq!(command_args("/profile", "/profile"), Some(""));
        assert_eq!(command_args("/profiles", "/profile"), None);
        assert_eq!(command_args("/skills", "/skill"), None);
        assert_eq!(command_args("/sessions", "/session"), None);
    }

    #[test]
    fn detects_repo_local_skill_mentions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let skill_dir = dir.path().join(".agents").join("skills").join("demo-skill");
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: demo-skill\ndescription: Demo\n---\n\n# Demo\n",
        )
        .expect("write skill");

        let mentions = mentioned_skill_names(
            &dir.path().to_path_buf(),
            "Please use @demo-skill for this.",
        )
        .expect("mentions");

        assert_eq!(mentions, vec!["demo-skill"]);
    }

    #[test]
    fn skill_mentions_require_boundaries() {
        assert!(contains_skill_mention(
            "Please use @demo-skill.",
            "@demo-skill"
        ));
        assert!(!contains_skill_mention(
            "Please use @demo-skill-extra.",
            "@demo-skill"
        ));
    }

    #[test]
    fn trace_dirs_are_listed_newest_first() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = trace_runs_root(dir.path());
        std::fs::create_dir_all(root.join("run-100")).expect("create old trace");
        std::fs::create_dir_all(root.join("run-300")).expect("create new trace");
        std::fs::create_dir_all(root.join("run-200")).expect("create middle trace");
        std::fs::create_dir_all(root.join("other")).expect("create ignored dir");
        std::fs::write(root.join("run-400"), "{}").expect("create ignored file");

        let runs = list_trace_dirs(&root, 2).expect("list trace dirs");
        let names = runs
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["run-300", "run-200"]);
    }

    #[test]
    fn latest_trace_dir_uses_highest_run_suffix() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = trace_runs_root(dir.path());
        std::fs::create_dir_all(root.join("run-1")).expect("create old trace");
        std::fs::create_dir_all(root.join("run-2")).expect("create latest trace");

        let latest = latest_trace_dir(&root).expect("latest trace");

        assert_eq!(latest.file_name().unwrap(), "run-2");
    }

    #[test]
    fn token_thresholds_resolve_to_estimated_chars() {
        let chars = resolve_char_threshold(
            "compact-after",
            None,
            Some(32_000),
            DEFAULT_COMPACT_AFTER_CHARS,
        )
        .expect("resolve threshold");

        assert_eq!(chars, 128_000);
    }

    #[test]
    fn char_thresholds_conflict_with_token_thresholds() {
        let error = resolve_char_threshold("max-input", Some(1), Some(1), 10)
            .expect_err("conflicting thresholds");

        assert!(
            error
                .to_string()
                .contains("pass either --max-input-chars or --max-input-tokens")
        );
    }

    #[test]
    fn scenario_repeat_must_be_in_supported_range() {
        validate_scenario_repeat(1).expect("repeat 1");
        validate_scenario_repeat(50).expect("max repeat");

        let zero = validate_scenario_repeat(0).expect_err("zero repeat");
        assert!(zero.to_string().contains("greater than 0"));

        let too_many = validate_scenario_repeat(51).expect_err("too many repeats");
        assert!(too_many.to_string().contains("<= 50"));
    }

    #[test]
    fn trace_diagnostic_filter_requires_all_requested_kinds() {
        let summary = json!({
            "diagnostics": [
                {"kind": "tool_failures"},
                {"kind": "tool_failure_recovered"}
            ]
        });

        assert!(trace_has_all_diagnostics(
            &summary,
            &["tool_failures".to_string()]
        ));
        assert!(trace_has_all_diagnostics(
            &summary,
            &[
                "tool_failures".to_string(),
                "tool_failure_recovered".to_string()
            ]
        ));
        assert!(!trace_has_all_diagnostics(
            &summary,
            &[
                "tool_failures".to_string(),
                "weak_compaction_shrink".to_string()
            ]
        ));
        assert!(!trace_has_all_diagnostics(
            &json!({}),
            &["tool_failures".to_string()]
        ));
    }

    #[test]
    fn trace_filter_label_includes_scenario_and_diagnostics() {
        assert_eq!(
            trace_filter_label(Some("tool-recovery"), &[]),
            "tool-recovery"
        );
        assert_eq!(
            trace_filter_label(None, &["tool_failures".to_string()]),
            "all diagnostics=tool_failures"
        );
        assert_eq!(
            trace_filter_label(
                Some("tool-recovery"),
                &[
                    "tool_failures".to_string(),
                    "tool_failure_recovered".to_string()
                ],
            ),
            "tool-recovery diagnostics=tool_failures,tool_failure_recovered"
        );
    }

    #[test]
    fn trace_export_record_wraps_summary_with_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let run = dir.path().join(".spark-runs").join("run-42");
        std::fs::create_dir_all(&run).expect("create trace dir");
        let display = PathBuf::from(".spark-runs").join("run-42");
        let summary = json!({
            "requests": 1,
            "tool_calls": 2,
        });

        let record = trace_export_record(dir.path(), &run, &display, Some(&summary));

        assert_eq!(record["type"], "trace");
        assert_eq!(
            record["trace_dir"],
            format!(".spark-runs{}run-42", std::path::MAIN_SEPARATOR)
        );
        assert!(
            record["trace_dir_abs"]
                .as_str()
                .unwrap()
                .ends_with("run-42")
        );
        assert_eq!(record["summary"]["requests"], 1);
        assert_eq!(record["summary"]["tool_calls"], 2);
    }

    #[test]
    fn active_session_matching_handles_same_path() {
        let path = PathBuf::from("session-a.json");
        let active = Some(path.clone());

        assert!(is_active_session(&active, &path));
        assert!(!is_active_session(
            &active,
            &PathBuf::from("session-b.json")
        ));
        assert!(!is_active_session(&None, &path));
    }

    #[test]
    fn session_display_name_uses_file_stem() {
        assert_eq!(
            session_name_for_display(&PathBuf::from("demo.session.json")),
            "demo.session"
        );
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
        assert!(prompt.contains("Work only under .spark-scenarios/file-edit"));
        assert!(prompt.contains("Use fs.edit or fs.replace"));
        assert!(prompt.contains("Use fs.write"));
    }

    #[test]
    fn file_edit_scenario_prepares_scratch_fixture() {
        let dir = tempfile::tempdir().expect("tempdir");

        prepare_profile_scenario(dir.path(), ProfileScenarioKind::FileEdit)
            .expect("prepare scenario");

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
        let prompts =
            profile_scenario_prompts(ProfileScenarioKind::FileOps, 45_000).expect("scenario");
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

        prepare_profile_scenario(dir.path(), ProfileScenarioKind::FileOps)
            .expect("prepare scenario");

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
        assert!(prompt.contains("Use fs.stat"));
        assert!(prompt.contains("Use fs.write"));
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

        assert_eq!(
            groups,
            vec![vec!["fs.read"], vec!["fs.stat"], vec!["fs.write"]]
        );
    }

    #[test]
    fn tool_recovery_scenario_declares_expected_exact_tool_calls() {
        let calls = profile_scenario_expected_tool_calls(ProfileScenarioKind::ToolRecovery);

        assert_eq!(calls.len(), 4);
        assert_eq!(calls[0]["tool"], "fs.read");
        assert_eq!(
            calls[0]["path"],
            ".spark-scenarios/tool-recovery/source/missing-note.md"
        );
        assert_eq!(calls[1]["tool"], "fs.stat");
        assert_eq!(
            calls[1]["path"],
            ".spark-scenarios/tool-recovery/source/note.md"
        );
        assert_eq!(calls[2]["tool"], "fs.read");
        assert_eq!(
            calls[2]["path"],
            ".spark-scenarios/tool-recovery/source/note.md"
        );
        assert_eq!(calls[3]["tool"], "fs.write");
        assert_eq!(
            calls[3]["path"],
            ".spark-scenarios/tool-recovery/recovery-summary.txt"
        );
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
    fn natural_compaction_scenario_uses_multiple_chat_turns() {
        let prompts = profile_scenario_prompts(ProfileScenarioKind::NaturalCompaction, 45_000)
            .expect("scenario");
        let total_chars = prompts.iter().map(String::len).sum::<usize>();

        assert_eq!(prompts.len(), 3);
        assert!(total_chars >= DEFAULT_COMPACT_AFTER_CHARS);
        assert!(total_chars / APPROX_CHARS_PER_TOKEN < 120_000);
        assert!(prompts[0].contains("turn 1/3"));
        assert!(prompts[1].contains("turn 2/3"));
        assert!(prompts[2].contains("fs.list on src with recursive=false"));
    }
}

async fn compile_skill_cached(
    runner: &agent::AgentRunner,
    cwd: &PathBuf,
    name: &str,
    refresh: bool,
) -> Result<skills::CompiledSkill> {
    if !refresh && let Some(cached) = skills::load_cached_if_fresh(cwd, name)? {
        return Ok(cached);
    }

    let (_, raw) = skills::source_text(cwd, name)?;
    match runner.compile_skill_summary(name, &raw).await {
        Ok(summary) => skills::compile_or_load_with_summary(cwd, name, true, Some(summary)),
        Err(error) => {
            eprintln!(
                "warning: Spark skill compile failed for `{name}`; using local fallback: {error:#}"
            );
            skills::compile_or_load(cwd, name, true)
        }
    }
}
