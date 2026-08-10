use std::path::PathBuf;

use anyhow::Result;

use crate::{
    agent,
    skill::{builtins, registry},
};

pub(crate) async fn handle_skill_command(
    runner: &mut agent::AgentRunner,
    cwd: &PathBuf,
    command: &str,
) -> Result<()> {
    let mut parts = command.split_whitespace();
    match parts.next() {
        None | Some("list") => {
            for skill in registry::list_status(cwd)? {
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
            for source in registry::discover_sources(cwd)? {
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
    for name in skill_names_for_prompt(cwd, text)? {
        let already_loaded = runner.loaded_skills().contains(&name);
        load_skill_into_runner(runner, cwd, &name, false).await?;
        if !already_loaded {
            loaded.push(name);
        }
    }
    Ok(loaded)
}

pub(crate) fn skill_names_for_prompt(cwd: &PathBuf, text: &str) -> Result<Vec<String>> {
    let mut names = mentioned_skill_names(cwd, text)?;
    names.extend(
        builtins::implicit_skill_names(text)
            .into_iter()
            .map(str::to_string),
    );
    names.sort();
    names.dedup();
    Ok(names)
}

pub(crate) fn mentioned_skill_names(cwd: &PathBuf, text: &str) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for source in registry::discover_sources(cwd)? {
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
) -> Result<registry::CompiledSkill> {
    if !refresh && let Some(cached) = registry::load_cached_if_fresh(cwd, name)? {
        return Ok(cached);
    }

    let (_, raw) = registry::source_text(cwd, name)?;
    match runner.compile_skill_summary(name, &raw).await {
        Ok(summary) => registry::compile_or_load_with_summary(cwd, name, true, Some(summary)),
        Err(error) => {
            tracing::warn!(
                skill = name,
                error = %format!("{error:#}"),
                "Spark skill compile failed; using local fallback"
            );
            registry::compile_or_load(cwd, name, true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_builtin_github_mentions_without_repo_skills() {
        let dir = tempfile::tempdir().expect("tempdir");

        let mentions = mentioned_skill_names(
            &dir.path().to_path_buf(),
            "Use @github to inspect the current pull request.",
        )
        .expect("mentions");

        assert_eq!(mentions, vec!["github"]);
    }

    #[test]
    fn implicitly_loads_github_for_pull_request_review_prompts() {
        let dir = tempfile::tempdir().expect("tempdir");

        let names = skill_names_for_prompt(
            &dir.path().to_path_buf(),
            "Review PR 42 on one of my GitHub repos and check its CI comments.",
        )
        .expect("skills");

        assert_eq!(names, vec!["github"]);
    }
}
