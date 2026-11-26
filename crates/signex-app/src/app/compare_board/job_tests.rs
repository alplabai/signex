//! #2462, #2460 — Compare Board with Schematic from the command line: the
//! query answers the rows the dialog shows and changes nothing, the apply
//! form by row id makes what the dialog's Apply makes (one board undo step,
//! one schematic undo group), a refusal applies nothing, a matching project
//! says so, and `board_status` counts the same differences.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_engine::pcb::eco::{CompareKind, Direction};
use signex_i18n::RenderTranslatable;
use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::pcb::{Footprint, PcbBoard, Point as BoardPoint, SymbolLink};
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use uuid::Uuid;

use super::CompareBoardMsg;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex, TabInfo, TabKind};
use crate::keymap::AppCommandId;

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
    .expect("a footprint");
    fp.position = BoardPoint::new(uuid.as_u128() as f64, 10.0);
    fp.schematic_link = Some(SymbolLink::new(InstancePath::root(), [unit]));
    fp
}

fn board() -> PcbBoard {
    let mut board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).expect("a board");
    board.footprints = vec![
        footprint(FP_R1, "R1", "22k", R1),
        footprint(FP_R2, "R7", "4k7", R2),
        footprint(FP_TP3, "TP3", "TestPoint", GONE),
    ];
    board
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
        requirements: Default::default(),
        parameters: Default::default(),
        bom_alternates: Vec::new(),
        unknown: Default::default(),
    }
}

struct Rig {
    app: Signex,
    root: PathBuf,
    board: PathBuf,
    _dir: tempfile::TempDir,
}

/// The project on disk with its root sheet and its board open, the board
/// in front.
fn rig() -> Rig {
    rig_with(true)
}

