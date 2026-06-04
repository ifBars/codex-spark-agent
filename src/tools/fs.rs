use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use regex::RegexBuilder;
use serde_json::{Value, json};

use super::ToolResult;
use super::paths::{
    display_rel, missing_parent_dirs, required_str, required_u64, resolve_under,
    resolve_under_for_write, should_skip_discovery_dir,
};

const FS_READ_DEFAULT_LIMIT: u64 = 120;
const FS_READ_MAX_LIMIT: u64 = 400;
const FS_READ_MAX_CONTENT_CHARS: usize = 12_000;
const FS_LIST_DEFAULT_LIMIT: usize = 80;
const FS_LIST_MAX_LIMIT: usize = 200;
const FS_LIST_MAX_ENTRIES_CHARS: usize = 12_000;
const FS_SEARCH_DEFAULT_LIMIT: usize = 50;
const FS_SEARCH_MAX_LIMIT: usize = 100;
const FS_SEARCH_MAX_SNIPPET_CHARS: usize = 600;

pub(super) fn fs_read(cwd: &Path, args: Value) -> Result<ToolResult> {
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
        .unwrap_or(FS_READ_DEFAULT_LIMIT)
        .clamp(1, FS_READ_MAX_LIMIT);
    let line_numbers = args
        .get("line_numbers")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let content = std::fs::read_to_string(&full)
        .with_context(|| format!("failed to read {}", full.display()))?;
    let total_lines = content.lines().count();
    let start_index = (offset - 1) as usize;
    let (lines, returned_lines, content_truncated) = bounded_read_window(
        content
            .lines()
            .skip(start_index)
            .take(limit as usize)
            .enumerate(),
        offset,
        line_numbers,
        FS_READ_MAX_CONTENT_CHARS,
    );
    let has_more = start_index.saturating_add(returned_lines) < total_lines || content_truncated;
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
            "content_truncated": content_truncated,
            "max_content_chars": FS_READ_MAX_CONTENT_CHARS,
        }),
        error: None,
    })
}

fn bounded_read_window<'a>(
    lines: impl Iterator<Item = (usize, &'a str)>,
    offset: u64,
    line_numbers: bool,
    max_content_chars: usize,
) -> (String, usize, bool) {
    let mut selected = Vec::new();
    let mut content_chars = 0usize;
    let mut returned_lines = 0usize;
    let mut content_truncated = false;

    for (idx, line) in lines {
        let formatted = if line_numbers {
            format!("{}: {}", offset + idx as u64, line)
        } else {
            line.to_string()
        };
        let separator_chars = usize::from(!selected.is_empty());
        let next_chars = content_chars + separator_chars + formatted.len();
        if next_chars > max_content_chars {
            content_truncated = true;
            if selected.is_empty() {
                selected.push(compact_middle(&formatted, max_content_chars));
                returned_lines += 1;
            }
            break;
        }
        content_chars = next_chars;
        selected.push(formatted);
        returned_lines += 1;
    }

    (selected.join("\n"), returned_lines, content_truncated)
}

pub(super) fn fs_list(cwd: &Path, args: Value) -> Result<ToolResult> {
    let path = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let recursive = args
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let max_depth = args.get("max_depth").and_then(Value::as_u64).unwrap_or(2) as usize;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(FS_LIST_DEFAULT_LIMIT)
        .clamp(1, FS_LIST_MAX_LIMIT);
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
    let chars_truncated = truncate_entries_to_budget(&mut entries, FS_LIST_MAX_ENTRIES_CHARS)?;
    truncated = truncated || chars_truncated;
    Ok(ToolResult {
        ok: true,
        data: json!({
            "path": display_rel(cwd, &root),
            "entries": entries,
            "truncated": truncated,
            "limit": limit,
            "returned_entries": entries.len(),
            "max_entries_chars": FS_LIST_MAX_ENTRIES_CHARS,
            "entries_truncated_by_chars": chars_truncated,
        }),
        error: None,
    })
}

fn truncate_entries_to_budget(entries: &mut Vec<Value>, max_chars: usize) -> Result<bool> {
    let mut truncated = false;
    while serde_json::to_string(entries)?.len() > max_chars {
        if entries.pop().is_none() {
            break;
        }
        truncated = true;
    }
    Ok(truncated)
}

