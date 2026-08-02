use super::types::{
    FirstVisibleReceipt, LaunchChallenge, LifecycleStatus, ReceiptReport, RunChallenge,
    UiReadyReceipt,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Instant,
};
use uuid::Uuid;

const CAPTURE_MODE: &str = "host_authoritative";
const CALIBRATION_REQUIRED: &str =
    "external visual calibration and runtime no-network review are required before countability";

#[derive(Debug)]
struct LaunchState {
    id: String,
    challenge: String,
    page_load_finished_ms: Option<u128>,
    ui_ready_ms: Option<u128>,
    ineligible_reason: Option<String>,
}

#[derive(Debug)]
struct RunState {
    id: String,
    challenge: String,
    submitted_ms: u128,
    first_visible_ms: Option<u128>,
    ineligible_reason: Option<String>,
}

/// Owns the one native monotonic clock domain used by lifecycle evidence.
///
/// This state deliberately does not persist receipts or opaque identifiers. The
/// Wave 1 aggregate export remains aggregate-only and cannot include them.
pub(crate) struct LifecycleHost {
    origin: Instant,
    launch: LaunchState,
    run: Option<RunState>,
    calibration_verified: bool,
    no_network_verified: bool,
    exact_build_verified: bool,
    report_path: Option<PathBuf>,
}

impl LifecycleHost {
    pub(crate) fn new(origin: Instant) -> Self {
        Self::with_report_path(origin, report_path_from_environment())
    }

    fn with_report_path(origin: Instant, report_path: Result<Option<PathBuf>, String>) -> Self {
        let mut host = Self {
            origin,
            launch: LaunchState {
                id: opaque_id("launch"),
                challenge: opaque_id("challenge"),
                page_load_finished_ms: None,
                ui_ready_ms: None,
                ineligible_reason: None,
            },
            run: None,
            // These gates must be backed by the external #11 calibration
            // artifact; production has no mechanism to assert them yet.
            calibration_verified: false,
            no_network_verified: false,
            exact_build_verified: false,
            report_path: None,
        };
        match report_path {
            Ok(path) => host.report_path = path,
            Err(reason) => host.launch.ineligible_reason = Some(reason),
        }
        host
    }

    pub(crate) fn launch_challenge(&self) -> LaunchChallenge {
        LaunchChallenge {
            launch_id: self.launch.id.clone(),
            challenge: self.launch.challenge.clone(),
        }
    }

    /// Tauri page-load completion is a diagnostic only. It must never be
    /// displayed or exported as a first-paint or first-visible measurement.
    pub(crate) fn record_page_load_finished(&mut self) -> Result<(), String> {
        if let Some(reason) = &self.launch.ineligible_reason {
            return Err(format!("launch lifecycle is ineligible: {reason}"));
        }
        if self.launch.ui_ready_ms.is_some() {
            return self.invalidate_launch("page-load completion arrived after ui_ready receipt");
        }
        if self.launch.page_load_finished_ms.is_some() {
            return self.invalidate_launch("duplicate page-load completion boundary");
        }
        self.launch.page_load_finished_ms = Some(self.elapsed_ms());
        self.write_report_if_configured()?;
        Ok(())
    }

    pub(crate) fn receive_ui_ready(
        &mut self,
        receipt: UiReadyReceipt,
    ) -> Result<ReceiptReport, String> {
        if receipt.launch_id != self.launch.id || receipt.challenge != self.launch.challenge {
            return Err("ui_ready receipt has a stale launch identifier or challenge".into());
        }
        if receipt.ack != "ui_ready" {
            return self.invalidate_launch("ui_ready receipt has an invalid acknowledgement");
        }
        if let Some(reason) = &self.launch.ineligible_reason {
            return Err(format!("launch lifecycle is ineligible: {reason}"));
        }
        if self.launch.page_load_finished_ms.is_none() {
            return self.invalidate_launch("ui_ready receipt arrived before page-load completion");
        }
        if self.launch.ui_ready_ms.is_some() {
            return Ok(ReceiptReport {
                accepted: true,
                idempotent: true,
                status: self.status(),
            });
        }
        self.launch.ui_ready_ms = Some(self.elapsed_ms());
        self.write_report_if_configured()?;
        Ok(ReceiptReport {
            accepted: true,
            idempotent: false,
            status: self.status(),
        })
    }

    pub(crate) fn begin_run(&mut self) -> Result<RunChallenge, String> {
        if let Some(reason) = &self.launch.ineligible_reason {
            return Err(format!("launch lifecycle is ineligible: {reason}"));
        }
        if self.launch.ui_ready_ms.is_none() {
            return self.invalidate_launch("run submission arrived before ui_ready receipt");
        }
        if self
            .run
            .as_ref()
            .is_some_and(|run| run.first_visible_ms.is_some())
        {
            // A visibility receipt closes the narrow measurement protocol for
            // that run. Later UI work is outside this receipt's scope, and a
            // new submission may receive a fresh opaque challenge.
            self.run = None;
        }
        if self.run.is_some() {
            return self.invalidate_launch("run submission arrived while another run was active");
        }
        let run = RunState {
            id: opaque_id("run"),
            challenge: opaque_id("challenge"),
            submitted_ms: self.elapsed_ms(),
            first_visible_ms: None,
            ineligible_reason: None,
        };
        let challenge = RunChallenge {
            run_id: run.id.clone(),
            challenge: run.challenge.clone(),
        };
        self.run = Some(run);
        self.write_report_if_configured()?;
        Ok(challenge)
    }

