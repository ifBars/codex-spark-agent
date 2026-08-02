#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::rc::Rc;

use proofline_native::{ProoflinePresentation, fixture_snapshot};
use slint::{Model, ModelRc, SharedString, VecModel};

slint::include_modules!();

#[cfg(target_os = "windows")]
fn initial_scale_factor() -> f32 {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetDpiForSystem() -> u32;
    }

    // SAFETY: GetDpiForSystem has no parameters or caller-owned pointers and is
    // available on the Windows versions supported by this spike.
    let dpi = unsafe { GetDpiForSystem() };
    (dpi.max(96) as f32) / 96.0
}

#[cfg(not(target_os = "windows"))]
fn initial_scale_factor() -> f32 {
    1.0
}

fn main() -> Result<(), slint::PlatformError> {
    // FemtoVG produced a blank surface on the Wave 1 Windows machine. Select
    // the deterministic software renderer so the spike cannot silently depend
    // on a machine-specific GPU path.
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;

    let presentation = ProoflinePresentation::from(&fixture_snapshot());
    let app = ProoflineApp::new()?;

    let history = Rc::new(VecModel::from(
        presentation
            .history
            .into_iter()
            .map(|row| HistoryEntry {
                title: row.title.into(),
                meta: row.meta.into(),
                selected: row.selected,
                completed: row.completed,
            })
            .collect::<Vec<_>>(),
    ));
    app.set_history(ModelRc::from(history.clone()));
    app.set_task_title(presentation.title.into());
    app.set_completion_meta(presentation.completion_meta.into());
    app.set_summary(presentation.summary.into());
    app.set_changed_files(ModelRc::new(VecModel::from(
        presentation
            .changed_files
            .into_iter()
            .map(|row| ChangedFileEntry {
                path: row.path.into(),
                additions: row.additions.into(),
                deletions: row.deletions.into(),
                evidence: row.evidence.into(),
            })
            .collect::<Vec<_>>(),
    )));
    app.set_validation(ModelRc::new(VecModel::from(
        presentation
            .validation
            .into_iter()
            .map(|row| ValidationEntry {
                command: row.command.into(),
                elapsed: row.elapsed.into(),
                passed: row.passed,
            })
            .collect::<Vec<_>>(),
    )));
    app.set_model_steps(presentation.model_steps_label.into());
    app.set_branch(presentation.status.branch.into());
    app.set_checkpoint(presentation.status.checkpoint.into());
    app.set_elapsed(presentation.status.elapsed.into());
    app.set_tokens(presentation.status.tokens.into());
    app.set_pricing(presentation.status.pricing.into());
    app.set_network_gate(SharedString::from(presentation.status.network_gate));

    let history_for_selection = history.clone();
    app.on_select_task(move |index| {
        let selected_row = usize::try_from(index).ok();
        for row_index in 0..history_for_selection.row_count() {
            if let Some(mut row) = history_for_selection.row_data(row_index) {
                row.selected = selected_row == Some(row_index);
                history_for_selection.set_row_data(row_index, row);
            }
        }
    });

    let activity_app = app.as_weak();
    app.on_toggle_activity(move || {
        if let Some(app) = activity_app.upgrade() {
            app.set_activity_expanded(!app.get_activity_expanded());
        }
    });

    // Slint 1.16 applies the declarative Window size as physical pixels on this
    // Windows host while rendering child geometry at the system DPI. Re-assert
    // the intended logical size in physical pixels after the native handle is
    // created so the full hierarchy is visible at 125% scaling.
    app.show()?;
    let scale = initial_scale_factor();
    app.window().set_size(slint::PhysicalSize::new(
        (1100.0 * scale).round() as u32,
        (650.0 * scale).round() as u32,
    ));
    slint::run_event_loop()
}
