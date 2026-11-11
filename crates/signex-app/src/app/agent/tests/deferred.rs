//! #2199 (decision comment 6771692137): an agent request whose edit reads the
//! disk is answered when the edit has run on its reads, not when it arrives.

use serde_json::json;

use super::{AgentCall, AgentMsg, AgentReply, Input, app_in, line};
use crate::app::Message;

#[test]
fn a_request_that_reads_the_disk_is_answered_after_its_edit_ran() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(dir.path());
    let file = "b.snxsch";
    let doc = dir.path().join(file).display().to_string();
    let started = revision_in(&app, dir.path(), file);
    // The link check of a sheet symbol's file is read off the UI thread.
    let edits = json!([{"id": "place_sheet_symbol", "args": {
        "start": "100,100", "end": "120,120", "name": "Child", "file": "child.snxsch"
    }}]);
    let text = line(
        &app,
        "apply_edits",
        json!({"document": doc, "expect_revision": started, "edits": edits}),
    );
    let (reply, mut answer) = AgentReply::channel();
    let call = AgentCall {
        pid: Some(4243),
        input: Input::Line(text),
        reply,
    };
    let task = app.update(Message::Agent(AgentMsg::Request(call)));
    assert_eq!(
        answer.try_recv().expect("the reply is kept"),
        None,
        "no reply before the reads land"
    );
    assert_eq!(
        revision_in(&app, dir.path(), file),
        started,
        "nothing ran yet"
    );

    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the reads land");
    let reply = answer
        .try_recv()
        .expect("the reply was sent")
        .expect("answered once the edit ran");
    let reply: serde_json::Value = serde_json::from_str(&reply).expect("JSON");
    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(reply["data"]["changed"], true, "{reply}");
    assert_eq!(reply["changed"], json!([doc]), "{reply}");
    assert!(revision_in(&app, dir.path(), file) > started);
}

/// What happens while a request waits for its reads is not in its reply:
/// neither a document changed meanwhile nor a Messages line logged meanwhile.
#[test]
fn a_waiting_reply_reports_only_what_its_own_edit_did() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(dir.path());
    let b = dir.path().join("b.snxsch");
    let edits = json!([{"id": "place_sheet_symbol", "args": {
        "start": "100,100", "end": "120,120", "name": "Child", "file": "child.snxsch"
    }}]);
    let text = line(
        &app,
        "apply_edits",
        json!({"document": b.display().to_string(), "expect_revision": revision_in(&app, dir.path(), "b.snxsch"), "edits": edits}),
    );
    let (reply, mut answer) = AgentReply::channel();
    let call = AgentCall {
        pid: Some(4244),
        input: Input::Line(text),
        reply,
    };
    let waiting = app.update(Message::Agent(AgentMsg::Request(call)));

    // While the first one's reads run, a sheet of no project changes and
    // Messages gets a line that is not the waiting edit's.
    let loose = dir.path().join("elsewhere").join("loose.snxsch");
    crate::app::handlers::editing_commands::net_color_driver::add_sheet_with(
        &mut app,
        &loose,
        signex_types::schematic::SchematicSheet::default(),
    );
    let junction = signex_types::schematic::Junction {
        uuid: uuid::Uuid::now_v7(),
        position: signex_types::schematic::Point::new(1.0, 1.0),
        ..Default::default()
    };
    app.document_state
        .engines
        .get_mut(&loose)
        .unwrap()
        .execute(signex_engine::Command::PlaceJunction { junction })
        .expect("an edit elsewhere");
    crate::diagnostics::log_info("meanwhile, pid 42990 did something else");

    crate::task_driver::TaskDriver::new()
        .settle(&mut app, waiting)
        .expect("the reads land");
    let reply = answer.try_recv().unwrap().expect("answered");
    let reply: serde_json::Value = serde_json::from_str(&reply).expect("JSON");
    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(
        reply["changed"],
        json!([b.display().to_string()]),
        "{reply}"
    );
    let lines = reply["messages"].to_string();
    assert!(!lines.contains("pid 42990"), "{lines}");
}

