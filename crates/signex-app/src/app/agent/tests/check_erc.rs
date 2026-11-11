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
