mod wave1;

use std::sync::Mutex;

use tauri::Manager;
use wave1::{
    AggregatePreview, AppendEventReport, FixtureRequest, PreflightReport, PurgeReport,
    RendererInteraction, StartSessionReport, Wave1Host,
};

struct Wave1State(Mutex<Wave1Host>);

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
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            app.manage(Wave1State(Mutex::new(Wave1Host::for_app(data_dir))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            wave1_preflight,
            wave1_start_session,
            wave1_append_event,
            wave1_preview_aggregate,
            wave1_purge_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running Proofline for Spark");
}
