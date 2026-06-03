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
use clap::{Parser, Subcommand};

const DEFAULT_MODEL: &str = "gpt-5.3-codex-spark";
const DEFAULT_COMPACT_AFTER_CHARS: usize = 160_000;
const DEFAULT_MAX_INPUT_CHARS: usize = 500_000;
const APPROX_CHARS_PER_TOKEN: usize = 4;

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
                    DEFAULT_COMPACT_AFTER_CHARS,
                    DEFAULT_MAX_INPUT_CHARS,
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
        Command::Traces { limit } => {
            let cwd = std::fs::canonicalize(".").unwrap_or_else(|_| PathBuf::from("."));
            for run in list_trace_dirs(&trace_runs_root(&cwd), limit)? {
                println!("{}", display_trace_dir(&cwd, &run).display());
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

fn trace_runs_root(cwd: &Path) -> PathBuf {
    cwd.join(".spark-runs")
}

fn display_trace_dir(cwd: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(cwd)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf())
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
        DEFAULT_COMPACT_AFTER_CHARS, command_args, contains_skill_mention, latest_trace_dir,
        list_trace_dirs, mentioned_skill_names, resolve_char_threshold, trace_runs_root,
    };

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
