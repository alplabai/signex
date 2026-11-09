//! A query and an `apply_edits` arrive on the agent channel while the person is
//! in the middle of something: the change lands, and nothing the person holds
//! moves.

use serde_json::json;

use super::{app_at_work, call, call_as, path, revision, snapshot, uid};
use crate::app::command::queries::tests::WIRE_A;

fn doc(file: &str) -> String {
    path(file).display().to_string()
}

#[test]
fn a_query_and_an_edit_land_while_the_person_is_mid_interaction_and_their_state_is_byte_equal() {
    for file in ["b.snxsch", "a.snxsch"] {
        let mut app = app_at_work();
        app.refresh_panel_ctx();
        let person = snapshot(&app);
        assert!(person.contains("wire_preview"), "{person}");
        assert!(person.contains("palette="), "{person}");
        let started = revision(&app, file);

        let read = call(&mut app, "read_document", json!({"document": doc(file)}));
        assert_eq!(read["status"], "answered", "{read}");
        assert_eq!(read["id"], "read_document");
        assert_eq!(read["data"]["revision"], started);
        assert_eq!(read["changed"], json!([]), "a query changes nothing");

        let edits = json!([{"id": "place_junction", "args": {"at": [20, 20]}}]);
        let applied = call(
            &mut app,
            "apply_edits",
            json!({"document": doc(file), "expect_revision": started, "edits": edits}),
        );
        assert_eq!(applied["status"], "answered", "{applied}");
        assert_eq!(applied["changed"], json!([doc(file)]), "{applied}");
        assert!(applied["messages"].is_array());

        assert!(revision(&app, file) > started, "the edit landed on {file}");
        assert_eq!(snapshot(&app), person, "the person's state, on {file}");
        // The wire the person selected is still the one selected.
        let selected = &app.interaction_state.active_canvas().selected;
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].uuid, uid(WIRE_A));
    }
}

#[test]
fn every_request_is_logged_with_the_client_pid_the_command_and_the_outcome() {
    // A pid no other test sends: the diagnostics ring is shared by every
    // test thread, so a common pid lets their lines into the count (#2227).
    const PID: u32 = 52_271;
    let mut app = app_at_work();
    let answered_line = "Agent request (pid 52271): `list_documents` -> answered";
    let refused_line = "Agent request (pid 52271): `undo` -> refused";
    let answered = crate::diagnostics::logged_count(answered_line);
    let refused = crate::diagnostics::logged_count(refused_line);

    let listed = call_as(&mut app, PID, "list_documents", json!({}));
    assert_eq!(listed["status"], "answered");
    call_as(&mut app, PID, "undo", json!({"document": doc("a.snxsch")}));

    assert_eq!(
        crate::diagnostics::logged_count(answered_line),
        answered + 1
    );
    assert_eq!(crate::diagnostics::logged_count(refused_line), refused + 1);
}

#[test]
fn query_pins_answers_on_the_agent_channel_and_changes_nothing() {
    use crate::app::handlers::menu::export::tests::current_placement::placed_twice_on_disk;

    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, _child) = placed_twice_on_disk(&dir);
    app.ui_state.agent_connections_enabled = true;
    app.agent.endpoint = Some("test-endpoint".into());
    // The project's sheet hierarchy lands with its first probe.
    crate::app::runtime::sheet_tree_tests::settle(&mut app, true);

    let reply = call(
        &mut app,
        "query_pins",
        json!({"document": top.display().to_string(), "designator": "R_TOP"}),
    );

    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(reply["data"]["count"], 1, "{reply}");
    assert_eq!(reply["changed"], json!([]));
}
