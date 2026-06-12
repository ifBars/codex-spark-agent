use anyhow::Result;
use std::io::Write;
use std::path::PathBuf;

pub(crate) mod markdown;
pub(crate) mod tui;

use crate::{agent, prompt_commands, session, skill, tools};

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

        match input {
            "/exit" | "/quit" => return Ok(()),
            "/help" => {
                println!(
                    "Commands: /help, /status, /mode, /reasoning, /ask, /work, /profile, /compact, /session, /new, /skill, /skills, /commands, /save, /clear, /exit"
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
