use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use regex::RegexBuilder;
use serde_json::{Value, json};

use super::ToolResult;
use super::paths::{
    display_rel, missing_parent_dirs, required_str, required_u64, resolve_read_path, resolve_under,
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

#[cfg(test)]
pub(super) fn fs_read(cwd: &Path, args: Value) -> Result<ToolResult> {
    fs_read_with_read_roots(cwd, &[], args)
}

pub(super) fn fs_read_with_read_roots(
    cwd: &Path,
    read_roots: &[PathBuf],
    args: Value,
) -> Result<ToolResult> {
    let path = required_str(&args, "path")?;
    let full = match resolve_read_path(cwd, read_roots, path) {
        Ok(path) => path,
        Err(error) => return Ok(failed_read_path_result(cwd, path, &error.to_string())),
    };
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
    let content = read_text_file(&full)?;
    let total_lines = content.lines().count();
    let total_chars = content.chars().count();
    let total_words = count_words(&content);
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
            "total_chars": total_chars,
            "total_words": total_words,
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

fn count_words(text: &str) -> usize {
    let mut count = 0usize;
    let mut in_word = false;
    for character in text.chars() {
        if character.is_alphanumeric() || character == '\'' {
            if !in_word {
                count += 1;
                in_word = true;
            }
        } else {
            in_word = false;
        }
    }
    count
}

fn failed_read_path_result(cwd: &Path, raw_path: &str, error: &str) -> ToolResult {
    ToolResult {
        ok: false,
        data: json!({
            "path": raw_path,
            "error_kind": "path_not_found",
            "message": format!("path not found: {raw_path}; file was not read"),
            "details": {
                "resolver_error": error,
                "suggestions": read_path_suggestions(cwd, raw_path, 5),
                "hint": "Retry with an exact path from suggestions, or use fs.list on the nearest existing parent.",
            },
        }),
        error: Some(format!("path not found: {raw_path}; file was not read")),
    }
}

fn read_path_suggestions(cwd: &Path, raw_path: &str, limit: usize) -> Vec<String> {
    let raw = Path::new(raw_path);
    if raw.is_absolute() {
        return Vec::new();
    }
    let Ok(cwd) = std::fs::canonicalize(cwd) else {
        return Vec::new();
    };

    let mut current = cwd.clone();
    let mut components = raw.components().peekable();
    while let Some(component) = components.next() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => {
                let candidate = current.join(part);
                if candidate.exists() {
                    current = candidate;
                    continue;
                }
                let rest = components
                    .filter_map(|component| match component {
                        std::path::Component::Normal(part) => Some(part.to_owned()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                return similar_child_paths(&cwd, &current, part, &rest, limit);
            }
            _ => return Vec::new(),
        }
    }

    Vec::new()
}

fn similar_child_paths(
    cwd: &Path,
    parent: &Path,
    missing: &std::ffi::OsStr,
    rest: &[std::ffi::OsString],
    limit: usize,
) -> Vec<String> {
    let Some(missing) = missing.to_str() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut suggestions = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name();
            let name_text = name.to_str()?;
            similar_path_component(missing, name_text).then(|| {
                let mut path = entry.path();
                for component in rest {
                    path.push(component);
                }
                display_rel(cwd, &path)
            })
        })
        .collect::<Vec<_>>();
    suggestions.sort();
    suggestions.truncate(limit);
    suggestions
}

fn similar_path_component(missing: &str, candidate: &str) -> bool {
    let missing = missing.to_ascii_lowercase();
    let candidate = candidate.to_ascii_lowercase();
    missing.contains(&candidate)
        || candidate.contains(&missing)
        || levenshtein_distance(&missing, &candidate)
            <= (missing.len().max(candidate.len()) / 3).max(2)
}

fn levenshtein_distance(left: &str, right: &str) -> usize {
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    let mut current = vec![0; right_chars.len() + 1];
    for (left_index, left_char) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let insertion = current[right_index] + 1;
            let deletion = previous[right_index + 1] + 1;
            let substitution = previous[right_index] + usize::from(left_char != *right_char);
            current[right_index + 1] = insertion.min(deletion).min(substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right_chars.len()]
}

#[cfg(test)]
pub(super) fn fs_list(cwd: &Path, args: Value) -> Result<ToolResult> {
    fs_list_with_read_roots(cwd, &[], args)
}

pub(super) fn fs_list_with_read_roots(
    cwd: &Path,
    read_roots: &[PathBuf],
    args: Value,
) -> Result<ToolResult> {
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
    let root = resolve_read_path(cwd, read_roots, path)?;
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

#[cfg(test)]
pub(super) fn fs_stat(cwd: &Path, args: Value) -> Result<ToolResult> {
    fs_stat_with_read_roots(cwd, &[], args)
}

pub(super) fn fs_stat_with_read_roots(
    cwd: &Path,
    read_roots: &[PathBuf],
    args: Value,
) -> Result<ToolResult> {
    let path = required_str(&args, "path")?;
    let full = if cwd.join(path).exists() {
        resolve_under(cwd, path)?
    } else if let Ok(path) = resolve_read_path(cwd, read_roots, path) {
        path
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

#[cfg(test)]
pub(super) fn fs_search(cwd: &Path, args: Value) -> Result<ToolResult> {
    fs_search_with_read_roots(cwd, &[], args)
}

pub(super) fn fs_search_with_read_roots(
    cwd: &Path,
    read_roots: &[PathBuf],
    args: Value,
) -> Result<ToolResult> {
    let query = required_str(&args, "query")?;
    let options = SearchOptions::from_args(cwd, read_roots, &args, query)?;
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
    fn from_args(
        cwd: &Path,
        read_roots: &[PathBuf],
        args: &'a Value,
        query: &'a str,
    ) -> Result<Self> {
        let path = args.get("path").and_then(Value::as_str).unwrap_or(".");
        let root = resolve_read_path(cwd, read_roots, path)?;
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
    if is_generated_discovery_path(&options.root) {
        command.arg("--no-ignore");
    }
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
        let Ok(content) = read_text_file(&path) else {
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
    for directory in [
        ".git",
        ".hg",
        ".svn",
        "target",
        "node_modules",
        ".spark",
        ".spark-runs",
        ".spark-scenarios",
        ".spark-profile",
        ".spark-codex",
    ] {
        if path_has_component(root, directory) {
            continue;
        }
        command.arg("--glob").arg(format!("!**/{directory}/**"));
    }
}

fn is_generated_discovery_path(path: &Path) -> bool {
    [
        ".git",
        ".hg",
        ".svn",
        "target",
        "node_modules",
        ".spark",
        ".spark-runs",
        ".spark-scenarios",
        ".spark-profile",
        ".spark-codex",
    ]
    .iter()
    .any(|directory| path_has_component(path, directory))
}

fn path_has_component(path: &Path, name: &str) -> bool {
    path.components()
        .any(|component| component.as_os_str().to_string_lossy() == name)
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
        let content = read_text_file(path)?;
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

fn read_text_file(path: &Path) -> Result<String> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    decode_text_bytes(&bytes).with_context(|| format!("failed to decode {}", path.display()))
}

fn decode_text_bytes(bytes: &[u8]) -> Result<String> {
    if let Some(content) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(content.to_vec()).context("invalid utf-8 text");
    }
    if let Some(content) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16_bytes(content, true);
    }
    if let Some(content) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16_bytes(content, false);
    }
    String::from_utf8(bytes.to_vec()).context("invalid utf-8 text")
}

fn decode_utf16_bytes(bytes: &[u8], little_endian: bool) -> Result<String> {
    if bytes.len() % 2 != 0 {
        anyhow::bail!("invalid utf-16 text: odd byte length");
    }
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| {
            if little_endian {
                u16::from_le_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_be_bytes([chunk[0], chunk[1]])
            }
        })
        .collect::<Vec<_>>();
    String::from_utf16(&units).context("invalid utf-16 text")
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
    let mut replace_plan = replacement_plan(&content, old, new);
    let mut display_line_numbers_stripped = false;
    let stripped_replace = strip_replace_display_line_number_prefixes(old, new);
    if replace_plan.replacements == 0
        && !replace_plan.ambiguous_leading_indent_match
        && let Some((stripped_old, stripped_new)) = stripped_replace.as_ref()
    {
        let stripped_plan = replacement_plan(&content, stripped_old, stripped_new);
        if stripped_plan.replacements > 0 || stripped_plan.ambiguous_leading_indent_match {
            replace_plan = stripped_plan;
            display_line_numbers_stripped = true;
        }
    }
    let replacements = replace_plan.replacements;
    if replace_plan.ambiguous_leading_indent_match {
        let message = format!(
            "old text matched {replacements} leading-indent-equivalent blocks; file was not changed"
        );
        return Ok(failed_file_tool_result(
            cwd,
            &full,
            "ambiguous_old_text",
            &message,
            json!({
                "actual_replacements": replacements,
                "hint": "Use fs.edit with an exact line range when leading-indent-equivalent old text is ambiguous.",
            }),
        ));
    }
    if let Some(expected) = args.get("expected_replacements").and_then(Value::as_u64)
        && replacements != expected as usize
    {
        let message = format!(
            "expected {expected} replacements but found {replacements}; file was not changed"
        );
        return Ok(failed_file_tool_result(
            cwd,
            &full,
            "replacement_count_mismatch",
            &message,
            json!({
                "expected_replacements": expected,
                "actual_replacements": replacements,
                "matches": text_match_contexts(&content, &replace_plan.old, 5),
                "hint": "Use fs.edit with the exact line range when the old text is ambiguous.",
            }),
        ));
    }
    if replacements == 0 {
        return Ok(failed_file_tool_result(
            cwd,
            &full,
            "old_text_not_found",
            "old text not found; file was not changed",
            json!({
                "hint": "Read the target lines or retry with text copied exactly from the latest file contents.",
            }),
        ));
    }
    let updated = content.replace(&replace_plan.old, &replace_plan.new);
    std::fs::write(&full, updated)
        .with_context(|| format!("failed to write {}", full.display()))?;
    Ok(ToolResult {
        ok: true,
        data: json!({
            "path": display_rel(cwd, &full),
            "replacements": replacements,
            "line_ending_normalized": replace_plan.line_ending_normalized,
            "leading_indent_normalized": replace_plan.leading_indent_normalized,
            "display_line_numbers_stripped": display_line_numbers_stripped,
        }),
        error: None,
    })
}

struct ReplacementPlan {
    old: String,
    new: String,
    replacements: usize,
    line_ending_normalized: bool,
    leading_indent_normalized: bool,
    ambiguous_leading_indent_match: bool,
}

fn replacement_plan(content: &str, old: &str, new: &str) -> ReplacementPlan {
    let exact_replacements = content.matches(old).count();
    if exact_replacements > 0 {
        return ReplacementPlan {
            old: old.to_string(),
            new: new.to_string(),
            replacements: exact_replacements,
            line_ending_normalized: false,
            leading_indent_normalized: false,
            ambiguous_leading_indent_match: false,
        };
    }

    if let Some((old_variant, new_variant)) = line_ending_variant_for_content(content, old, new) {
        let replacements = content.matches(&old_variant).count();
        if replacements > 0 {
            return ReplacementPlan {
                old: old_variant,
                new: new_variant,
                replacements,
                line_ending_normalized: true,
                leading_indent_normalized: false,
                ambiguous_leading_indent_match: false,
            };
        }
    }

    if let Some((old_variant, new_variant, replacements, line_ending_normalized)) =
        leading_indent_variant_for_content(content, old, new)
    {
        return ReplacementPlan {
            old: old_variant,
            new: new_variant,
            replacements,
            line_ending_normalized,
            leading_indent_normalized: true,
            ambiguous_leading_indent_match: false,
        };
    }

    if let Some((replacements, line_ending_normalized)) =
        ambiguous_leading_indent_matches(content, old)
    {
        return ReplacementPlan {
            old: old.to_string(),
            new: new.to_string(),
            replacements,
            line_ending_normalized,
            leading_indent_normalized: true,
            ambiguous_leading_indent_match: true,
        };
    }

    ReplacementPlan {
        old: old.to_string(),
        new: new.to_string(),
        replacements: 0,
        line_ending_normalized: false,
        leading_indent_normalized: false,
        ambiguous_leading_indent_match: false,
    }
}

fn strip_replace_display_line_number_prefixes(old: &str, new: &str) -> Option<(String, String)> {
    Some((
        strip_display_line_number_prefixes_from_block(old)?,
        strip_display_line_number_prefixes_from_block(new)?,
    ))
}

fn strip_display_line_number_prefixes_from_block(text: &str) -> Option<String> {
    let lines = normalized_lines_without_trailing_newline(text);
    if lines.is_empty() {
        return None;
    }

    let mut first_number = None;
    let mut stripped_lines = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        let (line_number, stripped) = strip_display_line_number_prefix(line)?;
        let expected_number = *first_number.get_or_insert(line_number) + index;
        if line_number != expected_number {
            return None;
        }
        stripped_lines.push(stripped);
    }

    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let stripped = stripped_lines.join(newline);
    if text.ends_with('\n') {
        Some(format!("{stripped}{newline}"))
    } else {
        Some(stripped)
    }
}

fn strip_display_line_number_prefix(line: &str) -> Option<(usize, &str)> {
    let colon_index = line.find(':')?;
    let line_number = line[..colon_index].parse::<usize>().ok()?;
    let rest = &line[colon_index + 1..];
    Some((line_number, rest.strip_prefix(' ').unwrap_or(rest)))
}

fn line_ending_variant_for_content(
    content: &str,
    old: &str,
    new: &str,
) -> Option<(String, String)> {
    if content.contains("\r\n") && old.contains('\n') && !old.contains("\r\n") {
        return Some((old.replace('\n', "\r\n"), new.replace('\n', "\r\n")));
    }
    if !content.contains("\r\n") && old.contains("\r\n") {
        return Some((old.replace("\r\n", "\n"), new.replace("\r\n", "\n")));
    }
    None
}

fn leading_indent_variant_for_content(
    content: &str,
    old: &str,
    new: &str,
) -> Option<(String, String, usize, bool)> {
    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let line_ending_normalized =
        (content.contains("\r\n") && old.contains('\n') && !old.contains("\r\n"))
            || (!content.contains("\r\n") && old.contains("\r\n"));
    let normalized_old = old.replace("\r\n", "\n");
    let normalized_new = new.replace("\r\n", "\n");
    let old_lines = normalized_lines_without_trailing_newline(&normalized_old);
    if old_lines.is_empty() {
        return None;
    }

    let content_lines = content.lines().collect::<Vec<_>>();
    if old_lines.len() > content_lines.len() {
        return None;
    }

    let mut matches = Vec::new();
    for start in 0..=(content_lines.len() - old_lines.len()) {
        let candidate = &content_lines[start..start + old_lines.len()];
        if candidate
            .iter()
            .zip(old_lines.iter())
            .all(|(actual, expected)| actual.trim_start() == expected.trim_start())
            && candidate
                .iter()
                .zip(old_lines.iter())
                .any(|(actual, expected)| actual != expected)
        {
            matches.push(candidate.join(newline));
        }
    }

    if matches.len() != 1 {
        return None;
    }
    Some((
        matches.remove(0),
        normalized_new.replace('\n', newline),
        1,
        line_ending_normalized,
    ))
}

fn ambiguous_leading_indent_matches(content: &str, old: &str) -> Option<(usize, bool)> {
    let line_ending_normalized =
        (content.contains("\r\n") && old.contains('\n') && !old.contains("\r\n"))
            || (!content.contains("\r\n") && old.contains("\r\n"));
    let normalized_old = old.replace("\r\n", "\n");
    let old_lines = normalized_lines_without_trailing_newline(&normalized_old);
    if old_lines.is_empty() {
        return None;
    }

    let content_lines = content.lines().collect::<Vec<_>>();
    if old_lines.len() > content_lines.len() {
        return None;
    }

    let matches = (0..=(content_lines.len() - old_lines.len()))
        .filter(|start| {
            let candidate = &content_lines[*start..*start + old_lines.len()];
            candidate
                .iter()
                .zip(old_lines.iter())
                .all(|(actual, expected)| actual.trim_start() == expected.trim_start())
                && candidate
                    .iter()
                    .zip(old_lines.iter())
                    .any(|(actual, expected)| actual != expected)
        })
        .count();

    (matches > 1).then_some((matches, line_ending_normalized))
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
            return Ok(failed_file_tool_result(
                cwd,
                &full,
                "line_range_out_of_bounds",
                &format!(
                    "end_line {end_line} exceeds file line count {line_count}; file was not changed"
                ),
                json!({
                    "start_line": start_line,
                    "end_line": end_line,
                    "line_count": line_count,
                    "available_range": line_range_context(&lines, start_line, line_count),
                    "hint": "Retry fs.edit with an end_line no larger than line_count, or use fs.write if replacing the whole file.",
                }),
            ));
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
    let mut replacement_text = replacement.to_string();
    let mut expected_old_indent_adjusted = false;
    let mut expected_old_line_numbers_stripped = false;
    if let Some(expected_old) = args.get("expected_old").and_then(Value::as_str)
        && !same_edit_expected_old_text(expected_old, &old_text)
    {
        let stripped_expected_old =
            strip_expected_old_line_number_prefixes(expected_old, start_line, end_line, newline);
        let mut candidate_expected_old = expected_old;
        if let Some(stripped) = stripped_expected_old.as_deref()
            && (same_edit_expected_old_text(stripped, &old_text)
                || leading_indent_tolerant_replacement(stripped, &old_text, replacement, newline)
                    .is_some())
        {
            candidate_expected_old = stripped;
            expected_old_line_numbers_stripped = true;
        }

        if same_edit_expected_old_text(candidate_expected_old, &old_text) {
            // Accept the exact line-range text after removing copied display line numbers.
        } else if let Some(adjusted) = leading_indent_tolerant_replacement(
            candidate_expected_old,
            &old_text,
            replacement,
            newline,
        ) {
            replacement_text = adjusted;
            expected_old_indent_adjusted = true;
        } else {
            return Ok(failed_file_tool_result(
                cwd,
                &full,
                "expected_old_mismatch",
                "expected_old did not match current line range; file was not changed",
                json!({
                    "start_line": start_line,
                    "end_line": end_line,
                    "current_text": compact_middle(&old_text, 2_000),
                    "current_lines": line_range_context(&lines, start_line, end_line),
                    "hint": "Retry with expected_old copied from current_text, adjust the line range, or omit expected_old only when the range was just verified.",
                }),
            ));
        }
    }

    let replacement_lines = replacement_text
        .strip_suffix("\r\n")
        .or_else(|| replacement_text.strip_suffix('\n'))
        .unwrap_or(&replacement_text)
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
    if had_trailing_newline || replacement_text.ends_with('\n') {
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
            "new_lines": replacement_text.lines().count(),
            "expected_old_indent_adjusted": expected_old_indent_adjusted,
            "expected_old_line_numbers_stripped": expected_old_line_numbers_stripped,
        }),
        error: None,
    })
}

