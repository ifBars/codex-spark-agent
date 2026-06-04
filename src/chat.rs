use anyhow::Result;
use std::io::Write;
use std::path::PathBuf;

use crate::{agent, chat_tui, sessions, skill_commands, tools};

pub(crate) async fn run_interactive_chat(
    runner: &mut agent::AgentRunner,
    session_name: Option<String>,
    cwd: PathBuf,
) -> Result<()> {
    chat_tui::run(runner, session_name, cwd).await
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
            sessions::handle_session_command(runner, &mut session_name, command.trim())?;
            continue;
        }

        if let Some(command) = command_args(input, "/new") {
            sessions::handle_new_session_command(runner, &mut session_name, command.trim())?;
            continue;
        }

        if input == "/skills" {
            skill_commands::handle_skill_command(runner, &cwd, "list").await?;
            continue;
        }

        if let Some(command) = command_args(input, "/skill") {
            skill_commands::handle_skill_command(runner, &cwd, command.trim()).await?;
            if let Some(name) = &session_name {
                runner.save_session_named(name)?;
            }
            continue;
        }

        match input {
            "/exit" | "/quit" => return Ok(()),
            "/help" => {
                println!(
                    "Commands: /help, /status, /mode, /ask, /work, /profile, /compact, /session, /new, /skill, /skills, /save, /clear, /exit"
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
            "/mode" => {
                println!("mode: {}", runner.mode().name());
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

        let mut save_after_run = false;
        if let Err(error) = skill_commands::load_skill_mentions(runner, &cwd, input).await {
            eprintln!("error: {error:#}");
        } else {
            if let Err(error) = runner.run(input).await {
                eprintln!("error: {error:#}");
                save_after_run = true;
            } else {
                save_after_run = true;
            }
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

pub(crate) fn parse_mode(input: &str) -> Option<tools::AgentMode> {
    match input {
        "ask" => Some(tools::AgentMode::Ask),
        "work" | "agent" => Some(tools::AgentMode::Work),
        _ => None,
    }
}
