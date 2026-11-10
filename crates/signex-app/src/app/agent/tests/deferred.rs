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

fn revision_in(app: &crate::app::Signex, dir: &std::path::Path, file: &str) -> u64 {
    app.document_state.engines[&dir.join(file)].revision()
}
