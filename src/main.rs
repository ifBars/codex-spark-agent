mod agent;
mod auth;
mod client;
mod config;
mod profiler;
mod skills;
mod tools;

use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

const DEFAULT_MODEL: &str = "gpt-5.3-codex-spark";

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
        #[arg(long, default_value_t = 160_000)]
        compact_after_chars: usize,
        /// Refuse to send request JSON above this many characters.
        #[arg(long, default_value_t = 500_000)]
        max_input_chars: usize,
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
    /// Summarize a .spark-runs/run-* trace for repeated tool calls and compaction behavior.
    AnalyzeTrace {
        /// Trace directory to analyze.
        dir: PathBuf,
    },
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
            max_input_chars,
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
            )?;
            let session_name = session.or_else(|| interactive.then(|| "default".to_string()));
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
                    160_000,
                    500_000,
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
        Command::AnalyzeTrace { dir } => {
            let summary = profiler::analyze_trace(&dir)?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
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
                    "Session commands: /session, /session list, /session open <name>, /session new <name>, /session use <name>"
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

        if let Err(error) = load_skill_mentions(runner, &cwd, input).await {
            eprintln!("error: {error:#}");
        } else if let Err(error) = runner.run(input).await {
            eprintln!("error: {error:#}");
        } else if let Some(path) = &session_path {
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

#[cfg(test)]
mod tests {
    use super::{command_args, contains_skill_mention, mentioned_skill_names};

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
