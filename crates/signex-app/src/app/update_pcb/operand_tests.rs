//! #2470 — `update_pcb_from_schematic` from operands: `document` names the
//! project, `dry_run=true` answers the change list and changes nothing,
//! `apply=true` executes it as the dialog's Execute does (one undo step) and
//! answers what it did. The fixture: R1 on the sheet (value 10k) and its
//! linked footprint on the board (value 1k), so the list holds one value
//! change that applies.

use serde_json::Value;
use signex_i18n::RenderTranslatable;
use signex_types::pcb::{Footprint, PcbBoard, SymbolLink};
use signex_types::schematic::{InstancePath, Symbol};
use uuid::Uuid;

use super::answer::{AnswerFor, UpdatePcbAnswer};
use super::tests::{document, headless_project_with, run};
use super::{UpdatePcbMsg, UpdatePcbState};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

const R1: Uuid = Uuid::from_u128(0x2470_0001);
const R1_FOOTPRINT: Uuid = Uuid::from_u128(0x2470_f001);

fn r1() -> Symbol {
    Symbol {
        uuid: R1,
        lib_id: "R".to_string(),
        reference: "R1".to_string(),
        value: "10k".to_string(),
        ..Default::default()
    }
}

fn r1_footprint() -> Footprint {
    Footprint {
        uuid: R1_FOOTPRINT,
        reference: "R1".to_string(),
        value: "1k".to_string(),
        footprint_id: "R0603".to_string(),
        position: signex_types::pcb::Point::new(10.0, 10.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: Vec::new(),
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: Some(SymbolLink::new(InstancePath::root(), [R1])),
        library_ref: None,
        component: Default::default(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn fixture(dir: &std::path::Path) -> Signex {
    let mut board: PcbBoard = signex_types::project::SetupProfile::Standard
        .pcb(4)
        .new_board();
    board.footprints.push(r1_footprint());
    headless_project_with(dir, board, vec![r1()])
}

fn board_engine<'a>(app: &'a Signex, dir: &std::path::Path) -> &'a signex_engine::pcb::BoardEngine {
    app.document_state
        .board_engine(&dir.join("demo.snxpcb"))
        .expect("the board is open")
}

fn board_value(app: &Signex, dir: &std::path::Path) -> String {
    let engine = board_engine(app, dir);
    let fp = engine
        .board()
        .footprints
        .iter()
        .find(|f| f.uuid == R1_FOOTPRINT);
    fp.expect("R1 on the board").value.clone()
}

fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("answered with the change list: {other:?}"),
    }
}

fn refusal(outcome: &CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("refused: {other:?}"),
    }
}

/// The row of `kind` for `object`.
fn row<'a>(answer: &'a Value, kind: &str, object: &str) -> &'a Value {
    answer["changes"]
        .as_array()
        .expect("changes")
        .iter()
        .find(|row| row["kind"] == kind && row["object"] == object)
        .unwrap_or_else(|| panic!("no {kind} row for {object}: {answer}"))
}

#[test]
fn a_dry_run_answers_the_change_list_and_changes_nothing() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path());
    let before = board_engine(&app, tmp.path()).revision();
    let answer = answered(run(&mut app, &[&document(tmp.path()), "dry_run=true"]));
    assert_eq!(answer["dry_run"], true, "{answer}");
    assert_eq!(answer["revision"], before, "{answer}");
    assert_eq!(answer["board_revision"], before, "{answer}");
    let value = row(&answer, "set_value", "R1");
    assert_eq!(value["action"], "change");
    assert_eq!(value["from"], "1k");
    assert_eq!(value["to"], "10k");
    assert_eq!(value["footprint"], R1_FOOTPRINT.to_string());
    assert_eq!(value["applies"], true);
    assert_eq!(value["blocked"], Value::Null);
    assert!(answer["applied"].as_u64() >= Some(1), "{answer}");
    assert_eq!(answer["execute_blocked"], Value::Null, "{answer}");
    // Nothing changed: the value, the revision, the history, no dialog.
    assert_eq!(board_value(&app, tmp.path()), "1k");
    let engine = board_engine(&app, tmp.path());
    assert_eq!(engine.revision(), before);
    assert!(!engine.can_undo(), "a dry run makes no undo step");
    assert!(app.ui_state.update_pcb.is_none(), "no dialog is left open");
}

