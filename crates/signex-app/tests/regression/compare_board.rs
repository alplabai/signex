//! #2462 — Compare Board with Schematic driven through UI messages on a
//! project read from disk: both change lists as one report with their
//! default directions, Apply as one board undo step and one schematic undo
//! group, a stale board refused with nothing applied, the confirmation
//! before a board deletion, and Export report… through the file port.

use std::path::{Path, PathBuf};

use signex_app::app::{
    CommandPaletteMsg, CompareBoardMsg, ContextAction, ContextMenuMsg, EditMsg, Message, Signex,
    TabDocument, TabInfo, TabKind,
};
use signex_engine::pcb::BoardEngine;
use signex_engine::pcb::eco::{BackKey, CompareKind, CompareReport, CompareRow, Direction, EcoKey};
use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::pcb::{Footprint, PcbBoard, Point as BoardPoint, SymbolLink};
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use tempfile::TempDir;
use uuid::Uuid;

use crate::menu_support::menu_row;
use crate::settle_support::settle;

/// Parts on the root sheet: R1 (10k) and R2 (4k7).
const R1: Uuid = Uuid::from_u128(0x101);
const R2: Uuid = Uuid::from_u128(0x102);
/// A unit of no part of the design: TP3 is linked to it.
const GONE: Uuid = Uuid::from_u128(0x999);
/// Board footprints: R1 with the value 22k, R2 renamed R7, and TP3.
const FP_R1: Uuid = Uuid::from_u128(0xf1);
const FP_R2: Uuid = Uuid::from_u128(0xf2);
const FP_TP3: Uuid = Uuid::from_u128(0xf3);

fn part(uuid: Uuid, reference: &str, value: &str) -> Symbol {
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

fn footprint(uuid: Uuid, reference: &str, value: &str, unit: Uuid) -> Footprint {
    let mut fp: Footprint = serde_json::from_value(serde_json::json!({
        "uuid": uuid, "reference": reference, "value": value,
        "position": {"x": 0.0, "y": 0.0}, "layer": "TopCopper"
    }))
    .unwrap();
    fp.position = BoardPoint::new(uuid.as_u128() as f64, 10.0);
    fp.schematic_link = Some(SymbolLink::new(InstancePath::root(), [unit]));
    fp
}

fn board() -> PcbBoard {
    let mut board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).unwrap();
    board.footprints = vec![
        footprint(FP_R1, "R1", "22k", R1),
        footprint(FP_R2, "R7", "4k7", R2),
        footprint(FP_TP3, "TP3", "TestPoint", GONE),
    ];
    board
}

struct Rig {
    app: Signex,
    root: PathBuf,
    board: PathBuf,
    dir: PathBuf,
    _tmp: TempDir,
}

