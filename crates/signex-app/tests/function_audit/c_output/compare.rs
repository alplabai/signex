//! Row m162: Compare Board with Schematic, on a project read from disk.

use std::path::PathBuf;

use signex_app::app::{CompareBoardMsg, JumpSide, Message, Signex, TabDocument, TabInfo, TabKind};
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::BoardEngine;
use signex_engine::pcb::eco::{CompareKind, CompareReport, CompareRow, Direction, EcoKey};
use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::pcb::{Footprint, PcbBoard, Point as BoardPoint, SymbolLink};
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use tempfile::TempDir;
use uuid::Uuid;

use super::support::{add_project, command, fresh_app, project_data, show, undo};
use crate::settle_support::settle;

const R1: Uuid = Uuid::from_u128(0xC101);
const R2: Uuid = Uuid::from_u128(0xC102);
/// A unit of no part of the design: TP3 is linked to it.
const GONE: Uuid = Uuid::from_u128(0xC999);
const FP_R1: Uuid = Uuid::from_u128(0xC0F1);
const FP_R2: Uuid = Uuid::from_u128(0xC0F2);
const FP_TP3: Uuid = Uuid::from_u128(0xC0F3);

fn symbol(uuid: Uuid, reference: &str, value: &str) -> Symbol {
    Symbol {
        uuid,
        lib_id: "R".to_string(),
        reference: reference.to_string(),
        value: value.to_string(),
        in_bom: true,
        on_board: true,
        ..Default::default()
    }
}

fn footprint(uuid: Uuid, reference: &str, value: &str, x: f64, unit: Uuid) -> Footprint {
    let mut fp: Footprint = serde_json::from_value(serde_json::json!({
        "uuid": uuid, "reference": reference, "value": value,
        "position": {"x": 0.0, "y": 0.0}, "layer": "TopCopper"
    }))
    .expect("a footprint");
    fp.position = BoardPoint::new(x, 10.0);
    fp.schematic_link = Some(SymbolLink::new(InstancePath::root(), [unit]));
    fp
}

/// R1 reads 22k on the board, R2 is named R7 there, and TP3 is extra.
fn differing_board() -> PcbBoard {
    let mut board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).expect("a board");
    board.footprints = vec![
        footprint(FP_R1, "R1", "22k", 10.0, R1),
        footprint(FP_R2, "R7", "4k7", 20.0, R2),
        footprint(FP_TP3, "TP3", "TestPoint", 30.0, GONE),
    ];
    board
}

fn matching_board() -> PcbBoard {
    let mut board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).expect("a board");
    board.footprints = vec![
        footprint(FP_R1, "R1", "10k", 10.0, R1),
        footprint(FP_R2, "R2", "4k7", 20.0, R2),
    ];
    board
}

struct Rig {
    app: Signex,
    root: PathBuf,
    board: PathBuf,
    _tmp: TempDir,
}

/// The project on disk with its root sheet in front at its save point; the
/// board stays closed until the compare opens it.
fn rig(board: PcbBoard) -> Rig {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![symbol(R1, "R1", "10k"), symbol(R2, "R2", "4k7")],
        ..Default::default()
    };
    let root = dir.join("root.snxsch");
    let text = SnxSchematic::new(sheet.clone())
        .write_string()
        .expect("encode sheet");
    std::fs::write(&root, text).expect("write sheet");
    let board_path = dir.join("Proj.snxpcb");
    let text = SnxPcb::new(board).write_string().expect("encode board");
    std::fs::write(&board_path, text).expect("write board");
    let data = project_data(&dir, &["root.snxsch"], Some("Proj.snxpcb"));
    let prj = dir.join("Proj.snxprj");
    signex_types::project::write_project(&prj, &data).expect("write project");

    let mut app = fresh_app();
    let id = add_project(&mut app, prj, data);
    let mut engine =
        signex_engine::Engine::new_with_path(sheet, Some(root.clone())).expect("engine");
    engine.mark_saved();
    app.document_state.engines.insert(root.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "root".to_string(),
        path: root.clone(),
        cached_document: None,
        project_id: Some(id),
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(root.clone());
    Rig {
        app,
        root,
        board: board_path,
        _tmp: tmp,
    }
}

