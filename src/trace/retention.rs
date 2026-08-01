use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::trace::commands::{list_trace_dirs, trace_runs_root};

const MILLIS_PER_DAY: u128 = 24 * 60 * 60 * 1_000;

#[derive(Debug)]
pub(crate) struct TraceRetentionPlan {
    pub(crate) workspace: PathBuf,
    pub(crate) runs_root: PathBuf,
    pub(crate) older_than_days: u64,
    pub(crate) cutoff_unix_ms: u128,
    pub(crate) total_runs: usize,
    pub(crate) candidates: Vec<TraceRetentionCandidate>,
}

#[derive(Debug)]
pub(crate) struct TraceRetentionCandidate {
    pub(crate) path: PathBuf,
    pub(crate) started_at_unix_ms: u128,
}

pub(crate) fn inspect_trace_retention(
    workspace: &Path,
    older_than_days: u64,
) -> Result<TraceRetentionPlan> {
    let now_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    inspect_trace_retention_at(workspace, older_than_days, now_unix_ms)
}

fn inspect_trace_retention_at(
    workspace: &Path,
    older_than_days: u64,
    now_unix_ms: u128,
) -> Result<TraceRetentionPlan> {
    let workspace = std::fs::canonicalize(workspace)
        .with_context(|| format!("failed to resolve workspace {}", workspace.display()))?;
    let runs_root = trace_runs_root(&workspace);
    let cutoff_unix_ms = now_unix_ms.saturating_sub(u128::from(older_than_days) * MILLIS_PER_DAY);
    let runs = list_trace_dirs(&runs_root, usize::MAX)?;
    let candidates = runs
        .iter()
        .filter_map(|path| {
            let started_at_unix_ms = trace_run_started_at(path)?;
            (started_at_unix_ms < cutoff_unix_ms).then(|| TraceRetentionCandidate {
                path: path.clone(),
                started_at_unix_ms,
            })
        })
        .collect();

    Ok(TraceRetentionPlan {
        workspace,
        runs_root,
        older_than_days,
        cutoff_unix_ms,
        total_runs: runs.len(),
        candidates,
    })
}

pub(crate) fn purge_trace_retention(plan: &TraceRetentionPlan, confirmed: bool) -> Result<usize> {
    if !confirmed {
        anyhow::bail!("trace purge requires explicit confirmation with --confirm");
    }

    let resolved_runs = plan
        .candidates
        .iter()
        .map(|candidate| resolve_trace_run_for_delete(&plan.runs_root, &candidate.path))
        .collect::<Result<Vec<_>>>()?;
    for run in &resolved_runs {
        std::fs::remove_dir_all(run)
            .with_context(|| format!("failed to remove trace directory {}", run.display()))?;
    }
    Ok(resolved_runs.len())
}

pub(crate) fn validate_purge_request(purge: bool, confirm: bool) -> Result<()> {
    if confirm && !purge {
        anyhow::bail!("--confirm requires --purge");
    }
    Ok(())
}

fn trace_run_started_at(path: &Path) -> Option<u128> {
    path.file_name()?
        .to_str()?
        .strip_prefix("run-")?
        .parse()
        .ok()
}

fn resolve_trace_run_for_delete(runs_root: &Path, path: &Path) -> Result<PathBuf> {
    let resolved_root = std::fs::canonicalize(runs_root)
        .with_context(|| format!("failed to resolve trace root {}", runs_root.display()))?;
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect trace directory {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        anyhow::bail!(
            "refusing to remove non-directory trace path {}",
            path.display()
        );
    }
    if trace_run_started_at(path).is_none() {
        anyhow::bail!(
            "refusing to remove unrecognized trace path {}",
            path.display()
        );
    }

    let resolved_run = std::fs::canonicalize(path)
        .with_context(|| format!("failed to resolve trace directory {}", path.display()))?;
    if resolved_run.parent() != Some(resolved_root.as_path()) {
        anyhow::bail!(
            "refusing to remove trace path outside {}: {}",
            resolved_root.display(),
            resolved_run.display()
        );
    }
    Ok(resolved_run)
}

