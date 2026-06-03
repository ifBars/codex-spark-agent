use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub ok: bool,
    pub data: Value,
    pub error: Option<String>,
}

pub fn builtin_tools() -> Vec<ToolDescriptor> {
    vec![
        ToolDescriptor {
            name: "fs.read".to_string(),
            description: "Read a UTF-8 text file under the workspace. Supports offset and limit line windows.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "offset": {"type": "integer", "minimum": 1},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 2000},
                    "line_numbers": {"type": "boolean"}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "fs.list".to_string(),
            description: "List files and directories under the workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "recursive": {"type": "boolean"},
                    "max_depth": {"type": "integer", "minimum": 0, "maximum": 8},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 2000}
                },
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "fs.write".to_string(),
            description: "Write a UTF-8 text file under the workspace, creating parent directories if needed.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "fs.search".to_string(),
            description: "Search UTF-8 files under the workspace for a literal query and return matching line snippets.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "path": {"type": "string"},
                    "case_sensitive": {"type": "boolean"},
                    "max_depth": {"type": "integer", "minimum": 0, "maximum": 12},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 500},
                    "context_lines": {"type": "integer", "minimum": 0, "maximum": 5}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "fs.replace".to_string(),
            description: "Replace exact UTF-8 text in one workspace file. Optionally require an expected replacement count.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "old": {"type": "string"},
                    "new": {"type": "string"},
                    "expected_replacements": {"type": "integer", "minimum": 1}
                },
                "required": ["path", "old", "new"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "fs.edit".to_string(),
            description: "Edit one UTF-8 file by replacing an inclusive 1-based line range. Use end_line one less than start_line to insert before start_line.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "start_line": {"type": "integer", "minimum": 1},
                    "end_line": {"type": "integer", "minimum": 0},
                    "replacement": {"type": "string"},
                    "expected_old": {"type": "string"}
                },
                "required": ["path", "start_line", "end_line", "replacement"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "cmd.exec".to_string(),
            description: "Execute a shell command in the workspace. Use PowerShell-compatible commands on Windows.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "workdir": {"type": "string"},
                    "timeout_ms": {"type": "integer", "minimum": 1000, "maximum": 120000}
                },
                "additionalProperties": false
            }),
        },
    ]
}

pub async fn invoke(cwd: &Path, tool_name: &str, args: Value) -> ToolResult {
    match invoke_inner(cwd, tool_name, args).await {
        Ok(result) => result,
        Err(error) => ToolResult {
            ok: false,
            data: json!({}),
            error: Some(error.to_string()),
        },
    }
}

async fn invoke_inner(cwd: &Path, tool_name: &str, args: Value) -> Result<ToolResult> {
    match tool_name {
        "fs.read" => fs_read(cwd, args),
        "fs.list" => fs_list(cwd, args),
        "fs.write" => fs_write(cwd, args),
        "fs.search" => fs_search(cwd, args),
        "fs.replace" => fs_replace(cwd, args),
        "fs.edit" => fs_edit(cwd, args),
        "cmd.exec" => cmd_exec(cwd, args).await,
        _ => anyhow::bail!("unknown tool: {tool_name}"),
    }
}