fn same_edit_expected_old_text(expected_old: &str, old_text: &str) -> bool {
    same_text_ignoring_line_endings(expected_old, old_text)
        || expected_old
            .strip_suffix("\r\n")
            .or_else(|| expected_old.strip_suffix('\n'))
            .is_some_and(|trimmed| same_text_ignoring_line_endings(trimmed, old_text))
}

fn strip_expected_old_line_number_prefixes(
    expected_old: &str,
    start_line: usize,
    end_line: usize,
    newline: &str,
) -> Option<String> {
    if end_line < start_line {
        return None;
    }

    let expected_lines = normalized_lines_without_trailing_newline(expected_old);
    let line_count = end_line - start_line + 1;
    if expected_lines.is_empty() || expected_lines.len() != line_count {
        return None;
    }

    let mut stripped_lines = Vec::with_capacity(expected_lines.len());
    for (index, line) in expected_lines.iter().enumerate() {
        let prefix = format!("{}:", start_line + index);
        let stripped = line.strip_prefix(&prefix)?;
        stripped_lines.push(stripped.strip_prefix(' ').unwrap_or(stripped));
    }

    let stripped = stripped_lines.join(newline);
    if expected_old.ends_with('\n') {
        Some(format!("{stripped}{newline}"))
    } else {
        Some(stripped)
    }
}