pub(super) fn fs_stat(cwd: &Path, args: Value) -> Result<ToolResult> {
    let path = required_str(&args, "path")?;
    let full = if cwd.join(path).exists() {
        resolve_under(cwd, path)?
    } else {
        resolve_under_for_write(cwd, path)?
    };
    if !full.exists() {
        return Ok(ToolResult {
            ok: true,
            data: json!({
                "path": display_rel(cwd, &full),
                "exists": false,
            }),
            error: None,
        });
    }
    let meta =
        std::fs::metadata(&full).with_context(|| format!("failed to stat {}", full.display()))?;
    Ok(ToolResult {
        ok: true,
        data: json!({
            "path": display_rel(cwd, &full),
            "exists": true,
            "is_dir": meta.is_dir(),
            "is_file": meta.is_file(),
            "is_symlink": meta.file_type().is_symlink(),
            "size": meta.len(),
            "readonly": meta.permissions().readonly(),
        }),
        error: None,
    })
}

pub(super) fn fs_write(cwd: &Path, args: Value) -> Result<ToolResult> {
    let path = required_str(&args, "path")?;
    let content = required_str(&args, "content")?;
    let full = resolve_under_for_write(cwd, path)?;
    let previous_bytes = std::fs::metadata(&full).ok().map(|metadata| metadata.len());
    let created_parent_dirs = missing_parent_dirs(cwd, &full);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&full, content)
        .with_context(|| format!("failed to write {}", full.display()))?;
    Ok(ToolResult {
        ok: true,
        data: json!({
            "path": display_rel(cwd, &full),
            "bytes": content.len(),
            "previous_bytes": previous_bytes,
            "created": previous_bytes.is_none(),
            "created_parent_dirs": created_parent_dirs,
        }),
        error: None,
    })
}

pub(super) fn fs_search(cwd: &Path, args: Value) -> Result<ToolResult> {
    let query = required_str(&args, "query")?;
    let options = SearchOptions::from_args(cwd, &args, query)?;
    match fs_search_with_ripgrep(cwd, &options) {
        Ok(Some(result)) => Ok(result),
        Ok(None) => fs_search_in_process(cwd, &options),
        Err(error) => Err(error),
    }
}

struct SearchOptions<'a> {
    query: &'a str,
    root: PathBuf,
    regex: bool,
    case_sensitive: bool,
    max_depth: usize,
    limit: usize,
    context_lines: usize,
}

impl<'a> SearchOptions<'a> {
    fn from_args(cwd: &Path, args: &'a Value, query: &'a str) -> Result<Self> {
        let path = args.get("path").and_then(Value::as_str).unwrap_or(".");
        let root = resolve_under(cwd, path)?;
        Ok(Self {
            query,
            root,
            regex: args.get("regex").and_then(Value::as_bool).unwrap_or(false),
            case_sensitive: args
                .get("case_sensitive")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            max_depth: args
                .get("max_depth")
                .and_then(Value::as_u64)
                .unwrap_or(6)
                .min(12) as usize,
            limit: args
                .get("limit")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
                .unwrap_or(FS_SEARCH_DEFAULT_LIMIT)
                .clamp(1, FS_SEARCH_MAX_LIMIT),
            context_lines: args
                .get("context_lines")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .min(3) as usize,
        })
    }
}

