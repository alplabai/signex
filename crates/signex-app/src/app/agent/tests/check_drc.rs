//! #2199 (decisions 6817821301, 6818011610): `check_drc` on the agent
//! channel is answered once, when its DRC run lands; the gate and the token
//! are checked again then; it says nothing in Messages about a file it does
//! not read.

use serde_json::{Value, json};

use super::{AgentCall, AgentMsg, AgentReply, Input, app_in, line, snapshot};
use crate::app::Message;
use crate::app::agent::journal::Journal;

/// [`app_in`] with the project listing `board.snxpcb`, written and closed.
fn app_with_board(dir: &std::path::Path) -> crate::app::Signex {
    let mut app = app_in(dir);
    let board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    let text = signex_types::format::SnxPcb::new(board)
        .write_string()
        .expect("encode");
    std::fs::write(dir.join("board.snxpcb"), text).expect("write board");
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    app
}

/// Today's limit, pinned so #2331 flips it: the held entry of a board open
/// in the app names no project (ownership follows listed sheets), so the
/// agent gate refuses a `check_drc` on it at once and no run starts.
#[test]
fn a_check_drc_on_a_board_open_in_the_app_is_refused_until_2331() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_board(dir.path());
    let path = dir.path().join("board.snxpcb");
    let board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    app.open_pcb_tab(path.clone(), "board.snxpcb".into(), board);
    let text = line(
        &app,
        "check_drc",
        json!({"document": path.display().to_string()}),
    );
    let reply = super::ask(&mut app, text);
    assert_eq!(reply["status"], "refused", "{reply}");
    assert_eq!(
        reply["reason"], "board.snxpcb is not part of a project open in this app",
        "{reply}"
    );
    assert_eq!(app.document_state.agent_edits.tickets, 0, "no run started");
}

/// Ask `check_drc` on the board as process `pid`, let `meanwhile` change
/// the app while the run is under way, land it and read the one reply.
fn check_with_change_meanwhile(
    pid: u32,
    meanwhile: impl FnOnce(&mut crate::app::Signex),
) -> (Value, crate::app::Signex, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_board(dir.path());
    let doc = dir.path().join("board.snxpcb").display().to_string();
    let text = line(&app, "check_drc", json!({"document": doc}));
    let (reply, mut answer) = AgentReply::channel();
    let call = AgentCall {
        pid: Some(pid),
        input: Input::Line(text),
        reply,
    };
    let task = app.update(Message::Agent(AgentMsg::Request(call)));
    assert_eq!(
        answer.try_recv().expect("the reply is kept"),
        None,
        "no reply before the run lands"
    );
    meanwhile(&mut app);
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run lands");
    let reply = answer.try_recv().unwrap().expect("answered");
    let reply: Value = serde_json::from_str(&reply).expect("JSON");
    (reply, app, dir)
}

#[test]
fn a_check_drc_request_is_answered_once_when_its_run_lands() {
    let mut view = None;
    let (reply, app, dir) = check_with_change_meanwhile(4261, |app| {
        app.agent.journals_on = true;
        view = Some(snapshot(app));
    });
    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(reply["data"]["check"], "drc", "{reply}");
    assert_eq!(reply["data"]["run"], 1, "{reply}");
    assert_eq!(reply["changed"], json!([]), "{reply}");
    assert!(app.agent.waiting.is_empty());
    assert!(app.document_state.agent_edits.finished.is_empty());
    assert_eq!(Some(snapshot(&app)), view, "the person's view is untouched");
    let edits = dir.path().join(".signex").join(Journal::Edits.file_name());
    assert!(!edits.exists(), "a query has no edit line");
}

/// A refused answer is not numbered or kept as a run.
fn no_run_kept(app: &crate::app::Signex, dir: &tempfile::TempDir) {
    let board = dir.path().join("board.snxpcb");
    let why = app
        .document_state
        .check_runs
        .keys_of(1, "drc", &board)
        .unwrap_err();
    assert!(why.contains("no such run"), "{why}");
}

#[test]
fn a_check_drc_waiting_when_connections_go_off_is_refused_not_answered() {
    let (reply, app, dir) = check_with_change_meanwhile(4262, |app| {
        app.ui_state.agent_connections_enabled = false;
    });
    assert_eq!(reply["status"], "refused", "{reply}");
    assert_eq!(reply["reason"], "agent connections are off", "{reply}");
    assert!(reply.get("data").is_none(), "{reply}");
    no_run_kept(&app, &dir);
}

#[test]
fn a_check_drc_waiting_when_the_session_token_changed_is_refused_not_answered() {
    let (reply, app, dir) = check_with_change_meanwhile(4263, |app| {
        app.agent.rotate_ids();
    });
    assert_eq!(reply["status"], "refused", "{reply}");
    assert_eq!(
        reply["reason"], "the session token is missing or wrong",
        "{reply}"
    );
    no_run_kept(&app, &dir);
}

/// The header read before a check is quiet for `check_drc` too: a page the
/// project names out of its folder is not named in Messages.
#[test]
fn a_check_drc_says_nothing_in_messages_about_a_page_it_does_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let name = format!("outside-{}.snxsch", uuid::Uuid::now_v7().simple());
    let outside = dir.path().parent().unwrap().join(&name);
    std::fs::write(&outside, "not read").unwrap();
    let mut app = app_with_board(dir.path());
    app.document_state.projects[0]
        .data
        .sheets
        .push(signex_types::project::SheetEntry {
            name: "Outside".into(),
            filename: format!("../{name}"),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        });
    let mark = crate::diagnostics::newest_entry_id();
    let doc = dir.path().join("board.snxpcb").display().to_string();
    let text = line(&app, "check_drc", json!({"document": doc}));
    let (reply, mut answer) = AgentReply::channel();
    let call = AgentCall {
        pid: Some(4264),
        input: Input::Line(text),
        reply,
    };
    let task = app.update(Message::Agent(AgentMsg::Request(call)));
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run lands");
    let reply: Value = serde_json::from_str(&answer.try_recv().unwrap().unwrap()).unwrap();
    std::fs::remove_file(&outside).ok();
    assert_eq!(reply["status"], "answered", "{reply}");
    let said: Vec<String> = crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .map(|e| e.message)
        .filter(|m| m.contains(&name))
        .collect();
    assert_eq!(said, Vec::<String>::new());
}
