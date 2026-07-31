use std::collections::BTreeMap;

use anyhow::{Result, bail};
use serde_json::{Value, json};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::subagent::SubagentReport;

pub(super) const DEFAULT_SUBAGENT_CONCURRENCY: usize = 3;
const MAX_SUBAGENT_CONCURRENCY: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkerMetadata {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) task: String,
    pub(super) model: String,
    pub(super) reasoning_effort: String,
    pub(super) mode: String,
    pub(super) ownership: Vec<String>,
}

#[derive(Debug)]
enum WorkerState {
    Running {
        cancellation: CancellationToken,
        handle: JoinHandle<Result<SubagentReport>>,
    },
    Completed(SubagentReport),
    Failed(String),
    Cancelled,
}

#[derive(Debug)]
struct Worker {
    metadata: WorkerMetadata,
    state: WorkerState,
}

#[derive(Debug)]
pub(super) struct SubagentTeam {
    max_concurrency: usize,
    next_worker: usize,
    workers: BTreeMap<String, Worker>,
}

impl Default for SubagentTeam {
    fn default() -> Self {
        Self::from_environment()
    }
}

impl SubagentTeam {
    pub(super) fn from_environment() -> Self {
        let max_concurrency = std::env::var("SPARK_SUBAGENT_MAX_CONCURRENCY")
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .filter(|value| (1..=MAX_SUBAGENT_CONCURRENCY).contains(value))
            .unwrap_or(DEFAULT_SUBAGENT_CONCURRENCY);
        Self {
            max_concurrency,
            next_worker: 1,
            workers: BTreeMap::new(),
        }
    }

    pub(super) fn max_concurrency(&self) -> usize {
        self.max_concurrency
    }

    pub(super) fn running_count(&self) -> usize {
        self.workers
            .values()
            .filter(|worker| matches!(worker.state, WorkerState::Running { .. }))
            .count()
    }

    pub(super) fn reserve_id(&mut self) -> Result<String> {
        if self.running_count() >= self.max_concurrency {
            bail!(
                "subagent concurrency limit reached ({}/{}); wait for, cancel, or reuse a worker before spawning another",
                self.running_count(),
                self.max_concurrency
            );
        }
        let id = format!("worker-{}", self.next_worker);
        self.next_worker = self.next_worker.saturating_add(1);
        Ok(id)
    }

    pub(super) fn insert_running(
        &mut self,
        metadata: WorkerMetadata,
        cancellation: CancellationToken,
        handle: JoinHandle<Result<SubagentReport>>,
    ) {
        self.workers.insert(
            metadata.id.clone(),
            Worker {
                metadata,
                state: WorkerState::Running {
                    cancellation,
                    handle,
                },
            },
        );
    }

    pub(super) async fn wait(&mut self, id: &str) -> Result<SubagentReport> {
        let worker = self
            .workers
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("unknown subagent `{id}`"))?;
        match &mut worker.state {
            WorkerState::Completed(report) => Ok(report.clone()),
            WorkerState::Failed(message) => bail!("subagent `{id}` failed: {message}"),
            WorkerState::Cancelled => bail!("subagent `{id}` was cancelled"),
            WorkerState::Running { .. } => {
                let state = std::mem::replace(&mut worker.state, WorkerState::Cancelled);
                let WorkerState::Running { handle, .. } = state else {
                    unreachable!("running worker state changed while waiting")
                };
                match handle.await {
                    Ok(Ok(report)) => {
                        worker.state = WorkerState::Completed(report.clone());
                        Ok(report)
                    }
                    Ok(Err(error)) => {
                        let message = error.to_string();
                        worker.state = WorkerState::Failed(message.clone());
                        bail!("subagent `{id}` failed: {message}")
                    }
                    Err(error) => {
                        let message = if error.is_cancelled() {
                            "worker task cancelled".to_string()
                        } else {
                            format!("worker task join failure: {error}")
                        };
                        worker.state = WorkerState::Failed(message.clone());
                        bail!("subagent `{id}` failed: {message}")
                    }
                }
            }
        }
    }

    pub(super) fn completed_report(&self, id: &str) -> Result<SubagentReport> {
        let worker = self
            .workers
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("unknown subagent `{id}`"))?;
        match &worker.state {
            WorkerState::Completed(report) => Ok(report.clone()),
            WorkerState::Running { .. } => {
                bail!("subagent `{id}` is still running; wait for it before sending a follow-up")
            }
            WorkerState::Failed(message) => bail!("subagent `{id}` failed: {message}"),
            WorkerState::Cancelled => bail!("subagent `{id}` was cancelled"),
        }
    }

    pub(super) fn metadata(&self, id: &str) -> Result<WorkerMetadata> {
        self.workers
            .get(id)
            .map(|worker| worker.metadata.clone())
            .ok_or_else(|| anyhow::anyhow!("unknown subagent `{id}`"))
    }

    pub(super) fn cancel(&mut self, id: Option<&str>) -> Result<Vec<String>> {
        let ids = match id {
            Some(id) => vec![id.to_string()],
            None => self.workers.keys().cloned().collect(),
        };
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut cancelled = Vec::new();
        for id in ids {
            let worker = self
                .workers
                .get_mut(&id)
                .ok_or_else(|| anyhow::anyhow!("unknown subagent `{id}`"))?;
            if let WorkerState::Running {
                cancellation,
                handle,
            } = &worker.state
            {
                cancellation.cancel();
                handle.abort();
                worker.state = WorkerState::Cancelled;
                cancelled.push(id);
            }
        }
        Ok(cancelled)
    }

    pub(super) fn status_json(&self) -> Value {
        json!({
            "max_concurrency": self.max_concurrency,
            "running": self.running_count(),
            "workers": self.workers.values().map(|worker| {
                let status = match &worker.state {
                    WorkerState::Running { handle, .. } if handle.is_finished() => "ready",
                    WorkerState::Running { .. } => "running",
                    WorkerState::Completed(_) => "completed",
                    WorkerState::Failed(_) => "failed",
                    WorkerState::Cancelled => "cancelled",
                };
                json!({
                    "id": worker.metadata.id,
                    "status": status,
                    "kind": worker.metadata.kind,
                    "task": worker.metadata.task,
                    "model": worker.metadata.model,
                    "reasoning_effort": worker.metadata.reasoning_effort,
                    "mode": worker.metadata.mode,
                    "ownership": worker.metadata.ownership,
                })
            }).collect::<Vec<_>>(),
        })
    }
}

impl Drop for SubagentTeam {
    fn drop(&mut self) {
        for worker in self.workers.values_mut() {
            if let WorkerState::Running {
                cancellation,
                handle,
            } = &worker.state
            {
                cancellation.cancel();
                handle.abort();
            }
        }
    }
}
