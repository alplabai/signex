//! #2199 step D (decision comment 6798772157): `check_erc` on the agent
//! channel is answered once, when its ERC run lands; it is a query, so it has
//! no edit journal line, and the person's view is not touched.

use serde_json::{Value, json};

use super::{AgentCall, AgentMsg, AgentReply, Input, app_in, line, snapshot};
use crate::app::Message;
use crate::app::agent::journal::Journal;

#[test]
fn a_check_erc_request_is_answered_once_when_its_run_lands() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(dir.path());
    app.agent.journals_on = true;
    let view = snapshot(&app);
    let doc = dir.path().join("b.snxsch").display().to_string();
    let text = line(&app, "check_erc", json!({"document": doc}));
    let (reply, mut answer) = AgentReply::channel();
    let call = AgentCall {
        pid: Some(4251),
        input: Input::Line(text),
        reply,
    };
    let task = app.update(Message::Agent(AgentMsg::Request(call)));
    assert_eq!(
        answer.try_recv().expect("the reply is kept"),
        None,
        "no reply before the run lands"
    );

    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run lands");
    let reply = answer
        .try_recv()
        .expect("the reply was sent")
        .expect("answered once the run landed");
    let reply: Value = serde_json::from_str(&reply).expect("JSON");
    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(reply["data"]["check"], "erc", "{reply}");
    assert_eq!(reply["data"]["run"], 1, "{reply}");
    assert_eq!(reply["changed"], json!([]), "{reply}");
    // Answered on the socket, so nothing is kept for a headless run.
    assert!(app.agent.waiting.is_empty());
    assert!(app.document_state.agent_edits.finished.is_empty());
    assert_eq!(snapshot(&app), view, "the person's view is untouched");
    let edits = dir.path().join(".signex").join(Journal::Edits.file_name());
    assert!(!edits.exists(), "a query has no edit line");
}

#[test]
fn a_check_erc_request_on_a_loose_sheet_is_refused_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(dir.path());
    let loose = dir.path().join("elsewhere").join("loose.snxsch");
    crate::app::handlers::editing_commands::net_color_driver::add_sheet_with(
        &mut app,
        &loose,
        signex_types::schematic::SchematicSheet::default(),
    );
    let text = line(
        &app,
        "check_erc",
        json!({"document": loose.display().to_string()}),
    );
    let reply = super::ask(&mut app, text);
    assert_eq!(reply["status"], "refused", "{reply}");
    assert_eq!(app.document_state.agent_edits.tickets, 0, "no run started");
}

/// Ask `check_erc` on `b` as process `pid`, let `meanwhile` change the app
/// while the run is under way, land the run and read the one reply.
fn check_with_change_meanwhile(
    pid: u32,
    meanwhile: impl FnOnce(&mut crate::app::Signex),
) -> (Value, crate::app::Signex, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(dir.path());
    let doc = dir.path().join("b.snxsch").display().to_string();
    let text = line(&app, "check_erc", json!({"document": doc}));
    let (reply, mut answer) = AgentReply::channel();
    let call = AgentCall {
        pid: Some(pid),
        input: Input::Line(text),
        reply,
    };
    let task = app.update(Message::Agent(AgentMsg::Request(call)));
    meanwhile(&mut app);
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run lands");
    let reply = answer.try_recv().unwrap().expect("answered");
    let reply: Value = serde_json::from_str(&reply).expect("JSON");
    (reply, app, dir)
}

/// A refused answer is not numbered or kept as a run.
fn no_run_kept(app: &crate::app::Signex, dir: &tempfile::TempDir) {
    let project = dir.path().join("P.snxprj");
    let why = app
        .document_state
        .check_runs
        .keys_of(1, "erc", &project)
        .unwrap_err();
    assert!(why.contains("no such run"), "{why}");
}

#[test]
fn a_check_erc_waiting_when_connections_go_off_is_refused_not_answered() {
    let (reply, app, dir) = check_with_change_meanwhile(4252, |app| {
        app.ui_state.agent_connections_enabled = false;
    });
    assert_eq!(reply["status"], "refused", "{reply}");
    assert_eq!(reply["reason"], "agent connections are off", "{reply}");
    assert!(reply.get("data").is_none(), "{reply}");
    no_run_kept(&app, &dir);
}

/// Connections turned off and on again while a check runs give the app a new
/// token: the request's token no longer opens it.
#[test]
fn a_check_erc_waiting_when_the_session_token_changed_is_refused_not_answered() {
    let (reply, app, dir) = check_with_change_meanwhile(4253, |app| {
        app.agent.rotate_ids();
    });
    assert_eq!(reply["status"], "refused", "{reply}");
    assert_eq!(
        reply["reason"], "the session token is missing or wrong",
        "{reply}"
    );
    assert!(reply.get("data").is_none(), "{reply}");
    no_run_kept(&app, &dir);
}