fn project_data(dir: &Path) -> signex_types::project::ProjectData {
    signex_types::project::ProjectData {
        document_id: None,
        description: None,
        name: "Proj".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("root.snxsch".to_string()),
        pcb_file: Some("Proj.snxpcb".to_string()),
        sheets: vec![signex_types::project::SheetEntry {
            name: "root.snxsch".to_string(),
            filename: "root.snxsch".to_string(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        }],
        variant_definitions: Vec::new(),
        active_variant: None,
        variant_settings: Default::default(),
        libraries: Vec::new(),
        enable_git: false,
        rules: Default::default(),
        net_classes: Default::default(),
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        agent_journal: Default::default(),
        schematic_design: Default::default(),
        fab_outputs: None,
        router_tool_diameter: None,
        bom_alternates: Vec::new(),
        requirements: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

/// The project on disk with its root sheet open in front; the board is
/// closed until the compare opens it.
fn rig() -> Rig {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().to_path_buf();
    let root_sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![part(R1, "R1", "10k"), part(R2, "R2", "4k7")],
        ..Default::default()
    };
    let root = dir.join("root.snxsch");
    let text = SnxSchematic::new(root_sheet.clone())
        .write_string()
        .unwrap();
    std::fs::write(&root, text).unwrap();
    let board_path = dir.join("Proj.snxpcb");
    std::fs::write(&board_path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let data = project_data(&dir);
    let prj = dir.join("Proj.snxprj");
    signex_types::project::write_project(&prj, &data).unwrap();

    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let id = app.document_state.mint_project_id();
    app.document_state
        .projects
        .push(signex_app::app::LoadedProject {
            id,
            path: prj,
            data,
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
    app.document_state.active_project = Some(id);
    let engine = signex_engine::Engine::new_with_path(root_sheet, Some(root.clone())).unwrap();
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
        dir,
        _tmp: tmp,
    }
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
        .expect("dialog open");
    assert!(
        state.is_ready(),
        "the report is ready: {:?}",
        state.failure()
    );
    state.report().expect("a report")
}

fn row<'a>(rig: &'a Rig, key: &EcoKey) -> &'a CompareRow {
    report(rig)
        .rows
        .iter()
        .find(|r| r.pcb.as_ref() == Some(key))
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
        .find(|t| t.path == rig.board)
        .and_then(|t| t.cached_document.as_ref())
        .and_then(TabDocument::as_pcb_engine)
        .expect("the board is open")
}

fn board_footprint(rig: &Rig, id: Uuid) -> Option<&Footprint> {
    engine(rig).board().footprints.iter().find(|f| f.uuid == id)
}

fn sheet_symbol(rig: &Rig, uuid: Uuid) -> &Symbol {
    rig.app.document_state.engines[&rig.root]
        .document()
        .symbols
        .iter()
        .find(|s| s.uuid == uuid)
        .expect("symbol")
}

fn open(rig: &mut Rig) {
    settle(&mut rig.app, menu_row("compare_board_with_schematic"));
}

#[test]
fn the_project_row_compares_both_ways_with_the_rel_01_defaults() {
    let mut rig = rig();
    open(&mut rig);
    let report = report(&rig);
    assert_eq!(report.rows.len(), 3, "{:#?}", report.rows);
    let extra = row(&rig, &EcoKey::RemoveFootprint(FP_TP3));
    assert_eq!(extra.kind, CompareKind::ExtraOnBoard);
    assert_eq!(
        extra.direction,
        Direction::Skip,
        "a board deletion starts skipped"
    );
    let designator = row(&rig, &EcoKey::SetDesignator(FP_R2));
    assert_eq!(designator.kind, CompareKind::ValueChange);
    assert_eq!(designator.back, Some(BackKey::Designator(FP_R2)), "paired");
    assert_eq!(designator.direction, Direction::UpdatePcb);
    let value = row(&rig, &EcoKey::SetValue(FP_R1));
    assert_eq!(value.back, Some(BackKey::Value(FP_R1)), "paired");
    assert_eq!(value.direction, Direction::UpdatePcb);
    let counts = report.counts();
    assert_eq!(
        (counts.to_pcb, counts.to_schematic, counts.skipped),
        (2, 0, 1)
    );
}

#[test]
fn the_board_right_click_row_opens_the_same_report() {
    let mut rig = rig();
    let id = signex_app::keymap::AppCommandId::new("compare_board_with_schematic").unwrap();
    settle(
        &mut rig.app,
        Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command {
            id,
            project: None,
        })),
    );
    assert_eq!(report(&rig).rows.len(), 3);
    send(&mut rig, CompareBoardMsg::CloseRequested);
    assert!(rig.app.ui_state.compare_board.is_none());
}

#[test]
fn apply_is_one_board_step_and_one_schematic_undo_group() {
    let mut rig = rig();
    open(&mut rig);
    pick(
        &mut rig,
        &EcoKey::SetDesignator(FP_R2),
        Direction::UpdateSchematic,
    );
    assert_eq!(report(&rig).counts().to_schematic, 1);
    send(&mut rig, CompareBoardMsg::ApplyRequested);
    assert!(rig.app.ui_state.compare_board.is_none(), "Apply closes it");
    assert_eq!(board_footprint(&rig, FP_R1).unwrap().value, "10k", "to PCB");
    assert_eq!(
        board_footprint(&rig, FP_R2).unwrap().reference,
        "R7",
        "the board kept its designator"
    );
    assert!(board_footprint(&rig, FP_TP3).is_some(), "skipped: kept");
    assert_eq!(engine(&rig).undo_len(), 1, "one board undo step");
    assert_eq!(sheet_symbol(&rig, R2).reference, "R7", "to the schematic");
    assert_eq!(
        sheet_symbol(&rig, R1).value,
        "10k",
        "the schematic kept its value"
    );

    // One undo on the sheet takes the schematic side back, and only it.
    let index = rig
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == rig.root)
        .unwrap();
    let window = rig.app.ui_state.main_window_id.unwrap();
    settle(
        &mut rig.app,
        Message::Tab {
            window_id: window,
            msg: signex_app::tab_bar::TabMessage::Select(index),
        },
    );
    settle(&mut rig.app, Message::Edit(EditMsg::Undo));
    assert_eq!(sheet_symbol(&rig, R2).reference, "R2", "one undo group");
    assert_eq!(board_footprint(&rig, FP_R1).unwrap().value, "10k");
}

