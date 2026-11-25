//! #2495: the agent socket's `if_revision`. A request whose document is still
//! at the revision it names runs; once the document moved, the same request is
//! refused and changes nothing.

use serde_json::{Value, json};

use super::{AgentCall, AgentMsg, AgentReply, Input, app_at_work, app_in, ask, path, revision};
use crate::app::{Message, Signex};

fn doc(file: &str) -> String {
    path(file).display().to_string()
}

/// A request line for `id` with `args` and the top-level `if_revision`.
fn line_at(app: &Signex, id: &str, args: Value, if_revision: Value) -> String {
    let token = app.agent.token().expect("a token").as_str().to_string();
    json!({"token": token, "id": id, "args": args, "if_revision": if_revision}).to_string()
}

fn color_args(file: &str) -> Value {
    json!({"document": doc(file), "at": [5, 0], "color": "#336699"})
}

#[test]
fn a_request_at_its_revision_runs_and_the_same_request_after_it_moved_is_refused() {
    let mut app = app_at_work();
    let started = revision(&app, "b.snxsch");

    let first = line_at(&app, "color_net", color_args("b.snxsch"), json!(started));
    let ran = ask(&mut app, first);
    assert_eq!(ran["status"], "ran", "{ran}");
    let moved = revision(&app, "b.snxsch");
    assert!(moved > started, "the edit moved the revision");

    let again = line_at(&app, "color_net", color_args("b.snxsch"), json!(started));
    let refused = ask(&mut app, again);
    assert_eq!(refused["status"], "refused", "{refused}");
    let reason = refused["reason"].as_str().unwrap_or_default();
    assert!(
        reason.contains(&format!(
            "changed since revision {started} (it is at {moved})"
        )),
        "{reason}"
    );
    assert_eq!(refused["changed"], json!([]), "nothing ran");
    assert_eq!(revision(&app, "b.snxsch"), moved);
}

/// A request that waits for its reads meets `if_revision` again when they
/// land: an edit made meanwhile refuses it, with the revision check's reason,
/// and it changes nothing.
#[test]
fn a_waiting_request_whose_document_moved_meanwhile_is_refused_when_it_lands() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut app = app_in(dir.path());
    let b = dir.path().join("b.snxsch");
    let started = app.document_state.engines[&b].revision();
    // The link check of a sheet symbol's file is read off the UI thread.
    let edits = json!([{"id": "place_sheet_symbol", "args": {
        "start": "100,100", "end": "120,120", "name": "Child", "file": "child.snxsch"
    }}]);
    let args = json!({
        "document": b.display().to_string(),
        "expect_revision": started,
        "edits": edits,
    });
    let (reply, mut answer) = AgentReply::channel();
    let call = AgentCall {
        pid: Some(4247),
        input: Input::Line(line_at(&app, "apply_edits", args, json!(started))),
        reply,
    };
    let task = app.update(Message::Agent(AgentMsg::Request(call)));
    assert_eq!(answer.try_recv().expect("kept"), None, "it waits");

    // The person edits the document while the reads run.
    let junction = signex_types::schematic::Junction {
        uuid: uuid::Uuid::now_v7(),
        position: signex_types::schematic::Point::new(1.0, 1.0),
        ..Default::default()
    };
    app.document_state
        .engines
        .get_mut(&b)
        .expect("b is open")
        .execute(signex_engine::Command::PlaceJunction { junction })
        .expect("an edit meanwhile");
    let moved = app.document_state.engines[&b].revision();
    assert!(moved > started);

    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the reads land");
    let reply = answer.try_recv().expect("sent").expect("answered");
    let reply: Value = serde_json::from_str(&reply).expect("JSON");
    assert_eq!(reply["status"], "refused", "{reply}");
    let reason = reply["reason"].as_str().unwrap_or_default();
    assert!(
        reason.contains(&format!(
            "changed since revision {started} (it is at {moved})"
        )),
        "{reason}"
    );
    assert_eq!(app.document_state.engines[&b].revision(), moved);
}

/// An installed extension's command is not on the socket in version 1, and
/// the reply says so rather than calling it unknown.
#[test]
fn an_installed_extension_command_is_refused_on_the_socket_with_its_reason() {
    let (host, _seen) = crate::extension::test_extension::sample_host();
    let mut app = app_at_work().with_extensions(host);
    let id = crate::extension::test_extension::SAY_HELLO;
    assert!(app.extensions().command(id).is_some(), "it is installed");
    let token = app.agent.token().expect("a token").as_str().to_string();
    let text = json!({"token": token, "id": id, "args": {}}).to_string();
    let reply = ask(&mut app, text);
    assert_eq!(reply["status"], "refused", "{reply}");
    assert_eq!(
        reply["reason"], "extension commands are not open to the agent socket in version 1",
        "{reply}"
    );
}

#[test]
fn a_query_at_its_revision_is_answered() {
    let mut app = app_at_work();
    let at = revision(&app, "a.snxsch");
    let text = line_at(
        &app,
        "read_document",
        json!({"document": doc("a.snxsch")}),
        json!(at),
    );
    let read = ask(&mut app, text);
    assert_eq!(read["status"], "answered", "{read}");
    assert_eq!(read["data"]["revision"], at);
}

#[test]
fn an_if_revision_that_is_not_a_whole_number_is_a_malformed_request() {
    let mut app = app_at_work();
    let text = line_at(
        &app,
        "read_document",
        json!({"document": doc("a.snxsch")}),
        json!("3"),
    );
    let reply = ask(&mut app, text);
    assert_eq!(reply["status"], "refused", "{reply}");
    let reason = reply["reason"].as_str().unwrap_or_default();
    assert!(reason.starts_with("malformed request"), "{reason}");
}