fn opened() -> Rig {
    let mut rig = rig(differing_board());
    command(&mut rig.app, "compare_board_with_schematic");
    rig
}

fn send(rig: &mut Rig, msg: CompareBoardMsg) {
    settle(&mut rig.app, Message::from(msg));
}

fn report(rig: &Rig) -> &CompareReport {
    let state = rig
        .app
        .ui_state
        .compare_board
        .as_ref()
        .expect("the dialog is open");
    assert!(state.is_ready(), "report ready: {:?}", state.failure());
    state.report().expect("a report")
}

fn row<'a>(rig: &'a Rig, key: &EcoKey) -> &'a CompareRow {
    report(rig)
        .rows
        .iter()
        .find(|row| row.pcb.as_ref() == Some(key))
        .unwrap_or_else(|| panic!("no row for {key:?}"))
}

fn pick(rig: &mut Rig, key: &EcoKey, direction: Direction) {
    let id = row(rig, key).id();
    send(
        rig,
        CompareBoardMsg::RowDirectionPicked { row: id, direction },
    );
}

fn engine(rig: &Rig) -> &BoardEngine {
    rig.app
        .document_state
        .tabs
        .iter()
        .find(|tab| tab.path == rig.board)
        .and_then(|tab| tab.cached_document.as_ref())
        .and_then(TabDocument::as_pcb_engine)
        .expect("the board is open")
}

fn board_part(rig: &Rig, id: Uuid) -> Option<&Footprint> {
    engine(rig).board().footprints.iter().find(|f| f.uuid == id)
}

fn sheet_part(rig: &Rig, id: Uuid) -> &Symbol {
    rig.app.document_state.engines[&rig.root]
        .document()
        .symbols
        .iter()
        .find(|symbol| symbol.uuid == id)
        .expect("the part is on the sheet")
}

fn board_json(rig: &Rig) -> String {
    serde_json::to_string(engine(rig).board()).expect("the board as JSON")
}

fn sheet_json(rig: &Rig) -> String {
    serde_json::to_string(rig.app.document_state.engines[&rig.root].document())
        .expect("the sheet as JSON")
}

fn dirty(rig: &Rig, board: bool) -> bool {
    let path = if board { &rig.board } else { &rig.root };
    rig.app.document_state.is_document_dirty(path)
}

fn counts(rig: &Rig) -> (usize, usize, usize) {
    let counts = report(rig).counts();
    (counts.to_pcb, counts.to_schematic, counts.skipped)
}

#[test]
fn a_new_report_takes_the_schematics_side_and_skips_the_board_deletion() {
    let rig = opened();
    assert_eq!(report(&rig).rows.len(), 3);
    let extra = row(&rig, &EcoKey::RemoveFootprint(FP_TP3));
    assert_eq!(extra.kind, CompareKind::ExtraOnBoard);
    assert_eq!(extra.direction, Direction::Skip);
    assert_eq!(
        row(&rig, &EcoKey::SetValue(FP_R1)).direction,
        Direction::UpdatePcb
    );
    assert_eq!(counts(&rig), (2, 0, 1));
}

#[test]
fn opening_the_report_changes_neither_document() {
    let rig = opened();
    assert!(!dirty(&rig, true), "the board stays at its save point");
    assert!(!dirty(&rig, false), "the sheet stays at its save point");
}

#[test]
fn a_rows_direction_moves_the_footer_count() {
    let mut rig = opened();
    pick(
        &mut rig,
        &EcoKey::SetDesignator(FP_R2),
        Direction::UpdateSchematic,
    );
    assert_eq!(counts(&rig), (1, 1, 1));
    pick(&mut rig, &EcoKey::SetDesignator(FP_R2), Direction::Skip);
    assert_eq!(counts(&rig), (1, 0, 2));
}