fn fs_read(cwd: &Path, args: Value) -> Result<ToolResult> {
    let path = required_str(&args, "path")?;
    let full = resolve_under(cwd, path)?;
    let offset = args
        .get("offset")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1);
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(500)
        .clamp(1, 2000);
    let line_numbers = args
        .get("line_numbers")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let content = std::fs::read_to_string(&full)
        .with_context(|| format!("failed to read {}", full.display()))?;
    let total_lines = content.lines().count();
    let start_index = (offset - 1) as usize;
    let selected = content
        .lines()
        .skip(start_index)
        .take(limit as usize)
        .collect::<Vec<_>>();
    let returned_lines = selected.len();
    let has_more = start_index.saturating_add(returned_lines) < total_lines;
    let lines = selected
        .into_iter()
        .enumerate()
        .map(|(idx, line)| {
            if line_numbers {
                format!("{}: {}", offset + idx as u64, line)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(ToolResult {
        ok: true,
        data: json!({
            "path": display_rel(cwd, &full),
            "content": lines,
            "offset": offset,
            "limit": limit,
            "returned_lines": returned_lines,
            "total_lines": total_lines,
            "has_more": has_more,
            "next_offset": has_more.then_some(offset + returned_lines as u64),
        }),
        error: None,
    })
}

fn fs_list(cwd: &Path, args: Value) -> Result<ToolResult> {
    let path = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let recursive = args
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let max_depth = args.get("max_depth").and_then(Value::as_u64).unwrap_or(2) as usize;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(200)
        .clamp(1, 2000) as usize;
    let root = resolve_under(cwd, path)?;
    let mut stack = vec![(root.clone(), 0_usize)];
    let mut entries = Vec::new();
    let mut truncated = false;
    while let Some((dir, depth)) = stack.pop() {
        for entry in
            std::fs::read_dir(&dir).with_context(|| format!("failed to list {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let meta = entry.metadata()?;
            if meta.is_dir() && should_skip_discovery_dir(&root, &path) {
                continue;
            }
            entries.push(json!({
                "path": display_rel(cwd, &path),
                "is_dir": meta.is_dir(),
                "is_file": meta.is_file(),
                "size": meta.len(),
                "depth": depth,
            }));
            if entries.len() >= limit {
                truncated = true;
                break;
            }
            if recursive && meta.is_dir() && depth < max_depth {
                stack.push((path, depth + 1));
            }
        }
        if truncated {
            break;
        }
    }
    entries.sort_by_key(|entry| entry["path"].as_str().unwrap_or_default().to_string());
    Ok(ToolResult {
        ok: true,
        data: json!({"path": display_rel(cwd, &root), "entries": entries, "truncated": truncated}),
        error: None,
    })
}

fn fs_write(cwd: &Path, args: Value) -> Result<ToolResult> {
    let path = required_str(&args, "path")?;
    let content = required_str(&args, "content")?;
    let full = resolve_under(cwd, path)?;
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&full, content)
        .with_context(|| format!("failed to write {}", full.display()))?;
    Ok(ToolResult {
        ok: true,
        data: json!({"path": display_rel(cwd, &full), "bytes": content.len()}),
        error: None,
    })
}

fn fs_search(cwd: &Path, args: Value) -> Result<ToolResult> {
    let query = required_str(&args, "query")?;
    if query.is_empty() {
        anyhow::bail!("query is required");
    }
    let path = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let case_sensitive = args
        .get("case_sensitive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let max_depth = args
        .get("max_depth")
        .and_then(Value::as_u64)
        .unwrap_or(6)
        .min(12) as usize;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(100)
        .clamp(1, 500) as usize;
    let context_lines = args
        .get("context_lines")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(5) as usize;
    let root = resolve_under(cwd, path)?;
    let needle = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    let mut matches = Vec::new();
    let mut files_scanned = 0usize;
    let mut truncated = false;
    let mut stack = vec![(root.clone(), 0usize)];
    while let Some((path, depth)) = stack.pop() {
        if matches.len() >= limit {
            truncated = true;
            break;
        }
        let metadata = match std::fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.is_dir() {
            if depth > max_depth || should_skip_discovery_dir(&root, &path) {
                continue;
            }
            let mut children = std::fs::read_dir(&path)
                .with_context(|| format!("failed to list {}", path.display()))?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect::<Vec<_>>();
            children.sort();
            for child in children.into_iter().rev() {
                stack.push((child, depth + 1));
            }
            continue;
        }
        if !metadata.is_file() || metadata.len() > 2_000_000 {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        files_scanned += 1;
        let lines = content.lines().collect::<Vec<_>>();
        for (idx, line) in lines.iter().enumerate() {
            let haystack = if case_sensitive {
                line.to_string()
            } else {
                line.to_lowercase()
            };
            if !haystack.contains(&needle) {
                continue;
            }
            let start = idx.saturating_sub(context_lines);
            let end = (idx + context_lines + 1).min(lines.len());
            let snippet = lines[start..end]
                .iter()
                .enumerate()
                .map(|(offset, text)| format!("{}: {}", start + offset + 1, text))
                .collect::<Vec<_>>()
                .join("\n");
            matches.push(json!({
                "path": display_rel(cwd, &path),
                "line": idx + 1,
                "snippet": snippet,
            }));
            if matches.len() >= limit {
                truncated = true;
                break;
            }
        }
    }

    Ok(ToolResult {
        ok: true,
        data: json!({
            "query": query,
            "path": display_rel(cwd, &root),
            "matches": matches,
            "files_scanned": files_scanned,
            "truncated": truncated,
        }),
        error: None,
    })
}

fn fs_replace(cwd: &Path, args: Value) -> Result<ToolResult> {
    let path = required_str(&args, "path")?;
    let old = required_str(&args, "old")?;
    if old.is_empty() {
        anyhow::bail!("old must not be empty");
    }
    let new = required_str(&args, "new")?;
    let full = resolve_under(cwd, path)?;
    let content = std::fs::read_to_string(&full)
        .with_context(|| format!("failed to read {}", full.display()))?;
    let replacements = content.matches(old).count();
    if let Some(expected) = args.get("expected_replacements").and_then(Value::as_u64)
        && replacements != expected as usize
    {
        anyhow::bail!(
            "expected {expected} replacements but found {replacements}; file was not changed"
        );
    }
    if replacements == 0 {
        anyhow::bail!("old text not found; file was not changed");
    }
    let updated = content.replace(old, new);
    std::fs::write(&full, updated)
        .with_context(|| format!("failed to write {}", full.display()))?;
    Ok(ToolResult {
        ok: true,
        data: json!({"path": display_rel(cwd, &full), "replacements": replacements}),
        error: None,
    })
}

fn fs_edit(cwd: &Path, args: Value) -> Result<ToolResult> {
    let path = required_str(&args, "path")?;
    let start_line = required_u64(&args, "start_line")? as usize;
    let end_line = required_u64(&args, "end_line")? as usize;
    let replacement = required_str(&args, "replacement")?;
    let full = resolve_under(cwd, path)?;
    let content = std::fs::read_to_string(&full)
        .with_context(|| format!("failed to read {}", full.display()))?;
    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let had_trailing_newline = content.ends_with('\n');
    let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
    let line_count = lines.len();

    if start_line == 0 {
        anyhow::bail!("start_line must be at least 1");
    }
    if end_line >= start_line {
        if end_line > line_count {
            anyhow::bail!("end_line {end_line} exceeds file line count {line_count}");
        }
    } else if end_line + 1 != start_line {
        anyhow::bail!("for insertion, end_line must be exactly start_line - 1");
    }
    if start_line > line_count + 1 {
        anyhow::bail!(
            "start_line {start_line} exceeds append position {}",
            line_count + 1
        );
    }

    let old_text = if end_line >= start_line {
        lines[start_line - 1..end_line].join(newline)
    } else {
        String::new()
    };
    if let Some(expected_old) = args.get("expected_old").and_then(Value::as_str)
        && expected_old != old_text
    {
        anyhow::bail!("expected_old did not match current line range; file was not changed");
    }

    let replacement_lines = replacement
        .strip_suffix("\r\n")
        .or_else(|| replacement.strip_suffix('\n'))
        .unwrap_or(replacement)
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let start_index = start_line - 1;
    let end_index = if end_line >= start_line {
        end_line
    } else {
        start_index
    };
    lines.splice(start_index..end_index, replacement_lines);

    let mut updated = lines.join(newline);
    if had_trailing_newline || replacement.ends_with('\n') {
        updated.push_str(newline);
    }
    std::fs::write(&full, updated)
        .with_context(|| format!("failed to write {}", full.display()))?;

    Ok(ToolResult {
        ok: true,
        data: json!({
            "path": display_rel(cwd, &full),
            "start_line": start_line,
            "end_line": end_line,
            "old_lines": if end_line >= start_line { end_line - start_line + 1 } else { 0 },
            "new_lines": replacement.lines().count(),
        }),
        error: None,
    })
}

async fn cmd_exec(cwd: &Path, args: Value) -> Result<ToolResult> {
    let workdir = args
        .get("workdir")
        .and_then(Value::as_str)
        .map(|path| resolve_under(cwd, path))
        .transpose()?
        .unwrap_or_else(|| cwd.to_path_buf());
    let timeout_ms = args
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(30000);

    let mut command = if let Some(shell_command) = args.get("command").and_then(Value::as_str) {
        shell_command_for(&workdir, shell_command)
    } else {
        let cmd = required_str(&args, "cmd")?;
        let mut command = Command::new(cmd);
        if let Some(argv) = args.get("args").and_then(Value::as_array) {
            command.args(argv.iter().filter_map(Value::as_str));
        }
        command.current_dir(&workdir);
        command
    };
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        command.output(),
    )
    .await
    .context("command timed out")??;

    Ok(ToolResult {
        ok: output.status.success(),
        data: json!({
            "code": output.status.code(),
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
            "workdir": display_rel(cwd, &workdir),
        }),
        error: None,
    })
}

fn shell_command_for(workdir: &Path, command: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", command])
            .current_dir(workdir);
        cmd
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut cmd = Command::new("sh");
        cmd.args(["-lc", command]).current_dir(workdir);
        cmd
    }
}

fn required_str<'a>(args: &'a Value, name: &str) -> Result<&'a str> {
    args.get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{name} is required"))
}