fn fs_search_with_ripgrep(cwd: &Path, options: &SearchOptions<'_>) -> Result<Option<ToolResult>> {
    let mut command = Command::new("rg");
    command
        .current_dir(cwd)
        .arg("--json")
        .arg("--line-number")
        .arg("--with-filename")
        .arg("--color")
        .arg("never")
        .arg("--no-messages")
        .arg("--hidden")
        .arg("--max-filesize")
        .arg("2M")
        .arg("--max-depth")
        .arg(options.max_depth.to_string());
    if !options.regex {
        command.arg("--fixed-strings");
    }
    if !options.case_sensitive {
        command.arg("--ignore-case");
    }
    add_ripgrep_skip_globs(&mut command, &options.root);
    command.arg("--").arg(options.query).arg(&options.root);

    let mut child = match command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("failed to start ripgrep"),
    };
    let stdout = child
        .stdout
        .take()
        .context("failed to capture ripgrep stdout")?;
    let mut stderr = child
        .stderr
        .take()
        .context("failed to capture ripgrep stderr")?;

    let mut matches = Vec::new();
    let mut unique_match_files = HashMap::<String, ()>::new();
    let mut file_cache = HashMap::<PathBuf, Vec<String>>::new();
    let mut files_scanned = None;
    let mut snippets_truncated = 0usize;
    let mut truncated = false;

    for line in BufReader::new(stdout).lines() {
        let line = line.context("failed to read ripgrep output")?;
        if line.trim().is_empty() {
            continue;
        }
        let event: Value =
            serde_json::from_str(&line).with_context(|| format!("invalid ripgrep json: {line}"))?;
        match event.get("type").and_then(Value::as_str) {
            Some("match") => {
                let data = &event["data"];
                let Some(path_text) = data["path"]["text"].as_str() else {
                    continue;
                };
                let Some(line_number) = data["line_number"].as_u64() else {
                    continue;
                };
                let full_path = path_from_ripgrep(cwd, path_text);
                let (snippet, snippet_truncated) = search_snippet(
                    &mut file_cache,
                    &full_path,
                    line_number as usize,
                    options.context_lines,
                )?;
                if snippet_truncated {
                    snippets_truncated += 1;
                }
                unique_match_files.insert(display_rel(cwd, &full_path), ());
                matches.push(json!({
                    "path": display_rel(cwd, &full_path),
                    "line": line_number,
                    "snippet": snippet,
                    "snippet_truncated": snippet_truncated,
                }));
                if matches.len() >= options.limit {
                    truncated = true;
                    let _ = child.kill();
                    break;
                }
            }
            Some("summary") => {
                files_scanned = event["data"]["stats"]["searches"]
                    .as_u64()
                    .map(|value| value as usize);
            }
            _ => {}
        }
    }

    let mut stderr_text = String::new();
    let _ = stderr.read_to_string(&mut stderr_text);
    let status = child.wait().context("failed to wait for ripgrep")?;
    if !truncated && !status.success() && status.code() != Some(1) {
        anyhow::bail!(
            "ripgrep search failed: {}",
            stderr_text.trim().if_empty("unknown ripgrep error")
        );
    }

    Ok(Some(search_result(
        cwd,
        options,
        matches,
        files_scanned.unwrap_or(unique_match_files.len()),
        truncated,
        snippets_truncated,
        "ripgrep",
    )))
}

