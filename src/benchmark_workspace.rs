use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;

use crate::cli::ProfileScenarioKind;

const WORKSPACE_ROOT: &str = ".spark-profile/benchmark-workspaces";

pub(crate) fn create_benchmark_workspace(
    source_cwd: &Path,
    suite: &str,
    scenario: ProfileScenarioKind,
    repeat_index: usize,
) -> Result<PathBuf> {
    let stamp = unix_millis();
    let workspace = source_cwd
        .join(WORKSPACE_ROOT)
        .join(format!("{suite}-{stamp}"))
        .join(format!("{}-{repeat_index}", scenario.name()));
    std::fs::create_dir_all(&workspace).map_err(|error| {
        anyhow::anyhow!(
            "failed to create benchmark workspace {}: {error}",
            workspace.display()
        )
    })?;
    Ok(workspace)
}

pub(crate) fn benchmark_read_roots(source_cwd: &Path, scenario_cwd: &Path) -> Vec<PathBuf> {
    if same_path(source_cwd, scenario_cwd) {
        Vec::new()
    } else {
        vec![source_cwd.to_path_buf()]
    }
}

pub(crate) fn mirror_trace_to_source(source_cwd: &Path, trace_dir: &Path) -> Result<PathBuf> {
    let Some(name) = trace_dir.file_name() else {
        anyhow::bail!("trace path has no directory name: {}", trace_dir.display());
    };
    let target = source_cwd.join(".spark-runs").join(name);
    if target.exists() {
        std::fs::remove_dir_all(&target).map_err(|error| {
            anyhow::anyhow!(
                "failed to reset mirrored trace {}: {error}",
                target.display()
            )
        })?;
    }
    copy_dir(trace_dir, &target)?;
    Ok(target)
}

fn copy_dir(source: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)
        .map_err(|error| anyhow::anyhow!("failed to create {}: {error}", target.display()))?;
    for entry in std::fs::read_dir(source)
        .map_err(|error| anyhow::anyhow!("failed to read {}: {error}", source.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if excluded_entry(&name) {
            continue;
        }
        let from = entry.path();
        let to = target.join(name.as_ref());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            copy_file(&from, &to)?;
        }
    }
    Ok(())
}

pub(crate) fn copy_file(source: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| anyhow::anyhow!("failed to create {}: {error}", parent.display()))?;
    }
    std::fs::copy(source, target).map_err(|error| {
        anyhow::anyhow!(
            "failed to copy {} to {}: {error}",
            source.display(),
            target.display()
        )
    })?;
    Ok(())
}

fn excluded_entry(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".codegraph"
            | ".spark-profile"
            | ".spark-runs"
            | ".spark-scenarios"
            | "target"
            | "node_modules"
            | ".vite"
            | "dist"
    )
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use crate::cli::ProfileScenarioKind;

    use super::*;

    #[test]
    fn benchmark_workspace_does_not_copy_source_repo() {
        let source = tempfile::tempdir().expect("source");
        std::fs::create_dir_all(source.path().join("src")).expect("src");
        std::fs::write(source.path().join("src").join("main.rs"), "fn main() {}\n")
            .expect("write source");
        std::fs::write(source.path().join("Cargo.toml"), "[package]\nname='x'\n")
            .expect("write cargo");

        let workspace = create_benchmark_workspace(
            source.path(),
            "real-world",
            ProfileScenarioKind::RepoSurvey,
            1,
        )
        .expect("workspace");

        assert!(workspace.exists());
        assert!(!workspace.join("src").exists());
        assert!(!workspace.join("Cargo.toml").exists());
    }

    #[test]
    fn benchmark_read_roots_include_source_for_isolated_workspace() {
        let source = tempfile::tempdir().expect("source");
        let workspace = tempfile::tempdir().expect("workspace");

        let roots = benchmark_read_roots(source.path(), workspace.path());

        assert_eq!(roots, vec![source.path().to_path_buf()]);
    }
}
