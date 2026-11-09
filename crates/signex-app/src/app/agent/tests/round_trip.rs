//! A query and an `apply_edits` arrive on the agent channel while the person is
//! in the middle of something: the change lands, and nothing the person holds
//! moves.

use serde_json::json;

use super::{app_at_work, call, path, revision, snapshot, uid};
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
    let mut app = app_at_work();
    let answered_line = "Agent request (pid 4242): `list_documents` -> answered";
    let refused_line = "Agent request (pid 4242): `undo` -> refused";
    let answered = crate::diagnostics::logged_count(answered_line);
    let refused = crate::diagnostics::logged_count(refused_line);

    let listed = call(&mut app, "list_documents", json!({}));
    assert_eq!(listed["status"], "answered");
    call(&mut app, "undo", json!({"document": doc("a.snxsch")}));

    assert_eq!(
        crate::diagnostics::logged_count(answered_line),
        answered + 1
    );
    assert_eq!(crate::diagnostics::logged_count(refused_line), refused + 1);
}
