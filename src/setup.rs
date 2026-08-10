use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use comfy_table::{Cell, Table};
use inquire::{Confirm, Select, error::InquireError};

use crate::{auth, codex_integration, config, session, skill};

#[derive(Debug, Clone)]
pub(crate) struct SetupOptions {
    pub(crate) cwd: PathBuf,
    pub(crate) non_interactive: bool,
    pub(crate) skip_login: bool,
    pub(crate) skip_skill_migration: bool,
    pub(crate) skill_source: Option<PathBuf>,
    pub(crate) codex: bool,
    pub(crate) force_codex: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SkillMigrationPlan {
    source_root: PathBuf,
    skills: Vec<SkillMigrationItem>,
    skipped_existing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SkillMigrationItem {
    name: String,
    source_dir: PathBuf,
    destination_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct SetupSummaryRow {
    step: String,
    status: String,
    detail: String,
}

impl SetupSummaryRow {
    fn new(step: impl Into<String>, status: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            step: step.into(),
            status: status.into(),
            detail: detail.into(),
        }
    }
}

pub(crate) async fn run(options: SetupOptions) -> Result<()> {
    let cwd = std::fs::canonicalize(&options.cwd)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| options.cwd.clone()));
    let mut rows = Vec::new();

    let app_dir = config::app_dir()?;
    std::fs::create_dir_all(&app_dir)
        .with_context(|| format!("failed to create {}", app_dir.display()))?;
    rows.push(SetupSummaryRow::new(
        "App directory",
        "ready",
        app_dir.display().to_string(),
    ));

    session::prepare_default_session_store(None)?;
    rows.push(SetupSummaryRow::new(
        "Session store",
        "ready",
        config::sessions_db_path()?.display().to_string(),
    ));

    if options.skip_login {
        rows.push(SetupSummaryRow::new(
            "Auth",
            "skipped",
            "run `spark setup` or `spark login --device` later",
        ));
    } else if confirm_or_default(
        "Sign in now with device-code auth?",
        true,
        options.non_interactive,
    )? {
        let tokens = auth::login_device_code().await?;
        let account = tokens
            .account_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        config::save_auth(&tokens)?;
        rows.push(SetupSummaryRow::new("Auth", "logged in", account));
    } else {
        rows.push(SetupSummaryRow::new(
            "Auth",
            "skipped",
            "run `spark login --device` when ready",
        ));
    }

    let migration_source = choose_skill_source(&cwd, &options)?;
    if let Some(source) = migration_source {
        let plan = plan_skill_migration(&cwd, &source)?;
        let copied = apply_skill_migration(&plan)?;
        rows.push(SetupSummaryRow::new(
            "Skills",
            "migrated",
            format!(
                "{} copied, {} existing",
                copied,
                plan.skipped_existing.len()
            ),
        ));
    } else {
        rows.push(SetupSummaryRow::new(
            "Skills",
            "skipped",
            "no skill migration requested",
        ));
    }

    let skill_count = skill::registry::discover_sources(&cwd)?.len();
    if skill_count > 0
        && confirm_or_default(
            "Refresh local skill cache summaries?",
            true,
            options.non_interactive,
        )?
    {
        let refreshed = refresh_skill_cache(&cwd)?;
        rows.push(SetupSummaryRow::new(
            "Skill cache",
            "ready",
            format!("{refreshed} refreshed"),
        ));
    } else {
        rows.push(SetupSummaryRow::new(
            "Skill cache",
            "skipped",
            format!("{skill_count} repo skill(s) found"),
        ));
    }

    if options.codex {
        let report = codex_integration::install(options.force_codex)?;
        rows.push(SetupSummaryRow::new(
            "Codex MCP",
            "ready",
            format!("spark_harness -> {}", report.spark_executable.display()),
        ));
        rows.push(SetupSummaryRow::new(
            "Codex explorer",
            "ready",
            match report.backup_path {
                Some(path) => format!(
                    "{} (previous file backed up to {})",
                    report.agent_path.display(),
                    path.display()
                ),
                None => report.agent_path.display().to_string(),
            },
        ));
    }

    println!("{}", render_setup_summary(&rows));
    Ok(())
}

fn choose_skill_source(cwd: &Path, options: &SetupOptions) -> Result<Option<PathBuf>> {
    if options.skip_skill_migration {
        return Ok(None);
    }
    if let Some(source) = &options.skill_source {
        return Ok(Some(source.clone()));
    }

    let sources = discover_skill_migration_sources()?;
    if sources.is_empty() {
        return Ok(None);
    }
    if options.non_interactive {
        return Ok(None);
    }
    let existing_repo_skills = skill::registry::discover_repo_sources(cwd)?.len();
    let should_migrate = confirm_or_default(
        "Migrate skills into this repo's .agents/skills?",
        existing_repo_skills == 0,
        false,
    )?;
    if !should_migrate {
        return Ok(None);
    }
    if sources.len() == 1 {
        return Ok(sources.into_iter().next());
    }

    let labels = sources
        .iter()
        .map(|source| source.display().to_string())
        .collect::<Vec<_>>();
    let selected = match Select::new("Skill source:", labels).prompt() {
        Ok(selected) => selected,
        Err(error) if is_prompt_cancelled(&error) => return Ok(None),
        Err(error) => return Err(error).context("skill source prompt failed"),
    };
    Ok(sources
        .into_iter()
        .find(|source| source.display().to_string() == selected))
}

