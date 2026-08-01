use super::{
    fixture::BundleFixture,
    host::Wave1Host,
    protector::{TestProtector, UnavailableProtector},
    types::{BuildIdentity, FixtureRequest, RendererInteraction},
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
    host_with_build(verified_build())
}
fn verified_build() -> BuildIdentity {
    BuildIdentity {
        git_sha: "a".repeat(40),
        dirty: false,
    }
}
fn host_with_build(build: BuildIdentity) -> (TempDir, Wave1Host) {
    let directory = TempDir::new().expect("temporary app data directory");
    let host = Wave1Host::new_with_build_identity_and_lifecycle(
        directory.path().to_path_buf(),
        BundleFixture::bundled(),
        Arc::new(TestProtector),
        build,
        true,
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
    let mut host = Wave1Host::new_with_build_identity_and_lifecycle(
        directory.path().to_path_buf(),
        BundleFixture::tampered(),
        Arc::new(TestProtector),
        verified_build(),
        true,
    );
    let report = host.preflight(request()).expect("preflight report");
    assert!(!report.countable);
    assert!(!report.fixture.verified);
    assert!(report.reason.unwrap_or_default().contains("does not match"));
}
#[test]
fn dirty_or_unknown_build_identity_fails_countability_closed() {
    for build in [
        BuildIdentity {
            git_sha: "unknown".into(),
            dirty: false,
        },
        BuildIdentity {
            git_sha: "b".repeat(40),
            dirty: true,
        },
    ] {
        let (_directory, mut host) = host_with_build(build);
        let report = host.preflight(request()).expect("preflight report");
        assert!(!report.countable);
        assert!(!report.fixture.build_verified);
        assert!(report.reason.unwrap_or_default().contains("build identity"));
    }
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
fn changed_renderer_fixture_source_rejects_countability() {
    let directory = TempDir::new().expect("temporary app data directory");
    let mut host = Wave1Host::new_with_build_identity_and_lifecycle(
        directory.path().to_path_buf(),
        BundleFixture::renderer_source_tampered(),
        Arc::new(TestProtector),
        verified_build(),
        true,
    );
    let report = host.preflight(request()).expect("preflight report");
    assert!(!report.countable);
    assert!(report
        .reason
        .unwrap_or_default()
        .contains("renderer fixture source"));
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
    assert_eq!(session.retention.retention_deadline_days, 30);
    assert_eq!(session.retention.retention_deadline_status, "active");
    let acknowledgement = host
        .append_event(event("activity_rendered", "success"))
        .expect("event writes");
    assert!(acknowledgement.acknowledged && acknowledgement.latency_ms.is_some());
    let aggregate = serde_json::to_value(host.preview_aggregate(true).expect("aggregate"))
        .expect("serialize aggregate");
    let allowed = [
        "schema",
        "event_count",
        "invalid_preflight_attempt_count",
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
    assert_eq!(aggregate["download_ready"], true);
    assert!(host.aggregate_path_for_test().is_file());
    let artifact =
        std::fs::read_to_string(host.aggregate_path_for_test()).expect("aggregate artifact");
    assert!(!artifact.contains("P01"));
    assert!(!artifact.contains("timestamp"));
}
#[test]
fn distinct_same_category_actions_are_durable_and_get_host_identifiers() {
    let (_directory, mut host) = host();
    host.start_session("P01".into(), request())
        .expect("session starts");
    let first = host
        .append_event(event("run_submitted", "success"))
        .expect("first event");
    let repeated = host
        .append_event(event("run_submitted", "success"))
        .expect("repeated action accepted");
    let second = host
        .append_event(event("activity_rendered", "success"))
        .expect("second event");
    assert!(first.acknowledged && repeated.acknowledged && second.acknowledged);
    assert_eq!(
        host.preview_aggregate(false).expect("preview").event_count,
        3
    );
    let events = host.events_for_test();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].thread_id, events[1].thread_id);
    assert_ne!(events[0].event_id, events[1].event_id);
    assert_eq!(events[0].sequence, 1);
    assert_eq!(events[1].sequence, 2);
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
    let mut host = Wave1Host::new_with_build_identity_and_lifecycle(
        directory.path().to_path_buf(),
        BundleFixture::bundled(),
        Arc::new(UnavailableProtector),
        verified_build(),
        true,
    );
    let report = host.preflight(request()).expect("preflight report");
    assert!(!report.countable);
    assert!(report.reason.unwrap_or_default().contains("unavailable"));
}
#[test]
fn retention_expiry_rejects_new_events_until_startup_crypto_erases_artifacts() {
    let (_directory, mut host) = host();
    host.start_session("P01".into(), request())
        .expect("session starts");
    host.append_event(event("run_submitted", "success"))
        .expect("event writes");
    let deadline = host.retention_deadline_for_test().expect("deadline set");
    assert!(host.retention_eligible_at_for_test(deadline.saturating_sub(1)));
    assert!(!host.retention_eligible_at_for_test(deadline));
    host.set_retention_deadline_for_test(0);
    assert!(host
        .append_event(event("activity_rendered", "success"))
        .is_err());
    assert!(host.ledger_path_for_test().is_file());
}

#[test]
fn expired_retention_crypto_erases_all_local_artifacts_on_preflight() {
    let (directory, mut host) = host();
    host.start_session("P01".into(), request())
        .expect("session starts");
    host.append_event(event("task_outcome", "failure"))
        .expect("event writes");
    host.preview_aggregate(true).expect("aggregate export");
    host.set_retention_deadline_for_test(0);
    assert!(host.key_path_for_test().is_file());
    assert!(host.metadata_path_for_test().is_file());

    let mut restarted = Wave1Host::new_with_build_identity_and_lifecycle(
        directory.path().to_path_buf(),
        BundleFixture::bundled(),
        Arc::new(TestProtector),
        verified_build(),
        true,
    );
    let report = restarted.preflight(request()).expect("expired preflight");
    assert!(!report.countable);
    assert!(report.reason.unwrap_or_default().contains("crypto-erased"));
    assert!(!restarted.ledger_path_for_test().exists());
    assert!(!restarted.key_path_for_test().exists());
    assert!(!restarted.aggregate_path_for_test().exists());
    assert!(!restarted.metadata_path_for_test().exists());
}

#[test]
fn invalid_preflight_attempts_are_privacy_safe_aggregate_denominators() {
    let (directory, mut host) = host();
    let mut mismatch = request();
    mismatch.sha256 = "not-a-bundle-hash".into();
    assert!(
        !host
            .preflight(mismatch.clone())
            .expect("mismatch")
            .countable
    );
    drop(host);

    let mut restarted = Wave1Host::new_with_build_identity_and_lifecycle(
        directory.path().to_path_buf(),
        BundleFixture::bundled(),
        Arc::new(TestProtector),
        verified_build(),
        true,
    );
    assert!(!restarted.preflight(mismatch).expect("mismatch").countable);
    let aggregate = restarted.preview_aggregate(false).expect("aggregate");
    assert_eq!(aggregate.event_count, 0);
    assert_eq!(aggregate.invalid_preflight_attempt_count, 2);
    let rendered = serde_json::to_string(&aggregate).expect("serialize aggregate");
    assert!(!rendered.contains("participant"));
    assert!(!rendered.contains("not-a-bundle-hash"));
}

#[test]
fn renderer_cannot_supply_host_owned_identity_time_or_sequence() {
    for field in ["thread_id", "event_id", "timestamp_ms", "sequence"] {
        let mut value = serde_json::json!({
            "event_type": "run_submitted",
            "participant_id": "P01",
            "task_id": "proofline-1",
            "outcome": "success",
            "capture_mode": "host_authoritative"
        });
        value
            .as_object_mut()
            .expect("object")
            .insert(field.into(), serde_json::Value::String("forged".into()));
        assert!(serde_json::from_value::<RendererInteraction>(value).is_err());
    }
}

#[test]
fn production_host_lifecycle_gap_fails_countability_closed() {
    let directory = TempDir::new().expect("temporary app data directory");
    let mut host = Wave1Host::new_with_build_identity(
        directory.path().to_path_buf(),
        BundleFixture::bundled(),
        Arc::new(TestProtector),
        verified_build(),
    );
    let report = host.preflight(request()).expect("preflight report");
    assert!(!report.countable);
    assert!(report
        .reason
        .unwrap_or_default()
        .contains("lifecycle boundary"));
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
    host.preview_aggregate(true).expect("aggregate export");
    assert!(host.aggregate_path_for_test().is_file());
    assert!(host.purge_session(false).is_err());
    let purge = host.purge_session(true).expect("confirmed purge");
    assert_ne!(started.session_namespace, purge.next_session_namespace);
    assert!(purge.purged);
    assert!(!host.aggregate_path_for_test().exists());
    assert_eq!(
        host.preview_aggregate(false).expect("preview").event_count,
        0
    );
}