    pub(crate) fn receive_first_visible(
        &mut self,
        receipt: FirstVisibleReceipt,
    ) -> Result<ReceiptReport, String> {
        let received_at_ms = self.elapsed_ms();
        let idempotent = {
            let Some(run) = self.run.as_mut() else {
                return Err("first_visible receipt has no active host-issued run".into());
            };
            if receipt.run_id != run.id || receipt.challenge != run.challenge {
                return Err("first_visible receipt has a stale run identifier or challenge".into());
            }
            if receipt.ack != "first_visible" {
                return self.invalidate_run("first_visible receipt has an invalid acknowledgement");
            }
            if let Some(reason) = &run.ineligible_reason {
                return Err(format!("run lifecycle is ineligible: {reason}"));
            }
            if run.first_visible_ms.is_some() {
                true
            } else {
                run.first_visible_ms = Some(received_at_ms);
                false
            }
        };
        if idempotent {
            return Ok(ReceiptReport {
                accepted: true,
                idempotent: true,
                status: self.status(),
            });
        }
        self.write_report_if_configured()?;
        Ok(ReceiptReport {
            accepted: true,
            idempotent: false,
            status: self.status(),
        })
    }

    pub(crate) fn status(&self) -> LifecycleStatus {
        let page_load_to_ui_ready_ms = self
            .launch
            .page_load_finished_ms
            .zip(self.launch.ui_ready_ms)
            .map(|(page_load, ui_ready)| ui_ready.saturating_sub(page_load));
        let run_to_first_visible_ms = self.run.as_ref().and_then(|run| {
            run.first_visible_ms
                .map(|visible| visible.saturating_sub(run.submitted_ms))
        });
        let ineligible_reason = self.launch.ineligible_reason.clone().or_else(|| {
            self.run
                .as_ref()
                .and_then(|run| run.ineligible_reason.clone())
        });
        LifecycleStatus {
            schema: "spark.proofline.lifecycle.status.v1".into(),
            capture_mode: CAPTURE_MODE.into(),
            // This remains deliberately false. The host's receipt ordering is
            // necessary evidence, not the external calibration/no-network gate.
            countable: false,
            process_to_page_load_ms: self.launch.page_load_finished_ms,
            process_to_ui_ready_ms: self.launch.ui_ready_ms,
            page_load_to_ui_ready_ms,
            run_to_first_visible_ms,
            page_load_finished: self.launch.page_load_finished_ms.is_some(),
            ui_ready_received: self.launch.ui_ready_ms.is_some(),
            first_visible_received: self
                .run
                .as_ref()
                .is_some_and(|run| run.first_visible_ms.is_some()),
            calibration_verified: self.calibration_verified,
            no_network_verified: self.no_network_verified,
            exact_build_verified: self.exact_build_verified,
            ineligible_reason,
            reason: Some(CALIBRATION_REQUIRED.into()),
        }
    }

    fn invalidate_launch<T>(&mut self, reason: impl Into<String>) -> Result<T, String> {
        let reason = reason.into();
        self.launch.ineligible_reason = Some(reason.clone());
        Err(reason)
    }

    fn invalidate_run<T>(&mut self, reason: impl Into<String>) -> Result<T, String> {
        let reason = reason.into();
        if let Some(run) = self.run.as_mut() {
            run.ineligible_reason = Some(reason.clone());
        }
        Err(reason)
    }

    fn write_report_if_configured(&mut self) -> Result<(), String> {
        let Some(path) = self.report_path.clone() else {
            return Ok(());
        };
        let bytes = match serde_json::to_vec(&self.status()) {
            Ok(bytes) => bytes,
            Err(_) => return self.invalidate_launch("lifecycle report could not be serialized"),
        };
        let Some(parent) = path.parent() else {
            return self.invalidate_launch("lifecycle report path must have a parent directory");
        };
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            return self.invalidate_launch("lifecycle report path must name a UTF-8 file");
        };
        let temporary = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
        let result = fs::write(&temporary, bytes).and_then(|()| fs::rename(&temporary, &path));
        if result.is_ok() {
            return Ok(());
        }
        let _ = fs::remove_file(temporary);
        self.invalidate_launch("lifecycle report sink could not be written")
    }

    fn elapsed_ms(&self) -> u128 {
        self.origin.elapsed().as_millis()
    }
}

fn report_path_from_environment() -> Result<Option<PathBuf>, String> {
    let Some(value) = env::var_os("SPARK_PROOFLINE_LIFECYCLE_REPORT_PATH") else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    validate_report_path(&path)?;
    Ok(Some(path))
}

fn validate_report_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("lifecycle report path must be an absolute file path".into());
    }
    if path.file_name().is_none() || path.is_dir() {
        return Err("lifecycle report path must name a file".into());
    }
    if !path.parent().is_some_and(Path::is_dir) {
        return Err("lifecycle report path parent directory is unavailable".into());
    }
    Ok(())
}

fn opaque_id(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4())
}

#[cfg(test)]
impl LifecycleHost {
    pub(crate) fn with_report_path_for_test(origin: Instant, path: Option<PathBuf>) -> Self {
        let validated = path.map_or(Ok(None), |path| {
            validate_report_path(&path).map(|()| Some(path))
        });
        Self::with_report_path(origin, validated)
    }
}