fn required_u64(args: &Value, name: &str) -> Result<u64> {
    args.get(name)
        .and_then(Value::as_u64)
        .with_context(|| format!("{name} is required"))
}

fn resolve_under(cwd: &Path, raw: &str) -> Result<PathBuf> {
    let cwd = std::fs::canonicalize(cwd)
        .with_context(|| format!("failed to resolve workspace {}", cwd.display()))?;
    let candidate = cwd.join(raw);
    let resolved = if candidate.exists() {
        std::fs::canonicalize(&candidate)?
    } else {
        let parent = candidate.parent().unwrap_or(&cwd);
        let canonical_parent = std::fs::canonicalize(parent)
            .with_context(|| format!("failed to resolve parent {}", parent.display()))?;
        canonical_parent.join(candidate.file_name().unwrap_or_default())
    };
    if !resolved.starts_with(&cwd) {
        anyhow::bail!("path escapes workspace: {}", raw);
    }
    Ok(resolved)
}

fn display_rel(cwd: &Path, path: &Path) -> String {
    let cwd = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    path.strip_prefix(&cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn should_skip_discovery_dir(root: &Path, path: &Path) -> bool {
    path != root
        && matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some(
                ".git"
                    | ".hg"
                    | ".svn"
                    | "target"
                    | "node_modules"
                    | ".spark"
                    | ".spark-runs"
                    | ".spark-profile"
                    | ".spark-codex"
            )
        )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn builtin_tools_do_not_include_synthetic_completion_tool() {
        let names = builtin_tools()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();

        assert_eq!(names.len(), 7);
        assert!(!names.iter().any(|name| name == "agent.complete"));
        assert!(names.iter().any(|name| name == "cmd.exec"));
    }

    #[test]
    fn fs_search_returns_matching_line_snippets() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("sample.txt"), "alpha\nneedle here\nomega\n")
            .expect("write sample");

        let result = fs_search(
            dir.path(),
            json!({"query": "needle", "path": ".", "context_lines": 1}),
        )
        .expect("search");

        assert!(result.ok);
        assert_eq!(result.data["matches"][0]["path"], "sample.txt");
        assert_eq!(result.data["matches"][0]["line"], 2);
        assert!(
            result.data["matches"][0]["snippet"]
                .as_str()
                .expect("snippet")
                .contains("1: alpha")
        );
    }

    #[test]
    fn fs_read_reports_window_metadata() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("sample.txt"), "one\ntwo\nthree\n").expect("write sample");

        let result = fs_read(
            dir.path(),
            json!({"path": "sample.txt", "offset": 1, "limit": 2}),
        )
        .expect("read");

        assert_eq!(result.data["returned_lines"], 2);
        assert_eq!(result.data["total_lines"], 3);
        assert_eq!(result.data["has_more"], true);
        assert_eq!(result.data["next_offset"], 3);
    }

    #[test]
    fn fs_list_skips_generated_dirs_during_recursive_discovery() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("target/debug")).expect("create target");
        std::fs::create_dir_all(dir.path().join(".spark-runs/run-1")).expect("create traces");
        std::fs::create_dir_all(dir.path().join("src")).expect("create src");
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").expect("write src");
        std::fs::write(dir.path().join("target/debug/generated.txt"), "generated\n")
            .expect("write generated");

        let result = fs_list(
            dir.path(),
            json!({"path": ".", "recursive": true, "max_depth": 4, "limit": 100}),
        )
        .expect("list");
        let entries = result.data["entries"].as_array().expect("entries");
        let paths = entries
            .iter()
            .filter_map(|entry| entry["path"].as_str())
            .collect::<Vec<_>>();

        assert!(paths.contains(&"src"));
        assert!(paths.contains(&"src/main.rs"));
        assert!(!paths.iter().any(|path| path.starts_with("target")));
        assert!(!paths.iter().any(|path| path.starts_with(".spark-runs")));
    }

    #[test]
    fn fs_search_skips_generated_dirs_during_recursive_discovery() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("target")).expect("create target");
        std::fs::create_dir_all(dir.path().join("src")).expect("create src");
        std::fs::write(dir.path().join("target/generated.txt"), "needle\n").expect("write target");
        std::fs::write(dir.path().join("src/main.rs"), "needle\n").expect("write src");

        let result = fs_search(
            dir.path(),
            json!({"query": "needle", "path": ".", "max_depth": 4}),
        )
        .expect("search");
        let matches = result.data["matches"].as_array().expect("matches");
        let paths = matches
            .iter()
            .filter_map(|entry| entry["path"].as_str())
            .collect::<Vec<_>>();

        assert_eq!(paths, vec!["src/main.rs"]);
    }

    #[test]
    fn fs_replace_requires_expected_count_before_writing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample.txt");
        std::fs::write(&path, "alpha beta beta\n").expect("write sample");

        let error = fs_replace(
            dir.path(),
            json!({
                "path": "sample.txt",
                "old": "beta",
                "new": "gamma",
                "expected_replacements": 1
            }),
        )
        .expect_err("expected count mismatch");

        assert!(error.to_string().contains("expected 1 replacements"));
        assert_eq!(
            std::fs::read_to_string(&path).expect("read unchanged"),
            "alpha beta beta\n"
        );
    }

    #[test]
    fn fs_replace_updates_exact_text() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample.txt");
        std::fs::write(&path, "alpha beta\n").expect("write sample");

        let result = fs_replace(
            dir.path(),
            json!({
                "path": "sample.txt",
                "old": "beta",
                "new": "gamma",
                "expected_replacements": 1
            }),
        )
        .expect("replace");

        assert!(result.ok);
        assert_eq!(result.data["replacements"], 1);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read updated"),
            "alpha gamma\n"
        );
    }

    #[test]
    fn fs_edit_replaces_inclusive_line_range() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample.txt");
        std::fs::write(&path, "one\ntwo\nthree\n").expect("write sample");

        let result = fs_edit(
            dir.path(),
            json!({
                "path": "sample.txt",
                "start_line": 2,
                "end_line": 2,
                "replacement": "TWO",
                "expected_old": "two"
            }),
        )
        .expect("edit");

        assert!(result.ok);
        assert_eq!(result.data["old_lines"], 1);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read updated"),
            "one\nTWO\nthree\n"
        );
    }

    #[test]
    fn fs_edit_inserts_when_end_line_precedes_start_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample.txt");
        std::fs::write(&path, "one\nthree\n").expect("write sample");

        let result = fs_edit(
            dir.path(),
            json!({
                "path": "sample.txt",
                "start_line": 2,
                "end_line": 1,
                "replacement": "two"
            }),
        )
        .expect("insert");

        assert!(result.ok);
        assert_eq!(result.data["old_lines"], 0);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read updated"),
            "one\ntwo\nthree\n"
        );
    }

    #[test]
    fn fs_edit_expected_old_mismatch_does_not_write() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample.txt");
        std::fs::write(&path, "one\ntwo\nthree\n").expect("write sample");

        let error = fs_edit(
            dir.path(),
            json!({
                "path": "sample.txt",
                "start_line": 2,
                "end_line": 2,
                "replacement": "TWO",
                "expected_old": "wrong"
            }),
        )
        .expect_err("expected mismatch");

        assert!(error.to_string().contains("expected_old did not match"));
        assert_eq!(
            std::fs::read_to_string(&path).expect("read unchanged"),
            "one\ntwo\nthree\n"
        );
    }
}
