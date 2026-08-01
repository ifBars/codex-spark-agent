use super::{
    crypto::{new_key, seal},
    fixture::{BundleFixture, VerifiedFixture},
    protector::{KeyProtector, WindowsDpapiProtector},
    types::{
        AggregatePreview, AppendEventReport, BuildIdentity, CategoryCount, FixtureRequest,
        FixtureVerification, LedgerEvent, PreflightReport, PurgeReport, RendererInteraction,
        RetentionStatus, StartSessionReport, TaskCount, CAPTURE_MODE, EVENT_SCHEMA,
    },
};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

pub(crate) struct Wave1Host {
    data_dir: PathBuf,
    fixture: BundleFixture,
    protector: Arc<dyn KeyProtector>,
    key: Option<[u8; 32]>,
    session_id: Option<String>,
    participant_id: Option<String>,
    namespace: String,
    sequence: u64,
    seen: HashSet<String>,
    events: Vec<LedgerEvent>,
    rotated: bool,
    started_at_ms: Option<u128>,
    first_activity_ms: Option<u128>,
}

impl Wave1Host {
    pub(crate) fn for_app(data_dir: PathBuf) -> Self {
        #[cfg(windows)]
        let protector: Arc<dyn KeyProtector> = Arc::new(WindowsDpapiProtector);
        #[cfg(not(windows))]
        let protector: Arc<dyn KeyProtector> = Arc::new(super::protector::UnavailableProtector);
        Self::new(data_dir, BundleFixture::bundled(), protector)
    }

    pub(crate) fn new(
        data_dir: PathBuf,
        fixture: BundleFixture,
        protector: Arc<dyn KeyProtector>,
    ) -> Self {
        Self {
            data_dir,
            fixture,
            protector,
            key: None,
            session_id: None,
            participant_id: None,
            namespace: new_namespace(),
            sequence: 0,
            seen: HashSet::new(),
            events: Vec::new(),
            rotated: false,
            started_at_ms: None,
            first_activity_ms: None,
        }
    }

    pub(crate) fn preflight(
        &mut self,
        requested: FixtureRequest,
    ) -> Result<PreflightReport, String> {
        let verified = match self.fixture.preflight() {
            Ok(value) => value,
            Err(reason) => return Ok(self.report(false, None, false, Some(reason))),
        };
        if !fixture_matches(&requested, &verified) {
            return Ok(self.report(
                false,
                Some(verified),
                true,
                Some("renderer fixture identity does not match bundled verified bytes".into()),
            ));
        }
        match self.load_or_create_key() {
            Ok(()) => Ok(self.report(true, Some(verified), true, None)),
            Err(reason) => Ok(self.report(false, Some(verified), true, Some(reason))),
        }
    }

    pub(crate) fn start_session(
        &mut self,
        participant_id: String,
        requested: FixtureRequest,
    ) -> Result<StartSessionReport, String> {
        validate_participant(&participant_id)?;
        let preflight = self.preflight(requested)?;
        if !preflight.countable {
            return Err(preflight
                .reason
                .unwrap_or_else(|| "Wave 1 preflight failed closed".into()));
        }
        self.session_id = Some(Uuid::new_v4().to_string());
        self.participant_id = Some(participant_id.clone());
        self.namespace = new_namespace();
        self.sequence = 0;
        self.seen.clear();
        self.events.clear();
        self.rotated = false;
        self.started_at_ms = Some(now_ms());
        self.first_activity_ms = None;
        Ok(StartSessionReport {
            capture_mode: CAPTURE_MODE.into(),
            countable: true,
            participant_id,
            session_namespace: self.namespace.clone(),
            fixture: preflight.fixture,
            retention: preflight.retention,
        })
    }

