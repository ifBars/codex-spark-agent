use crate::model::{
    ChangedFile, CompletedTask, ProoflineSnapshotV1, RunState, StatusRibbonSnapshot,
    TaskHistoryEntry, ValidationCheck, ValidationState,
};

/// Static evidence fixture. It is intentionally the only data source in this spike.
pub fn fixture_snapshot() -> ProoflineSnapshotV1 {
    ProoflineSnapshotV1 {
        selected_run_id: "fork-aware-usage-history",
        task_history: vec![
            TaskHistoryEntry {
                run_id: "fork-aware-usage-history",
                title: "Add fork-aware usage history",
                completed_at: "10:42 AM",
                elapsed: "31s",
                state: RunState::Completed,
            },
            TaskHistoryEntry {
                run_id: "improve-error-messaging",
                title: "Improve error messaging",
                completed_at: "9:58 AM",
                elapsed: "18s",
                state: RunState::Completed,
            },
            TaskHistoryEntry {
                run_id: "refactor-session-store",
                title: "Refactor session store",
                completed_at: "9:22 AM",
                elapsed: "27s",
                state: RunState::Completed,
            },
            TaskHistoryEntry {
                run_id: "tighten-quota-validation",
                title: "Tighten quota validation",
                completed_at: "8:47 AM",
                elapsed: "21s",
                state: RunState::Pending,
            },
        ],
        selected_task: CompletedTask {
            title: "Add fork-aware usage history",
            completed_at: "10:42 AM",
            elapsed: "31s",
            summary: "Spark added fork-aware usage history so usage tracking correctly attributes tokens and cost to the originating fork lineage. History queries now include fork context.",
            changed_files: vec![
                ChangedFile {
                    path: "src/usage/history.rs",
                    additions: 182,
                    deletions: 10,
                    evidence: "Fork-aware history queries and lineage resolution",
                },
                ChangedFile {
                    path: "src/usage/record.rs",
                    additions: 64,
                    deletions: 6,
                    evidence: "Record fork_id and parent_run_id on usage events",
                },
                ChangedFile {
                    path: "src/db/schema.rs",
                    additions: 38,
                    deletions: 2,
                    evidence: "Add fork_id to usage history table and indexes",
                },
                ChangedFile {
                    path: "migrations/20240521_add_fork_id.sql",
                    additions: 26,
                    deletions: 0,
                    evidence: "Schema migration for fork_id column and index",
                },
                ChangedFile {
                    path: "src/api/history.rs",
                    additions: 41,
                    deletions: 3,
                    evidence: "Expose fork-aware history in API responses",
                },
                ChangedFile {
                    path: "tests/usage_history_fork.rs",
                    additions: 96,
                    deletions: 1,
                    evidence: "Integration tests for fork lineage and attribution",
                },
            ],
            validation: vec![
                ValidationCheck {
                    command: "cargo fmt --all -- --check",
                    elapsed: "2s",
                    state: ValidationState::Passed,
                },
                ValidationCheck {
                    command: "cargo clippy --workspace --all-targets -- -D warnings",
                    elapsed: "6s",
                    state: ValidationState::Passed,
                },
                ValidationCheck {
                    command: "cargo test --workspace --all-features",
                    elapsed: "19s",
                    state: ValidationState::Passed,
                },
                ValidationCheck {
                    command: "cargo test --test usage_history_fork",
                    elapsed: "3s",
                    state: ValidationState::Passed,
                },
            ],
            model_steps: 8,
        },
        status: StatusRibbonSnapshot {
            branch: "main",
            checkpoint: "23f7c9a",
            elapsed: "31s",
            tokens: "18,742 in · 4,396 out",
            pricing: "Unavailable",
            network_gate: "Network gate pending",
        },
    }
}