#[test]
fn a_board_that_moved_since_the_read_refuses_apply_and_applies_nothing() {
    let mut rig = rig();
    open(&mut rig);
    pick(
        &mut rig,
        &EcoKey::SetDesignator(FP_R2),
        Direction::UpdateSchematic,
    );
    let board = rig.board.clone();
    rig.app
        .execute_board_commands(
            &board,
            vec![signex_engine::pcb::BoardCommand::MoveFootprint {
                footprint: FP_TP3,
                to: BoardPoint::new(40.0, 20.0),
            }],
            None,
        )
        .unwrap();
    assert!(rig.app.compare_board_stale());
    send(&mut rig, CompareBoardMsg::ApplyRequested);
    let state = rig.app.ui_state.compare_board.as_ref().expect("still open");
    assert!(state.error().is_some(), "the refusal is shown");
    assert_eq!(
        board_footprint(&rig, FP_R1).unwrap().value,
        "22k",
        "nothing applied"
    );
    assert_eq!(sheet_symbol(&rig, R2).reference, "R2", "nothing applied");
    assert_eq!(engine(&rig).undo_len(), 1, "only the move");
}

#[test]
fn a_chosen_board_deletion_asks_first_and_cancel_applies_nothing() {
    let mut rig = rig();
    open(&mut rig);
    pick(
        &mut rig,
        &EcoKey::RemoveFootprint(FP_TP3),
        Direction::UpdatePcb,
    );
    let deletions: Vec<_> = report(&rig)
        .board_deletions()
        .into_iter()
        .map(|r| r.object.clone())
        .collect();
    assert_eq!(
        deletions,
        vec![signex_engine::pcb::eco::CompareObject::Designator(
            "TP3".into()
        )]
    );
    send(&mut rig, CompareBoardMsg::ApplyRequested);
    let state = rig.app.ui_state.compare_board.as_ref().unwrap();
    assert!(state.is_confirming(), "the confirmation lists the deletion");
    assert!(board_footprint(&rig, FP_TP3).is_some(), "not yet");
    send(&mut rig, CompareBoardMsg::ApplyCancelled);
    let state = rig.app.ui_state.compare_board.as_ref().unwrap();
    assert!(!state.is_confirming());
    assert!(
        board_footprint(&rig, FP_TP3).is_some(),
        "Cancel applies nothing"
    );
    assert_eq!(board_footprint(&rig, FP_R1).unwrap().value, "22k");

    send(&mut rig, CompareBoardMsg::ApplyRequested);
    send(&mut rig, CompareBoardMsg::ApplyConfirmed);
    assert!(rig.app.ui_state.compare_board.is_none());
    assert!(board_footprint(&rig, FP_TP3).is_none(), "removed");
    assert_eq!(engine(&rig).undo_len(), 1, "one board undo step");
}

#[test]
fn set_shown_rows_leaves_rows_that_cannot_take_the_direction() {
    let mut rig = rig();
    open(&mut rig);
    send(
        &mut rig,
        CompareBoardMsg::KindFilterPicked(Some(CompareKind::ValueChange)),
    );
    let state = rig.app.ui_state.compare_board.as_ref().unwrap();
    assert_eq!(state.shown_rows().len(), 2);
    send(
        &mut rig,
        CompareBoardMsg::ShownRowsDirectionPicked(Direction::UpdateSchematic),
    );
    assert_eq!(
        row(&rig, &EcoKey::SetValue(FP_R1)).direction,
        Direction::UpdateSchematic
    );
    assert_eq!(
        row(&rig, &EcoKey::RemoveFootprint(FP_TP3)).direction,
        Direction::Skip,
        "not shown: unchanged"
    );
    send(&mut rig, CompareBoardMsg::KindFilterPicked(None));
    send(
        &mut rig,
        CompareBoardMsg::ShownRowsDirectionPicked(Direction::UpdateSchematic),
    );
    assert_eq!(
        row(&rig, &EcoKey::RemoveFootprint(FP_TP3)).direction,
        Direction::Skip,
        "Extra on board cannot update the schematic yet"
    );
}

