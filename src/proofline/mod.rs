//! Privacy-minimized read models for the Proofline desktop surface.
//!
//! This module deliberately projects saved sessions instead of exposing the
//! persisted agent snapshot. `AgentSnapshot::input` may contain prompts,
//! tool arguments, and provider conversation state, so it is not part of the
//! public Proofline snapshot contract.

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::Serialize;

use crate::session::store::{SessionRecord, SessionStore};

pub(crate) const SNAPSHOT_SCHEMA_VERSION: &str = "spark.proofline.v1";

#[derive(Debug, Serialize)]
pub(crate) struct ProoflineSnapshot {
    pub(crate) schema_version: &'static str,
    pub(crate) kind: &'static str,
    pub(crate) captured_at_unix_ms: u64,
    pub(crate) sessions: Vec<SessionSummary>,
    pub(crate) active_session: Option<ActiveSession>,
    pub(crate) conversation: Unavailable,
    pub(crate) changed_files: Unavailable,
    pub(crate) validations: Unavailable,
    pub(crate) checkpoints: Unavailable,
    pub(crate) approvals: Unavailable,
    pub(crate) usage: UsageAvailability,
    pub(crate) fork_lineage: Unavailable,
    pub(crate) capabilities: Capabilities,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct SessionSummary {
    pub(crate) name: String,
    pub(crate) updated_at_unix_seconds: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct ActiveSession {
    pub(crate) name: String,
    pub(crate) updated_at_unix_seconds: i64,
    pub(crate) request_sequence: usize,
    pub(crate) mode: &'static str,
    pub(crate) reasoning_effort: String,
    pub(crate) memory_enabled: bool,
    pub(crate) goal_present: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct Unavailable {
    availability: &'static str,
    reason: &'static str,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct UsageAvailability {
    availability: &'static str,
    reason: &'static str,
    pricing: Unavailable,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct Capabilities {
    mode_policy: ModePolicy,
    os_sandboxing: Unavailable,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct ModePolicy {
    availability: &'static str,
    mode: Option<&'static str>,
    summary: &'static str,
}

/// Produce a read-only snapshot of the default session store.
///
/// An absent session database is treated as an empty store. Unlike the chat
/// path, this intentionally does not create a database, migrate legacy files,
/// prune sessions, or update `last_opened_at`.
pub(crate) fn snapshot_default(requested_session: Option<&str>) -> Result<ProoflineSnapshot> {
    let store = SessionStore::open_existing_default()?;
    snapshot_from_store(store.as_ref(), requested_session)
}

pub(crate) fn snapshot_from_store(
    store: Option<&SessionStore>,
    requested_session: Option<&str>,
) -> Result<ProoflineSnapshot> {
    let sessions = store.map_or(Ok(Vec::new()), SessionStore::list)?;
    let sessions = recent_first(sessions);
    let active_session = select_active_session(store, &sessions, requested_session)?;
    let capabilities = active_session_capabilities(active_session.as_ref());

    Ok(ProoflineSnapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        kind: "snapshot",
        captured_at_unix_ms: unix_time_millis(),
        sessions,
        active_session,
        conversation: unavailable(
            "Saved agent input may contain private provider conversation state and is intentionally omitted.",
        ),
        changed_files: unavailable(
            "No typed changed-file records are persisted by the current session store.",
        ),
        validations: unavailable(
            "No typed validation records are persisted by the current session store.",
        ),
        checkpoints: unavailable(
            "Goal state is summarized only; checkpoint records are not exposed by this snapshot.",
        ),
        approvals: unavailable("The current harness has no persisted approval record surface."),
        usage: UsageAvailability {
            availability: "unavailable",
            reason: "This snapshot does not scan local usage history.",
            pricing: unavailable(
                "No authoritative price or per-run cost source is available in this snapshot.",
            ),
        },
        fork_lineage: unavailable("Saved Spark sessions do not persist per-thread fork lineage."),
        capabilities,
    })
}

fn recent_first(mut records: Vec<SessionRecord>) -> Vec<SessionSummary> {
    records.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.name.cmp(&right.name))
    });
    records
        .into_iter()
        .map(|record| SessionSummary {
            name: record.name,
            updated_at_unix_seconds: record.updated_at,
        })
        .collect()
}

fn select_active_session(
    store: Option<&SessionStore>,
    sessions: &[SessionSummary],
    requested_session: Option<&str>,
) -> Result<Option<ActiveSession>> {
    let Some(selected) = requested_session
        .and_then(|name| sessions.iter().find(|session| session.name == name))
        .or_else(|| {
            requested_session
                .is_none()
                .then(|| sessions.first())
                .flatten()
        })
    else {
        if let Some(name) = requested_session {
            anyhow::bail!("saved session `{name}` does not exist");
        }
        return Ok(None);
    };

    let snapshot = store
        .expect("a selected session requires a store")
        .peek(&selected.name)?
        .ok_or_else(|| anyhow::anyhow!("saved session `{}` does not exist", selected.name))?;

    Ok(Some(ActiveSession {
        name: selected.name.clone(),
        updated_at_unix_seconds: selected.updated_at_unix_seconds,
        request_sequence: snapshot.request_seq,
        mode: snapshot.mode.name(),
        reasoning_effort: snapshot.reasoning_effort,
        memory_enabled: snapshot.memory_enabled,
        goal_present: snapshot.goal.is_some(),
    }))
}

fn active_session_capabilities(active_session: Option<&ActiveSession>) -> Capabilities {
    let mode_policy = match active_session.map(|session| session.mode) {
        Some("ask") => ModePolicy {
            availability: "reported",
            mode: Some("ask"),
            summary: "Ask is a harness tool policy that permits only the configured read-only native tools.",
        },
        Some("work") => ModePolicy {
            availability: "reported",
            mode: Some("work"),
            summary: "Work is a harness tool policy that permits configured native tools, including command execution.",
        },
        Some(_) => ModePolicy {
            availability: "unavailable",
            mode: None,
            summary: "The saved mode is not recognized by this build.",
        },
        None => ModePolicy {
            availability: "unavailable",
            mode: None,
            summary: "No saved session is selected, so no mode policy is reported.",
        },
    };
    Capabilities {
        mode_policy,
        os_sandboxing: unavailable(
            "Mode policy is not OS sandboxing; this snapshot does not establish process or filesystem confinement.",
        ),
    }
}

fn unavailable(reason: &'static str) -> Unavailable {
    Unavailable {
        availability: "unavailable",
        reason,
    }
}

fn unix_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentSnapshot;
    use crate::profiler::AgentProfiler;
    use crate::tools::AgentMode;

