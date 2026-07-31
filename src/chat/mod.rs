use anyhow::Result;
use std::io::Write;
use std::path::PathBuf;

pub(crate) mod markdown;
pub(crate) mod tui;

use crate::{agent, mcp, prompt_commands, session, skill, tools};

#[derive(Debug, Clone, Copy)]
pub(crate) struct SlashCommand {
    pub(crate) name: &'static str,
    pub(crate) usage: &'static str,
    pub(crate) description: &'static str,
}

pub(crate) const SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "/help",
        usage: "/help",
        description: "Show command help",
    },
    SlashCommand {
        name: "/status",
        usage: "/status",
        description: "Show context and profiler status",
    },
    SlashCommand {
        name: "/mode",
        usage: "/mode [ask|work]",
        description: "Show or change tool mode",
    },
    SlashCommand {
        name: "/reasoning",
        usage: "/reasoning [low|medium|high|xhigh]",
        description: "Show or change reasoning effort",
    },
    SlashCommand {
        name: "/goal",
        usage: "/goal [run|pause|resume|clear|objective]",
        description: "Manage a durable multi-turn objective",
    },
    SlashCommand {
        name: "/subagent",
        usage: "/subagent [flags] [explore|research|review|plan] <task>",
        description: "Run one isolated worker and save its compact brief",
    },
    SlashCommand {
        name: "/agents",
        usage: "/agents [list|cancel [worker-id]]",
        description: "List or cancel managed parallel workers",
    },
    SlashCommand {
        name: "/memory",
        usage: "/memory [on|off|status|path|show|add <note>]",
        description: "Toggle markdown memory for this session",
    },
    SlashCommand {
        name: "/mcp",
        usage: "/mcp [list|enable|disable|reset|refresh]",
        description: "Manage workspace MCP servers",
    },
    SlashCommand {
        name: "/ask",
        usage: "/ask",
        description: "Switch to read-only mode",
    },
    SlashCommand {
        name: "/work",
        usage: "/work",
        description: "Switch to work mode",
    },
    SlashCommand {
        name: "/profile",
        usage: "/profile",
        description: "Show profiler summary JSON",
    },
    SlashCommand {
        name: "/compact",
        usage: "/compact",
        description: "Compact conversation context now",
    },
    SlashCommand {
        name: "/session",
        usage: "/session [list|open|new|use|rename|delete]",
        description: "Manage saved sessions",
    },
    SlashCommand {
        name: "/new",
        usage: "/new [name]",
        description: "Start a new session",
    },
    SlashCommand {
        name: "/skill",
        usage: "/skill [load|refresh|list]",
        description: "Manage skills",
    },
    SlashCommand {
        name: "/skills",
        usage: "/skills",
        description: "List available skills",
    },
    SlashCommand {
        name: "/commands",
        usage: "/commands",
        description: "List reusable prompt commands",
    },
    SlashCommand {
        name: "/save",
        usage: "/save",
        description: "Save the current session",
    },
    SlashCommand {
        name: "/clear",
        usage: "/clear",
        description: "Clear the conversation",
    },
    SlashCommand {
        name: "/exit",
        usage: "/exit",
        description: "Exit chat",
    },
    SlashCommand {
        name: "/quit",
        usage: "/quit",
        description: "Exit chat",
    },
];

pub(crate) async fn run_interactive_chat(
    runner: &mut agent::AgentRunner,
    session_name: Option<String>,
    cwd: PathBuf,
) -> Result<()> {
    tui::run(runner, session_name, cwd).await
}