fn fs_search_in_process(cwd: &Path, options: &SearchOptions<'_>) -> Result<ToolResult> {
    let matcher = SearchMatcher::new(options)?;
    let mut matches = Vec::new();
    let mut files_scanned = 0usize;
    let mut truncated = false;
    let mut snippets_truncated = 0usize;
    let mut stack = vec![(options.root.clone(), 0usize)];
    while let Some((path, depth)) = stack.pop() {
        if matches.len() >= options.limit {
            truncated = true;
            break;
        }
        let metadata = match std::fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.is_dir() {
            if depth > options.max_depth || should_skip_discovery_dir(&options.root, &path) {
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
            if !matcher.is_match(line) {
                continue;
            }
            let start = idx.saturating_sub(options.context_lines);
            let end = (idx + options.context_lines + 1).min(lines.len());
            let raw_snippet = lines[start..end]
                .iter()
                .enumerate()
                .map(|(offset, text)| format!("{}: {}", start + offset + 1, text))
                .collect::<Vec<_>>()
                .join("\n");
            let snippet_truncated = raw_snippet.len() > FS_SEARCH_MAX_SNIPPET_CHARS;
            if snippet_truncated {
                snippets_truncated += 1;
            }
            matches.push(json!({
                "path": display_rel(cwd, &path),
                "line": idx + 1,
                "snippet": compact_middle(&raw_snippet, FS_SEARCH_MAX_SNIPPET_CHARS),
                "snippet_truncated": snippet_truncated,
            }));
            if matches.len() >= options.limit {
                truncated = true;
                break;
            }
        }
    }

    Ok(search_result(
        cwd,
        options,
        matches,
        files_scanned,
        truncated,
        snippets_truncated,
        "fallback",
    ))
}

enum SearchMatcher {
    Literal {
        needle: String,
        case_sensitive: bool,
    },
    Regex(regex::Regex),
}

impl SearchMatcher {
    fn new(options: &SearchOptions<'_>) -> Result<Self> {
        if options.regex {
            let regex = RegexBuilder::new(options.query)
                .case_insensitive(!options.case_sensitive)
                .build()
                .with_context(|| format!("invalid regex query: {}", options.query))?;
            return Ok(Self::Regex(regex));
        }
        let needle = if options.case_sensitive {
            options.query.to_string()
        } else {
            options.query.to_lowercase()
        };
        Ok(Self::Literal {
            needle,
            case_sensitive: options.case_sensitive,
        })
    }

    fn is_match(&self, line: &str) -> bool {
        match self {
            Self::Literal {
                needle,
                case_sensitive,
            } => {
                if *case_sensitive {
                    line.contains(needle)
                } else {
                    line.to_lowercase().contains(needle)
                }
            }
            Self::Regex(regex) => regex.is_match(line),
        }
    }
}

fn add_ripgrep_skip_globs(command: &mut Command, root: &Path) {
    if is_generated_discovery_dir(root) {
        return;
    }
    for directory in [
        ".git",
        ".hg",
        ".svn",
        "target",
        "node_modules",
        ".spark",
        ".spark-runs",
        ".spark-profile",
        ".spark-codex",
    ] {
        command.arg("--glob").arg(format!("!**/{directory}/**"));
    }
}

fn is_generated_discovery_dir(path: &Path) -> bool {
    matches!(
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

fn path_from_ripgrep(cwd: &Path, path_text: &str) -> PathBuf {
    let path = PathBuf::from(path_text);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn search_snippet(
    file_cache: &mut HashMap<PathBuf, Vec<String>>,
    path: &Path,
    line_number: usize,
    context_lines: usize,
) -> Result<(String, bool)> {
    if !file_cache.contains_key(path) {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read matched file {}", path.display()))?;
        file_cache.insert(
            path.to_path_buf(),
            content.lines().map(str::to_string).collect(),
        );
    }
    let lines = file_cache.get(path).context("matched file cache missing")?;
    let idx = line_number.saturating_sub(1).min(lines.len());
    let start = idx.saturating_sub(context_lines);
    let end = (idx + context_lines + 1).min(lines.len());
    let raw_snippet = lines[start..end]
        .iter()
        .enumerate()
        .map(|(offset, text)| format!("{}: {}", start + offset + 1, text))
        .collect::<Vec<_>>()
        .join("\n");
    let snippet_truncated = raw_snippet.len() > FS_SEARCH_MAX_SNIPPET_CHARS;
    Ok((
        compact_middle(&raw_snippet, FS_SEARCH_MAX_SNIPPET_CHARS),
        snippet_truncated,
    ))
}

fn search_result(
    cwd: &Path,
    options: &SearchOptions<'_>,
    matches: Vec<Value>,
    files_scanned: usize,
    truncated: bool,
    snippets_truncated: usize,
    engine: &str,
) -> ToolResult {
    ToolResult {
        ok: true,
        data: json!({
            "query": options.query,
            "path": display_rel(cwd, &options.root),
            "matches": matches,
            "files_scanned": files_scanned,
            "truncated": truncated,
            "limit": options.limit,
            "regex": options.regex,
            "engine": engine,
            "max_snippet_chars": FS_SEARCH_MAX_SNIPPET_CHARS,
            "snippets_truncated": snippets_truncated,
        }),
        error: None,
    }
}

trait EmptyStrFallback {
    fn if_empty<'a>(&'a self, fallback: &'a str) -> &'a str;
}

impl EmptyStrFallback for str {
    fn if_empty<'a>(&'a self, fallback: &'a str) -> &'a str {
        if self.is_empty() { fallback } else { self }
    }
}

fn compact_middle(raw: &str, max_chars: usize) -> String {
    if raw.len() <= max_chars {
        return raw.to_string();
    }
    let marker = "\n...[truncated]...\n";
    let budget = max_chars.saturating_sub(marker.len());
    let head_len = budget.saturating_mul(3) / 4;
    let tail_len = budget.saturating_sub(head_len);
    let head = raw.chars().take(head_len).collect::<String>();
    let tail = raw
        .chars()
        .rev()
        .take(tail_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{head}{marker}{tail}")
}

pub(super) fn fs_replace(cwd: &Path, args: Value) -> Result<ToolResult> {
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

pub(super) fn fs_edit(cwd: &Path, args: Value) -> Result<ToolResult> {
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

pub(super) fn fs_rename(cwd: &Path, args: Value) -> Result<ToolResult> {
    let from = required_str(&args, "from")?;
    let to = required_str(&args, "to")?;
    let source = resolve_under(cwd, from)?;
    let destination = resolve_under_for_write(cwd, to)?;
    if destination.exists() {
        anyhow::bail!("destination already exists; nothing was moved");
    }
    let created_parent_dirs = missing_parent_dirs(cwd, &destination);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let metadata = std::fs::metadata(&source)
        .with_context(|| format!("failed to inspect {}", source.display()))?;
    std::fs::rename(&source, &destination).with_context(|| {
        format!(
            "failed to rename {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(ToolResult {
        ok: true,
        data: json!({
            "from": display_rel(cwd, &source),
            "to": display_rel(cwd, &destination),
            "is_file": metadata.is_file(),
            "is_dir": metadata.is_dir(),
            "bytes": metadata.len(),
            "created_parent_dirs": created_parent_dirs,
        }),
        error: None,
    })
}
