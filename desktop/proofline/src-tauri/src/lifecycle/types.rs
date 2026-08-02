use serde::{Deserialize, Serialize};

/// A host-issued, opaque launch acknowledgement challenge. This is delivered to
/// the renderer but is never part of aggregate measurement export.
#[derive(Debug, Clone, Serialize)]
pub struct LaunchChallenge {
    pub launch_id: String,
    pub challenge: String,
}

/// The renderer has no authority over timing, sequence, or lifecycle phase.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiReadyReceipt {
    pub launch_id: String,
    pub challenge: String,
    pub ack: String,
}

/// A host-issued, opaque run visibility acknowledgement challenge.
#[derive(Debug, Clone, Serialize)]
pub struct RunChallenge {
    pub run_id: String,
    pub challenge: String,
}

/// The renderer cannot provide a timestamp or claim compositor first paint.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirstVisibleReceipt {
    pub run_id: String,
    pub challenge: String,
    pub ack: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LifecycleStatus {
    pub schema: String,
    pub capture_mode: String,
    pub countable: bool,
    pub process_to_page_load_ms: Option<u128>,
    pub process_to_ui_ready_ms: Option<u128>,
    pub page_load_to_ui_ready_ms: Option<u128>,
    pub run_to_first_visible_ms: Option<u128>,
    pub page_load_finished: bool,
    pub ui_ready_received: bool,
    pub first_visible_received: bool,
    pub calibration_verified: bool,
    pub no_network_verified: bool,
    pub exact_build_verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ineligible_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReceiptReport {
    pub accepted: bool,
    pub idempotent: bool,
    pub status: LifecycleStatus,
}
