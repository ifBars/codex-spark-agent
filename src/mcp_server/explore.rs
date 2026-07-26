use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::agent::AgentRunner;
use crate::tools::AgentMode;
use crate::{
    DEFAULT_COMPACT_AFTER_CHARS, DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS, DEFAULT_MAX_INPUT_CHARS,
    DEFAULT_MODEL,
};

const MAX_TASK_CHARS: usize = 16_000;
const MAX_CONTEXT_CHARS: usize = 64_000;
const MAX_STARTING_PATHS: usize = 32;
const MAX_INSTRUCTION_CHARS: usize = 64_000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExploreArgs {
    task: String,
    #[serde(default)]
    cwd: Option<PathBuf>,
    #[serde(default)]
    starting_paths: Vec<String>,
    #[serde(default)]
    context: Option<String>,
    #[serde(default)]
    reasoning_effort: Option<String>,
    #[serde(default)]
    trace: bool,
}

#[derive(Debug)]
struct InstructionContext {
    body: String,
    paths: Vec<String>,
}

pub(super) fn tool_definition() -> Value {
    json!({
        "name": "explore_repo",
        "title": "Explore a repository with Spark",
        "description": "Use proactively for non-trivial read-only repository exploration, file discovery, call-chain tracing, architecture reconnaissance, and hypothesis checks before the parent Codex agent edits or decides. This runs GPT-5.3-Codex-Spark through the dedicated Spark harness and returns a compact evidence brief. Do not use for a single known file or lookup that one direct read/search can answer.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "One concrete exploration question with the desired evidence."
                },
                "cwd": {
                    "type": "string",
                    "description": "Absolute repository or workspace directory. Defaults to the MCP server process working directory."
                },
                "starting_paths": {
                    "type": "array",
                    "items": {"type": "string"},
                    "maxItems": MAX_STARTING_PATHS,
                    "description": "Optional repository-relative files or directories Spark should inspect first."
                },
                "context": {
                    "type": "string",
                    "description": "Only task-relevant parent Codex constraints, decisions, skill guidance, or memory. Do not forward secrets or the full parent transcript."
                },
                "reasoning_effort": {
                    "type": "string",
                    "enum": ["low", "medium", "high", "xhigh"],
                    "default": "medium"
                },
                "trace": {
                    "type": "boolean",
                    "default": false,
                    "description": "Save the harness trace under the explored workspace's ignored .spark-runs directory."
                }
            },
            "required": ["task"],
            "additionalProperties": false
        }
    })
}

pub(super) async fn run(arguments: Value, cancellation: CancellationToken) -> Result<String> {
    let args: ExploreArgs =
        serde_json::from_value(arguments).context("invalid explore_repo arguments")?;
    validate_args(&args)?;

    let cwd = resolve_cwd(args.cwd.as_deref())?;
    let reasoning_effort = args
        .reasoning_effort
        .as_deref()
        .unwrap_or("medium")
        .to_string();
    let instruction_context = load_instruction_context(&cwd, &args.starting_paths)?;
    let prompt = build_prompt(&args, &instruction_context);
    let auth = crate::config::load_auth()?;
    let mut runner = AgentRunner::new_with_reasoning_effort(
        auth,
        cwd.clone(),
        DEFAULT_MODEL.to_string(),
        reasoning_effort.clone(),
        args.trace,
        false,
        DEFAULT_COMPACT_AFTER_CHARS,
        DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS,
        DEFAULT_MAX_INPUT_CHARS,
        false,
        None,
        false,
        Some(json!({
            "mcp_tool": "explore_repo",
            "starting_paths": args.starting_paths,
            "instruction_files": instruction_context.paths,
        })),
        AgentMode::Ask,
    )?;
    runner.use_buffered_display();
    runner.disable_mcp();
    runner.disable_subagents();
    runner.set_system_prompt(Some(
        "You are the read-only Spark repository explorer behind a native Codex MCP bridge. Use only local filesystem evidence. Do not use web search, edit files, execute commands, or delegate. Stop when the concrete question is answered and return the requested compact evidence brief."
            .to_string(),
    ));

    runner.run_with_cancel_to_text(&prompt, cancellation).await
}

fn validate_args(args: &ExploreArgs) -> Result<()> {
    let task = args.task.trim();
    if task.is_empty() {
        bail!("task is required");
    }
    if task.len() > MAX_TASK_CHARS {
        bail!("task exceeds {MAX_TASK_CHARS} characters");
    }
    if args.starting_paths.len() > MAX_STARTING_PATHS {
        bail!("starting_paths exceeds {MAX_STARTING_PATHS} entries");
    }
    for path in &args.starting_paths {
        let path = Path::new(path);
        if path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
            bail!(
                "starting_paths must stay relative to cwd: {}",
                path.display()
            );
        }
    }
    if args
        .context
        .as_deref()
        .is_some_and(|context| context.len() > MAX_CONTEXT_CHARS)
    {
        bail!("context exceeds {MAX_CONTEXT_CHARS} characters");
    }
    if let Some(reasoning_effort) = args.reasoning_effort.as_deref()
        && !matches!(reasoning_effort, "low" | "medium" | "high" | "xhigh")
    {
        bail!("reasoning_effort must be low, medium, high, or xhigh");
    }
    Ok(())
}

