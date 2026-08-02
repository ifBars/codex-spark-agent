use super::{FirstVisibleReceipt, LifecycleHost, UiReadyReceipt};
use std::{fs, path::PathBuf, time::Instant};
use tempfile::TempDir;

fn host() -> LifecycleHost {
    LifecycleHost::new(Instant::now())
}

fn host_with_report(path: Option<PathBuf>) -> LifecycleHost {
    LifecycleHost::with_report_path_for_test(Instant::now(), path)
}

fn ready_receipt(host: &LifecycleHost) -> UiReadyReceipt {
    let challenge = host.launch_challenge();
    UiReadyReceipt {
        launch_id: challenge.launch_id,
        challenge: challenge.challenge,
        ack: "ui_ready".into(),
    }
}

#[test]
fn host_stamps_monotonic_launch_boundaries_in_required_order() {
    let mut host = host();
    host.record_page_load_finished()
        .expect("page load finished");
    let receipt = host
        .receive_ui_ready(ready_receipt(&host))
        .expect("ui ready");
    assert!(receipt.accepted);
    assert!(!receipt.idempotent);
    assert!(receipt.status.process_to_page_load_ms.is_some());
    assert!(receipt.status.process_to_ui_ready_ms.is_some());
    assert!(receipt.status.page_load_to_ui_ready_ms.is_some());
    assert!(!receipt.status.countable);
}

#[test]
fn identical_ui_ready_receipts_are_idempotent() {
    let mut host = host();
    host.record_page_load_finished()
        .expect("page load finished");
    let receipt = ready_receipt(&host);
    assert!(
        !host
            .receive_ui_ready(receipt.clone())
            .expect("first receipt")
            .idempotent
    );
    assert!(
        host.receive_ui_ready(receipt)
            .expect("identical receipt")
            .idempotent
    );
}

#[test]
fn stale_tokens_reject_without_tainting_the_active_launch() {
    let mut host = host();
    host.record_page_load_finished()
        .expect("page load finished");
    let mut stale = ready_receipt(&host);
    stale.challenge = "challenge-for-another-launch".into();
    assert!(host.receive_ui_ready(stale).is_err());
    assert!(host.status().ineligible_reason.is_none());
    assert!(host.receive_ui_ready(ready_receipt(&host)).is_ok());
}

#[test]
fn out_of_order_or_duplicate_page_boundaries_fail_closed() {
    let mut out_of_order = host();
    assert!(out_of_order
        .receive_ui_ready(ready_receipt(&out_of_order))
        .is_err());
    assert!(out_of_order.status().ineligible_reason.is_some());

    let mut duplicate = host();
    duplicate
        .record_page_load_finished()
        .expect("first page load");
    assert!(duplicate.record_page_load_finished().is_err());
    assert!(duplicate.status().ineligible_reason.is_some());
}

#[test]
fn missing_ui_ready_boundary_prevents_run_timing() {
    let mut host = host();
    host.record_page_load_finished()
        .expect("page load finished");
    assert!(host.begin_run().is_err());
    assert!(host.status().ineligible_reason.is_some());
}

#[test]
fn contradictory_duplicate_receipt_fails_closed() {
    let mut host = host();
    host.record_page_load_finished()
        .expect("page load finished");
    let receipt = ready_receipt(&host);
    host.receive_ui_ready(receipt.clone())
        .expect("first receipt");
    let contradictory = UiReadyReceipt {
        ack: "first_visible".into(),
        ..receipt
    };
    assert!(host.receive_ui_ready(contradictory).is_err());
    assert!(host.status().ineligible_reason.is_some());
}

#[test]
fn renderer_receipts_reject_forged_timing_and_identity_fields() {
    for field in ["timestamp_ms", "sequence", "duration_ms", "phase"] {
        let mut value = serde_json::json!({
            "launch_id": "launch-x",
            "challenge": "challenge-x",
            "ack": "ui_ready"
        });
        value
            .as_object_mut()
            .expect("receipt object")
            .insert(field.into(), serde_json::Value::String("forged".into()));
        assert!(serde_json::from_value::<UiReadyReceipt>(value).is_err());
    }
}

#[test]
fn run_visibility_is_host_stamped_and_opaque_receipts_are_idempotent() {
    let mut host = host();
    host.record_page_load_finished()
        .expect("page load finished");
    host.receive_ui_ready(ready_receipt(&host))
        .expect("ui ready");
    let run = host.begin_run().expect("run challenge");
    let receipt = FirstVisibleReceipt {
        run_id: run.run_id,
        challenge: run.challenge,
        ack: "first_visible".into(),
    };
    assert!(
        !host
            .receive_first_visible(receipt.clone())
            .expect("first visible")
            .idempotent
    );
    let status = host
        .receive_first_visible(receipt)
        .expect("idempotent visible")
        .status;
    assert!(status.first_visible_received);
    assert!(status.run_to_first_visible_ms.is_some());
}