    fn snapshot_with_input(input: &str, mode: AgentMode) -> AgentSnapshot {
        let snapshot = AgentSnapshot {
            schema_version: 1,
            input: vec![serde_json::json!({
                "role": "user",
                "content": [{"type": "input_text", "text": input}]
            })],
            request_seq: 7,
            profiler: AgentProfiler::default(),
            loaded_skills: vec!["rust-patterns".to_string()],
            mode,
            reasoning_effort: "high".to_string(),
            goal: None,
            memory_enabled: true,
        };
        let mut serialized = serde_json::to_value(snapshot).expect("serialize snapshot");
        serialized["goal"] = serde_json::json!({
            "objective": "secret goal",
            "status": "running",
            "checkpoint": 0,
            "progress_log": []
        });
        serde_json::from_value(serialized).expect("deserialize snapshot with goal")
    }

    fn temp_store() -> (tempfile::TempDir, SessionStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = SessionStore::open_at(
            dir.path().join("sessions.sqlite3"),
            dir.path().join("sessions"),
        )
        .expect("open store");
        (dir, store)
    }

    #[test]
    fn sessions_are_recent_first_and_default_to_the_latest_session() {
        let (dir, store) = temp_store();
        store
            .save("older", &snapshot_with_input("older", AgentMode::Work))
            .expect("save older");
        store
            .save("latest", &snapshot_with_input("latest", AgentMode::Ask))
            .expect("save latest");
        let conn = rusqlite::Connection::open(dir.path().join("sessions.sqlite3")).expect("db");
        conn.execute(
            "UPDATE sessions SET updated_at = 10 WHERE name = 'older'",
            [],
        )
        .expect("set older timestamp");
        conn.execute(
            "UPDATE sessions SET updated_at = 20 WHERE name = 'latest'",
            [],
        )
        .expect("set latest timestamp");

        let snapshot = snapshot_from_store(Some(&store), None).expect("snapshot");

        assert_eq!(snapshot.sessions[0].name, "latest");
        assert_eq!(snapshot.sessions[1].name, "older");
        assert_eq!(snapshot.active_session.expect("active").name, "latest");
    }

    #[test]
    fn explicit_session_selection_and_missing_session_are_unambiguous() {
        let (_dir, store) = temp_store();
        store
            .save("other", &snapshot_with_input("input", AgentMode::Work))
            .expect("save session");

        let snapshot = snapshot_from_store(Some(&store), Some("other")).expect("snapshot");
        assert_eq!(snapshot.active_session.expect("active").name, "other");

        let error = snapshot_from_store(Some(&store), Some("missing")).expect_err("missing");
        assert_eq!(error.to_string(), "saved session `missing` does not exist");
    }

    #[test]
    fn empty_store_has_no_active_session() {
        let (_dir, store) = temp_store();

        let snapshot = snapshot_from_store(Some(&store), None).expect("snapshot");

        assert!(snapshot.sessions.is_empty());
        assert!(snapshot.active_session.is_none());
        assert_eq!(snapshot.schema_version, SNAPSHOT_SCHEMA_VERSION);
    }

    #[test]
    fn projection_omits_transcript_and_marks_unsupported_surfaces_unavailable() {
        let (_dir, store) = temp_store();
        store
            .save(
                "private",
                &snapshot_with_input("very-secret-transcript-and-tool-payload", AgentMode::Ask),
            )
            .expect("save session");

        let snapshot = snapshot_from_store(Some(&store), None).expect("snapshot");
        let value = serde_json::to_value(snapshot).expect("serialize");
        let rendered = value.to_string();

        assert!(!rendered.contains("very-secret-transcript-and-tool-payload"));
        assert!(!rendered.contains("secret goal"));
        assert_eq!(value["conversation"]["availability"], "unavailable");
        assert_eq!(value["changed_files"]["availability"], "unavailable");
        assert_eq!(value["validations"]["availability"], "unavailable");
        assert_eq!(value["checkpoints"]["availability"], "unavailable");
        assert_eq!(value["approvals"]["availability"], "unavailable");
        assert_eq!(value["usage"]["pricing"]["availability"], "unavailable");
        assert_eq!(value["fork_lineage"]["availability"], "unavailable");
        assert_eq!(value["capabilities"]["mode_policy"]["mode"], "ask");
        assert_eq!(
            value["capabilities"]["os_sandboxing"]["availability"],
            "unavailable"
        );
    }
}