fn leading_indent_tolerant_replacement(
    expected_old: &str,
    old_text: &str,
    replacement: &str,
    newline: &str,
) -> Option<String> {
    let expected_lines = normalized_lines_without_trailing_newline(expected_old);
    let old_lines = normalized_lines_without_trailing_newline(old_text);
    let replacement_lines = normalized_lines_without_trailing_newline(replacement);
    if expected_lines.is_empty()
        || expected_lines.len() != old_lines.len()
        || replacement_lines.len() != old_lines.len()
    {
        return None;
    }

    let mut differs_only_by_leading_indent = false;
    for (expected, old) in expected_lines.iter().zip(old_lines.iter()) {
        if expected.trim_start() != old.trim_start() {
            return None;
        }
        if expected != old {
            differs_only_by_leading_indent = true;
        }
    }
    if !differs_only_by_leading_indent {
        return None;
    }

    let adjusted = replacement_lines
        .iter()
        .zip(old_lines.iter())
        .map(|(replacement_line, old_line)| {
            if replacement_line.is_empty() || starts_with_whitespace(replacement_line) {
                (*replacement_line).to_string()
            } else {
                format!("{}{}", leading_whitespace(old_line), replacement_line)
            }
        })
        .collect::<Vec<_>>()
        .join(newline);

    if replacement.ends_with('\n') {
        Some(format!("{adjusted}{newline}"))
    } else {
        Some(adjusted)
    }
}

