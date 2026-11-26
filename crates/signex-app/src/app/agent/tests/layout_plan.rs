//! #2942: the layout plan through a running Signex. `layout_plan_set` on the
//! agent channel sets one stage of the named board as one step, and
//! `layout_plan_get` and `layout_plan_status` answer as they do headless and
//! change nothing.

use serde_json::{Value, json};

use super::{app_in, call};
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;

/// [`app_in`] with the project's board `board.snxpcb` open in the app.
fn app_with_board(dir: &std::path::Path) -> (Signex, String) {
    let mut app = app_in(dir);
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.join("board.snxpcb");
    let board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    app.open_pcb_tab(path.clone(), "board.snxpcb".into(), board);
    (app, path.display().to_string())
}

fn headless(app: &mut Signex, id: &str, document: &str) -> Value {
    let command = AppCommandId::new(id).expect("a catalog id");
    let operand = format!("document={document}");
    let args = CommandArgs::parse_operands(&command, &[&operand]).expect("operands parse");
    match app.run_headless_command_in_test(&command, args).0 {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

#[test]
fn an_agent_sets_reads_and_judges_the_plan_of_the_named_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());

    let reply = call(
        &mut app,
        "layout_plan_set",
        json!({"document": board, "stage": "pours", "pours": uuid::Uuid::nil().to_string()}),
    );
    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(reply["data"]["changed"], true, "{reply}");
    assert_eq!(reply["data"]["stage"], "pours", "{reply}");
    let changed = reply["changed"].as_array().expect("changed");
    assert_eq!(changed.len(), 1, "the board changed: {reply}");
    let warnings = reply["data"]["warnings"].as_array().expect("warnings");
    assert!(
        warnings[0]
            .as_str()
            .unwrap_or_default()
            .contains("is not on the board"),
        "{reply}"
    );

    let read = call(&mut app, "layout_plan_get", json!({"document": board}));
    assert_eq!(read["status"], "answered", "{read}");
    assert_eq!(read["changed"], json!([]), "a read changes nothing");
    assert_eq!(read["data"]["stages"], reply["data"]["stages"]);
    assert_eq!(read["data"], headless(&mut app, "layout_plan_get", &board));

    let status = call(&mut app, "layout_plan_status", json!({"document": board}));
    assert_eq!(status["status"], "answered", "{status}");
    assert_eq!(status["changed"], json!([]));
    assert_eq!(
        status["data"],
        headless(&mut app, "layout_plan_status", &board)
    );
    let pours = &status["data"]["stages"][2];
    assert_eq!(pours["state"], "failing", "the pour is not there: {status}");
}

#[test]
fn a_refused_set_on_the_agent_channel_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let reply = call(
        &mut app,
        "layout_plan_set",
        json!({"document": board, "stage": "checks", "weights": "edges:1"}),
    );
    assert_eq!(reply["status"], "refused", "{reply}");
    assert_eq!(reply["changed"], json!([]), "{reply}");
    let read = call(&mut app, "layout_plan_get", json!({"document": board}));
    assert_eq!(read["data"]["empty"], true, "{read}");
}
