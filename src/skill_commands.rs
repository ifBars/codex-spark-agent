use std::path::PathBuf;

use anyhow::Result;

use crate::{agent, skills};

pub(crate) async fn handle_skill_command(
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
                runner.emit_system_message(format!(
                    "{}{} [{}] - {}",
                    skill.name, loaded, skill.cache_status, skill.description
                ));
            }
        }
        Some("refresh") => {
            let mut refreshed = 0usize;
            for source in skills::discover_sources(cwd)? {
                compile_skill_cached(runner, cwd, &source.name, true).await?;
                refreshed += 1;
            }
            runner.emit_system_message(format!("refreshed {refreshed} skill(s)"));
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

pub(crate) async fn load_skill_into_runner(
    runner: &mut agent::AgentRunner,
    cwd: &PathBuf,
    name: &str,
    refresh: bool,
) -> Result<()> {
    let skill = compile_skill_cached(runner, cwd, name, refresh).await?;
    if runner.load_skill_context(&skill.name, &skill.summary) {
        runner.emit_system_message(format!("loaded skill: {}", skill.name));
    } else {
        runner.emit_system_message(format!("skill already loaded: {}", skill.name));
    }
    Ok(())
}

pub(crate) async fn load_skill_mentions(
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

pub(crate) fn mentioned_skill_names(cwd: &PathBuf, text: &str) -> Result<Vec<String>> {
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

pub(crate) fn contains_skill_mention(text: &str, mention: &str) -> bool {
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

pub(crate) async fn compile_skill_cached(
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
