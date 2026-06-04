use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{agent, config};

pub(crate) fn handle_session_command(
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

pub(crate) fn handle_new_session_command(
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

pub(crate) fn is_active_session(session_path: &Option<PathBuf>, target: &Path) -> bool {
    session_path
        .as_ref()
        .is_some_and(|active| normalize_session_path(active) == normalize_session_path(target))
}

fn normalize_session_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub(crate) fn session_name_for_display(path: &Path) -> String {
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
