//! #2339: `check_requirements` on the agent channel is answered once its run
//! lands when it names a sheet of the project; the project file itself is
//! refused at the agent gate, as for every command.

use serde_json::{Value, json};

use super::{AgentCall, AgentMsg, AgentReply, Input, app_in, line};
use crate::app::Message;

/// Ask `check_requirements` on `path` as process `pid`, land the run and
/// read the one reply.
fn check_settled(app: &mut crate::app::Signex, pid: u32, path: &std::path::Path) -> Value {
    let text = line(
        app,
        "check_requirements",
        json!({"document": path.display().to_string()}),
    );
    let (reply, mut answer) = AgentReply::channel();
    let call = AgentCall {
        pid: Some(pid),
        input: Input::Line(text),
        reply,
    };
    let task = app.update(Message::Agent(AgentMsg::Request(call)));
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the run lands");
    let reply = answer.try_recv().unwrap().expect("answered");
    serde_json::from_str(&reply).expect("JSON")
}

#[test]
fn a_check_requirements_on_a_sheet_of_the_project_is_answered() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(dir.path());
    app.document_state.projects[0].data.requirements = serde_json::from_value(json!([
        {"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 80},
    ]))
    .expect("requirements");
    let reply = check_settled(&mut app, 4271, &dir.path().join("a.snxsch"));
    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(reply["data"]["check"], "requirements", "{reply}");
    assert_eq!(
        reply["data"]["findings"][0]["message"], "The project lists no board",
        "{reply}"
    );
}

#[test]
fn a_check_requirements_naming_the_project_file_is_refused_at_the_gate() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(dir.path());
    let reply = check_settled(&mut app, 4272, &dir.path().join("P.snxprj"));
    assert_eq!(reply["status"], "refused", "{reply}");
    let reason = reply["reason"].as_str().unwrap_or_default();
    assert!(
        reason.contains("is not part of a project open in this app"),
        "{reason}"
    );
}
