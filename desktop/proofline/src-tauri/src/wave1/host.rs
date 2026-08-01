use super::{
    crypto::{new_key, open, seal},
    fixture::{BundleFixture, VerifiedFixture},
    protector::{KeyProtector, WindowsDpapiProtector},
    types::{
        AggregatePreview, AppendEventReport, BuildIdentity, CategoryCount, FixtureRequest,
        FixtureVerification, LedgerEvent, PreflightReport, PurgeReport, RendererInteraction,
        RetentionStatus, StartSessionReport, TaskCount, CAPTURE_MODE, EVENT_SCHEMA,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

#[derive(Debug, Default, Serialize, Deserialize)]
struct HostMetadata {
    retention_deadline_ms: Option<u128>,
    invalid_preflight_attempt_count: usize,
}

pub(crate) struct Wave1Host {
    data_dir: PathBuf,
    fixture: BundleFixture,
    protector: Arc<dyn KeyProtector>,
    key: Option<[u8; 32]>,
    session_id: Option<String>,
    thread_id: Option<String>,
    participant_id: Option<String>,
    namespace: String,
    sequence: u64,
    events: Vec<LedgerEvent>,
    rotated: bool,
    started_at_ms: Option<u128>,
    first_activity_ms: Option<u128>,
    retention_deadline_ms: Option<u128>,
    metadata: HostMetadata,
    build: BuildIdentity,
    lifecycle_contract_verified: bool,
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
        Self::new_with_build_identity(data_dir, fixture, protector, BuildIdentity::embedded())
    }

    pub(crate) fn new_with_build_identity(
        data_dir: PathBuf,
        fixture: BundleFixture,
        protector: Arc<dyn KeyProtector>,
        build: BuildIdentity,
    ) -> Self {
        Self::new_with_build_identity_and_lifecycle(data_dir, fixture, protector, build, false)
    }

    pub(crate) fn new_with_build_identity_and_lifecycle(
        data_dir: PathBuf,
        fixture: BundleFixture,
        protector: Arc<dyn KeyProtector>,
        build: BuildIdentity,
        lifecycle_contract_verified: bool,
    ) -> Self {
        Self {
            data_dir,
            fixture,
            protector,
            key: None,
            session_id: None,
            thread_id: None,
            participant_id: None,
            namespace: new_namespace(),
            sequence: 0,
            events: Vec::new(),
            rotated: false,
            started_at_ms: None,
            first_activity_ms: None,
            retention_deadline_ms: None,
            metadata: HostMetadata::default(),
            build,
            lifecycle_contract_verified,
        }
    }

    pub(crate) fn preflight(
        &mut self,
        requested: FixtureRequest,
    ) -> Result<PreflightReport, String> {
        if let Err(reason) = self.load_or_create_key() {
            return Ok(self.report(false, None, false, Some(reason)));
        }
        if self.erase_expired_storage_if_needed()? {
            return Ok(self.report(
                false,
                None,
                false,
                Some(
                    "retention window expired; encrypted local artifacts were crypto-erased".into(),
                ),
            ));
        }
        let verified = match self.fixture.preflight() {
            Ok(value) => value,
            Err(reason) => {
                self.record_invalid_preflight_attempt()?;
                return Ok(self.report(false, None, false, Some(reason)));
            }
        };
        if !fixture_matches(&requested, &verified) {
            self.record_invalid_preflight_attempt()?;
            return Ok(self.report(
                false,
                Some(verified),
                true,
                Some("renderer fixture identity does not match bundled verified bytes".into()),
            ));
        }
        if !self.build.is_verified() {
            return Ok(self.report(
                false,
                Some(verified),
                true,
                Some("embedded build identity is unavailable or dirty".into()),
            ));
        }
        if !self.lifecycle_contract_verified {
            return Ok(self.report(
                false,
                Some(verified),
                true,
                Some(
                    "native lifecycle boundary is not verified; countability remains fail-closed"
                        .into(),
                ),
            ));
        }
        Ok(self.report(true, Some(verified), true, None))
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
        self.thread_id = Some(Uuid::new_v4().to_string());
        self.participant_id = Some(participant_id.clone());
        self.namespace = new_namespace();
        self.sequence = 0;
        self.events.clear();
        self.rotated = false;
        self.started_at_ms = Some(now_ms());
        self.first_activity_ms = None;
        self.retention_deadline_ms = self.started_at_ms.map(retention_deadline);
        self.metadata.retention_deadline_ms = self.retention_deadline_ms;
        self.persist_metadata()?;
        Ok(StartSessionReport {
            capture_mode: CAPTURE_MODE.into(),
            countable: true,
            participant_id,
            session_namespace: self.namespace.clone(),
            fixture: preflight.fixture,
            retention: self.retention(),
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
        if !self.retention_eligible_at(now_ms()) {
            return Err(
                "retention window expired; explicitly purge before starting a new session".into(),
            );
        }
        let key = self
            .key
            .ok_or_else(|| "protected ledger key is unavailable".to_owned())?;
        let latency_ms = session_lifecycle_latency(
            &interaction.event_type,
            self.started_at_ms,
            &mut self.first_activity_ms,
        );
        self.sequence += 1;
        let event = LedgerEvent {
            schema: EVENT_SCHEMA.into(),
            thread_id: self
                .thread_id
                .clone()
                .ok_or_else(|| "host thread identity is unavailable".to_owned())?,
            event_id: Uuid::new_v4().to_string(),
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
        self.events.push(event);
        Ok(AppendEventReport {
            acknowledged: true,
            event_type: self.events.last().expect("event added").event_type.clone(),
            latency_ms,
        })
    }

    pub(crate) fn preview_aggregate(&mut self, download: bool) -> Result<AggregatePreview, String> {
        let mut task_counts = BTreeMap::new();
        let mut outcome_counts = BTreeMap::new();
        for event in &self.events {
            *task_counts.entry(event.task_id.clone()).or_insert(0) += 1;
            if event.event_type == "task_outcome" {
                *outcome_counts.entry(event.outcome.clone()).or_insert(0) += 1;
            }
        }
        let mut aggregate = AggregatePreview {
            schema: EVENT_SCHEMA.into(),
            event_count: self.events.len(),
            invalid_preflight_attempt_count: self.metadata.invalid_preflight_attempt_count,
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
            download_ready: false,
        };
        if download
            && self.session_id.is_some()
            && self.key.is_some()
            && self.retention_eligible_at(now_ms())
        {
            aggregate.download_ready = true;
            let bytes = serde_json::to_vec(&aggregate)
                .map_err(|_| "could not serialize aggregate-only export".to_owned())?;
            fs::write(self.aggregate_path(), bytes)
                .map_err(|error| format!("could not write aggregate-only export: {error}"))?;
            if !self.aggregate_path().is_file() {
                return Err("aggregate-only export could not be confirmed".into());
            }
        }
        Ok(aggregate)
    }

    pub(crate) fn purge_session(&mut self, confirm: bool) -> Result<PurgeReport, String> {
        if !confirm {
            return Err("purge requires explicit confirmation".into());
        }
        self.key = None;
        self.crypto_erase_artifacts()?;
        self.session_id = None;
        self.thread_id = None;
        self.participant_id = None;
        self.sequence = 0;
        self.events.clear();
        self.namespace = new_namespace();
        self.rotated = true;
        self.started_at_ms = None;
        self.first_activity_ms = None;
        self.retention_deadline_ms = None;
        self.metadata = HostMetadata::default();
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
        let build_verified = self.build.is_verified();
        let fixture = verified
            .map(|fixture| FixtureVerification {
                id: fixture.id,
                revision: fixture.revision,
                sha256: fixture.sha256,
                verified: evidence_verified,
                build_verified,
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
            countable: countable && build_verified,
            fixture,
            retention: self.retention(),
            build: self.build.clone(),
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
            retention_deadline_days: 30,
            retention_deadline_status: match self.retention_deadline_ms {
                Some(deadline) if now_ms() >= deadline => "expired".into(),
                Some(_) => "active".into(),
                None => "not_started".into(),
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
        self.load_metadata()?;
        Ok(())
    }
    fn load_metadata(&mut self) -> Result<(), String> {
        let path = self.metadata_path();
        if !path.exists() {
            self.metadata = HostMetadata::default();
            self.retention_deadline_ms = None;
            return Ok(());
        }
        let key = self
            .key
            .ok_or_else(|| "protected ledger key is unavailable".to_owned())?;
        let plaintext = open(
            &key,
            &fs::read(path)
                .map_err(|error| format!("could not read retention metadata: {error}"))?,
        )?;
        self.metadata = serde_json::from_slice(&plaintext)
            .map_err(|_| "retention metadata is malformed".to_owned())?;
        self.retention_deadline_ms = self.metadata.retention_deadline_ms;
        Ok(())
    }
    fn persist_metadata(&self) -> Result<(), String> {
        let key = self
            .key
            .ok_or_else(|| "protected ledger key is unavailable".to_owned())?;
        let bytes = serde_json::to_vec(&self.metadata)
            .map_err(|_| "could not serialize retention metadata".to_owned())?;
        let ciphertext = seal(&key, &bytes)?;
        fs::write(self.metadata_path(), ciphertext)
            .map_err(|error| format!("could not write retention metadata: {error}"))
    }
    fn record_invalid_preflight_attempt(&mut self) -> Result<(), String> {
        self.metadata.invalid_preflight_attempt_count = self
            .metadata
            .invalid_preflight_attempt_count
            .saturating_add(1);
        self.persist_metadata()
    }
    fn erase_expired_storage_if_needed(&mut self) -> Result<bool, String> {
        if !self
            .retention_deadline_ms
            .is_some_and(|deadline| now_ms() >= deadline)
        {
            return Ok(false);
        }
        self.crypto_erase_artifacts()?;
        self.key = None;
        self.session_id = None;
        self.thread_id = None;
        self.participant_id = None;
        self.events.clear();
        self.retention_deadline_ms = None;
        self.metadata = HostMetadata::default();
        self.rotated = true;
        Ok(true)
    }
    fn crypto_erase_artifacts(&self) -> Result<(), String> {
        for path in [
            self.ledger_path(),
            self.key_path(),
            self.aggregate_path(),
            self.metadata_path(),
        ] {
            if path.exists() {
                fs::remove_file(path).map_err(|error| {
                    format!("could not crypto-erase Wave 1 local artifact: {error}")
                })?;
            }
        }
        Ok(())
    }
    fn key_path(&self) -> PathBuf {
        self.data_dir.join("wave1-ledger.key.dpapi")
    }
    fn ledger_path(&self) -> PathBuf {
        self.data_dir.join("wave1-ledger.events.enc")
    }
    fn aggregate_path(&self) -> PathBuf {
        self.data_dir.join("wave1-aggregate.json")
    }
    fn metadata_path(&self) -> PathBuf {
        self.data_dir.join("wave1-retention.metadata.enc")
    }

    fn retention_eligible_at(&self, now: u128) -> bool {
        self.retention_deadline_ms
            .is_some_and(|deadline| now < deadline)
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
fn session_lifecycle_latency(
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
fn retention_deadline(started_at: u128) -> u128 {
    started_at.saturating_add(30 * 24 * 60 * 60 * 1_000)
}
#[cfg(test)]
impl Wave1Host {
    pub(crate) fn ledger_path_for_test(&self) -> PathBuf {
        self.ledger_path()
    }
    pub(crate) fn aggregate_path_for_test(&self) -> PathBuf {
        self.aggregate_path()
    }
    pub(crate) fn key_path_for_test(&self) -> PathBuf {
        self.key_path()
    }
    pub(crate) fn metadata_path_for_test(&self) -> PathBuf {
        self.metadata_path()
    }
    pub(crate) fn events_for_test(&self) -> &[LedgerEvent] {
        &self.events
    }
    pub(crate) fn retention_eligible_at_for_test(&self, now: u128) -> bool {
        self.retention_eligible_at(now)
    }
    pub(crate) fn retention_deadline_for_test(&self) -> Option<u128> {
        self.retention_deadline_ms
    }
    pub(crate) fn set_retention_deadline_for_test(&mut self, deadline: u128) {
        self.retention_deadline_ms = Some(deadline);
        self.metadata.retention_deadline_ms = Some(deadline);
        self.persist_metadata()
            .expect("persist test retention deadline");
    }
}
