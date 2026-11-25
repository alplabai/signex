//! #2495: the agent socket's `if_revision`. A request whose document is still
//! at the revision it names runs; once the document moved, the same request is
//! refused and changes nothing.

use serde_json::{Value, json};

use super::{app_at_work, ask, path, revision};
use crate::app::Signex;

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
