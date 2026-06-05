use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct SkillSource {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledSkill {
    pub name: String,
    pub description: String,
    pub source_path: String,
    pub source_hash: String,
    pub compiled_at: u64,
    pub summary: String,
    pub triggers: Vec<String>,
    pub full_text_chars: usize,
}

pub fn list_status(cwd: &Path) -> Result<Vec<SkillStatus>> {
    let mut statuses = Vec::new();
    for source in discover_sources(cwd)? {
        let raw = std::fs::read_to_string(&source.path)
            .with_context(|| format!("failed to read {}", source.path.display()))?;
        let metadata = parse_frontmatter(&raw);
        let source_hash = sha256_hex(raw.as_bytes());
        let cache_path = cache_path(cwd, &source.name)?;
        let cache_status = match read_cached(&cache_path) {
            Ok(cached) if cached.source_hash == source_hash => "fresh",
            Ok(_) => "stale",
            Err(_) => "missing",
        }
        .to_string();
        statuses.push(SkillStatus {
            name: source.name,
            description: metadata.description.unwrap_or_default(),
            source_path: display_rel(cwd, &source.path),
            cache_status,
        });
    }
    statuses.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(statuses)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillStatus {
    pub name: String,
    pub description: String,
    pub source_path: String,
    pub cache_status: String,
}

pub fn compile_or_load(cwd: &Path, name: &str, refresh: bool) -> Result<CompiledSkill> {
    compile_or_load_with_summary(cwd, name, refresh, None)
}

pub fn compile_or_load_with_summary(
    cwd: &Path,
    name: &str,
    refresh: bool,
    summary_override: Option<String>,
) -> Result<CompiledSkill> {
    let source = find_source(cwd, name)?;
    let raw = std::fs::read_to_string(&source.path)
        .with_context(|| format!("failed to read {}", source.path.display()))?;
    let source_hash = sha256_hex(raw.as_bytes());
    let cache_path = cache_path(cwd, &source.name)?;

    if summary_override.is_none()
        && !refresh
        && let Ok(cached) = read_cached(&cache_path)
        && cached.source_hash == source_hash
    {
        return Ok(cached);
    }

    let compiled = compile_source(cwd, &source, &raw, source_hash, summary_override)?;
    write_cached(&cache_path, &compiled)?;
    Ok(compiled)
}

pub fn load_cached_if_fresh(cwd: &Path, name: &str) -> Result<Option<CompiledSkill>> {
    let source = find_source(cwd, name)?;
    let raw = std::fs::read_to_string(&source.path)
        .with_context(|| format!("failed to read {}", source.path.display()))?;
    let source_hash = sha256_hex(raw.as_bytes());
    let cache_path = cache_path(cwd, &source.name)?;
    let Ok(cached) = read_cached(&cache_path) else {
        return Ok(None);
    };
    Ok((cached.source_hash == source_hash).then_some(cached))
}

pub fn source_text(cwd: &Path, name: &str) -> Result<(SkillSource, String)> {
    let source = find_source(cwd, name)?;
    let raw = std::fs::read_to_string(&source.path)
        .with_context(|| format!("failed to read {}", source.path.display()))?;
    Ok((source, raw))
}

pub fn discover_sources(cwd: &Path) -> Result<Vec<SkillSource>> {
    let skills_dir = cwd.join(".agents").join("skills");
    if !skills_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sources = Vec::new();
    for entry in std::fs::read_dir(&skills_dir)
        .with_context(|| format!("failed to list {}", skills_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path().join("SKILL.md");
        if !path.exists() {
            continue;
        }
        let folder_name = entry.file_name().to_string_lossy().to_string();
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let metadata = parse_frontmatter(&raw);
        sources.push(SkillSource {
            name: metadata.name.unwrap_or(folder_name),
            path,
        });
    }
    sources.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(sources)
}

fn find_source(cwd: &Path, name: &str) -> Result<SkillSource> {
    discover_sources(cwd)?
        .into_iter()
        .find(|source| source.name == name)
        .with_context(|| format!("skill `{name}` not found under .agents/skills"))
}

fn compile_source(
    cwd: &Path,
    source: &SkillSource,
    raw: &str,
    source_hash: String,
    summary_override: Option<String>,
) -> Result<CompiledSkill> {
    let metadata = parse_frontmatter(raw);
    let description = metadata.description.unwrap_or_default();
    let body = strip_frontmatter(raw);
    let triggers = extract_section_bullets(body, "When to Use", 12);
    let mut summary_parts = Vec::new();

    if !description.is_empty() {
        summary_parts.push(format!("Description: {description}"));
    }
    push_section(&mut summary_parts, body, "When to Use", 1600);
    push_section(&mut summary_parts, body, "Workflow", 2200);
    push_section(&mut summary_parts, body, "Core Principles", 2600);
    push_section(&mut summary_parts, body, "Useful Defaults", 1600);
    push_section(&mut summary_parts, body, "Output Shape", 1200);
    if summary_parts.len() == usize::from(!description.is_empty()) {
        summary_parts.push(take_chars(body.trim(), 5000));
    }

    let summary = summary_override.unwrap_or_else(|| {
        format!(
            "Spark skill: {}\n\n{}",
            source.name,
            summary_parts.join("\n\n")
        )
    });

    Ok(CompiledSkill {
        name: source.name.clone(),
        description,
        source_path: display_rel(cwd, &source.path),
        source_hash,
        compiled_at: now_secs(),
        summary,
        triggers,
        full_text_chars: raw.len(),
    })
}

fn cache_path(cwd: &Path, name: &str) -> Result<PathBuf> {
    if !is_safe_skill_name(name) {
        anyhow::bail!("invalid skill name `{name}`");
    }
    Ok(cwd
        .join(".spark")
        .join("skills")
        .join(format!("{name}.json")))
}

fn read_cached(path: &Path) -> Result<CompiledSkill> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("failed to parse {}", path.display()))
}

fn write_cached(path: &Path, skill: &CompiledSkill) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(skill)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

#[derive(Default)]
struct SkillMetadata {
    name: Option<String>,
    description: Option<String>,
}

fn parse_frontmatter(raw: &str) -> SkillMetadata {
    let Some(rest) = raw.strip_prefix("---") else {
        return SkillMetadata::default();
    };
    let Some((frontmatter, _)) = rest.split_once("---") else {
        return SkillMetadata::default();
    };

    let mut metadata = SkillMetadata::default();
    for line in frontmatter.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_matches('"').to_string();
        match key.trim() {
            "name" => metadata.name = Some(value),
            "description" => metadata.description = Some(value),
            _ => {}
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

fn push_section(parts: &mut Vec<String>, body: &str, heading: &str, max_chars: usize) {
    if let Some(section) = extract_section(body, heading) {
        let trimmed = take_chars(section.trim(), max_chars);
        if !trimmed.is_empty() {
            parts.push(format!("{heading}:\n{trimmed}"));
        }
    }
}

fn extract_section(body: &str, heading: &str) -> Option<String> {
    let mut in_section = false;
    let mut collected = Vec::new();
    for line in body.lines() {
        if is_heading(line, heading) {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with('#') {
            break;
        }
        if in_section {
            collected.push(line);
        }
    }
    (!collected.is_empty()).then(|| {
        let len = collected.iter().map(|line| line.len() + 1).sum::<usize>();
        let mut section = String::with_capacity(len);
        for line in collected {
            section.push_str(line);
            section.push('\n');
        }
        section
    })
}

fn extract_section_bullets(body: &str, heading: &str, limit: usize) -> Vec<String> {
    extract_section(body, heading)
        .map(|section| {
            section
                .lines()
                .filter_map(|line| {
                    line.trim()
                        .strip_prefix("- ")
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .map(str::to_string)
                })
                .take(limit)
                .collect()
        })
        .unwrap_or_default()
}

fn is_heading(line: &str, heading: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('#') && trimmed.trim_start_matches('#').trim() == heading
}

fn take_chars(value: &str, max_chars: usize) -> String {
    if value.len() <= max_chars {
        return value.to_string();
    }
    let mut out = value.chars().take(max_chars).collect::<String>();
    out.push_str("\n...[skill summary truncated]...");
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn display_rel(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_safe_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_skill_and_reuses_cache_when_hash_matches() {
        let dir = tempfile::tempdir().expect("tempdir");
        let skill_dir = dir.path().join(".agents").join("skills").join("demo");
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: demo
description: Demo skill
---

# Demo

## When to Use

- Rust work
- Code review

## Workflow

1. Inspect.
2. Patch.
"#,
        )
        .expect("write skill");

        let first = compile_or_load(dir.path(), "demo", false).expect("compile");
        let second = compile_or_load(dir.path(), "demo", false).expect("load cache");

        assert_eq!(first.source_hash, second.source_hash);
        assert_eq!(first.triggers, vec!["Rust work", "Code review"]);
        assert!(first.summary.contains("Workflow:"));
        assert!(dir.path().join(".spark/skills/demo.json").exists());
    }
}