/// [`rig`]; with `open_board` false the board stays closed on disk.
fn rig_with(open_board: bool) -> Rig {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("a temp dir");
    let root_sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![part(R1, "R1", "10k"), part(R2, "R2", "4k7")],
        ..Default::default()
    };
    let root = dir.path().join("root.snxsch");
    let text = SnxSchematic::new(root_sheet.clone())
        .write_string()
        .expect("encode the sheet");
    std::fs::write(&root, text).expect("write the sheet");
    let board_path = dir.path().join("Proj.snxpcb");
    let text = SnxPcb::new(board())
        .write_string()
        .expect("encode the board");
    std::fs::write(&board_path, text).expect("write the board");
    let data = project_data(dir.path());
    let prj = dir.path().join("Proj.snxprj");
    signex_types::project::write_project(&prj, &data).expect("write the project");

    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(crate::app::LoadedProject {
        id,
        path: prj,
        data,
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    let engine = signex_engine::Engine::new_with_path(root_sheet, Some(root.clone()))
        .expect("the sheet's engine");
    app.document_state.engines.insert(root.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "root".to_string(),
        path: root.clone(),
        project_id: Some(id),
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(root.clone());
    if open_board {
        app.open_pcb_tab(board_path.clone(), "Proj".to_string(), board());
    }
    Rig {
        app,
        root,
        board: board_path,
        _dir: dir,
    }
}

fn document(path: &Path) -> String {
    format!("document={}", path.display())
}

/// `id` with `operands` through the headless entry, to its outcome.
fn run(rig: &mut Rig, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    rig.app.run_headless_command_in_test(&command, args).0
}

fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

fn check(rig: &mut Rig) -> Value {
    let board = document(&rig.board);
    answered(run(rig, "check_board_against_schematic", &[&board]))
}

/// The id of the difference of `kind` about `designator`.
fn id_of(answer: &Value, kind: &str, designator: &str) -> String {
    answer["differences"]
        .as_array()
        .expect("differences")
        .iter()
        .find(|row| row["kind"] == kind && row["object"]["designator"] == designator)
        .and_then(|row| row["id"].as_str())
        .unwrap_or_else(|| panic!("no {kind} row for {designator}: {answer}"))
        .to_string()
}

fn board_footprint(rig: &Rig, id: Uuid) -> Option<Footprint> {
    let engine = rig.app.document_state.board_engine(&rig.board)?;
    engine
        .board()
        .footprints
        .iter()
        .find(|f| f.uuid == id)
        .cloned()
}

fn board_undo_len(rig: &Rig) -> usize {
    rig.app
        .document_state
        .board_engine(&rig.board)
        .expect("the board is open")
        .undo_len()
}

fn sheet_symbol(rig: &Rig, uuid: Uuid) -> Symbol {
    rig.app.document_state.engines[&rig.root]
        .document()
        .symbols
        .iter()
        .find(|s| s.uuid == uuid)
        .cloned()
        .expect("symbol")
}

fn revisions(rig: &Rig) -> (Option<u64>, u64) {
    let board = rig
        .app
        .document_state
        .board_engine(&rig.board)
        .map(|engine| engine.revision());
    (board, rig.app.document_state.engines[&rig.root].revision())
}

fn settle(rig: &mut Rig, msg: CompareBoardMsg) {
    let task = rig.app.update(Message::from(msg));
    crate::task_driver::TaskDriver::new()
        .settle(&mut rig.app, task)
        .expect("the dialog's work lands");
}

#[test]
fn the_query_answers_the_rows_the_dialog_shows_and_changes_nothing() {
    let mut rig = rig();
    let before = revisions(&rig);
    let answer = check(&mut rig);
    assert_eq!(revisions(&rig), before, "no edit");
    assert!(rig.app.ui_state.compare_board.is_none(), "no dialog");
    assert_eq!(answer["matches"], false, "{answer}");
    assert_eq!(answer["count"], 3, "{answer}");
    assert_eq!(answer["summary"], "3 differences");
    assert_eq!(answer["board"], rig.board.display().to_string());

    settle(&mut rig, CompareBoardMsg::Requested);
    let state = rig.app.ui_state.compare_board.as_ref().expect("the dialog");
    let report = state.report().expect("its report");
    let shown: Vec<(String, &str, &str)> = report
        .rows
        .iter()
        .map(|row| (row.token(), row.kind.token(), row.direction.token()))
        .collect();
    let asked: Vec<(String, &str, &str)> = answer["differences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["id"].as_str().unwrap().to_string(),
                row["kind"].as_str().unwrap(),
                row["default"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(asked, shown, "the same rows, kinds and defaults");
    assert_eq!(report.revision(), answer["revision"]);
}

#[test]
fn apply_by_id_makes_what_the_dialogs_apply_makes() {
    // The dialog: R2's designator to the schematic, the rest as it starts.
    let mut by_dialog = rig();
    settle(&mut by_dialog, CompareBoardMsg::Requested);
    let row = {
        let report = by_dialog.app.ui_state.compare_board.as_ref().unwrap();
        let report = report.report().unwrap();
        report
            .rows
            .iter()
            .find(|row| row.kind == CompareKind::ValueChange && row.jump.footprint == Some(FP_R2))
            .expect("the designator row")
            .id()
    };
    settle(
        &mut by_dialog,
        CompareBoardMsg::RowDirectionPicked {
            row,
            direction: Direction::UpdateSchematic,
        },
    );
    settle(&mut by_dialog, CompareBoardMsg::ApplyRequested);
    assert!(by_dialog.app.ui_state.compare_board.is_none(), "applied");

    // The command line: the same rows by id.
    let mut by_id = rig();
    let answer = check(&mut by_id);
    let value = id_of(&answer, "value_change", "R1");
    let designator = id_of(&answer, "value_change", "R2");
    let board = document(&by_id.board);
    let to_board = format!("to_board={value}");
    let to_schematic = format!("to_schematic={designator}");
    let expect = format!("expect_revision={}", answer["revision"]);
    let applied = answered(run(
        &mut by_id,
        "apply_board_schematic_differences",
        &[&board, &to_board, &to_schematic, &expect],
    ));
    assert_eq!(applied["board_changes"], 1, "{applied}");
    assert_eq!(applied["schematic_changes"], 1, "{applied}");
    assert_eq!(applied["sheets"], 1, "{applied}");
    assert_eq!(applied["to_board"][0], value.as_str());
    assert_eq!(applied["to_schematic"][0], designator.as_str());
    assert_eq!(
        applied["removed_from_board"].as_array().map(Vec::len),
        Some(0)
    );

    for rig in [&by_dialog, &by_id] {
        assert_eq!(board_footprint(rig, FP_R1).unwrap().value, "10k", "to PCB");
        assert_eq!(board_footprint(rig, FP_R2).unwrap().reference, "R7");
        assert!(board_footprint(rig, FP_TP3).is_some(), "skipped: kept");
        assert_eq!(board_undo_len(rig), 1, "one board undo step");
        assert_eq!(sheet_symbol(rig, R2).reference, "R7", "to the schematic");
        assert_eq!(sheet_symbol(rig, R1).value, "10k");
    }
    assert_eq!(applied["revision"], revisions(&by_id).0.unwrap());

    // One undo on the sheet takes the schematic side back, and only it.
    let index = by_id
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == by_id.root)
        .unwrap();
    let window = by_id.app.ui_state.main_window_id.unwrap();
    let _ = by_id.app.update(Message::Tab {
        window_id: window,
        msg: crate::tab_bar::TabMessage::Select(index),
    });
    let _ = by_id.app.update(Message::Edit(crate::app::EditMsg::Undo));
    assert_eq!(sheet_symbol(&by_id, R2).reference, "R2", "one undo group");
    assert_eq!(board_footprint(&by_id, FP_R1).unwrap().value, "10k");
}

#[test]
fn a_refused_apply_changes_nothing() {
    let mut rig = rig();
    let answer = check(&mut rig);
    let before = revisions(&rig);
    let board = document(&rig.board);
    let tp3 = id_of(&answer, "extra_on_board", "TP3");

    let unknown = refusal(run(
        &mut rig,
        "apply_board_schematic_differences",
        &[&board, "to_board=feedfacefeedface"],
    ));
    assert!(unknown.contains("no difference has the id"), "{unknown}");

    let blocked = format!("to_schematic={tp3}");
    let blocked = refusal(run(
        &mut rig,
        "apply_board_schematic_differences",
        &[&board, &blocked],
    ));
    assert!(blocked.contains("not built yet"), "{blocked}");

    let to_board = format!("to_board={tp3}");
    let stale = refusal(run(
        &mut rig,
        "apply_board_schematic_differences",
        &[&board, &to_board, "expect_revision=999"],
    ));
    assert!(stale.contains("check it again"), "{stale}");

    let none = refusal(run(
        &mut rig,
        "apply_board_schematic_differences",
        &[&board],
    ));
    assert!(none.contains("`to_board` or `to_schematic`"), "{none}");

    assert_eq!(revisions(&rig), before, "nothing applied");
    assert!(board_footprint(&rig, FP_TP3).is_some());
}

#[test]
fn after_every_row_goes_to_the_board_the_project_matches() {
    let mut rig = rig();
    let answer = check(&mut rig);
    let ids: Vec<&str> = answer["differences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    let board = document(&rig.board);
    let to_board = format!("to_board={}", ids.join(","));
    let applied = answered(run(
        &mut rig,
        "apply_board_schematic_differences",
        &[&board, &to_board],
    ));
    assert_eq!(applied["board_changes"], 3, "{applied}");
    let tp3 = id_of(&answer, "extra_on_board", "TP3");
    assert_eq!(applied["removed_from_board"][0], tp3.as_str());
    assert!(board_footprint(&rig, FP_TP3).is_none(), "removed");
    assert_eq!(board_undo_len(&rig), 1, "one board undo step");

    let again = check(&mut rig);
    assert_eq!(again["matches"], true, "{again}");
    assert_eq!(again["summary"], "Board matches schematic");
    assert_eq!(again["count"], 0);
}

#[test]
fn board_status_counts_the_same_differences_as_the_check() {
    let mut rig = rig();
    let answer = check(&mut rig);
    let board = document(&rig.board);
    let status = answered(run(&mut rig, "board_status", &[&board]));
    assert_eq!(status["board"], rig.board.display().to_string());
    assert_eq!(status["parts"]["total"], 3, "{status}");
    assert_eq!(status["parts"]["unplaced"], Value::Null, "no outline");
    assert_eq!(status["drc"]["ran"], false);
    assert_eq!(status["connections"]["unrouted"], 0);
    assert_eq!(status["routed_length_mm"], 0.0);
    assert_eq!(status["schematic"]["matches"], false);
    assert_eq!(status["schematic"]["differences"], answer["count"]);
    assert_eq!(status["schematic"]["summary"], answer["summary"]);
}

#[test]
fn a_closed_board_is_refused_with_the_reason() {
    let mut rig = rig_with(false);
    let named = document(&rig.root);
    let why = refusal(run(&mut rig, "check_board_against_schematic", &[&named]));
    assert!(why.contains("is not open"), "{why}");
}
