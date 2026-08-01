use serde::{Deserialize, Serialize};

pub const EVENT_SCHEMA: &str = "spark.proofline.validation.v1";
pub const AGGREGATE_SCHEMA: &str = "spark.proofline.validation.aggregate.v1";
pub const CAPTURE_MODE: &str = "host_authoritative";

#[derive(Debug, Clone, Serialize)]
pub struct BuildIdentity {
    pub git_sha: String,
    pub dirty: bool,
}

impl BuildIdentity {
    pub(crate) fn embedded() -> Self {
        Self {
            git_sha: option_env!("PROOFLINE_BUILD_GIT_SHA")
                .unwrap_or("unknown")
                .into(),
            dirty: option_env!("PROOFLINE_BUILD_GIT_DIRTY") == Some("true"),
        }
    }

    pub(crate) fn is_verified(&self) -> bool {
        self.git_sha.len() == 40
            && self.git_sha.bytes().all(|byte| byte.is_ascii_hexdigit())
            && !self.dirty
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FixtureVerification {
    pub id: String,
    pub revision: String,
    pub sha256: String,
    pub verified: bool,
    pub build_verified: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureRequest {
    pub id: String,
    pub revision: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RetentionStatus {
    pub status: String,
    pub purge_status: String,
    pub retention_deadline_days: u8,
    pub retention_deadline_status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreflightReport {
    pub capture_mode: String,
    pub countable: bool,
    pub fixture: FixtureVerification,
    pub retention: RetentionStatus,
    pub build: BuildIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StartSessionReport {
    pub capture_mode: String,
    pub countable: bool,
    pub participant_id: String,
    pub session_namespace: String,
    pub fixture: FixtureVerification,
    pub retention: RetentionStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendererInteraction {
    pub event_type: String,
    pub participant_id: String,
    pub task_id: String,
    pub outcome: String,
    pub capture_mode: String,
}

impl RendererInteraction {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(
            self.event_type.as_str(),
            "run_submitted" | "activity_rendered" | "task_outcome"
        ) {
            return Err("event_type is not a Wave 1 category".into());
        }
        if !matches!(
            self.task_id.as_str(),
            "proofline-1" | "proofline-2" | "proofline-3" | "proofline-4" | "proofline-5"
        ) {
            return Err("task_id is not a Wave 1 category".into());
        }
        if !matches!(
            self.outcome.as_str(),
            "success" | "failure" | "hinted" | "abandoned"
        ) {
            return Err("outcome is not a Wave 1 category".into());
        }
        if !matches!(
            self.participant_id.as_bytes(),
            [b'P', b'0'..=b'9', b'0'..=b'9']
        ) || self.participant_id == "P00"
        {
            return Err("participant_id must be a pseudonymous P01 through P99 identifier".into());
        }
        if self.capture_mode != CAPTURE_MODE {
            return Err("capture_mode must be host_authoritative".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AppendEventReport {
    pub acknowledged: bool,
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AggregatePreview {
    pub schema: String,
    pub event_count: usize,
    pub invalid_preflight_attempt_count: usize,
    pub task_counts: Vec<TaskCount>,
    pub outcome_counts: Vec<CategoryCount>,
    pub hint_count: usize,
    pub abandonment_count: usize,
    pub first_activity_ms: Option<u128>,
    pub retention: RetentionStatus,
    pub download_ready: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskCount {
    pub task_id: String,
    pub count: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct CategoryCount {
    pub outcome: String,
    pub count: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct PurgeReport {
    pub purged: bool,
    pub next_session_namespace: String,
    pub retention: RetentionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LedgerEvent {
    pub(crate) schema: String,
    pub(crate) thread_id: String,
    pub(crate) event_id: String,
    pub(crate) namespace: String,
    pub(crate) sequence: u64,
    pub(crate) timestamp_ms: u128,
    pub(crate) participant_id: String,
    pub(crate) task_id: String,
    pub(crate) event_type: String,
    pub(crate) outcome: String,
}