/// #2749 A: a pick made while the net rows recompute would be overwritten
/// when they land, so it is refused and the row keeps what it shows.
#[test]
fn a_pick_while_the_net_rows_recompute_is_refused() {
    let mut rig = rig();
    open(&mut rig);
    let footprint = row(&rig, &EcoKey::RemoveFootprint(FP_TP3)).id();
    let value = row(&rig, &EcoKey::SetValue(FP_R1)).id();
    let recompute = rig
        .app
        .update(Message::from(CompareBoardMsg::RowDirectionPicked {
            row: footprint,
            direction: Direction::UpdatePcb,
        }));
    let state = rig.app.ui_state.compare_board.as_ref().unwrap();
    assert!(!state.directions_live(), "the recompute is running");

    let _ = rig
        .app
        .update(Message::from(CompareBoardMsg::RowDirectionPicked {
            row: value,
            direction: Direction::Skip,
        }));
    let _ = rig
        .app
        .update(Message::from(CompareBoardMsg::ShownRowsDirectionPicked(
            Direction::Skip,
        )));
    crate::drive::drive(&mut rig.app, recompute);

    assert_eq!(
        row(&rig, &EcoKey::SetValue(FP_R1)).direction,
        Direction::UpdatePcb,
        "the refused pick changed nothing"
    );
    assert_eq!(
        row(&rig, &EcoKey::RemoveFootprint(FP_TP3)).direction,
        Direction::UpdatePcb,
        "the pick that started the recompute stands"
    );
}

/// #2749 B: a pick while Compare again reads starts no job, so the read
/// lands and the report shows what it read.
#[test]
fn a_pick_during_compare_again_does_not_swallow_the_read() {
    let mut rig = rig();
    open(&mut rig);
    assert_eq!(report(&rig).compared.parts, 2);
    let footprint = row(&rig, &EcoKey::RemoveFootprint(FP_TP3)).id();
    let mut sheet = rig.app.document_state.engines[&rig.root].document().clone();
    sheet.symbols.push(part(Uuid::from_u128(0x103), "R3", "1k"));
    let engine = signex_engine::Engine::new_with_path(sheet, Some(rig.root.clone())).unwrap();
    rig.app
        .document_state
        .engines
        .insert(rig.root.clone(), engine);

    let read = rig
        .app
        .update(Message::from(CompareBoardMsg::CompareAgainRequested));
    let _ = rig
        .app
        .update(Message::from(CompareBoardMsg::RowDirectionPicked {
            row: footprint,
            direction: Direction::UpdatePcb,
        }));
    crate::drive::drive(&mut rig.app, read);

    assert_eq!(report(&rig).compared.parts, 3, "the new read landed");
    assert_eq!(
        row(&rig, &EcoKey::RemoveFootprint(FP_TP3)).direction,
        Direction::Skip,
        "the refused pick changed nothing"
    );
}

#[test]
fn export_report_writes_through_the_file_port() {
    let mut rig = rig();
    open(&mut rig);
    let target = rig.dir.join("compare-board-report.txt");
    let answers = signex_app::io_ports::HeadlessAnswers::default();
    answers.replace_all([("path".to_string(), target.to_string_lossy().into_owned())]);
    rig.app.ui_state.io = signex_app::io_ports::IoPorts::headless(answers);
    send(&mut rig, CompareBoardMsg::ExportRequested);
    let text = std::fs::read_to_string(&target).expect("the report was written");
    assert!(text.starts_with("Compare Board with Schematic\n"), "{text}");
    assert!(text.contains("TP3"), "{text}");
    assert!(
        rig.app.ui_state.compare_board.is_some(),
        "the dialog stays open"
    );
}

#[test]
fn a_jump_link_keeps_the_dialog_open() {
    let mut rig = rig();
    open(&mut rig);
    let target = row(&rig, &EcoKey::SetValue(FP_R1)).jump.clone();
    send(
        &mut rig,
        CompareBoardMsg::JumpRequested {
            side: signex_app::app::JumpSide::Board,
            target,
        },
    );
    assert!(rig.app.ui_state.compare_board.is_some());
}

/// The palette entry point runs the same command.
#[test]
fn the_palette_opens_the_report() {
    let mut rig = rig();
    settle(
        &mut rig.app,
        Message::CommandPalette(CommandPaletteMsg::Open),
    );
    settle(
        &mut rig.app,
        Message::CommandPalette(CommandPaletteMsg::QueryChanged(
            "Compare board with schematic".into(),
        )),
    );
    settle(
        &mut rig.app,
        Message::CommandPalette(CommandPaletteMsg::ExecuteSelected),
    );
    assert_eq!(report(&rig).rows.len(), 3);
}
