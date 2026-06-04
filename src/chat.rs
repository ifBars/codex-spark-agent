use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;

use crate::{agent, sessions, skill_commands};

pub(crate) async fn run_interactive_chat(
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
            sessions::handle_session_command(runner, &mut session_path, command.trim())?;
            continue;
        }

        if let Some(command) = command_args(input, "/new") {
            sessions::handle_new_session_command(runner, &mut session_path, command.trim())?;
            continue;
        }

        if input == "/skills" {
            skill_commands::handle_skill_command(runner, &cwd, "list").await?;
            continue;
        }

        if let Some(command) = command_args(input, "/skill") {
            skill_commands::handle_skill_command(runner, &cwd, command.trim()).await?;
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
        if let Err(error) = skill_commands::load_skill_mentions(runner, &cwd, input).await {
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

pub(crate) fn command_args<'a>(input: &'a str, command: &str) -> Option<&'a str> {
    if input == command {
        return Some("");
    }
    input
        .strip_prefix(command)
        .and_then(|rest| rest.strip_prefix(char::is_whitespace))
}