/// Turning agent connections off while a request waits for its reads stops
/// it: the reply is refused, as a new request would be, and nothing applies.
#[test]
fn a_waiting_request_is_refused_once_agent_connections_are_turned_off() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(dir.path());
    let file = "b.snxsch";
    let doc = dir.path().join(file).display().to_string();
    let started = revision_in(&app, dir.path(), file);
    let edits = json!([{"id": "place_sheet_symbol", "args": {
        "start": "100,100", "end": "120,120", "name": "Child", "file": "child.snxsch"
    }}]);
    let text = line(
        &app,
        "apply_edits",
        json!({"document": doc, "expect_revision": started, "edits": edits}),
    );
    let (reply, mut answer) = AgentReply::channel();
    let call = AgentCall {
        pid: Some(4245),
        input: Input::Line(text),
        reply,
    };
    let task = app.update(Message::Agent(AgentMsg::Request(call)));
    app.ui_state.agent_connections_enabled = false;
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the reads land");
    let reply = answer.try_recv().unwrap().expect("answered");
    let reply: serde_json::Value = serde_json::from_str(&reply).expect("JSON");
    assert_eq!(reply["status"], "refused", "{reply}");
    assert_eq!(reply["reason"], "agent connections are off", "{reply}");
    assert_eq!(revision_in(&app, dir.path(), file), started);
}

/// Connections turned off and on again while a request waits give the app a
/// new token: the request's token no longer opens it.
#[test]
fn a_waiting_request_is_refused_once_the_session_token_changed() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(dir.path());
    let file = "b.snxsch";
    let doc = dir.path().join(file).display().to_string();
    let started = revision_in(&app, dir.path(), file);
    let edits = json!([{"id": "place_sheet_symbol", "args": {
        "start": "100,100", "end": "120,120", "name": "Child", "file": "child.snxsch"
    }}]);
    let text = line(
        &app,
        "apply_edits",
        json!({"document": doc, "expect_revision": started, "edits": edits}),
    );
    let (reply, mut answer) = AgentReply::channel();
    let call = AgentCall {
        pid: Some(4246),
        input: Input::Line(text),
        reply,
    };
    let task = app.update(Message::Agent(AgentMsg::Request(call)));
    app.agent.rotate_ids();
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the reads land");
    let reply = answer.try_recv().unwrap().expect("answered");
    let reply: serde_json::Value = serde_json::from_str(&reply).expect("JSON");
    assert_eq!(reply["status"], "refused", "{reply}");
    assert_eq!(
        reply["reason"], "the session token is missing or wrong",
        "{reply}"
    );
    assert_eq!(revision_in(&app, dir.path(), file), started);
}

/// A request that waited is journaled when its edit runs, once, as one that
/// did not wait: the outcome and revisions of that run, not of the wait.
#[test]
fn a_waiting_request_is_journaled_once_when_its_edit_ran() {
    use crate::app::agent::journal::{EditEntry, Journal, Report, read_journal};
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(dir.path());
    app.agent.journals_on = true;
    let file = "b.snxsch";
    let doc = dir.path().join(file).display().to_string();
    let started = revision_in(&app, dir.path(), file);
    let edits = json!([{"id": "place_sheet_symbol", "args": {
        "start": "100,100", "end": "120,120", "name": "Child", "file": "child.snxsch"
    }}]);
    let text = line(
        &app,
        "apply_edits",
        json!({"document": doc, "expect_revision": started, "edits": edits}),
    );
    let (reply, mut answer) = AgentReply::channel();
    let call = AgentCall {
        pid: Some(4247),
        input: Input::Line(text),
        reply,
    };
    let task = app.update(Message::Agent(AgentMsg::Request(call)));
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the reads land");
    let reply = answer.try_recv().unwrap().expect("answered");
    let reply: serde_json::Value = serde_json::from_str(&reply).expect("JSON");
    assert_eq!(reply["status"], "answered", "{reply}");

    let journal = dir.path().join(".signex").join(Journal::Edits.file_name());
    let report: Report<EditEntry> = read_journal(&journal).expect("the journal");
    let entries = report.entries;
    assert_eq!(entries.len(), 1, "{entries:?}");
    let entry = &entries[0];
    assert_eq!(entry.command, "apply_edits");
    assert_eq!(entry.outcome, "ran", "{entry:?}");
    assert_eq!(entry.pid, Some(4247));
    assert_eq!(entry.revision_before, Some(started));
    assert_eq!(
        entry.revision_after,
        Some(revision_in(&app, dir.path(), file))
    );
    assert_eq!(entry.group, reply["data"]["group"].as_u64(), "{reply}");
}

fn revision_in(app: &crate::app::Signex, dir: &std::path::Path, file: &str) -> u64 {
    app.document_state.engines[&dir.join(file)].revision()
}
