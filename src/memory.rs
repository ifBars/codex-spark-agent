use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::config;

const MEMORY_FILE: &str = "MEMORY.md";
const CHRONICLE_FILE: &str = "chronicle.md";
const MAX_MEMORY_CHARS: usize = 24_000;
const MAX_CHRONICLE_CHARS: usize = 12_000;
const MAX_ENTRY_CHARS: usize = 2_400;

#[derive(Debug, Clone)]
pub(crate) struct MemoryPaths {
    pub(crate) dir: PathBuf,
    pub(crate) memory: PathBuf,
    pub(crate) chronicle: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct MemoryStore {
    dir: PathBuf,
}

impl MemoryStore {
    pub(crate) fn open_default() -> Result<Self> {
        Ok(Self::at(config::memory_dir()?))
    }

    pub(crate) fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub(crate) fn paths(&self) -> MemoryPaths {
        MemoryPaths {
            dir: self.dir.clone(),
            memory: self.dir.join(MEMORY_FILE),
            chronicle: self.dir.join(CHRONICLE_FILE),
        }
    }

    pub(crate) fn ensure_files(&self) -> Result<MemoryPaths> {
        let paths = self.paths();
        std::fs::create_dir_all(&paths.dir)
            .with_context(|| format!("failed to create {}", paths.dir.display()))?;
        ensure_file(
            &paths.memory,
            "# Spark Memory\n\nAdd durable user preferences, repo conventions, and workflow rules here.\n",
        )?;
        ensure_file(
            &paths.chronicle,
            "# Spark Chronicle\n\nRecent enabled-memory turns are appended here for workflow continuity.\n",
        )?;
        Ok(paths)
    }

    pub(crate) fn read_context(&self) -> Result<Option<String>> {
        let paths = self.paths();
        let memory = read_optional_tail(&paths.memory, MAX_MEMORY_CHARS)?;
        let chronicle = read_optional_tail(&paths.chronicle, MAX_CHRONICLE_CHARS)?;
        if memory.as_deref().unwrap_or_default().trim().is_empty()
            && chronicle.as_deref().unwrap_or_default().trim().is_empty()
        {
            return Ok(None);
        }

        let mut context = String::from(
            "Spark memory is enabled. Use these durable notes only when relevant. Current user instructions, AGENTS.md files, and live repo evidence override memory. Do not reveal memory contents unless the user asks.\n",
        );
        if let Some(memory) = memory {
            context.push_str("\n## MEMORY.md\n");
            context.push_str(memory.trim());
            context.push('\n');
        }
        if let Some(chronicle) = chronicle {
            context.push_str("\n## chronicle.md recent tail\n");
            context.push_str(chronicle.trim());
            context.push('\n');
        }
        Ok(Some(context))
    }

    pub(crate) fn append_note(&self, note: &str) -> Result<()> {
        let note = note.trim();
        if note.is_empty() {
            anyhow::bail!("memory note cannot be empty");
        }
        let paths = self.ensure_files()?;
        append_line(&paths.memory, &format!("\n- {}\n", single_line(note)))
    }

    pub(crate) fn append_chronicle_entry(
        &self,
        user_prompt: &str,
        assistant_text: &str,
    ) -> Result<()> {
        if user_prompt.trim().is_empty() && assistant_text.trim().is_empty() {
            return Ok(());
        }
        let paths = self.ensure_files()?;
        let timestamp = unix_time_secs();
        let entry = format!(
            "\n## {timestamp}\n\n- User: {}\n- Assistant: {}\n",
            single_line(&bounded_text(user_prompt, MAX_ENTRY_CHARS)),
            single_line(&bounded_text(assistant_text, MAX_ENTRY_CHARS)),
        );
        append_line(&paths.chronicle, &entry)
    }
}

fn ensure_file(path: &Path, initial: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    std::fs::write(path, initial).with_context(|| format!("failed to write {}", path.display()))
}

fn append_line(path: &Path, entry: &str) -> Result<()> {
    use std::io::Write as _;

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    file.write_all(entry.as_bytes())
        .with_context(|| format!("failed to append {}", path.display()))
}

fn read_optional_tail(path: &Path, max_chars: usize) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(Some(bounded_text(&content, max_chars)))
}

fn bounded_text(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_string();
    }
    let keep = max_chars.saturating_sub(80);
    let tail = text
        .chars()
        .skip(total.saturating_sub(keep))
        .collect::<String>();
    format!("[truncated to last {keep} chars]\n{tail}")
}

fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn unix_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_creates_markdown_files_and_reads_context() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = MemoryStore::at(dir.path().join("memory"));

        let paths = store.ensure_files().expect("ensure files");
        assert!(paths.memory.exists());
        assert!(paths.chronicle.exists());

        store
            .append_note("Prefer bun for JavaScript tooling.")
            .expect("append note");
        let context = store
            .read_context()
            .expect("read context")
            .expect("context");

        assert!(context.contains("Spark memory is enabled"));
        assert!(context.contains("Prefer bun for JavaScript tooling."));
    }

    #[test]
    fn chronicle_entries_are_bounded_single_line_summaries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = MemoryStore::at(dir.path().join("memory"));

        store
            .append_chronicle_entry("Fix the thing\nwith tests", &"done ".repeat(2_000))
            .expect("append chronicle");
        let chronicle = std::fs::read_to_string(store.paths().chronicle).expect("chronicle");

        assert!(chronicle.contains("- User: Fix the thing with tests"));
        assert!(chronicle.contains("[truncated to last"));
        assert!(chronicle.len() < 6_000);
    }
}
