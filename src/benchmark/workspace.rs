use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;

use crate::cli::ProfileScenarioKind;

const WORKSPACE_ROOT: &str = ".spark-profile/benchmark-workspaces";
const ASSET_RIPPER_EXPLORATION_ROOT: &str =
    r"C:\Users\ghost\Desktop\Coding\ScheduleOne\AssetRipper_export_20260718_070918";
const FIVEM_EXPLORATION_ROOT: &str = r"C:\Users\ghost\Desktop\Coding\FiveM\fivem-master";
const CPP2IL_EXPLORATION_ROOT: &str = r"C:\Users\ghost\Desktop\Coding\Cpp2IL";
const IL2CPP_INTEROP_EXPLORATION_ROOT: &str = r"C:\Users\ghost\Desktop\Coding\Il2CppInterop";

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

fn is_repo_survey(kind: ProfileScenarioKind) -> bool {
    matches!(
        kind,
        ProfileScenarioKind::RepoSurvey
            | ProfileScenarioKind::SteamNetworkLibSurvey
            | ProfileScenarioKind::S1ApiSurvey
            | ProfileScenarioKind::RepoArchitectureSurvey
            | ProfileScenarioKind::BenchmarkDesignSurvey
    )
}

pub(crate) fn benchmark_read_roots(
    source_cwd: &Path,
    scenario_cwd: &Path,
    kind: ProfileScenarioKind,
) -> Vec<PathBuf> {
    if let Some(root) = exploration_read_root(kind) {
        return vec![root];
    }
    if is_repo_survey(kind) && !same_path(source_cwd, scenario_cwd) {
        vec![source_cwd.to_path_buf()]
    } else {
        Vec::new()
    }
}

pub(crate) fn is_external_exploration(kind: ProfileScenarioKind) -> bool {
    matches!(
        kind,
        ProfileScenarioKind::AssetRipperExploration
            | ProfileScenarioKind::FiveMExploration
            | ProfileScenarioKind::Cpp2IlExploration
            | ProfileScenarioKind::Il2CppInteropExploration
    )
}

fn exploration_read_root(kind: ProfileScenarioKind) -> Option<PathBuf> {
    let root = match kind {
        ProfileScenarioKind::AssetRipperExploration => ASSET_RIPPER_EXPLORATION_ROOT,
        ProfileScenarioKind::FiveMExploration => FIVEM_EXPLORATION_ROOT,
        ProfileScenarioKind::Cpp2IlExploration => CPP2IL_EXPLORATION_ROOT,
        ProfileScenarioKind::Il2CppInteropExploration => IL2CPP_INTEROP_EXPLORATION_ROOT,
        _ => return None,
    };
    Some(PathBuf::from(root))
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
    fn benchmark_read_roots_include_source_for_repo_survey() {
        let source = tempfile::tempdir().expect("source");
        let workspace = tempfile::tempdir().expect("workspace");

        let roots = benchmark_read_roots(
            source.path(),
            workspace.path(),
            ProfileScenarioKind::RepoSurvey,
        );

        assert_eq!(roots, vec![source.path().to_path_buf()]);
    }

    #[test]
    fn benchmark_read_roots_deny_source_for_fixture_scenario() {
        let source = tempfile::tempdir().expect("source");
        let workspace = tempfile::tempdir().expect("workspace");

        let roots = benchmark_read_roots(
            source.path(),
            workspace.path(),
            ProfileScenarioKind::PrecisePatch,
        );

        assert!(roots.is_empty());
    }

    #[test]
    fn exploration_scenarios_use_their_external_read_only_roots() {
        let source = tempfile::tempdir().expect("source");
        let workspace = tempfile::tempdir().expect("workspace");

        let cases = [
            (
                ProfileScenarioKind::AssetRipperExploration,
                ASSET_RIPPER_EXPLORATION_ROOT,
            ),
            (
                ProfileScenarioKind::FiveMExploration,
                FIVEM_EXPLORATION_ROOT,
            ),
            (
                ProfileScenarioKind::Cpp2IlExploration,
                CPP2IL_EXPLORATION_ROOT,
            ),
            (
                ProfileScenarioKind::Il2CppInteropExploration,
                IL2CPP_INTEROP_EXPLORATION_ROOT,
            ),
        ];

        for (scenario, expected) in cases {
            assert_eq!(
                benchmark_read_roots(source.path(), workspace.path(), scenario),
                vec![PathBuf::from(expected)]
            );
        }
    }
}
