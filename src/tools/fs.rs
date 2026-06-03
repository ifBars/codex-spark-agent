use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::ToolResult;
use super::paths::{
    display_rel, missing_parent_dirs, required_str, required_u64, resolve_under,
    resolve_under_for_write, should_skip_discovery_dir,
};

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
