//! Mockup row 194: Home View sets the board view to rotation 0°, Flip off,
//! and fits, in one message, with no undo step and no dirty mark; on a sheet
//! it fits. View ▸ Home View runs the same.

use std::path::PathBuf;

use crate::app::{FileMsg, Message, PcbMsg, PcbViewMsg, Signex, UiMsg, ViewRotationMsg};
use crate::canvas::overlay_camera_tests::graphite_app_with_selected_wire;
use crate::menu_bar::MenuMessage;

/// The main window showing a 50 x 30 mm board outline.
fn board_app() -> (Signex, PathBuf) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let mut board = crate::app::handlers::document_files::save_async_tests::empty_board();
    board.outline = [(0.0, 0.0), (50.0, 0.0), (50.0, 30.0), (0.0, 30.0)]
        .into_iter()
        .map(|(x, y)| signex_types::pcb::Point::new(x, y))
        .collect();
    let path = PathBuf::from("/w/home.snxpcb");
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        disk: None,
        path: path.clone(),
        title: "home".to_string(),
        result: Ok(Box::new(board)),
    }));
    assert!(app.has_active_pcb());
    app.sync_pcb_canvas_from_visible_board();
    (app, path)
}

/// Turn the board view 90° and flip it, then drop any queued fit.
fn turn_and_flip(app: &mut Signex, path: &PathBuf) {
    let _ = app.update(Message::Pcb(PcbMsg::ViewRotation(ViewRotationMsg::AngleSet(90.0))));
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::BoardFlipToggled)));
    let canvas = &app.interaction_state.pcb_canvas;
    assert_eq!(canvas.view_rotation_deg(), 90.0);
    assert!(app.pcb_layer_view(path).flipped());
    assert!(canvas.view_flipped());
    canvas.pending_fit.set(None);
}

fn assert_home(app: &Signex, path: &PathBuf) {
    let canvas = &app.interaction_state.pcb_canvas;
    assert_eq!(canvas.view_rotation_deg(), 0.0, "rotation");
    assert!(!app.pcb_layer_view(path).flipped(), "Flip still on");
    assert!(!canvas.view_flipped(), "the canvas is still mirrored");
    assert!(canvas.pending_fit.get().is_some(), "no fit queued");
}

#[test]
fn home_resets_the_turn_and_the_flip_and_fits_in_one_message() {
    let (mut app, path) = board_app();
    turn_and_flip(&mut app, &path);
    let before = app.command_context(None);
    let _ = app.update(Message::Ui(UiMsg::HomeViewRequested));
    assert_home(&app, &path);
    // View state only: no undo step, nothing dirty.
    let after = app.command_context(None);
    assert_eq!(after.can_undo, before.can_undo, "Home made an undo step");
    assert!(!after.can_undo);
    assert!(!after.has_dirty_documents, "Home marked a document dirty");
}

/// At home already, Home still fits and leaves rotation and flip as they are.
#[test]
fn home_at_home_only_fits() {
    let (mut app, path) = board_app();
    app.interaction_state.pcb_canvas.pending_fit.set(None);
    let _ = app.update(Message::Ui(UiMsg::HomeViewRequested));
    assert_home(&app, &path);
}

/// View ▸ Home View publishes `home_view`, which runs the same.
#[test]
fn the_view_menu_row_runs_home_view() {
    let (mut app, path) = board_app();
    turn_and_flip(&mut app, &path);
    let _ = app.update(Message::Menu(MenuMessage::command("home_view")));
    assert_home(&app, &path);
}

/// On a sheet Home fits the sheet: there is no turn or flip to reset.
#[test]
fn home_on_a_sheet_fits_it() {
    let (mut app, _main) = graphite_app_with_selected_wire();
    assert!(app.has_active_schematic());
    let canvas = &app.interaction_state.canvas;
    canvas.cancel_pending_fit();
    let _ = app.update(Message::Ui(UiMsg::HomeViewRequested));
    assert!(app.interaction_state.canvas.pending_fit.get().is_some());
}
