use super::{
    fixture::BundleFixture,
    host::Wave1Host,
    protector::{TestProtector, UnavailableProtector},
    types::{FixtureRequest, RendererInteraction},
};
use std::sync::Arc;
use tempfile::TempDir;

const FIXTURE_SHA: &str = "7829776e9aea00a0d182d00cddc3337f07659d728fbea9b31b30fdc05f36b3bf";
fn request() -> FixtureRequest {
    FixtureRequest {
        id: "proofline-wave1-local".into(),
        revision: "2026-08-01.1".into(),
        sha256: FIXTURE_SHA.into(),
    }
}
fn host() -> (TempDir, Wave1Host) {
    let directory = TempDir::new().expect("temporary app data directory");
    let host = Wave1Host::new(
        directory.path().to_path_buf(),
        BundleFixture::bundled(),
        Arc::new(TestProtector),
    );
    (directory, host)
}
fn event(event_type: &str, outcome: &str) -> RendererInteraction {
    RendererInteraction {
        event_type: event_type.into(),
        participant_id: "P01".into(),
        task_id: "proofline-1".into(),
        outcome: outcome.into(),
        capture_mode: "host_authoritative".into(),
    }
}

#[test]
fn privacy_allowlist_rejects_freeform_renderer_content() {
    assert!(event("run_submitted", "success").validate().is_ok());
    assert!(RendererInteraction {
        event_type: "open C:/private/prompt".into(),
        ..event("run_submitted", "success")
    }
    .validate()
    .is_err());
    assert!(RendererInteraction {
        participant_id: "P01 note".into(),
        ..event("run_submitted", "success")
    }
    .validate()
    .is_err());
}
#[test]
fn tampered_bundled_evidence_rejects_countability() {
    let directory = TempDir::new().expect("temporary app data directory");
    let mut host = Wave1Host::new(
        directory.path().to_path_buf(),
        BundleFixture::tampered(),
        Arc::new(TestProtector),
    );
    let report = host.preflight(request()).expect("preflight report");
    assert!(!report.countable);
    assert!(!report.fixture.verified);
    assert!(report.reason.unwrap_or_default().contains("does not match"));
}
#[test]
fn fixture_request_must_match_actual_bundled_bytes() {
    let (_directory, mut host) = host();
    let mut mismatch = request();
    mismatch.sha256 = "not-a-bundle-hash".into();
    let report = host.preflight(mismatch).expect("preflight report");
    assert!(!report.countable);
    assert!(report.reason.unwrap_or_default().contains("does not match"));
}

#[test]
fn bridge_contract_uses_snake_case_aggregate_only_dtos() {
    let (_directory, mut host) = host();
    let preflight = serde_json::to_value(host.preflight(request()).expect("preflight"))
        .expect("serialize preflight");
    assert_eq!(preflight["capture_mode"], "host_authoritative");
    assert_eq!(preflight["fixture"]["sha256"], FIXTURE_SHA);
    assert!(preflight.get("captureMode").is_none());
    let session = host
        .start_session("P01".into(), request())
        .expect("session starts");
    assert_eq!(session.participant_id, "P01");
    let acknowledgement = host
        .append_event(event("activity_rendered", "success"))
        .expect("event writes");
    assert!(acknowledgement.acknowledged && acknowledgement.latency_ms.is_some());
    let aggregate = serde_json::to_value(host.preview_aggregate(true).expect("aggregate"))
        .expect("serialize aggregate");
    let allowed = [
        "schema",
        "event_count",
        "task_counts",
        "outcome_counts",
        "hint_count",
        "abandonment_count",
        "first_activity_ms",
        "retention",
        "download_ready",
    ];
    assert!(aggregate
        .as_object()
        .expect("aggregate object")
        .keys()
        .all(|key| allowed.contains(&key.as_str())));
}
#[test]
fn events_are_monotonic_and_deduplicated() {
    let (_directory, mut host) = host();
    host.start_session("P01".into(), request())
        .expect("session starts");
    let first = host
        .append_event(event("run_submitted", "success"))
        .expect("first event");
    let duplicate = host
        .append_event(event("run_submitted", "success"))
        .expect("duplicate accepted");
    let second = host
        .append_event(event("activity_rendered", "success"))
        .expect("second event");
    assert!(first.acknowledged && duplicate.acknowledged && second.acknowledged);
    assert_eq!(
        host.preview_aggregate(false).expect("preview").event_count,
        2
    );
}
#[test]
fn encrypted_ledger_does_not_contain_event_plaintext() {
    let (_directory, mut host) = host();
    host.start_session("P01".into(), request())
        .expect("session starts");
    host.append_event(event("run_submitted", "success"))
        .expect("event writes");
    let ledger = std::fs::read_to_string(host.ledger_path_for_test()).expect("ciphertext ledger");
    assert!(!ledger.contains("run_submitted"));
    assert!(!ledger.contains("proofline-1"));
    assert!(!ledger.contains("P01"));
}
#[test]
fn unavailable_protected_storage_fails_countability_closed() {
    let directory = TempDir::new().expect("temporary app data directory");
    let mut host = Wave1Host::new(
        directory.path().to_path_buf(),
        BundleFixture::bundled(),
        Arc::new(UnavailableProtector),
    );
    let report = host.preflight(request()).expect("preflight report");
    assert!(!report.countable);
    assert!(report.reason.unwrap_or_default().contains("unavailable"));
}
#[test]
fn aggregate_preview_has_counts_but_no_session_or_event_identifiers() {
    let (_directory, mut host) = host();
    host.start_session("P01".into(), request())
        .expect("session starts");
    host.append_event(event("task_outcome", "hinted"))
        .expect("event writes");
    let rendered =
        serde_json::to_string(&host.preview_aggregate(false).expect("aggregate preview"))
            .expect("serialize preview");
    assert!(rendered.contains("hinted"));
    assert!(!rendered.contains("P01"));
    assert!(!rendered.contains("timestamp"));
    assert!(!rendered.contains("namespace"));
}
#[test]
fn confirmed_purge_crypto_erases_key_and_rotates_namespace() {
    let (_directory, mut host) = host();
    let started = host
        .start_session("P01".into(), request())
        .expect("session starts");
    host.append_event(event("task_outcome", "failure"))
        .expect("event writes");
    assert!(host.purge_session(false).is_err());
    let purge = host.purge_session(true).expect("confirmed purge");
    assert_ne!(started.session_namespace, purge.next_session_namespace);
    assert!(purge.purged);
    assert_eq!(
        host.preview_aggregate(false).expect("preview").event_count,
        0
    );
}
