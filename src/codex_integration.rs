use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use directories::BaseDirs;

const MCP_SERVER_NAME: &str = "spark_harness";
const EXPLORER_AGENT: &str = r#"name = "explorer"
description = "Read-only repository explorer backed by the dedicated GPT-5.3-Codex-Spark harness. Use for file discovery, call-chain tracing, architecture reconnaissance, and hypothesis checks before the parent agent edits or decides."
model = "gpt-5.3-codex-spark"
model_reasoning_effort = "medium"
sandbox_mode = "read-only"
developer_instructions = """
Act as the native Codex explorer bridge.

For non-trivial repository exploration, call mcp__spark_harness__explore_repo once and let that tool perform the repository inspection through the dedicated Spark harness. Pass one concrete task, the absolute workspace cwd, useful starting paths, and only task-relevant parent constraints or decisions. Do not forward secrets or the full parent transcript.

Return the tool's compact evidence brief to the parent. Do not edit files. Do not independently repeat the exploration unless the MCP tool is unavailable; if it is unavailable, report that clearly and fall back to a small read-only local inspection.
"""
"#;

#[derive(Debug)]
pub(crate) struct CodexInstallReport {
    pub(crate) spark_executable: PathBuf,
    pub(crate) agent_path: PathBuf,
    pub(crate) backup_path: Option<PathBuf>,
}

pub(crate) fn install(force: bool) -> Result<CodexInstallReport> {
    let source_executable =
        std::env::current_exe().context("failed to resolve the current Spark executable")?;
    let base_dirs = BaseDirs::new().context("failed to resolve the user home directory")?;
    let spark_executable = stage_mcp_executable(
        &source_executable,
        &base_dirs.home_dir().join(".spark-codex"),
    )?;
    register_mcp_server(&spark_executable, force)?;

    let agent_path = base_dirs
        .home_dir()
        .join(".codex")
        .join("agents")
        .join("explorer.toml");
    let backup_path = install_explorer_agent(&agent_path, force)?;

    Ok(CodexInstallReport {
        spark_executable,
        agent_path,
        backup_path,
    })
}

fn stage_mcp_executable(source: &Path, app_dir: &Path) -> Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let executable_name = if cfg!(windows) {
        format!("spark-mcp-{timestamp}.exe")
    } else {
        format!("spark-mcp-{timestamp}")
    };
    let destination = app_dir.join("mcp").join(executable_name);
    let parent = destination
        .parent()
        .context("staged MCP executable path has no parent")?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;
    std::fs::copy(source, &destination).with_context(|| {
        format!(
            "failed to stage MCP executable from {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(destination)
}

fn register_mcp_server(spark_executable: &Path, force: bool) -> Result<()> {
    let existing = codex_output(&["mcp", "get", MCP_SERVER_NAME, "--json"])?;
    if existing.status.success() {
        if !force {
            bail!(
                "Codex MCP server `{MCP_SERVER_NAME}` already exists; rerun with --force-codex to replace it"
            );
        }
        ensure_success(
            codex_output(&["mcp", "remove", MCP_SERVER_NAME])?,
            "remove existing Codex MCP registration",
        )?;
    }

    let executable = spark_executable
        .to_str()
        .context("Spark executable path is not valid Unicode")?;
    ensure_success(
        codex_output(&[
            "mcp",
            "add",
            MCP_SERVER_NAME,
            "--",
            executable,
            "mcp-server",
        ])?,
        "register Spark MCP server with Codex",
    )
}

fn install_explorer_agent(path: &Path, force: bool) -> Result<Option<PathBuf>> {
    if let Ok(existing) = std::fs::read_to_string(path)
        && existing == EXPLORER_AGENT
    {
        return Ok(None);
    }
    let backup_path = if path.exists() {
        if !force {
            bail!(
                "Codex explorer agent already exists at {}; rerun with --force-codex to replace it",
                path.display()
            );
        }
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let backup = path.with_extension(format!("toml.backup-{timestamp}"));
        std::fs::copy(path, &backup).with_context(|| {
            format!(
                "failed to back up {} to {}",
                path.display(),
                backup.display()
            )
        })?;
        Some(backup)
    } else {
        None
    };

    let parent = path.parent().context("Codex explorer path has no parent")?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;
    std::fs::write(path, EXPLORER_AGENT)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(backup_path)
}

fn codex_output(args: &[&str]) -> Result<Output> {
    Command::new("codex")
        .args(args)
        .output()
        .with_context(|| format!("failed to run `codex {}`", args.join(" ")))
}

fn ensure_success(output: Output, action: &str) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if stderr.is_empty() { stdout } else { stderr };
    bail!("{action} failed: {detail}")
}

#[cfg(test)]
mod tests {
    use super::{EXPLORER_AGENT, install_explorer_agent, stage_mcp_executable};

    #[test]
    fn explorer_agent_routes_through_mcp_without_turn_limits() {
        assert!(EXPLORER_AGENT.contains("mcp__spark_harness__explore_repo"));
        assert!(EXPLORER_AGENT.contains("gpt-5.3-codex-spark"));
        assert!(!EXPLORER_AGENT.contains("max_turn"));
    }

    #[test]
    fn force_install_preserves_existing_agent_backup() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("agents/explorer.toml");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("agent dir");
        std::fs::write(&path, "old agent").expect("old agent");

        let backup = install_explorer_agent(&path, true)
            .expect("install")
            .expect("backup");

        assert_eq!(
            std::fs::read_to_string(&path).expect("new agent"),
            EXPLORER_AGENT
        );
        assert_eq!(
            std::fs::read_to_string(backup).expect("backup"),
            "old agent"
        );
    }

    #[test]
    fn mcp_executable_is_staged_at_an_immutable_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("spark-source.exe");
        std::fs::write(&source, b"spark binary").expect("source");

        let staged = stage_mcp_executable(&source, &dir.path().join("app")).expect("stage");

        assert!(staged.starts_with(dir.path().join("app/mcp")));
        assert_eq!(
            std::fs::read(staged).expect("staged bytes"),
            b"spark binary"
        );
    }
}
