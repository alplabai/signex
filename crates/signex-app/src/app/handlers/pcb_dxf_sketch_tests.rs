//! #2794: Import DXF into a board sketch. The dialog changes nothing, Cancel
//! and Esc drop it, and Import is one sketch undo step that leaves the board
//! alone until Bake.

use std::path::PathBuf;
use std::sync::Arc;

use signex_engine::pcb::dxf::DxfSource;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::state::SketchDxfMsg;
use crate::app::{BoardSketchMsg, DxfImportMsg, Message, PcbMsg, Signex};

/// A 10 x 10 mm square on BOARD and a hole on HOLES.
const FILE: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\nBOARD\n10\n0\n20\n0\n11\n10\n21\n0\n\
0\nLINE\n8\nBOARD\n10\n10\n20\n0\n11\n10\n21\n10\n\
0\nLINE\n8\nBOARD\n10\n10\n20\n10\n11\n0\n21\n10\n\
0\nLINE\n8\nBOARD\n10\n0\n20\n10\n11\n0\n21\n0\n\
0\nCIRCLE\n8\nHOLES\n10\n5\n20\n5\n40\n1\n\
0\nENDSEC\n0\nEOF\n";

fn dxf(event: DxfImportMsg) -> Message {
    Message::Pcb(PcbMsg::DxfImport(event))
}

/// An app with an empty board in sketch mode and the dialog open on [`FILE`].
fn with_dialog() -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).expect("a board");
    let path = PathBuf::from("dxf-sketch.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    let _ = app.update(Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::ModeToggled)));
    let source = DxfSource::read(FILE).expect("the file reads");
    let _ = app.update(dxf(DxfImportMsg::SketchRead {
        board: path.clone(),
        path: PathBuf::from("lid.dxf"),
        source: Ok(Arc::new(source)),
    }));
    (app, path)
}

fn dialog_open(app: &Signex) -> bool {
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    session.dxf_dialog.is_some()
}

fn entity_count(app: &Signex) -> usize {
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    session.working_sketch().entities.len()
}

fn choose(app: &mut Signex, event: SketchDxfMsg) {
    let _ = app.update(dxf(DxfImportMsg::SketchDialog(event)));
}

#[test]
fn the_open_dialog_changes_nothing() {
    let (app, path) = with_dialog();
    assert!(dialog_open(&app));
    assert_eq!(entity_count(&app), 0);
    let board = app.document_state.pcb_board(&path).expect("the board");
    assert!(board.sketch.is_none());
}

#[test]
fn cancel_drops_the_dialog_and_leaves_no_undo_step() {
    let (mut app, path) = with_dialog();
    choose(&mut app, SketchDxfMsg::ScaleEdited("2".into()));
    choose(&mut app, SketchDxfMsg::Cancelled);
    assert!(!dialog_open(&app));
    assert_eq!(entity_count(&app), 0);
    assert_eq!(app.board_sketch_history(&path), Some((false, false)));
}

#[test]
fn esc_answers_the_dialog_first() {
    let (app, _path) = with_dialog();
    assert!(matches!(
        app.board_sketch_escape(),
        Some(Message::Pcb(PcbMsg::DxfImport(DxfImportMsg::SketchDialog(
            SketchDxfMsg::Cancelled
        ))))
    ));
}

#[test]
fn import_is_one_undo_step_and_leaves_the_board_alone() {
    let (mut app, path) = with_dialog();
    choose(&mut app, SketchDxfMsg::Confirmed);
    assert!(!dialog_open(&app));
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    let sketch = session.working_sketch();
    assert_eq!(
        sketch.entities.iter().filter(|e| e.board_outline).count(),
        4
    );
    assert_eq!(
        sketch
            .entities
            .iter()
            .filter(|e| e.mounting_hole.is_some())
            .count(),
        1
    );
    let board = app.document_state.pcb_board(&path).expect("the board");
    assert!(board.sketch.is_none(), "the board waits for Bake");

    assert_eq!(app.step_board_sketch_history(&path, true), Some(true));
    assert_eq!(entity_count(&app), 0);
    assert_eq!(app.step_board_sketch_history(&path, true), Some(false));
}

#[test]
fn a_bad_scale_blocks_import() {
    let (mut app, _path) = with_dialog();
    choose(&mut app, SketchDxfMsg::ScaleEdited("0".into()));
    choose(&mut app, SketchDxfMsg::Confirmed);
    assert!(dialog_open(&app), "the dialog stays for a new value");
    assert_eq!(entity_count(&app), 0);
}