#[allow(dead_code)]
pub(crate) async fn run_line_interactive_chat(
    runner: &mut agent::AgentRunner,
    mut session_name: Option<String>,
    cwd: PathBuf,
) -> Result<()> {
    if let Some(name) = &session_name {
        println!("Spark interactive chat. Session: {name}");
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
            session::handle_session_command(runner, &mut session_name, command.trim())?;
            continue;
        }

        if let Some(command) = command_args(input, "/new") {
            session::handle_new_session_command(runner, &mut session_name, command.trim())?;
            continue;
        }

        if input == "/skills" {
            skill::commands::handle_skill_command(runner, &cwd, "list").await?;
            continue;
        }

        if input == "/commands" {
            for command in prompt_commands::discover_commands(&cwd)? {
                if command.description.is_empty() {
                    println!("{} ({})", command.name, command.source_path);
                } else {
                    println!(
                        "{} - {} ({})",
                        command.name, command.description, command.source_path
                    );
                }
            }
            continue;
        }

        if let Some(command) = command_args(input, "/skill") {
            skill::commands::handle_skill_command(runner, &cwd, command.trim()).await?;
            if let Some(name) = &session_name {
                runner.save_session_named(name)?;
            }
            continue;
        }

        if let Some(command) = command_args(input, "/goal") {
            match handle_goal_command(runner, command.trim()).await {
                Ok(message) => {
                    println!("{message}");
                    if let Some(name) = &session_name {
                        runner.save_session_named(name)?;
                    }
                }
                Err(error) => eprintln!("error: {error:#}"),
            }
            continue;
        }

        if let Some(command) = command_args(input, "/subagent") {
            match handle_subagent_command(runner, command.trim()).await {
                Ok(report) => {
                    runner.record_subagent_report(&report);
                    println!("{}", agent::report_prompt(&report));
                    if let Some(name) = &session_name {
                        runner.save_session_named(name)?;
                    }
                }
                Err(error) => eprintln!("error: {error:#}"),
            }
            continue;
        }

        if let Some(command) = command_args(input, "/agents") {
            match handle_agents_command(runner, command.trim()) {
                Ok(message) => println!("{message}"),
                Err(error) => eprintln!("error: {error:#}"),
            }
            continue;
        }

        if let Some(command) = command_args(input, "/memory") {
            match handle_memory_command(runner, command.trim()) {
                Ok(message) => {
                    println!("{message}");
                    if let Some(name) = &session_name {
                        runner.save_session_named(name)?;
                    }
                }
                Err(error) => eprintln!("error: {error:#}"),
            }
            continue;
        }

        if let Some(command) = command_args(input, "/mcp") {
            match handle_mcp_command(runner, &cwd, command.trim()).await {
                Ok(message) => println!("{message}"),
                Err(error) => eprintln!("error: {error:#}"),
            }
            continue;
        }

        match input {
            "/exit" | "/quit" => return Ok(()),
            "/help" => {
                println!(
                    "Commands: /help, /status, /mode, /reasoning, /goal, /subagent, /agents, /memory, /mcp, /ask, /work, /profile, /compact, /session, /new, /skill, /skills, /commands, /save, /clear, /exit"
                );
                println!(
                    "Goal commands: /goal, /goal <objective>, /goal run [checkpoints], /goal pause, /goal resume, /goal clear"
                );
                println!(
                    "Workers: /subagent [--model parent|gpt-5.6-luna] [--reasoning low|medium|high|xhigh] [--mode ask|work --ownership path1,path2] explore|research|review|plan <task>; model tools support spawn/wait/followup/cancel/list (default concurrency: 3). /agents lists or cancels active workers."
                );
                println!(
                    "Memory commands: /memory, /memory on, /memory off, /memory add <durable note>, /memory show"
                );
                println!(
                    "MCP commands: /mcp, /mcp enable <name>, /mcp disable <name>, /mcp reset <name>, /mcp refresh"
                );
                println!(
                    "Session commands: /session, /session list, /session open <name>, /session new <name>, /session use <name>, /session rename [old] <new>, /session delete <name>"
                );
                println!(
                    "Skill commands: /skills, /skill load <name>, /skill refresh, /skill list"
                );
                println!(
                    "Prompt commands: /commands lists .agents/commands, .spark/commands, and .claude/commands; /<name> [args] expands and runs a Markdown prompt."
                );
                continue;
            }
            "/status" => {
                println!("conversation input JSON chars: {}", runner.input_chars()?);
                println!("{}", runner.profile_status());
                continue;
            }
            "/mode" => {
                println!("mode: {}", runner.mode().name());
                continue;
            }
            "/reasoning" => {
                println!("reasoning: {}", runner.reasoning_effort());
                continue;
            }
            "/ask" => {
                runner.set_mode(tools::AgentMode::Ask);
                println!("mode: ask");
                if let Some(name) = &session_name {
                    runner.save_session_named(name)?;
                }
                continue;
            }
            "/work" => {
                runner.set_mode(tools::AgentMode::Work);
                println!("mode: work");
                if let Some(name) = &session_name {
                    runner.save_session_named(name)?;
                }
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
                        if let Some(name) = &session_name {
                            runner.save_session_named(name)?;
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
                if let Some(name) = &session_name {
                    runner.save_session_named(name)?;
                }
                println!("conversation cleared");
                continue;
            }
            "/save" => {
                if let Some(name) = &session_name {
                    runner.save_session_named(name)?;
                    println!("saved session: {name}");
                } else {
                    println!("no session configured; start with --session <name>");
                }
                continue;
            }
            _ => {}
        }

        if let Some(command) = command_args(input, "/mode") {
            match parse_mode(command.trim()) {
                Some(mode) => {
                    runner.set_mode(mode);
                    println!("mode: {}", mode.name());
                    if let Some(name) = &session_name {
                        runner.save_session_named(name)?;
                    }
                }
                None => {
                    eprintln!("usage: /mode ask|work");
                }
            }
            continue;
        }

        if let Some(command) = command_args(input, "/reasoning") {
            match parse_reasoning_effort(command.trim()) {
                Some(reasoning_effort) => {
                    runner.set_reasoning_effort(reasoning_effort);
                    println!("reasoning: {}", runner.reasoning_effort());
                    if let Some(name) = &session_name {
                        runner.save_session_named(name)?;
                    }
                }
                None => {
                    eprintln!("usage: /reasoning low|medium|high|xhigh");
                }
            }
            continue;
        }

        let expanded_prompt = if input.starts_with('/') {
            match prompt_commands::expand_slash_command(&cwd, input)? {
                Some(prompt) => prompt,
                None => {
                    eprintln!("{}", unknown_slash_command_warning(input));
                    continue;
                }
            }
        } else {
            input.to_string()
        };

        if expanded_prompt != input {
            println!(
                "expanded command: {}",
                slash_command_token(input).unwrap_or(input)
            );
        }

        let mut save_after_run = false;
        if let Err(error) =
            skill::commands::load_skill_mentions(runner, &cwd, &expanded_prompt).await
        {
            eprintln!("error: {error:#}");
        } else if let Err(error) = runner.run(&expanded_prompt).await {
            eprintln!("error: {error:#}");
            save_after_run = true;
        } else {
            save_after_run = true;
        }

        if save_after_run && let Some(name) = &session_name {
            runner.save_session_named(name)?;
        }
    }
}