#[test]
fn apply_answers_what_it_did_as_one_undo_step() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path());
    let before = board_engine(&app, tmp.path()).revision();
    let answer = answered(run(&mut app, &[&document(tmp.path()), "apply=true"]));
    assert_eq!(answer["dry_run"], false, "{answer}");
    assert_eq!(answer["revision"], before, "{answer}");
    assert_ne!(
        answer["board_revision"], before,
        "the board moved: {answer}"
    );
    assert_eq!(row(&answer, "set_value", "R1")["applies"], true);
    assert_eq!(board_value(&app, tmp.path()), "10k");
    let engine = board_engine(&app, tmp.path());
    assert_eq!(answer["board_revision"], engine.revision(), "{answer}");
    assert_eq!(engine.undo_len(), 1, "one undo step");
    assert!(app.ui_state.update_pcb.is_none(), "no dialog is left open");
}

/// `--dry-run` adds `dry_run=true` to a script's `apply=true`: nothing
/// changes.
#[test]
fn dry_run_wins_over_apply() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path());
    let doc = document(tmp.path());
    let answer = answered(run(&mut app, &[&doc, "apply=true", "dry_run=true"]));
    assert_eq!(answer["dry_run"], true, "{answer}");
    assert_eq!(board_value(&app, tmp.path()), "1k");
    assert!(!board_engine(&app, tmp.path()).can_undo());
}

/// The dialog (the menu row's message) and the operand form list the same
/// changes, and Execute makes the same edit as `apply=true`.
#[test]
fn the_dialog_lists_and_executes_what_the_operand_form_answers() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut gui = fixture(tmp.path());
    let task = gui.update(Message::from(UpdatePcbMsg::Requested { apply: false }));
    crate::task_driver::TaskDriver::new()
        .settle(&mut gui, task)
        .expect("the list lands");
    let state: &UpdatePcbState = gui.ui_state.update_pcb.as_deref().expect("the dialog");
    assert!(state.is_ready(), "{:?}", state.failure());
    let list = state.list().expect("a list").clone();
    let shown = UpdatePcbAnswer::of(
        &list,
        AnswerFor {
            dry_run: true,
            project: &tmp.path().join("demo.snxprj"),
            board: &tmp.path().join("demo.snxpcb"),
            board_revision: list.revision,
            execute_blocked: None,
            net_classes: Vec::new(),
        },
    );

    let other = tempfile::tempdir().expect("tmp");
    let mut cli = fixture(other.path());
    let answer = answered(run(&mut cli, &[&document(other.path()), "dry_run=true"]));
    let shown = serde_json::to_value(shown).expect("JSON");
    assert_eq!(answer["changes"], shown["changes"], "the same rows");
    assert_eq!(answer["counts"], shown["counts"]);

    let task = gui.update(Message::from(UpdatePcbMsg::ExecuteRequested));
    crate::task_driver::TaskDriver::new()
        .settle(&mut gui, task)
        .expect("executed");
    let _ = answered(run(&mut cli, &[&document(other.path()), "apply=true"]));
    assert_eq!(board_value(&gui, tmp.path()), "10k");
    assert_eq!(board_value(&cli, other.path()), "10k");
    assert_eq!(board_engine(&gui, tmp.path()).undo_len(), 1);
    assert_eq!(board_engine(&cli, other.path()).undo_len(), 1);
}

#[test]
fn a_document_of_no_open_project_is_refused() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path());
    let elsewhere = format!("document={}", tmp.path().join("elsewhere.snxprj").display());
    let outcome = run(&mut app, &[&elsewhere, "dry_run=true"]);
    assert!(refusal(&outcome).contains("did not run"), "{outcome:?}");
    assert!(app.ui_state.update_pcb.is_none());
}

/// A bare headless run is asked for `apply=true` or `dry_run=true`, and
/// opens nothing: no dialog nobody sees.
#[test]
fn a_headless_run_without_operands_is_asked_for_apply_or_dry_run() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path());
    let id = AppCommandId::new("update_pcb_from_schematic").expect("id");
    let (outcome, _task) = app.run_headless_command_in_test(&id, CommandArgs::none());
    let why = refusal(&outcome);
    assert!(
        why.contains("apply=true") && why.contains("dry_run=true"),
        "{outcome:?}"
    );
    assert!(app.ui_state.update_pcb.is_none(), "no dialog opened");
}