#[test]
fn visible_run_receipt_closes_its_narrow_protocol_for_the_next_submission() {
    let mut host = host();
    host.record_page_load_finished()
        .expect("page load finished");
    host.receive_ui_ready(ready_receipt(&host))
        .expect("ui ready");
    let first = host.begin_run().expect("first run");
    host.receive_first_visible(FirstVisibleReceipt {
        run_id: first.run_id,
        challenge: first.challenge,
        ack: "first_visible".into(),
    })
    .expect("first visible");
    let second = host.begin_run().expect("second run");
    assert_ne!(second.run_id, "");
    assert_ne!(second.challenge, "");
}

#[test]
fn status_is_privacy_safe_and_never_exports_challenges_or_identifiers() {
    let host = host();
    let challenge = host.launch_challenge();
    let rendered = serde_json::to_string(&host.status()).expect("status json");
    assert!(rendered.contains("spark.proofline.lifecycle.status.v1"));
    assert!(!rendered.contains(&challenge.launch_id));
    assert!(!rendered.contains(&challenge.challenge));
    assert!(!rendered.contains("first_paint"));
    assert!(rendered.contains("process_to_page_load_ms"));
}

#[test]
fn absent_report_sink_writes_nothing() {
    let directory = TempDir::new().expect("temporary report directory");
    let report = directory.path().join("lifecycle.json");
    let mut host = host_with_report(None);
    host.record_page_load_finished()
        .expect("page load finished");
    assert!(!report.exists());
}

#[test]
fn report_sink_replaces_with_atomic_public_status_only_content() {
    let directory = TempDir::new().expect("temporary report directory");
    let report = directory.path().join("lifecycle.json");
    fs::write(&report, "old report").expect("seed old report");
    let mut host = host_with_report(Some(report.clone()));
    let challenge = host.launch_challenge();
    host.record_page_load_finished()
        .expect("page load finished");
    let rendered = fs::read_to_string(&report).expect("report status");
    let status: serde_json::Value = serde_json::from_str(&rendered).expect("valid json status");
    assert_eq!(status["page_load_finished"], true);
    assert!(!rendered.contains("old report"));
    assert!(!rendered.contains(&challenge.launch_id));
    assert!(!rendered.contains(&challenge.challenge));
    for forbidden in ["timestamp", "process_id", "prompt", "path", "event"] {
        assert!(!status
            .as_object()
            .expect("status object")
            .contains_key(forbidden));
    }
    assert!(directory
        .path()
        .read_dir()
        .expect("report directory")
        .all(|entry| entry.expect("entry").path() == report));
}

#[test]
fn report_sink_updates_after_each_lifecycle_boundary() {
    let directory = TempDir::new().expect("temporary report directory");
    let report = directory.path().join("lifecycle.json");
    let mut host = host_with_report(Some(report.clone()));
    host.record_page_load_finished()
        .expect("page load finished");
    let page: serde_json::Value =
        serde_json::from_slice(&fs::read(&report).expect("page report")).expect("page json");
    assert_eq!(page["page_load_finished"], true);
    assert_eq!(page["ui_ready_received"], false);
    host.receive_ui_ready(ready_receipt(&host))
        .expect("ui ready");
    let ready: serde_json::Value =
        serde_json::from_slice(&fs::read(&report).expect("ready report")).expect("ready json");
    assert_eq!(ready["ui_ready_received"], true);
    let run = host.begin_run().expect("run submitted");
    let submitted: serde_json::Value =
        serde_json::from_slice(&fs::read(&report).expect("run report")).expect("run json");
    assert_eq!(submitted["first_visible_received"], false);
    host.receive_first_visible(FirstVisibleReceipt {
        run_id: run.run_id,
        challenge: run.challenge,
        ack: "first_visible".into(),
    })
    .expect("first visible");
    let visible: serde_json::Value =
        serde_json::from_slice(&fs::read(&report).expect("visible report")).expect("visible json");
    assert_eq!(visible["first_visible_received"], true);
}

#[test]
fn invalid_or_unwritable_report_sink_fails_closed_without_a_crash() {
    let relative = host_with_report(Some(PathBuf::from("lifecycle.json")));
    assert!(relative
        .status()
        .ineligible_reason
        .expect("relative path rejection")
        .contains("absolute"));

    let directory = TempDir::new().expect("temporary report directory");
    let report = directory.path().join("lifecycle.json");
    let mut host = host_with_report(Some(report));
    fs::remove_dir_all(directory.path()).expect("remove temporary report directory");
    assert!(host.record_page_load_finished().is_err());
    assert!(host
        .status()
        .ineligible_reason
        .expect("write failure")
        .contains("report sink"));
}