    pub(crate) fn append_event(
        &mut self,
        interaction: RendererInteraction,
    ) -> Result<AppendEventReport, String> {
        interaction.validate()?;
        if self.session_id.is_none()
            || self.participant_id.as_deref() != Some(&interaction.participant_id)
        {
            return Err("event participant does not match an active countable session".into());
        }
        let key = self
            .key
            .ok_or_else(|| "protected ledger key is unavailable".to_owned())?;
        let fingerprint = interaction.fingerprint();
        let latency_ms = lifecycle_latency(
            &interaction.event_type,
            self.started_at_ms,
            &mut self.first_activity_ms,
        );
        if self.seen.contains(&fingerprint) {
            return Ok(AppendEventReport {
                acknowledged: true,
                event_type: interaction.event_type,
                latency_ms,
            });
        }
        self.sequence += 1;
        let event = LedgerEvent {
            schema: EVENT_SCHEMA.into(),
            namespace: self.namespace.clone(),
            sequence: self.sequence,
            timestamp_ms: now_ms(),
            participant_id: interaction.participant_id,
            task_id: interaction.task_id,
            event_type: interaction.event_type,
            outcome: interaction.outcome,
        };
        let ciphertext = seal(
            &key,
            &serde_json::to_vec(&event)
                .map_err(|_| "could not serialize Wave 1 event".to_owned())?,
        )?;
        fs::create_dir_all(&self.data_dir)
            .map_err(|error| format!("could not create ledger location: {error}"))?;
        let mut ledger = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.ledger_path())
            .map_err(|error| format!("could not open encrypted ledger: {error}"))?;
        writeln!(ledger, "{ciphertext}")
            .map_err(|error| format!("could not append encrypted ledger: {error}"))?;
        self.seen.insert(fingerprint);
        self.events.push(event);
        Ok(AppendEventReport {
            acknowledged: true,
            event_type: self.events.last().expect("event added").event_type.clone(),
            latency_ms,
        })
    }

    pub(crate) fn preview_aggregate(&self, download: bool) -> Result<AggregatePreview, String> {
        let mut task_counts = BTreeMap::new();
        let mut outcome_counts = BTreeMap::new();
        for event in &self.events {
            *task_counts.entry(event.task_id.clone()).or_insert(0) += 1;
            if event.event_type == "task_outcome" {
                *outcome_counts.entry(event.outcome.clone()).or_insert(0) += 1;
            }
        }
        Ok(AggregatePreview {
            schema: EVENT_SCHEMA.into(),
            event_count: self.events.len(),
            task_counts: task_counts
                .into_iter()
                .map(|(task_id, count)| TaskCount { task_id, count })
                .collect(),
            outcome_counts: outcome_counts
                .into_iter()
                .map(|(outcome, count)| CategoryCount { outcome, count })
                .collect(),
            hint_count: self
                .events
                .iter()
                .filter(|event| event.event_type == "task_outcome" && event.outcome == "hinted")
                .count(),
            abandonment_count: self
                .events
                .iter()
                .filter(|event| event.event_type == "task_outcome" && event.outcome == "abandoned")
                .count(),
            first_activity_ms: self.first_activity_ms,
            retention: self.retention(),
            download_ready: download && self.session_id.is_some() && self.key.is_some(),
        })
    }

    pub(crate) fn purge_session(&mut self, confirm: bool) -> Result<PurgeReport, String> {
        if !confirm {
            return Err("purge requires explicit confirmation".into());
        }
        self.key = None;
        for path in [self.ledger_path(), self.key_path()] {
            if path.exists() {
                fs::remove_file(path)
                    .map_err(|error| format!("could not crypto-erase Wave 1 ledger: {error}"))?;
            }
        }
        self.session_id = None;
        self.participant_id = None;
        self.sequence = 0;
        self.seen.clear();
        self.events.clear();
        self.namespace = new_namespace();
        self.rotated = true;
        self.started_at_ms = None;
        self.first_activity_ms = None;
        Ok(PurgeReport {
            purged: true,
            next_session_namespace: self.namespace.clone(),
            retention: self.retention(),
        })
    }

    fn report(
        &self,
        countable: bool,
        verified: Option<VerifiedFixture>,
        evidence_verified: bool,
        reason: Option<String>,
    ) -> PreflightReport {
        let fixture = verified
            .map(|fixture| FixtureVerification {
                id: fixture.id,
                revision: fixture.revision,
                sha256: fixture.sha256,
                verified: evidence_verified,
                build_verified: true,
            })
            .unwrap_or(FixtureVerification {
                id: "unavailable".into(),
                revision: "unavailable".into(),
                sha256: "unavailable".into(),
                verified: false,
                build_verified: false,
            });
        PreflightReport {
            capture_mode: CAPTURE_MODE.into(),
            countable,
            fixture,
            retention: self.retention(),
            build: BuildIdentity {
                git_sha: option_env!("PROOFLINE_BUILD_GIT_SHA")
                    .unwrap_or("unknown")
                    .into(),
                dirty: option_env!("PROOFLINE_BUILD_GIT_DIRTY") == Some("true"),
            },
            reason,
        }
    }
    fn retention(&self) -> RetentionStatus {
        RetentionStatus {
            status: if self.key.is_some() {
                "encrypted_local".into()
            } else {
                "not_persisted".into()
            },
            purge_status: if self.rotated {
                "crypto_erased".into()
            } else {
                "ready".into()
            },
        }
    }
    fn load_or_create_key(&mut self) -> Result<(), String> {
        if self.key.is_some() {
            return Ok(());
        }
        fs::create_dir_all(&self.data_dir)
            .map_err(|error| format!("could not create protected storage: {error}"))?;
        let path = self.key_path();
        let material = if path.exists() {
            self.protector.unprotect(
                &fs::read(path)
                    .map_err(|error| format!("could not read protected ledger key: {error}"))?,
            )?
        } else {
            let key = new_key();
            let protected = self.protector.protect(&key)?;
            fs::write(path, protected)
                .map_err(|error| format!("could not write protected ledger key: {error}"))?;
            key.to_vec()
        };
        self.key = Some(
            material
                .try_into()
                .map_err(|_| "protected ledger key has invalid length".to_owned())?,
        );
        Ok(())
    }
    fn key_path(&self) -> PathBuf {
        self.data_dir.join("wave1-ledger.key.dpapi")
    }
    fn ledger_path(&self) -> PathBuf {
        self.data_dir.join("wave1-ledger.events.enc")
    }
}

fn fixture_matches(requested: &FixtureRequest, verified: &VerifiedFixture) -> bool {
    requested.id == verified.id
        && requested.revision == verified.revision
        && requested.sha256 == verified.sha256
}
fn validate_participant(value: &str) -> Result<(), String> {
    if matches!(value.as_bytes(), [b'P', b'0'..=b'9', b'0'..=b'9']) && value != "P00" {
        Ok(())
    } else {
        Err("participant_id must be a pseudonymous P01 through P99 identifier".into())
    }
}
fn lifecycle_latency(
    event_type: &str,
    started_at: Option<u128>,
    first_activity: &mut Option<u128>,
) -> Option<u128> {
    if !matches!(event_type, "app_ready" | "activity_rendered") {
        return None;
    }
    let latency = started_at.map(|started| now_ms().saturating_sub(started));
    if event_type == "activity_rendered" && first_activity.is_none() {
        *first_activity = latency;
    }
    latency
}
fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
fn new_namespace() -> String {
    format!("wave1-{}", Uuid::new_v4())
}
#[cfg(test)]
impl Wave1Host {
    pub(crate) fn ledger_path_for_test(&self) -> PathBuf {
        self.ledger_path()
    }
}