/// No `document`: the run acts on the active project's board (Caner,
/// 2026-11-25), the old `apply=true` call included.
#[test]
fn with_no_document_the_active_projects_board_is_updated() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path());
    let answer = answered(run(&mut app, &["dry_run=true"]));
    let board = tmp.path().join("demo.snxpcb").display().to_string();
    assert_eq!(answer["board"], board, "{answer}");
    assert_eq!(row(&answer, "set_value", "R1")["applies"], true);
    assert_eq!(board_value(&app, tmp.path()), "1k", "a dry run");

    let answer = answered(run(&mut app, &["apply=true"]));
    assert_eq!(answer["board"], board, "{answer}");
    assert_eq!(board_value(&app, tmp.path()), "10k");
    assert_eq!(
        board_engine(&app, tmp.path()).undo_len(),
        1,
        "one undo step"
    );
}

/// One project open and none made active: its board is still the default.
#[test]
fn with_one_project_open_and_none_active_its_board_is_the_default() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path());
    app.document_state.active_project = None;
    let answer = answered(run(&mut app, &["dry_run=true"]));
    let board = tmp.path().join("demo.snxpcb").display().to_string();
    assert_eq!(answer["board"], board, "{answer}");
}

/// The fixture's project in `first` and a second one in `second`, each with
/// R1 at 1k on its open board; each tab belongs to its own project.
fn two_projects(first: &std::path::Path, second: &std::path::Path) -> Signex {
    let mut app = fixture(first);
    let mut board: PcbBoard = signex_types::project::SetupProfile::Standard
        .pcb(4)
        .new_board();
    board.footprints.push(r1_footprint());
    app.open_pcb_tab(second.join("demo.snxpcb"), "demo.snxpcb".to_string(), board);
    super::tests::project(&mut app, second);
    let ids = [
        app.document_state.projects[0].id,
        app.document_state.projects[1].id,
    ];
    for tab in &mut app.document_state.tabs {
        let owner = usize::from(tab.path.starts_with(second));
        tab.project_id = Some(ids[owner]);
    }
    app
}

/// The other project's board: still 1k, with no undo step.
fn assert_untouched(app: &Signex, dir: &std::path::Path) {
    assert_eq!(board_value(app, dir), "1k", "{} untouched", dir.display());
    assert!(!board_engine(app, dir).can_undo(), "{}", dir.display());
}

/// Two projects, the active one with a board: its board is updated and the
/// other project's is left alone.
#[test]
fn with_two_projects_the_active_ones_board_is_updated() {
    let tmp = tempfile::tempdir().expect("tmp");
    let other = tempfile::tempdir().expect("tmp");
    let mut app = two_projects(tmp.path(), other.path());
    app.document_state.active_project = Some(app.document_state.projects[0].id);
    let answer = answered(run(&mut app, &["apply=true"]));
    let board = tmp.path().join("demo.snxpcb").display().to_string();
    assert_eq!(answer["board"], board, "{answer}");
    assert_eq!(board_value(&app, tmp.path()), "10k");
    assert_untouched(&app, other.path());
}

/// Two projects, the active one with no board: refused, listing the other
/// project's board, and nothing changes; that board is never taken instead.
#[test]
fn with_two_projects_and_no_board_on_the_active_one_the_run_is_refused() {
    let tmp = tempfile::tempdir().expect("tmp");
    let other = tempfile::tempdir().expect("tmp");
    let mut app = two_projects(tmp.path(), other.path());
    app.document_state.active_project = Some(app.document_state.projects[0].id);
    app.document_state.projects[0].data.pcb_file = None;
    let why = refusal(&run(&mut app, &["apply=true"]));
    assert!(why.contains("needs `document=`"), "{why}");
    assert!(why.contains("the active project has no board"), "{why}");
    let listed = other.path().join("demo.snxpcb").display().to_string();
    assert!(why.contains(&listed), "{listed} is listed: {why}");
    let own = tmp.path().join("demo.snxpcb").display().to_string();
    assert!(!why.contains(&own), "the active project's own file: {why}");
    assert!(app.ui_state.update_pcb.is_none(), "nothing opened");
    assert_untouched(&app, tmp.path());
    assert_untouched(&app, other.path());
}

