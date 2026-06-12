use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

const ARGS_PLACEHOLDER: &str = "{{args}}";
const CLAUDE_ARGS_PLACEHOLDER: &str = "$ARGUMENTS";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct PromptCommand {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) source_path: String,
}

#[derive(Debug, Clone)]
struct PromptCommandSource {
    command: PromptCommand,
    path: PathBuf,
}

pub(crate) fn discover_commands(cwd: &Path) -> Result<Vec<PromptCommand>> {
    Ok(discover_sources(cwd)?
        .into_iter()
        .map(|source| source.command)
        .collect())
}

pub(crate) fn expand_command(cwd: &Path, name: &str, args: &str) -> Result<String> {
    let source = find_source(cwd, name)?;
    let raw = std::fs::read_to_string(&source.path)
        .with_context(|| format!("failed to read {}", source.path.display()))?;
    Ok(expand_body(strip_frontmatter(&raw).trim(), args))
}

pub(crate) fn expand_slash_command(cwd: &Path, input: &str) -> Result<Option<String>> {
    let Some(token) = crate::chat::slash_command_token(input) else {
        return Ok(None);
    };
    let Some(name) = token.strip_prefix('/') else {
        return Ok(None);
    };
    if name.is_empty() {
        return Ok(None);
    }
    let args = crate::chat::command_args(input, token).unwrap_or("").trim();
    match expand_command(cwd, name, args) {
        Ok(expanded) => Ok(Some(expanded)),
        Err(error) if is_not_found_error(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

fn discover_sources(cwd: &Path) -> Result<Vec<PromptCommandSource>> {
    let mut sources = Vec::new();
    for commands_dir in [
        cwd.join(".agents").join("commands"),
        cwd.join(".spark").join("commands"),
        cwd.join(".claude").join("commands"),
    ] {
        collect_command_dir(cwd, &commands_dir, &commands_dir, &mut sources)?;
    }
    sources.sort_by(|left, right| left.command.name.cmp(&right.command.name));
    sources.dedup_by(|left, right| left.command.name == right.command.name);
    Ok(sources)
}

fn collect_command_dir(
    cwd: &Path,
    root_dir: &Path,
    commands_dir: &Path,
    sources: &mut Vec<PromptCommandSource>,
) -> Result<()> {
    if !commands_dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(commands_dir)
        .with_context(|| format!("failed to list {}", commands_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_command_dir(cwd, root_dir, &path, sources)?;
            continue;
        }
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = command_name(root_dir, &path) else {
            continue;
        };
        if !is_safe_command_name(&name) {
            continue;
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let metadata = parse_frontmatter(&raw);
        sources.push(PromptCommandSource {
            command: PromptCommand {
                name,
                description: metadata.description.unwrap_or_default(),
                source_path: display_rel(cwd, &path),
            },
            path,
        });
    }
    Ok(())
}

fn find_source(cwd: &Path, name: &str) -> Result<PromptCommandSource> {
    if !is_safe_command_name(name) {
        anyhow::bail!("invalid command name `{name}`");
    }
    discover_sources(cwd)?
        .into_iter()
        .find(|source| source.command.name == name)
        .with_context(|| {
            format!(
                "command `{name}` not found under .agents/commands, .spark/commands, or .claude/commands"
            )
        })
}

fn expand_body(body: &str, args: &str) -> String {
    let args = args.trim();
    if body.contains(ARGS_PLACEHOLDER) {
        return body.replace(ARGS_PLACEHOLDER, args).trim_end().to_string();
    }
    if body.contains(CLAUDE_ARGS_PLACEHOLDER) {
        return body
            .replace(CLAUDE_ARGS_PLACEHOLDER, args)
            .trim_end()
            .to_string();
    }
    if args.is_empty() {
        return body.trim_end().to_string();
    }
    format!("{}\n\nArguments: {args}", body.trim_end())
}

#[derive(Default)]
struct CommandMetadata {
    description: Option<String>,
}

fn parse_frontmatter(raw: &str) -> CommandMetadata {
    let Some(rest) = raw.strip_prefix("---") else {
        return CommandMetadata::default();
    };
    let Some((frontmatter, _)) = rest.split_once("---") else {
        return CommandMetadata::default();
    };

    let mut metadata = CommandMetadata::default();
    for line in frontmatter.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.trim() == "description" {
            metadata.description = Some(value.trim().trim_matches('"').to_string());
        }
    }
    metadata
}

fn strip_frontmatter(raw: &str) -> &str {
    let Some(rest) = raw.strip_prefix("---") else {
        return raw;
    };
    let Some((_, body)) = rest.split_once("---") else {
        return raw;
    };
    body.trim_start()
}

fn is_safe_command_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
}

fn command_name(root_dir: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root_dir).ok()?;
    let mut parts = rel
        .iter()
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>();
    let file = parts.pop()?;
    let stem = Path::new(file.as_ref()).file_stem()?.to_string_lossy();
    parts.push(stem);
    Some(parts.join(":"))
}

fn display_rel(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_not_found_error(error: &anyhow::Error) -> bool {
    error
        .to_string()
        .contains("not found under .agents/commands, .spark/commands, or .claude/commands")
}