pub(crate) fn command_args<'a>(input: &'a str, command: &str) -> Option<&'a str> {
    if input == command {
        return Some("");
    }
    input
        .strip_prefix(command)
        .and_then(|rest| rest.strip_prefix(char::is_whitespace))
}

pub(crate) fn slash_command_token(input: &str) -> Option<&str> {
    let trimmed = input.trim_start();
    if !trimmed.starts_with('/') {
        return None;
    }
    Some(trimmed.split_whitespace().next().unwrap_or(trimmed))
}

pub(crate) fn matching_slash_commands(input: &str) -> Vec<SlashCommand> {
    let Some(token) = slash_command_token(input) else {
        return Vec::new();
    };
    SLASH_COMMANDS
        .iter()
        .copied()
        .filter(|command| command.name.starts_with(token))
        .collect()
}

pub(crate) fn unknown_slash_command_warning(input: &str) -> String {
    let token = slash_command_token(input).unwrap_or(input.trim());
    format!("unknown command: {token}. Type /help for commands.")
}

pub(crate) fn parse_mode(input: &str) -> Option<tools::AgentMode> {
    match input {
        "ask" => Some(tools::AgentMode::Ask),
        "work" | "agent" => Some(tools::AgentMode::Work),
        _ => None,
    }
}

pub(crate) fn parse_reasoning_effort(input: &str) -> Option<&'static str> {
    match input {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "xhigh" => Some("xhigh"),
        _ => None,
    }
}

