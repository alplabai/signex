//! #2794: Import DXF into a board sketch. The dialog changes nothing, Cancel
//! and Esc drop it, and Import is one sketch undo step that leaves the board
//! alone until Bake.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_engine::pcb::dxf::DxfSource;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::state::SketchDxfMsg;
use crate::app::{BoardSketchMsg, DxfImportMsg, Message, PcbMsg, Signex};
use crate::library::editor::footprint::SketchTool;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

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

/// One 10 x 10 mm square drawn on both BOARD and ASSEMBLY.
const TWO_LAYERS: &str = "0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\nBOARD\n10\n0\n20\n0\n11\n10\n21\n0\n\
0\nLINE\n8\nBOARD\n10\n10\n20\n0\n11\n10\n21\n10\n\
0\nLINE\n8\nBOARD\n10\n10\n20\n10\n11\n0\n21\n10\n\
0\nLINE\n8\nBOARD\n10\n0\n20\n10\n11\n0\n21\n0\n\
0\nLINE\n8\nASSEMBLY\n10\n0\n20\n0\n11\n10\n21\n0\n\
0\nLINE\n8\nASSEMBLY\n10\n10\n20\n0\n11\n10\n21\n10\n\
0\nLINE\n8\nASSEMBLY\n10\n10\n20\n10\n11\n0\n21\n10\n\
0\nLINE\n8\nASSEMBLY\n10\n0\n20\n10\n11\n0\n21\n0\n\
0\nENDSEC\n0\nEOF\n";

/// A square and four holes, all on layer 0.
const ONE_LAYER: &str = "0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\n0\n10\n0\n20\n0\n11\n10\n21\n0\n\
0\nLINE\n8\n0\n10\n10\n20\n0\n11\n10\n21\n10\n\
0\nLINE\n8\n0\n10\n10\n20\n10\n11\n0\n21\n10\n\
0\nLINE\n8\n0\n10\n0\n20\n10\n11\n0\n21\n0\n\
0\nCIRCLE\n8\n0\n10\n2\n20\n2\n40\n0.5\n\
0\nCIRCLE\n8\n0\n10\n8\n20\n2\n40\n0.5\n\
0\nCIRCLE\n8\n0\n10\n8\n20\n8\n40\n0.5\n\
0\nCIRCLE\n8\n0\n10\n2\n20\n8\n40\n0.5\n\
0\nENDSEC\n0\nEOF\n";

/// An app with an empty board in sketch mode and the dialog open on [`FILE`].
fn with_dialog() -> (Signex, PathBuf) {
    with_dialog_on(FILE)
}

/// Open the dialog on `file` over the sketch of the board at `path`.
fn read_into_dialog(app: &mut Signex, path: &Path, file: &str) {
    let source = DxfSource::read(file).expect("the file reads");
    let _ = app.update(dxf(DxfImportMsg::SketchRead {
        board: path.to_path_buf(),
        path: PathBuf::from("lid.dxf"),
        source: Ok(Arc::new(source)),
    }));
}

/// An app with an empty board in sketch mode and the dialog open on `file`.
fn with_dialog_on(file: &str) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).expect("a board");
    let path = PathBuf::from("dxf-sketch.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    let _ = app.update(Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::ModeToggled)));
    read_into_dialog(&mut app, &path, file);
    (app, path)
}

/// Bake the working copy of the sketch against the board's nets.
fn bake(app: &Signex, path: &Path) -> Result<(), signex_bake::BoardBakeError> {
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    let board = app.document_state.pcb_board(path).expect("the board");
    signex_bake::bake_board(&session.working_sketch(), &board.nets).map(|_| ())
}

fn set_tool(app: &mut Signex, path: &Path, tool: SketchTool) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ActiveBarSetSketchTool(tool)),
    }));
}

fn active_tool(app: &Signex) -> SketchTool {
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    session.editor.state.active_tool
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
fn an_outline_and_its_holes_on_one_layer_import_and_bake() {
    let (mut app, path) = with_dialog_on(ONE_LAYER);
    choose(&mut app, SketchDxfMsg::Confirmed);
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    let sketch = session.working_sketch();
    let outline = sketch.entities.iter().filter(|e| e.board_outline).count();
    let holes = sketch
        .entities
        .iter()
        .filter(|e| e.mounting_hole.is_some())
        .count();
    assert_eq!((outline, holes), (4, 4));
    bake(&app, &path).expect("one outline and four holes bake");
}

#[test]
fn a_corner_shared_across_two_targets_does_not_branch_the_outline() {
    let (mut app, path) = with_dialog_on(TWO_LAYERS);
    choose(&mut app, SketchDxfMsg::Confirmed);
    bake(&app, &path).expect("the outline keeps its own corners");
}

#[test]
fn the_open_card_holds_the_sketch_still() {
    let (mut app, path) = with_dialog();
    choose(&mut app, SketchDxfMsg::Confirmed);
    assert_eq!(app.step_board_sketch_history(&path, true), Some(true));
    assert_eq!(entity_count(&app), 0);

    read_into_dialog(&mut app, &path, FILE);
    assert_eq!(app.board_sketch_history(&path), Some((false, false)));
    assert_eq!(app.step_board_sketch_history(&path, false), Some(false));
    assert_eq!(entity_count(&app), 0, "Redo waits for the card");
    assert!(app.active_footprint_editor_mut().is_none());
    assert!(app.footprint_editor_at_mut(&path).is_none());
    set_tool(&mut app, &path, SketchTool::Line);
    assert!(active_tool(&app) == SketchTool::Select, "no edit lands");

    choose(&mut app, SketchDxfMsg::Cancelled);
    assert!(app.active_footprint_editor_mut().is_some());
    set_tool(&mut app, &path, SketchTool::Line);
    assert!(active_tool(&app) == SketchTool::Line, "edits are back");
    assert_eq!(app.step_board_sketch_history(&path, false), Some(true));
    assert!(entity_count(&app) > 0, "Redo is back");
}

#[test]
fn a_bad_scale_blocks_import() {
    let (mut app, _path) = with_dialog();
    choose(&mut app, SketchDxfMsg::ScaleEdited("0".into()));
    choose(&mut app, SketchDxfMsg::Confirmed);
    assert!(dialog_open(&app), "the dialog stays for a new value");
    assert_eq!(entity_count(&app), 0);
}