#[cfg(test)]
mod tests {
    use super::{
        MILLIS_PER_DAY, TraceRetentionCandidate, TraceRetentionPlan, inspect_trace_retention_at,
        purge_trace_retention, validate_purge_request,
    };

    #[test]
    fn inspection_selects_only_timestamped_runs_older_than_the_policy() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let root = workspace.path().join(".spark-runs");
        let now = 100 * MILLIS_PER_DAY;
        std::fs::create_dir_all(root.join(format!("run-{}", now - 31 * MILLIS_PER_DAY)))
            .expect("create old trace");
        std::fs::create_dir_all(root.join(format!("run-{}", now - 30 * MILLIS_PER_DAY + 1)))
            .expect("create boundary trace");
        std::fs::create_dir_all(root.join("run-not-a-timestamp"))
            .expect("create unrecognized trace");

        let plan = inspect_trace_retention_at(workspace.path(), 30, now).expect("inspect traces");

        assert_eq!(plan.total_runs, 3);
        assert_eq!(plan.candidates.len(), 1);
        let expected_name = format!("run-{}", now - 31 * MILLIS_PER_DAY);
        assert_eq!(
            plan.candidates[0]
                .path
                .file_name()
                .and_then(|name| name.to_str()),
            Some(expected_name.as_str())
        );
    }

    #[test]
    fn inspection_of_a_workspace_without_traces_is_empty() {
        let workspace = tempfile::tempdir().expect("tempdir");

        let plan = inspect_trace_retention_at(workspace.path(), 30, 100 * MILLIS_PER_DAY)
            .expect("inspect empty trace root");

        assert_eq!(plan.total_runs, 0);
        assert!(plan.candidates.is_empty());
        assert!(!plan.runs_root.exists());
    }

    #[test]
    fn purge_requires_confirmation_and_removes_only_planned_run_directories() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let root = workspace.path().join(".spark-runs");
        let now = 100 * MILLIS_PER_DAY;
        let old = root.join(format!("run-{}", now - 31 * MILLIS_PER_DAY));
        let recent = root.join(format!("run-{}", now - 5 * MILLIS_PER_DAY));
        std::fs::create_dir_all(old.join("nested")).expect("create old trace");
        std::fs::create_dir_all(&recent).expect("create recent trace");
        std::fs::write(old.join("nested/trace.json"), "sensitive trace data")
            .expect("write old trace");

        let plan = inspect_trace_retention_at(workspace.path(), 30, now).expect("inspect traces");
        assert!(purge_trace_retention(&plan, false).is_err());
        assert!(old.exists());

        assert_eq!(purge_trace_retention(&plan, true).expect("purge traces"), 1);
        assert!(!old.exists());
        assert!(recent.exists());
    }

    #[test]
    fn confirmation_requires_purge_intent() {
        assert!(validate_purge_request(false, true).is_err());
        validate_purge_request(false, false).expect("inspection is valid");
        validate_purge_request(true, false).expect("dry run is valid");
        validate_purge_request(true, true).expect("confirmed purge is valid");
    }

    #[test]
    fn purge_refuses_a_candidate_outside_the_resolved_trace_root() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        let root = workspace.path().join(".spark-runs");
        std::fs::create_dir_all(&root).expect("create trace root");
        let outside_run = outside.path().join("run-1");
        std::fs::create_dir_all(&outside_run).expect("create outside run");
        let plan = TraceRetentionPlan {
            workspace: workspace.path().to_path_buf(),
            runs_root: root,
            older_than_days: 30,
            cutoff_unix_ms: 1,
            total_runs: 1,
            candidates: vec![TraceRetentionCandidate {
                path: outside_run.clone(),
                started_at_unix_ms: 0,
            }],
        };

        let error = purge_trace_retention(&plan, true).expect_err("outside path must be rejected");

        assert!(error.to_string().contains("outside"));
        assert!(outside_run.exists());
    }
}