fn discover_skill_migration_sources() -> Result<Vec<PathBuf>> {
    let mut sources = Vec::new();
    if let Some(base_dirs) = directories::BaseDirs::new() {
        let home = base_dirs.home_dir();
        sources.push(home.join(".agents").join("skills"));
        sources.push(home.join(".codex").join("skills"));
    }
    sources.retain(|source| source.exists() && contains_skills(source));
    sources.sort();
    sources.dedup();
    Ok(sources)
}

fn contains_skills(root: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    entries
        .filter_map(|entry| entry.ok())
        .any(|entry| entry.path().join("SKILL.md").exists())
}

fn plan_skill_migration(cwd: &Path, source_root: &Path) -> Result<SkillMigrationPlan> {
    let destination_root = cwd.join(".agents").join("skills");
    let mut skills = Vec::new();
    let mut skipped_existing = Vec::new();

    for entry in std::fs::read_dir(source_root)
        .with_context(|| format!("failed to list {}", source_root.display()))?
    {
        let entry = entry?;
        let source_dir = entry.path();
        if !source_dir.join("SKILL.md").exists() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let destination_dir = destination_root.join(&name);
        if destination_dir.exists() {
            skipped_existing.push(name);
            continue;
        }
        skills.push(SkillMigrationItem {
            name,
            source_dir,
            destination_dir,
        });
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name));
    skipped_existing.sort();

    Ok(SkillMigrationPlan {
        source_root: source_root.to_path_buf(),
        skills,
        skipped_existing,
    })
}

fn apply_skill_migration(plan: &SkillMigrationPlan) -> Result<usize> {
    for skill in &plan.skills {
        copy_dir_recursive(&skill.source_dir, &skill.destination_dir)?;
    }
    Ok(plan.skills.len())
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    for entry in
        std::fs::read_dir(source).with_context(|| format!("failed to list {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else {
            std::fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn refresh_skill_cache(cwd: &Path) -> Result<usize> {
    let mut refreshed = 0usize;
    for source in skill::registry::discover_sources(cwd)? {
        skill::registry::compile_or_load(cwd, &source.name, true)?;
        refreshed += 1;
    }
    Ok(refreshed)
}

fn confirm_or_default(prompt: &str, default: bool, non_interactive: bool) -> Result<bool> {
    if non_interactive {
        return Ok(default);
    }
    match Confirm::new(prompt).with_default(default).prompt() {
        Ok(answer) => Ok(answer),
        Err(error) if is_prompt_cancelled(&error) => Ok(default),
        Err(InquireError::NotTTY) => Ok(default),
        Err(error) => Err(error).with_context(|| format!("prompt failed: {prompt}")),
    }
}

fn is_prompt_cancelled(error: &InquireError) -> bool {
    matches!(
        error,
        InquireError::OperationCanceled | InquireError::OperationInterrupted
    )
}

fn render_setup_summary(rows: &[SetupSummaryRow]) -> String {
    let mut table = Table::new();
    table.set_header(["Step", "Status", "Detail"]);
    for row in rows {
        table.add_row([
            Cell::new(&row.step),
            Cell::new(&row.status),
            Cell::new(&row.detail),
        ]);
    }
    table.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_migration_plan_skips_existing_skills_and_keeps_new_sources_sorted() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cwd = temp.path().join("repo");
        let source = temp.path().join("source");
        std::fs::create_dir_all(cwd.join(".agents/skills/rust-patterns")).expect("repo skill dir");
        std::fs::create_dir_all(source.join("alpha")).expect("source alpha");
        std::fs::create_dir_all(source.join("rust-patterns")).expect("source rust-patterns");
        std::fs::write(cwd.join(".agents/skills/rust-patterns/SKILL.md"), "# Local")
            .expect("local skill");
        std::fs::write(source.join("alpha/SKILL.md"), "# Alpha").expect("alpha skill");
        std::fs::write(source.join("rust-patterns/SKILL.md"), "# Global").expect("global skill");

        let plan = plan_skill_migration(&cwd, &source).expect("plan");

        assert_eq!(plan.source_root, source);
        assert_eq!(plan.skills.len(), 1);
        assert_eq!(plan.skills[0].name, "alpha");
        assert_eq!(plan.skipped_existing, vec!["rust-patterns"]);
    }

    #[test]
    fn setup_summary_table_mentions_key_first_run_steps() {
        let rendered = render_setup_summary(&[
            SetupSummaryRow::new("App directory", "ready", "C:/Users/me/.spark-codex"),
            SetupSummaryRow::new("Auth", "skipped", "run spark setup later"),
            SetupSummaryRow::new("Skills", "migrated", "2 copied"),
        ]);

        assert!(rendered.contains("App directory"));
        assert!(rendered.contains("Auth"));
        assert!(rendered.contains("Skills"));
        assert!(rendered.contains("migrated"));
    }
}
