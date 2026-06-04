use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

pub(super) fn required_str<'a>(args: &'a Value, name: &str) -> Result<&'a str> {
    args.get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{name} is required"))
}

pub(super) fn required_u64(args: &Value, name: &str) -> Result<u64> {
    args.get(name)
        .and_then(Value::as_u64)
        .with_context(|| format!("{name} is required"))
}

pub(super) fn resolve_under(cwd: &Path, raw: &str) -> Result<PathBuf> {
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

pub(super) fn resolve_under_for_write(cwd: &Path, raw: &str) -> Result<PathBuf> {
    let cwd = std::fs::canonicalize(cwd)
        .with_context(|| format!("failed to resolve workspace {}", cwd.display()))?;
    let raw_path = Path::new(raw);
    if raw_path.is_absolute() {
        anyhow::bail!("path escapes workspace: {}", raw);
    }

    let mut relative = PathBuf::new();
    for component in raw_path.components() {
        match component {
            std::path::Component::Normal(part) => relative.push(part),
            std::path::Component::CurDir => {}
            _ => anyhow::bail!("path escapes workspace: {}", raw),
        }
    }
    if relative.as_os_str().is_empty() {
        anyhow::bail!("path is required");
    }

    let candidate = cwd.join(&relative);
    if candidate.exists() {
        let resolved = std::fs::canonicalize(&candidate)
            .with_context(|| format!("failed to resolve path {}", candidate.display()))?;
        if !resolved.starts_with(&cwd) {
            anyhow::bail!("path escapes workspace: {}", raw);
        }
        return Ok(resolved);
    }

    let mut existing_parent = candidate
        .parent()
        .with_context(|| format!("path is required: {raw}"))?;
    let mut missing_components = Vec::new();
    while !existing_parent.exists() {
        if let Some(name) = existing_parent.file_name() {
            missing_components.push(name.to_owned());
        }
        existing_parent = existing_parent
            .parent()
            .with_context(|| format!("path escapes workspace: {raw}"))?;
    }

    let resolved_parent = std::fs::canonicalize(existing_parent)
        .with_context(|| format!("failed to resolve parent {}", existing_parent.display()))?;
    if !resolved_parent.starts_with(&cwd) {
        anyhow::bail!("path escapes workspace: {}", raw);
    }

    let mut resolved = resolved_parent;
    for component in missing_components.iter().rev() {
        resolved.push(component);
    }
    if let Some(name) = candidate.file_name() {
        resolved.push(name);
    }
    Ok(resolved)
}

pub(super) fn missing_parent_dirs(cwd: &Path, target: &Path) -> Vec<String> {
    let cwd = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    let mut missing = Vec::<PathBuf>::new();
    let Some(mut current) = target.parent() else {
        return Vec::new();
    };
    while current.starts_with(&cwd) && !current.exists() {
        missing.push(current.to_path_buf());
        let Some(parent) = current.parent() else {
            break;
        };
        current = parent;
    }
    missing.reverse();
    missing
        .into_iter()
        .map(|path| display_rel(&cwd, &path))
        .collect()
}

pub(super) fn display_rel(cwd: &Path, path: &Path) -> String {
    let cwd = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    path.strip_prefix(&cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn should_skip_discovery_dir(root: &Path, path: &Path) -> bool {
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