/// Headless with several projects open and none active: refused, listing
/// every open project's board, and nothing changes.
#[test]
fn with_several_projects_and_none_active_the_run_is_refused_with_the_list() {
    let tmp = tempfile::tempdir().expect("tmp");
    let other = tempfile::tempdir().expect("tmp");
    let mut app = two_projects(tmp.path(), other.path());
    app.document_state.active_project = None;
    let id = AppCommandId::new("update_pcb_from_schematic").expect("id");
    let args = CommandArgs::parse_operands(&id, &["apply=true"]).expect("operands");
    let (outcome, _task) = app.run_headless_command_in_test(&id, args);
    let why = refusal(&outcome);
    assert!(why.contains("no project is active"), "{why}");
    for dir in [tmp.path(), other.path()] {
        let board = dir.join("demo.snxpcb").display().to_string();
        assert!(why.contains(&board), "{board} is listed: {why}");
    }
    assert!(app.ui_state.update_pcb.is_none(), "nothing opened");
    assert_untouched(&app, tmp.path());
    assert_untouched(&app, other.path());
}

/// No `document` and no board in the open projects: refused for the
/// missing board.
#[test]
fn with_no_document_and_no_board_the_run_is_refused() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path());
    app.document_state.projects[0].data.pcb_file = None;
    let outcome = run(&mut app, &["apply=true"]);
    assert!(refusal(&outcome).contains("no board yet"), "{outcome:?}");
    assert!(app.ui_state.update_pcb.is_none(), "nothing opened");
    assert_eq!(board_value(&app, tmp.path()), "1k", "nothing changed");
}

/// No `document` and no project open: refused as on trunk.
#[test]
fn with_no_document_and_no_project_the_run_is_refused() {
    let (mut app, _t) = Signex::new();
    let outcome = run(&mut app, &["dry_run=true"]);
    assert!(
        refusal(&outcome).contains("not part of a project"),
        "{outcome:?}"
    );
    assert!(app.ui_state.update_pcb.is_none(), "nothing opened");
}

/// The headless entry waits for the list and answers it.
#[test]
fn the_headless_entry_answers_the_list() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path());
    let id = AppCommandId::new("update_pcb_from_schematic").expect("id");
    let doc = document(tmp.path());
    let args = CommandArgs::parse_operands(&id, &[&doc, "dry_run=true"]).expect("operands");
    let (outcome, _task) = app.run_headless_command_in_test(&id, args);
    let answer = answered(outcome);
    assert_eq!(row(&answer, "set_value", "R1")["applies"], true);
}

/// The person's dialog stays theirs: a run from operands is refused while
/// it is open.
#[test]
fn a_run_while_the_dialog_is_open_is_refused() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path());
    let task = app.update(Message::from(UpdatePcbMsg::Requested { apply: false }));
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the list lands");
    let outcome = run(&mut app, &[&document(tmp.path()), "dry_run=true"]);
    assert!(
        refusal(&outcome).contains("already open or running"),
        "{outcome:?}"
    );
    let state = app.ui_state.update_pcb.as_ref().expect("the dialog");
    assert!(
        app.agent.update_pcb_job.is_none() && !state.apply,
        "still the person's"
    );
}

/// The dialog does not open over a run that waits for its list, and the run
/// is still answered.
#[test]
fn the_dialog_does_not_open_over_a_waiting_run() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path());
    let id = AppCommandId::new("update_pcb_from_schematic").expect("id");
    let doc = document(tmp.path());
    let args = CommandArgs::parse_operands(&id, &[&doc, "dry_run=true"]).expect("operands");
    let (CommandOutcome::Deferred { ticket }, task) = app.run_operands(&id, &args) else {
        panic!("the run waits for its list");
    };
    let _ = app.update(Message::from(UpdatePcbMsg::Requested { apply: false }));
    assert!(app.ui_state.update_pcb.is_some(), "the run");
    assert!(app.agent.update_pcb_job.is_some(), "the run is kept");
    assert!(
        app.document_state.error_notice.is_some(),
        "the person is told"
    );
    let (outcome, _) = app.settle_deferred(ticket, task);
    assert_eq!(row(&answered(outcome), "set_value", "R1")["applies"], true);
}

/// A board the project names that cannot be opened: the run answers why.
#[test]
fn a_board_that_cannot_be_opened_is_answered_why() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = fixture(tmp.path());
    app.document_state.projects[0].data.pcb_file = Some("missing.snxpcb".to_string());
    let outcome = run(&mut app, &[&document(tmp.path()), "apply=true"]);
    assert!(refusal(&outcome).contains("did not run"), "{outcome:?}");
    assert!(app.ui_state.update_pcb.is_none(), "the run ended");
}