fn resolve_cwd(cwd: Option<&Path>) -> Result<PathBuf> {
    let cwd = cwd
        .map(Path::to_path_buf)
        .unwrap_or(std::env::current_dir()?);
    let cwd = std::fs::canonicalize(&cwd)
        .with_context(|| format!("failed to resolve workspace {}", cwd.display()))?;
    if !cwd.is_dir() {
        bail!("workspace is not a directory: {}", cwd.display());
    }
    Ok(cwd)
}

fn build_prompt(args: &ExploreArgs, instructions: &InstructionContext) -> String {
    let starting_paths = if args.starting_paths.is_empty() {
        "(none supplied)".to_string()
    } else {
        args.starting_paths
            .iter()
            .map(|path| format!("- {path}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let parent_context = args
        .context
        .as_deref()
        .map(str::trim)
        .filter(|context| !context.is_empty())
        .unwrap_or("(none supplied)");
    let instruction_context = if instructions.body.is_empty() {
        "(no applicable AGENTS.md files found)".to_string()
    } else {
        instructions.body.clone()
    };

    format!(
        "Question:\n{}\n\nStarting paths:\n{}\n\nTask-relevant context forwarded by native Codex:\n{}\n\nApplicable repository instructions:\n{}\n\nReturn:\n1. Answer\n2. Evidence with repository-relative file paths and line numbers\n3. Risks or unknowns\n4. Recommended next inspection or validation step\n\nKeep the brief compact. Do not edit files or use the network.",
        args.task.trim(),
        starting_paths,
        parent_context,
        instruction_context,
    )
}

fn load_instruction_context(cwd: &Path, starting_paths: &[String]) -> Result<InstructionContext> {
    let repo_root = cwd
        .ancestors()
        .find(|path| path.join(".git").exists())
        .unwrap_or(cwd);
    let mut candidates = BTreeSet::new();
    collect_agents_between(repo_root, cwd, &mut candidates);
    for starting_path in starting_paths {
        let joined = cwd.join(starting_path);
        let directory = if joined.is_dir() {
            joined
        } else {
            joined.parent().unwrap_or(cwd).to_path_buf()
        };
        if directory.starts_with(repo_root) {
            collect_agents_between(repo_root, &directory, &mut candidates);
        }
    }

    let mut body = String::new();
    let mut paths = Vec::new();
    for path in candidates {
        if !path.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let relative = path
            .strip_prefix(repo_root)
            .unwrap_or(&path)
            .display()
            .to_string();
        let section = format!("\n### {relative}\n{}\n", raw.trim());
        if body.len() + section.len() > MAX_INSTRUCTION_CHARS {
            break;
        }
        body.push_str(&section);
        paths.push(relative);
    }
    Ok(InstructionContext {
        body: body.trim().to_string(),
        paths,
    })
}

fn collect_agents_between(root: &Path, target: &Path, candidates: &mut BTreeSet<PathBuf>) {
    if !target.starts_with(root) {
        return;
    }
    let mut current = root.to_path_buf();
    candidates.insert(current.join("AGENTS.md"));
    let Ok(relative) = target.strip_prefix(root) else {
        return;
    };
    for component in relative.components() {
        current.push(component.as_os_str());
        candidates.insert(current.join("AGENTS.md"));
    }
}

#[cfg(test)]
mod tests {
    use super::{ExploreArgs, load_instruction_context, tool_definition, validate_args};
    use serde_json::json;

    #[test]
    fn explorer_tool_has_no_turn_budget() {
        let tool = tool_definition();
        let schema = &tool["inputSchema"]["properties"];
        assert!(schema.get("max_turns").is_none());
        assert_eq!(tool["name"], "explore_repo");
        assert!(tool.get("outputSchema").is_none());
    }

    #[test]
    fn explorer_rejects_parent_path_starting_points() {
        let args: ExploreArgs = serde_json::from_value(json!({
            "task": "Trace the parser",
            "starting_paths": ["../outside"]
        }))
        .expect("args");
        assert!(validate_args(&args).is_err());
    }

    #[test]
    fn nested_agents_files_are_loaded_in_scope_order() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(".git")).expect("git dir");
        std::fs::create_dir_all(dir.path().join("src/parser")).expect("nested dir");
        std::fs::write(dir.path().join("AGENTS.md"), "root rules").expect("root agents");
        std::fs::write(dir.path().join("src/AGENTS.md"), "src rules").expect("src agents");

        let context = load_instruction_context(dir.path(), &["src/parser/input.rs".to_string()])
            .expect("instruction context");

        let normalized = context
            .paths
            .iter()
            .map(|path| path.replace('\\', "/"))
            .collect::<Vec<_>>();
        assert_eq!(normalized, vec!["AGENTS.md", "src/AGENTS.md"]);
        assert!(context.body.find("root rules") < context.body.find("src rules"));
    }
}
