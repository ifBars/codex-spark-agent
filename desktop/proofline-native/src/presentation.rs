use crate::model::{ProoflineSnapshotV1, RunState, ValidationState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProoflinePresentation {
    pub history: Vec<HistoryRow>,
    pub title: String,
    pub completion_meta: String,
    pub summary: String,
    pub changed_files: Vec<ChangedFileRow>,
    pub validation: Vec<ValidationRow>,
    pub model_steps_label: String,
    pub status: StatusRibbon,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryRow {
    pub title: String,
    pub meta: String,
    pub selected: bool,
    pub completed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFileRow {
    pub path: String,
    pub additions: String,
    pub deletions: String,
    pub evidence: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationRow {
    pub command: String,
    pub elapsed: String,
    pub passed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusRibbon {
    pub branch: String,
    pub checkpoint: String,
    pub elapsed: String,
    pub tokens: String,
    pub pricing: String,
    pub network_gate: String,
}

impl From<&ProoflineSnapshotV1> for ProoflinePresentation {
    fn from(snapshot: &ProoflineSnapshotV1) -> Self {
        Self {
            history: snapshot
                .task_history
                .iter()
                .map(|task| HistoryRow {
                    title: task.title.to_owned(),
                    meta: format!("{} · {}", task.completed_at, task.elapsed),
                    selected: task.run_id == snapshot.selected_run_id,
                    completed: matches!(task.state, RunState::Completed),
                })
                .collect(),
            title: snapshot.selected_task.title.to_owned(),
            completion_meta: format!(
                "COMPLETED  ·  {}  ·  {}",
                snapshot.selected_task.completed_at, snapshot.selected_task.elapsed
            ),
            summary: snapshot.selected_task.summary.to_owned(),
            changed_files: snapshot
                .selected_task
                .changed_files
                .iter()
                .map(|file| ChangedFileRow {
                    path: file.path.to_owned(),
                    additions: format!("+{}", file.additions),
                    deletions: format!("-{}", file.deletions),
                    evidence: file.evidence.to_owned(),
                })
                .collect(),
            validation: snapshot
                .selected_task
                .validation
                .iter()
                .map(|check| ValidationRow {
                    command: check.command.to_owned(),
                    elapsed: check.elapsed.to_owned(),
                    passed: matches!(check.state, ValidationState::Passed),
                })
                .collect(),
            model_steps_label: format!("{} steps", snapshot.selected_task.model_steps),
            status: StatusRibbon {
                branch: snapshot.status.branch.to_owned(),
                checkpoint: snapshot.status.checkpoint.to_owned(),
                elapsed: snapshot.status.elapsed.to_owned(),
                tokens: snapshot.status.tokens.to_owned(),
                pricing: snapshot.status.pricing.to_owned(),
                network_gate: snapshot.status.network_gate.to_owned(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture_snapshot;

    #[test]
    fn fixture_mapping_keeps_evidence_and_network_gate_visible() {
        let snapshot = fixture_snapshot();
        let presentation = ProoflinePresentation::from(&snapshot);

        assert_eq!(presentation.history.len(), 4);
        assert_eq!(
            presentation
                .history
                .iter()
                .filter(|row| row.selected)
                .count(),
            1
        );
        assert_eq!(presentation.changed_files.len(), 6);
        assert_eq!(presentation.validation.len(), 4);
        assert!(presentation.validation.iter().all(|row| row.passed));
        assert_eq!(presentation.status.network_gate, "Network gate pending");
    }

    #[test]
    fn selected_history_row_is_derived_from_the_snapshot_identity() {
        let mut snapshot = fixture_snapshot();
        snapshot.selected_run_id = "refactor-session-store";

        let presentation = ProoflinePresentation::from(&snapshot);
        assert_eq!(presentation.history[2].title, "Refactor session store");
        assert!(presentation.history[2].selected);
        assert!(!presentation.history[0].selected);
    }
}
