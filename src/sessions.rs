use anyhow::Result;

use crate::{agent, session_store};

pub(crate) fn handle_session_command(
    runner: &mut agent::AgentRunner,
    session_name: &mut Option<String>,
    command: &str,
) -> Result<()> {
    let store = session_store::SessionStore::open_default()?;
    let mut parts = command.split_whitespace();
    let action = parts.next();
    match action {
        None => {
            print_session_status(runner, session_name)?;
        }
        Some("list") => {
            for session in store.list_names()? {
                runner.emit_system_message(session);
            }
        }
        Some("save") => {
            save_current_session(runner, session_name)?;
        }
        Some("open" | "switch") => {
            let name = required_session_arg(parts.next(), "open")?;
            if !store.exists(name)? {
                anyhow::bail!("session `{name}` does not exist");
            }
            switch_session(
                runner,
                session_name,
                name.to_string(),
                /*load_existing*/ true,
            )?;
        }
        Some("new") => {
            let name = parts
                .next()
                .map(str::to_string)
                .unwrap_or_else(timestamp_session_name);
            switch_session(runner, session_name, name, /*load_existing*/ false)?;
        }
        Some("use") => {
            let name = required_session_arg(parts.next(), "use")?;
            switch_session(runner, session_name, name.to_string(), store.exists(name)?)?;
        }
        Some("rename" | "mv") => {
            let first = required_session_arg(parts.next(), "rename")?;
            let second = parts.next();
            rename_session(runner, session_name, first, second)?;
        }
        Some("delete" | "rm") => {
            let name = required_session_arg(parts.next(), "delete")?;
            delete_session(runner, session_name, name)?;
        }
        Some(name) => {
            switch_session(runner, session_name, name.to_string(), store.exists(name)?)?;
        }
    }
    Ok(())
}

pub(crate) fn handle_new_session_command(
    runner: &mut agent::AgentRunner,
    session_name: &mut Option<String>,
    command: &str,
) -> Result<()> {
    let name = command
        .split_whitespace()
        .next()
        .map(str::to_string)
        .unwrap_or_else(timestamp_session_name);
    switch_session(runner, session_name, name, /*load_existing*/ false)
}

fn switch_session(
    runner: &mut agent::AgentRunner,
    session_name: &mut Option<String>,
    target: String,
    load_existing: bool,
) -> Result<()> {
    if let Some(current) = session_name.as_ref() {
        runner.save_session_named(current)?;
    }
    if load_existing {
        runner.load_session_named(&target)?;
        runner.emit_system_message(format!("opened session: {target}"));
    } else {
        runner.clear_conversation();
        runner.save_session_named(&target)?;
        runner.emit_system_message(format!("new session: {target}"));
    }
    *session_name = Some(target);
    Ok(())
}

fn rename_session(
    runner: &mut agent::AgentRunner,
    session_name: &mut Option<String>,
    first: &str,
    second: Option<&str>,
) -> Result<()> {
    let store = session_store::SessionStore::open_default()?;
    let (source, new_name) = match second {
        Some(new_name) => (first.to_string(), new_name),
        None => {
            let Some(current) = session_name.as_ref() else {
                anyhow::bail!("/session rename <new> requires an active session");
            };
            (current.clone(), first)
        }
    };
    if is_active_session(session_name, &source) {
        runner.save_session_named(&source)?;
    }
    store.rename(&source, new_name)?;
    if is_active_session(session_name, &source) {
        *session_name = Some(new_name.to_string());
    }
    runner.emit_system_message(format!("renamed session: {source} -> {new_name}"));
    Ok(())
}

fn delete_session(
    runner: &mut agent::AgentRunner,
    session_name: &Option<String>,
    name: &str,
) -> Result<()> {
    if is_active_session(session_name, name) {
        anyhow::bail!("cannot delete the active session; switch or start /new first");
    }
    session_store::SessionStore::open_default()?.delete(name)?;
    runner.emit_system_message(format!("deleted session: {name}"));
    Ok(())
}

pub(crate) fn is_active_session(session_name: &Option<String>, target: &str) -> bool {
    session_name.as_ref().is_some_and(|active| active == target)
}

fn save_current_session(
    runner: &mut agent::AgentRunner,
    session_name: &Option<String>,
) -> Result<()> {
    let Some(name) = session_name else {
        runner.emit_system_message("no session configured; use /session new <name>");
        return Ok(());
    };
    runner.save_session_named(name)?;
    runner.emit_system_message(format!("saved session: {name}"));
    Ok(())
}

fn print_session_status(
    runner: &mut agent::AgentRunner,
    session_name: &Option<String>,
) -> Result<()> {
    if let Some(name) = session_name {
        runner.emit_system_message(format!("session: {name}"));
    } else {
        runner.emit_system_message("session: none");
    }
    runner.emit_system_message(format!(
        "conversation input JSON chars: {}",
        runner.input_chars()?
    ));
    runner.emit_system_message(runner.profile_status());
    Ok(())
}

fn required_session_arg<'a>(arg: Option<&'a str>, command: &str) -> Result<&'a str> {
    arg.ok_or_else(|| anyhow::anyhow!("/session {command} requires a session name"))
}

pub(crate) fn timestamp_session_name() -> String {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("chat-{now_secs}")
}

pub(crate) fn prepare_default_session_store(protected_session_name: Option<&str>) -> Result<()> {
    session_store::prepare_default_store(protected_session_name)
}
