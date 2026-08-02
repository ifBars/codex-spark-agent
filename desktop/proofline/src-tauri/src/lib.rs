mod lifecycle;
mod profile;
mod wave1;

use std::{sync::Mutex, time::Instant};

use lifecycle::{
    FirstVisibleReceipt, LaunchChallenge, LifecycleHost, LifecycleStatus, ReceiptReport,
    RunChallenge, UiReadyReceipt,
};
use tauri::{webview::PageLoadEvent, Manager};
use wave1::{
    AggregatePreview, AppendEventReport, FixtureRequest, PreflightReport, PurgeReport,
    RendererInteraction, StartSessionReport, Wave1Host,
};

struct Wave1State(Mutex<Wave1Host>);
struct LifecycleState(Mutex<LifecycleHost>);

#[tauri::command(rename_all = "snake_case")]
fn proofline_lifecycle_begin(
    state: tauri::State<'_, LifecycleState>,
) -> Result<LaunchChallenge, String> {
    state
        .0
        .lock()
        .map_err(|_| "Proofline lifecycle state is unavailable".to_owned())
        .map(|host| host.launch_challenge())
}

#[tauri::command(rename_all = "snake_case")]
fn proofline_lifecycle_ui_ready(
    receipt: UiReadyReceipt,
    state: tauri::State<'_, LifecycleState>,
) -> Result<ReceiptReport, String> {
    state
        .0
        .lock()
        .map_err(|_| "Proofline lifecycle state is unavailable".to_owned())?
        .receive_ui_ready(receipt)
}

#[tauri::command(rename_all = "snake_case")]
fn proofline_lifecycle_run_submitted(
    state: tauri::State<'_, LifecycleState>,
) -> Result<RunChallenge, String> {
    state
        .0
        .lock()
        .map_err(|_| "Proofline lifecycle state is unavailable".to_owned())?
        .begin_run()
}

#[tauri::command(rename_all = "snake_case")]
fn proofline_lifecycle_first_visible(
    receipt: FirstVisibleReceipt,
    state: tauri::State<'_, LifecycleState>,
) -> Result<ReceiptReport, String> {
    state
        .0
        .lock()
        .map_err(|_| "Proofline lifecycle state is unavailable".to_owned())?
        .receive_first_visible(receipt)
}

#[tauri::command(rename_all = "snake_case")]
fn proofline_lifecycle_status(
    state: tauri::State<'_, LifecycleState>,
) -> Result<LifecycleStatus, String> {
    state
        .0
        .lock()
        .map_err(|_| "Proofline lifecycle state is unavailable".to_owned())
        .map(|host| host.status())
}

#[tauri::command(rename_all = "snake_case")]
fn wave1_preflight(
    fixture: FixtureRequest,
    state: tauri::State<'_, Wave1State>,
) -> Result<PreflightReport, String> {
    state
        .0
        .lock()
        .map_err(|_| "Wave 1 host state is unavailable".to_owned())?
        .preflight(fixture)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "snake_case")]
fn wave1_start_session(
    participant_id: String,
    fixture: FixtureRequest,
    state: tauri::State<'_, Wave1State>,
) -> Result<StartSessionReport, String> {
    state
        .0
        .lock()
        .map_err(|_| "Wave 1 host state is unavailable".to_owned())?
        .start_session(participant_id, fixture)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "snake_case")]
fn wave1_append_event(
    event: RendererInteraction,
    state: tauri::State<'_, Wave1State>,
) -> Result<AppendEventReport, String> {
    state
        .0
        .lock()
        .map_err(|_| "Wave 1 host state is unavailable".to_owned())?
        .append_event(event)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "snake_case")]
fn wave1_preview_aggregate(
    download: Option<bool>,
    state: tauri::State<'_, Wave1State>,
) -> Result<AggregatePreview, String> {
    state
        .0
        .lock()
        .map_err(|_| "Wave 1 host state is unavailable".to_owned())?
        .preview_aggregate(download.unwrap_or(false))
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "snake_case")]
fn wave1_purge_session(
    confirm: Option<bool>,
    state: tauri::State<'_, Wave1State>,
) -> Result<PurgeReport, String> {
    state
        .0
        .lock()
        .map_err(|_| "Wave 1 host state is unavailable")?
        .purge_session(confirm == Some(true))
        .map_err(|error| error.to_string())
}

pub fn run() {
    // Keep this as the first operation in `run`: every lifecycle duration is
    // measured from this single process-local monotonic origin.
    let process_started = Instant::now();
    tauri::Builder::default()
        .on_page_load(|webview, payload| {
            if webview.label() != "main" || payload.event() != PageLoadEvent::Finished {
                return;
            }
            let state = webview.app_handle().state::<LifecycleState>();
            if let Ok(mut host) = state.0.lock() {
                // This is page-load completion only, never a paint/visibility claim.
                let _ = host.record_page_load_finished();
            };
        })
        .setup(move |app| {
            // `create: false` in tauri.conf.json prevents Tauri from creating
            // the main webview before this calibration-only profile validation.
            // An invalid profile root therefore exits before WebView2 launches.
            let profile_directory =
                profile::calibration_profile_directory().map_err(std::io::Error::other)?;
            let main_window = app
                .config()
                .app
                .windows
                .iter()
                .find(|window| window.label == "main")
                .ok_or_else(|| std::io::Error::other("main window configuration is unavailable"))?;
            let data_dir = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            app.manage(Wave1State(Mutex::new(Wave1Host::for_app(data_dir))));
            app.manage(LifecycleState(Mutex::new(LifecycleHost::new(
                process_started,
            ))));
            let mut builder = tauri::WebviewWindowBuilder::from_config(app.handle(), main_window)?;
            if let Some(profile_directory) = profile_directory {
                builder = builder.data_directory(profile_directory);
            }
            builder.build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            proofline_lifecycle_begin,
            proofline_lifecycle_ui_ready,
            proofline_lifecycle_run_submitted,
            proofline_lifecycle_first_visible,
            proofline_lifecycle_status,
            wave1_preflight,
            wave1_start_session,
            wave1_append_event,
            wave1_preview_aggregate,
            wave1_purge_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running Proofline for Spark");
}
