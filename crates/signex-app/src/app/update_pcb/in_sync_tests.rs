//! #3060 — the sample sheet with its corpus board matches: Update PCB lists
//! nothing, a dry run answers no change and applied 0, an apply leaves the
//! board revision as it was, and the dialog shows no row and executes
//! nothing. The board's U1 tab (pad 4, on +3V3, not on the symbol) used to
//! read "+3V3 → +3V3" and count as one change.

use serde_json::Value;
use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::schematic::SchematicSheet;

use super::UpdatePcbMsg;
use super::tests::{document, headless_project_with_sheet, run};
use crate::app::command::CommandOutcome;
use crate::app::{Message, Signex};

const BOARD: &str =
    include_str!("../../../../signex-types/tests/corpus/pcb/real/linear-regulator.snxpcb");

fn sample_sheet() -> SchematicSheet {
    SnxSchematic::parse(crate::sample_project::SHEET_SOURCE)
        .expect("the sample sheet parses")
        .sheet
}

fn fixture(dir: &std::path::Path, sheet: SchematicSheet) -> Signex {
    let board = SnxPcb::parse(BOARD).expect("the corpus board parses").board;
    headless_project_with_sheet(dir, board, sheet)
}

fn board_path(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join("demo.snxpcb")
}

fn revision(app: &Signex, dir: &std::path::Path) -> u64 {
    app.document_state
        .board_engine(&board_path(dir))
        .expect("the board is open")
        .revision()
}

fn tab_locked(app: &Signex, dir: &std::path::Path) -> bool {
    let engine = app.document_state.board_engine(&board_path(dir));
    let board = engine.expect("the board is open").board();
    let u1 = board.footprints.iter().find(|f| f.reference == "U1");
    let tab = u1.and_then(|f| f.pads.iter().find(|p| p.number == "4"));
    tab.expect("U1 pad 4").net_locked
}

fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("answered with the change list: {other:?}"),
    }
}

fn changes(answer: &Value) -> &Vec<Value> {
    answer["changes"].as_array().expect("changes")
}

#[test]
fn a_dry_run_on_the_matching_sample_lists_nothing() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path(), sample_sheet());
    let before = revision(&app, tmp.path());
    let answer = answered(run(&mut app, &[&document(tmp.path()), "dry_run=true"]));
    assert_eq!(changes(&answer).len(), 0, "{answer}");
    assert_eq!(answer["applied"], 0, "{answer}");
    assert_eq!(answer["counts"]["change"], 0, "{answer}");
    assert_eq!(answer["pad_locks"], serde_json::json!([]), "{answer}");
    assert_eq!(revision(&app, tmp.path()), before);
}

#[test]
fn an_apply_on_the_matching_sample_changes_nothing() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path(), sample_sheet());
    let before = revision(&app, tmp.path());
    let answer = answered(run(&mut app, &[&document(tmp.path()), "apply=true"]));
    assert_eq!(changes(&answer).len(), 0, "{answer}");
    assert_eq!(answer["applied"], 0, "{answer}");
    assert_eq!(answer["board_revision"], before, "{answer}");
    assert_eq!(answer["pad_locks"], serde_json::json!([]), "{answer}");
    assert_eq!(revision(&app, tmp.path()), before, "the board did not move");
    assert!(!tab_locked(&app, tmp.path()));
}

/// The dialog, opened and executed through `Signex::update`.
#[test]
fn the_dialog_on_the_matching_sample_shows_no_row_and_executes_nothing() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path(), sample_sheet());
    let before = revision(&app, tmp.path());
    let project = app.document_state.active_project.expect("a project");
    let task = app.update(Message::from(UpdatePcbMsg::RequestedForProject(project)));
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("settled");
    let state = app
        .ui_state
        .update_pcb
        .as_ref()
        .expect("the dialog is open");
    assert!(state.is_ready(), "{:?}", state.failure());
    let list = state.list().expect("a list");
    assert!(list.rows.is_empty(), "{:?}", list.rows);
    assert!(state.shown_rows().is_empty());
    assert_eq!(state.execute_count(), 0);

    let task = app.update(Message::from(UpdatePcbMsg::ExecuteRequested));
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("settled");
    assert_eq!(
        revision(&app, tmp.path()),
        before,
        "Execute changed nothing"
    );
    assert!(!tab_locked(&app, tmp.path()));
}

/// A real difference on the same fixture is still listed, and applying it
/// takes the tab's lock with it.
#[test]
fn a_real_difference_on_the_sample_is_listed_and_applied() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut sheet = sample_sheet();
    let r1 = sheet.symbols.iter_mut().find(|s| s.reference == "R1");
    let r1 = r1.expect("R1 on the sample sheet");
    let old = r1.value.clone();
    r1.value = format!("{old}0");
    let mut app = fixture(tmp.path(), sheet);

    let answer = answered(run(&mut app, &[&document(tmp.path()), "dry_run=true"]));
    let rows = changes(&answer);
    assert_eq!(rows.len(), 1, "{answer}");
    assert_eq!(rows[0]["kind"], "set_value", "{answer}");
    assert_eq!(rows[0]["from"], old.as_str());
    assert_eq!(rows[0]["to"], format!("{old}0"));
    assert_eq!(answer["applied"], 1, "{answer}");
    assert_eq!(answer["pad_locks"], serde_json::json!(["U1.4"]), "{answer}");

    let before = revision(&app, tmp.path());
    let said = crate::diagnostics::logged_count(LOCKED_LINE);
    let answer = answered(run(&mut app, &[&document(tmp.path()), "apply=true"]));
    assert_eq!(answer["applied"], 1, "{answer}");
    assert_eq!(answer["pad_locks"], serde_json::json!(["U1.4"]), "{answer}");
    assert_ne!(revision(&app, tmp.path()), before, "the board moved");
    assert!(tab_locked(&app, tmp.path()), "the tab locks with the row");
    assert!(
        crate::diagnostics::logged_count(LOCKED_LINE) > said,
        "Messages says the pad was locked"
    );
}

const LOCKED_LINE: &str = "locked 1 pad with no symbol pin";