pub(crate) async fn handle_mcp_command(
    runner: &mut agent::AgentRunner,
    cwd: &std::path::Path,
    command: &str,
) -> Result<String> {
    let mut parts = command.split_whitespace();
    match parts.next() {
        None | Some("list") => {
            if parts.next().is_some() {
                anyhow::bail!("usage: /mcp [list]");
            }
            let statuses = mcp::configured_server_statuses(cwd)?;
            if statuses.is_empty() {
                return Ok("No MCP servers configured. Add one to global Codex config, .mcp.json, or .spark/mcp.json.".to_string());
            }
            let mut lines = vec![format!(
                "MCP servers (workspace state: {})",
                mcp::mcp_state_path(cwd).display()
            )];
            if mcp::mcp_disabled_by_env() {
                lines.push("all MCP discovery is disabled by SPARK_DISABLE_MCP".to_string());
            }
            for status in statuses {
                lines.push(format!(
                    "{}  {}{}",
                    if status.enabled { "on " } else { "off" },
                    status.name,
                    if status.overridden {
                        "  (workspace override)"
                    } else {
                        ""
                    }
                ));
            }
            Ok(lines.join("\n"))
        }
        Some(action @ ("enable" | "disable")) => {
            let name = parts
                .next()
                .ok_or_else(|| anyhow::anyhow!("usage: /mcp {action} <name>"))?;
            if parts.next().is_some() {
                anyhow::bail!("usage: /mcp {action} <name>");
            }
            let enabled = action == "enable";
            mcp::set_server_enabled(cwd, name, enabled)?;
            runner.invalidate_mcp_registry();
            Ok(format!(
                "MCP server `{name}` {} for this workspace. The next agent request will reload MCP tools.",
                if enabled { "enabled" } else { "disabled" }
            ))
        }
        Some("reset") => {
            let name = parts
                .next()
                .ok_or_else(|| anyhow::anyhow!("usage: /mcp reset <name>"))?;
            if parts.next().is_some() {
                anyhow::bail!("usage: /mcp reset <name>");
            }
            let removed = mcp::reset_server_enabled(cwd, name)?;
            runner.invalidate_mcp_registry();
            Ok(if removed {
                format!("MCP server `{name}` now follows its configured enabled state.")
            } else {
                format!("MCP server `{name}` had no workspace override.")
            })
        }
        Some("refresh") => {
            if parts.next().is_some() {
                anyhow::bail!("usage: /mcp refresh");
            }
            let (tool_count, warnings) = runner.refresh_mcp_registry().await;
            let mut message = format!("MCP registry refreshed: {tool_count} tools available.");
            if !warnings.is_empty() {
                message.push_str("\nWarnings:\n- ");
                message.push_str(&warnings.join("\n- "));
            }
            Ok(message)
        }
        Some(_) => {
            anyhow::bail!("usage: /mcp [list|enable <name>|disable <name>|reset <name>|refresh]")
        }
    }
}

pub(crate) async fn handle_goal_command(
    runner: &mut agent::AgentRunner,
    command: &str,
) -> Result<String> {
    let command = command.trim();
    if command.is_empty() {
        return Ok(runner.goal_status_line());
    }

    if command == "clear" {
        runner.clear_goal();
        return Ok("goal cleared".to_string());
    }
    if command == "pause" {
        runner.pause_goal()?;
        return Ok(runner.goal_status_line());
    }
    if command == "resume" {
        runner.resume_goal()?;
        return Ok(runner.goal_status_line());
    }
    if let Some(rest) = command_args(command, "run") {
        let checkpoints = parse_goal_checkpoint_count(rest.trim())?;
        let report = runner
            .run_goal_checkpoints(checkpoints, tokio_util::sync::CancellationToken::new())
            .await?;
        return Ok(format!(
            "goal run: checkpoints={} status={}\n{}",
            report.checkpoints_run,
            report.status.name(),
            report.summary
        ));
    }
    if let Some(rest) = command_args(command, "set") {
        runner.set_goal(rest.trim())?;
        return Ok(runner.goal_status_line());
    }

    runner.set_goal(command)?;
    Ok(runner.goal_status_line())
}

pub(crate) async fn handle_subagent_command(
    runner: &mut agent::AgentRunner,
    command: &str,
) -> Result<agent::SubagentReport> {
    let (kind, task, options) = parse_subagent_command(command)?;
    runner.run_subagent_with_options(kind, task, options).await
}

pub(crate) fn handle_agents_command(
    runner: &mut agent::AgentRunner,
    command: &str,
) -> Result<String> {
    let command = command.trim();
    if command.is_empty() || command == "list" {
        return Ok(serde_json::to_string_pretty(&runner.subagent_status())?);
    }
    if let Some(id) = command_args(command, "cancel") {
        let id = id.trim();
        return Ok(serde_json::to_string_pretty(
            &runner.cancel_subagents((!id.is_empty()).then_some(id))?,
        )?);
    }
    anyhow::bail!("usage: /agents [list|cancel [worker-id]]")
}