/// Decision 2026-11-19: Update schematic is not available for Extra on board.
#[test]
fn an_extra_board_part_refuses_update_schematic() {
    let mut rig = opened();
    pick(
        &mut rig,
        &EcoKey::RemoveFootprint(FP_TP3),
        Direction::UpdateSchematic,
    );
    assert_eq!(
        row(&rig, &EcoKey::RemoveFootprint(FP_TP3)).direction,
        Direction::Skip
    );
}

#[test]
fn a_kind_chip_filters_the_rows_and_set_shown_rows_touches_only_those() {
    let mut rig = opened();
    send(
        &mut rig,
        CompareBoardMsg::KindFilterPicked(Some(CompareKind::ValueChange)),
    );
    let state = rig.app.ui_state.compare_board.as_ref().expect("open");
    assert_eq!(state.shown_rows().len(), 2);
    assert_eq!(state.filter(), Some(CompareKind::ValueChange));

    send(
        &mut rig,
        CompareBoardMsg::ShownRowsDirectionPicked(Direction::UpdateSchematic),
    );
    assert_eq!(counts(&rig), (0, 2, 1));
    assert_eq!(
        row(&rig, &EcoKey::RemoveFootprint(FP_TP3)).direction,
        Direction::Skip,
        "a row the filter hides keeps its choice"
    );

    send(&mut rig, CompareBoardMsg::KindFilterPicked(None));
    let state = rig.app.ui_state.compare_board.as_ref().expect("open");
    assert_eq!(state.shown_rows().len(), 3);
}

#[test]
fn apply_changes_both_documents_each_way_and_marks_both_dirty() {
    let mut rig = opened();
    pick(
        &mut rig,
        &EcoKey::SetDesignator(FP_R2),
        Direction::UpdateSchematic,
    );
    send(&mut rig, CompareBoardMsg::ApplyRequested);
    assert!(rig.app.ui_state.compare_board.is_none(), "Apply closes it");
    assert_eq!(board_part(&rig, FP_R1).expect("R1").value, "10k");
    assert_eq!(board_part(&rig, FP_R2).expect("R7").reference, "R7");
    assert!(board_part(&rig, FP_TP3).is_some(), "a skipped row is kept");
    assert_eq!(sheet_part(&rig, R2).reference, "R7");
    assert_eq!(sheet_part(&rig, R1).value, "10k");
    assert_eq!(engine(&rig).undo_len(), 1, "one board undo step");
    assert!(dirty(&rig, true));
    assert!(dirty(&rig, false));
}

#[test]
fn one_undo_on_the_board_takes_back_the_whole_pcb_side() {
    let mut rig = opened();
    let before = board_json(&rig);
    send(&mut rig, CompareBoardMsg::ApplyRequested);
    assert_ne!(board_json(&rig), before);
    let board = rig.board.clone();
    show(&mut rig.app, &board);
    undo(&mut rig.app);
    assert_eq!(board_json(&rig), before);
    assert!(!dirty(&rig, true), "undo is back at the board's save point");
}

#[test]
fn one_undo_on_the_sheet_takes_back_the_schematic_side_only() {
    let mut rig = opened();
    pick(
        &mut rig,
        &EcoKey::SetDesignator(FP_R2),
        Direction::UpdateSchematic,
    );
    send(&mut rig, CompareBoardMsg::ApplyRequested);
    let root = rig.root.clone();
    show(&mut rig.app, &root);
    undo(&mut rig.app);
    assert_eq!(sheet_part(&rig, R2).reference, "R2");
    assert_eq!(
        board_part(&rig, FP_R1).expect("R1").value,
        "10k",
        "the board side stays"
    );
}

