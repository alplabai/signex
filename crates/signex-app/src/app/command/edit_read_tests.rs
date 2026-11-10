//! #2199 (decision comment 6771692137): an edit from operands whose checks
//! need the disk reads it off the UI thread first and runs when the reads
//! land, with the verdict it always had; a target that changed meanwhile is
//! refused; an edit list reads once; an edit that needs no disk runs at once.

use serde_json::json;
use signex_types::schematic::{Point, SchematicSheet};

use super::part_operand_tests::library;
use super::project_name_tests::project_app;
use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::keymap::AppCommandId;

fn args(id: &str, operands: &[&str]) -> (AppCommandId, CommandArgs) {
    let id = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&id, operands).expect("operands parse");
    (id, args)
}

fn wires(app: &Signex) -> usize {
    let path = app
        .document_state
        .edit_path()
        .expect("a target")
        .to_path_buf();
    app.document_state.engines[&path].document().wires.len()
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn a_check_that_needs_a_closed_sheet_runs_when_its_read_lands_with_the_same_verdict() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let before = wires(&app);
    let (id, call) = args("place_wire", &["start=5.08,0", "end=10.16,0"]);
    let (outcome, task) = app.run_operands(&id, &call);
    let CommandOutcome::Deferred { ticket } = outcome else {
        panic!("not deferred: {outcome:?}");
    };
    assert_eq!(wires(&app), before, "nothing runs before the read lands");
    let (outcome, _) = app.settle_deferred(ticket, task);
    // The child's names reach the net: the verdict a direct read gave.
    assert!(refusal(outcome).contains("reach it from other sheets"));
    assert_eq!(wires(&app), before);
    let (id, call) = args(
        "place_wire",
        &["start=5.08,0", "end=10.16,0", "allow_merge=true"],
    );
    let path = app.document_state.edit_path().unwrap().to_path_buf();
    let revision = app.document_state.engines[&path].revision();
    let (outcome, _) = app.run_operands_settled(&id, &call);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    assert!(app.document_state.engines[&path].revision() > revision);
}

#[test]
fn an_edit_whose_sheet_changed_while_its_reads_ran_is_refused_and_applies_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let (id, call) = args(
        "place_wire",
        &["start=5.08,0", "end=10.16,0", "allow_merge=true"],
    );
    let (CommandOutcome::Deferred { ticket }, task) = app.run_operands(&id, &call) else {
        panic!("not deferred");
    };
    // The person edits the sheet before the read lands.
    assert!(app.place_junction_at(Point::new(30.48, 30.48)));
    let before = wires(&app);
    let (outcome, _) = app.settle_deferred(ticket, task);
    assert!(refusal(outcome).contains("changed while its project and files were read"));
    assert_eq!(wires(&app), before);
}

#[test]
fn an_edit_list_with_a_part_and_a_wire_is_read_once_and_lands_as_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let lib = library();
    app.library.open_library(lib.snxlib.clone()).expect("mount");
    let path = app.document_state.edit_path().unwrap().to_path_buf();
    let revision = app.document_state.engines[&path].revision();
    let edits = json!([
        {"id": "place_component", "args": {"library": lib.id.to_string(), "part": "SNX-TWO", "at": "50.8,50.8"}},
        // From U1.1's tip; a pin reference waits for the hierarchy walk this
        // fixture never runs.
        {"id": "place_wire", "args": {"start": "50.8,50.8", "end": "60.96,50.8", "allow_open": true}},
    ]);
    let (id, call) = args(
        "apply_edits",
        &[
            &format!("document={}", path.display()),
            &format!("expect_revision={revision}"),
            &format!("edits={edits}"),
        ],
    );
    let (outcome, _) = app.run_operands_settled(&id, &call);
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["changed"], true, "{answer}");
    assert_eq!(answer["results"][0]["designator"], "U1");
    assert_eq!(app.document_state.agent_edits.tickets, 1, "one read");
    assert!(app.document_state.agent_edits.reads.is_none());
}

#[test]
fn an_edit_that_needs_no_disk_runs_at_once() {
    let (mut app, _task) = Signex::new();
    let path = std::env::temp_dir().join("snx-2199-loose.snxsch");
    crate::app::handlers::editing_commands::net_color_driver::add_sheet_with(
        &mut app,
        &path,
        SchematicSheet::default(),
    );
    app.ui_state.snap_enabled = false;
    let (id, call) = args("place_junction", &["at=5.08,5.08"]);
    let (outcome, _) = app.run_operands(&id, &call);
    assert_eq!(outcome, CommandOutcome::Ran);
    assert_eq!(app.document_state.agent_edits.tickets, 0);
}
