#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProoflineSnapshotV1 {
    pub selected_run_id: &'static str,
    pub task_history: Vec<TaskHistoryEntry>,
    pub selected_task: CompletedTask,
    pub status: StatusRibbonSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskHistoryEntry {
    pub run_id: &'static str,
    pub title: &'static str,
    pub completed_at: &'static str,
    pub elapsed: &'static str,
    pub state: RunState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedTask {
    pub title: &'static str,
    pub completed_at: &'static str,
    pub elapsed: &'static str,
    pub summary: &'static str,
    pub changed_files: Vec<ChangedFile>,
    pub validation: Vec<ValidationCheck>,
    pub model_steps: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: &'static str,
    pub additions: u16,
    pub deletions: u16,
    pub evidence: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationCheck {
    pub command: &'static str,
    pub elapsed: &'static str,
    pub state: ValidationState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    Completed,
    Pending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationState {
    Passed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusRibbonSnapshot {
    pub branch: &'static str,
    pub checkpoint: &'static str,
    pub elapsed: &'static str,
    pub tokens: &'static str,
    pub pricing: &'static str,
    pub network_gate: &'static str,
}