fn normalized_lines_without_trailing_newline(text: &str) -> Vec<&str> {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
        .lines()
        .collect()
}

fn starts_with_whitespace(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|character| character.is_whitespace())
}

fn leading_whitespace(text: &str) -> &str {
    let end = text
        .char_indices()
        .find_map(|(index, character)| (!character.is_whitespace()).then_some(index))
        .unwrap_or(text.len());
    &text[..end]
}

fn same_text_ignoring_line_endings(left: &str, right: &str) -> bool {
    left == right || normalize_line_endings(left) == normalize_line_endings(right)
}

fn normalize_line_endings(value: &str) -> String {
    value.replace("\r\n", "\n")
}

fn failed_file_tool_result(
    cwd: &Path,
    full: &Path,
    error_kind: &str,
    message: &str,
    details: Value,
) -> ToolResult {
    ToolResult {
        ok: false,
        data: json!({
            "path": display_rel(cwd, full),
            "error_kind": error_kind,
            "message": message,
            "details": details,
        }),
        error: Some(message.to_string()),
    }
}

fn text_match_contexts(content: &str, pattern: &str, limit: usize) -> Vec<Value> {
    if pattern.is_empty() {
        return Vec::new();
    }
    content
        .match_indices(pattern)
        .take(limit)
        .map(|(start, matched)| {
            let start_line = line_number_at_byte(content, start);
            let end_line = line_number_at_byte(content, start + matched.len());
            json!({
                "start_line": start_line,
                "end_line": end_line,
                "text": compact_middle(matched, 1_000),
            })
        })
        .collect()
}

fn line_number_at_byte(content: &str, byte_index: usize) -> usize {
    content[..byte_index.min(content.len())]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn line_range_context(lines: &[String], start_line: usize, end_line: usize) -> Value {
    if lines.is_empty() || start_line == 0 || start_line > lines.len() || end_line < start_line {
        return json!({
            "start_line": start_line,
            "end_line": end_line,
            "content": "",
        });
    }
    let clamped_end = end_line.min(lines.len());
    json!({
        "start_line": start_line,
        "end_line": clamped_end,
        "content": compact_middle(&lines[start_line - 1..clamped_end].join("\n"), 2_000),
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