pub(crate) fn handle_memory_command(
    runner: &mut agent::AgentRunner,
    command: &str,
) -> Result<String> {
    let command = command.trim();
    if command.is_empty() || command == "status" || command == "path" || command == "paths" {
        return runner.memory_status();
    }
    if command == "on" || command == "enable" {
        runner.set_memory_enabled(true)?;
        return Ok(format!("memory enabled\n{}", runner.memory_status()?));
    }
    if command == "off" || command == "disable" {
        runner.set_memory_enabled(false)?;
        return Ok(format!("memory disabled\n{}", runner.memory_status()?));
    }
    if command == "show" {
        return runner.memory_context_preview();
    }
    if let Some(note) = command_args(command, "add") {
        runner.append_memory_note(note.trim())?;
        return Ok(format!("memory note added\n{}", runner.memory_status()?));
    }
    anyhow::bail!("usage: /memory [on|off|status|path|show|add <note>]")
}

fn parse_goal_checkpoint_count(input: &str) -> Result<usize> {
    if input.is_empty() {
        return Ok(3);
    }
    let count = input
        .parse::<usize>()
        .map_err(|_| anyhow::anyhow!("usage: /goal run [checkpoint-count]"))?;
    if count == 0 || count > 20 {
        anyhow::bail!("goal checkpoint count must be between 1 and 20");
    }
    Ok(count)
}

pub(crate) fn parse_subagent_command(
    command: &str,
) -> Result<(agent::SubagentKind, &str, agent::SubagentRunOptions)> {
    let command = command.trim();
    let mut options = agent::SubagentRunOptions::default();
    let mut search_start = 0usize;
    let mut kind = None;
    let mut task_start = None;

    while search_start < command.len() {
        let rest = command[search_start..].trim_start();
        search_start = command.len() - rest.len();
        if rest.is_empty() {
            break;
        }
        let (token, after_token) = split_token(rest);
        if token == "--model" {
            let (value, consumed) = parse_flag_value(after_token, "--model")?;
            options.model = Some(value.to_string());
            search_start += token.len() + consumed;
            continue;
        }
        if token == "--reasoning" || token == "--reasoning-effort" {
            let (value, consumed) = parse_flag_value(after_token, "--reasoning")?;
            options.reasoning_effort = Some(value.to_string());
            search_start += token.len() + consumed;
            continue;
        }
        if token == "--mode" {
            let (value, consumed) = parse_flag_value(after_token, "--mode")?;
            options.mode = Some(match value {
                "ask" => crate::tools::AgentMode::Ask,
                "work" => crate::tools::AgentMode::Work,
                _ => anyhow::bail!("--mode must be ask or work"),
            });
            search_start += token.len() + consumed;
            continue;
        }
        if token == "--ownership" {
            let (value, consumed) = parse_flag_value(after_token, "--ownership")?;
            options.ownership = value
                .split(',')
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .map(str::to_string)
                .collect();
            search_start += token.len() + consumed;
            continue;
        }
        if kind.is_none() {
            kind = agent::SubagentKind::parse(token);
            if kind.is_none() {
                anyhow::bail!("usage: /subagent [flags] explore|research|review|plan <task>");
            }
            search_start += token.len();
            continue;
        }

        task_start = Some(search_start);
        break;
    }

    let kind = kind.ok_or_else(|| {
        anyhow::anyhow!("usage: /subagent [flags] explore|research|review|plan <task>")
    })?;
    let task = command[task_start.unwrap_or(search_start)..].trim();
    if task.is_empty() {
        anyhow::bail!("subagent task is required");
    }
    let validation_args = serde_json::json!({
        "model": options.model,
        "reasoning_effort": options.reasoning_effort,
        "mode": options.mode.map(|mode| mode.name()),
        "ownership": options.ownership,
    });
    options = agent::SubagentRunOptions::from_tool_args(&validation_args)?;
    Ok((kind, task, options))
}

fn split_token(input: &str) -> (&str, &str) {
    let end = input.find(char::is_whitespace).unwrap_or(input.len());
    (&input[..end], &input[end..])
}

fn parse_flag_value<'a>(input: &'a str, flag: &str) -> Result<(&'a str, usize)> {
    let trimmed = input.trim_start();
    let skipped = input.len() - trimmed.len();
    if trimmed.is_empty() {
        anyhow::bail!("{flag} requires a value");
    }
    let (value, rest) = split_token(trimmed);
    Ok((
        value,
        skipped + value.len() + (trimmed.len() - rest.len() - value.len()),
    ))
}
