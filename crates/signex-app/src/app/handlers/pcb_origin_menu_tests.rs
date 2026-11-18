//! 2026-11-18: Edit ▸ Origin sets, resets and snaps the board origin, each
//! as one undo step that marks the board changed; a read-only board greys
//! every row.

use std::path::{Path, PathBuf};

use pcb_geom::ClickMode;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::project::SetupProfile;

use crate::app::command::is_enabled;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::pcb_canvas::PcbCanvasMsg;

/// A board saved before #2384: no origin (the model's zero), and an
/// outline from (10, 20) to (60, 50).
fn old_board() -> PcbBoard {
    let mut board = SetupProfile::Standard.pcb(2).new_board();
    board.options.origin = Default::default();
    board.outline = vec![
        Point::new(10.0, 20.0),
        Point::new(60.0, 20.0),
        Point::new(60.0, 50.0),
        Point::new(10.0, 50.0),
    ];
    board
}

fn open(dir: &Path, board: PcbBoard) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path)
}

fn run(app: &mut Signex, id: &str) {
    let command = AppCommandId::new(id).expect("a catalog id");
    let _ = app.update(Message::Menu(MenuMessage::Command(command)));
}

fn origin(app: &Signex, path: &Path) -> Point {
    let board = app.document_state.pcb_board(path).expect("board");
    board.options.origin.at
}

fn near(p: Point, x: f64, y: f64) -> bool {
    (p.x - x).abs() < 1e-6 && (p.y - y).abs() < 1e-6
}

#[test]
fn set_takes_the_next_click_as_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = open(dir.path(), old_board());
    assert_eq!(app.document_state.dirty_document_count(), 0);
    run(&mut app, "set_board_origin");
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::Clicked {
        at: Point::new(12.0, 48.0),
        tolerance_mm: 0.05,
        mode: ClickMode::Replace,
    }));
    assert!(near(origin(&app, &path), 12.0, 48.0), "{:?}", origin(&app, &path));
    assert_eq!(app.document_state.dirty_document_count(), 1);
    assert!(app.interaction_state.pcb_origin_place.is_none());
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(origin(&app, &path), Point::ZERO);
}

#[test]
fn reset_goes_to_the_outline_bottom_left_and_snap_to_the_nearest_corner() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = open(dir.path(), old_board());
    run(&mut app, "reset_board_origin");
    assert_eq!(origin(&app, &path), Point::new(10.0, 50.0));

    // The pointer near the top-right corner of the outline.
    app.ui_state.cursor_x = 58.0;
    app.ui_state.cursor_y = 22.0;
    app.ui_state.cursor_document = Some(path.clone());
    run(&mut app, "snap_board_origin_to_corner");
    assert_eq!(origin(&app, &path), Point::new(60.0, 20.0));

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(origin(&app, &path), Point::new(10.0, 50.0));
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(origin(&app, &path), Point::ZERO);
}

#[test]
fn a_read_only_board_greys_every_origin_row_and_keeps_its_origin() {
    let dir = tempfile::tempdir().unwrap();
    let mut board = old_board();
    board.unknown.newer_minor_source = Some("snxpcb/9.9".into());
    let (mut app, path) = open(dir.path(), board);
    let ctx = app.command_context(None);
    for id in [
        "set_board_origin",
        "reset_board_origin",
        "snap_board_origin_to_corner",
    ] {
        let command = AppCommandId::new(id).expect("a catalog id");
        assert!(!is_enabled(&command, &ctx), "{id} on a read-only board");
    }
    let _ = app.update(Message::Pcb(crate::app::PcbMsg::Origin(
        crate::app::contracts::BoardOriginMsg::ResetPicked,
    )));
    assert_eq!(origin(&app, &path), Point::ZERO);
}