#[test]
fn a_chosen_board_deletion_asks_first_and_cancel_applies_nothing() {
    let mut rig = opened();
    let (board, sheet) = (board_json(&rig), sheet_json(&rig));
    pick(
        &mut rig,
        &EcoKey::RemoveFootprint(FP_TP3),
        Direction::UpdatePcb,
    );
    assert_eq!(report(&rig).board_deletions().len(), 1);
    send(&mut rig, CompareBoardMsg::ApplyRequested);
    let state = rig.app.ui_state.compare_board.as_ref().expect("open");
    assert!(state.is_confirming());
    assert_eq!(board_json(&rig), board, "nothing before the answer");

    send(&mut rig, CompareBoardMsg::ApplyCancelled);
    let state = rig.app.ui_state.compare_board.as_ref().expect("open");
    assert!(!state.is_confirming());
    assert_eq!(board_json(&rig), board);
    assert_eq!(sheet_json(&rig), sheet);
    assert!(!dirty(&rig, true));

    send(&mut rig, CompareBoardMsg::ApplyRequested);
    send(&mut rig, CompareBoardMsg::ApplyConfirmed);
    assert!(board_part(&rig, FP_TP3).is_none(), "confirmed: removed");
    assert_eq!(engine(&rig).undo_len(), 1);
}

#[test]
fn close_leaves_both_documents_as_they_were() {
    let mut rig = opened();
    let (board, sheet) = (board_json(&rig), sheet_json(&rig));
    pick(
        &mut rig,
        &EcoKey::SetValue(FP_R1),
        Direction::UpdateSchematic,
    );
    send(&mut rig, CompareBoardMsg::CloseRequested);
    assert!(rig.app.ui_state.compare_board.is_none());
    assert_eq!(board_json(&rig), board);
    assert_eq!(sheet_json(&rig), sheet);
    assert!(!dirty(&rig, true));
    assert!(!dirty(&rig, false));
}

#[test]
fn apply_with_every_row_skipped_changes_nothing() {
    let mut rig = opened();
    let (board, sheet) = (board_json(&rig), sheet_json(&rig));
    send(
        &mut rig,
        CompareBoardMsg::ShownRowsDirectionPicked(Direction::Skip),
    );
    assert_eq!(report(&rig).counts().applied(), 0);
    send(&mut rig, CompareBoardMsg::ApplyRequested);
    assert!(rig.app.ui_state.compare_board.is_some());
    assert_eq!(board_json(&rig), board);
    assert_eq!(sheet_json(&rig), sheet);
}

#[test]
fn a_board_edited_after_the_read_refuses_apply_and_applies_nothing() {
    let mut rig = opened();
    let board = rig.board.clone();
    rig.app
        .execute_board_commands(
            &board,
            vec![BoardCommand::MoveFootprint {
                footprint: FP_TP3,
                to: BoardPoint::new(40.0, 20.0),
            }],
            None,
        )
        .expect("the move");
    let moved = board_json(&rig);
    assert!(rig.app.compare_board_stale());
    send(&mut rig, CompareBoardMsg::ApplyRequested);
    let state = rig.app.ui_state.compare_board.as_ref().expect("open");
    assert!(state.error().is_some(), "the refusal is shown");
    assert_eq!(board_json(&rig), moved, "nothing applied");
    assert_eq!(sheet_part(&rig, R2).reference, "R2");
}

#[test]
fn compare_again_keeps_the_directions_chosen() {
    let mut rig = opened();
    pick(
        &mut rig,
        &EcoKey::SetValue(FP_R1),
        Direction::UpdateSchematic,
    );
    send(&mut rig, CompareBoardMsg::CompareAgainRequested);
    assert_eq!(
        row(&rig, &EcoKey::SetValue(FP_R1)).direction,
        Direction::UpdateSchematic
    );
}

#[test]
fn a_jump_link_keeps_the_dialog_open_and_edits_nothing() {
    let mut rig = opened();
    let target = row(&rig, &EcoKey::SetValue(FP_R1)).jump.clone();
    send(
        &mut rig,
        CompareBoardMsg::JumpRequested {
            side: JumpSide::Board,
            target,
        },
    );
    assert!(rig.app.ui_state.compare_board.is_some());
    assert!(!dirty(&rig, true));
}

#[test]
fn a_board_that_matches_the_schematic_reports_no_differences() {
    let mut rig = rig(matching_board());
    command(&mut rig.app, "compare_board_with_schematic");
    let report = report(&rig);
    assert!(report.rows.is_empty(), "{:#?}", report.rows);
    assert_eq!(report.compared.parts, 2);
    assert_eq!(report.compared.footprints, 2);
    assert_eq!(report.counts().applied(), 0);
}
