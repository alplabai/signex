//! #2253: what an agent does and who connected is written to the project's
//! `.signex/` journals, off the update thread, and never in the way of the
//! command. The fixture is the agent channel's own (`app_in`), with journaling
//! on, over a folder of its own.

use serde_json::{Value, json};

use super::{AgentMsg, Input, app_in, ask_with_task, line};
use crate::app::agent::journal::{AccessEntry, EditEntry, Journal, Report, read_journal};
use crate::app::{Message, Signex};
use crate::task_driver::TaskDriver;

struct Fixture {
    dir: tempfile::TempDir,
    app: Signex,
    driver: TaskDriver,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(dir.path());
    app.agent.journals_on = true;
    Fixture {
        dir,
        app,
        driver: TaskDriver::new(),
    }
}

impl Fixture {
    fn doc(&self, file: &str) -> String {
        self.dir.path().join(file).display().to_string()
    }

    /// Run a request and drive what it left behind (the journal's write).
    fn ask(&mut self, id: &str, args: Value) -> Value {
        let text = line(&self.app, id, args);
        self.ask_line(text)
    }

    fn ask_line(&mut self, text: String) -> Value {
        let (reply, task) = ask_with_task(&mut self.app, Input::Line(text));
        self.driver.settle(&mut self.app, task).unwrap();
        reply
    }

    fn send(&mut self, message: AgentMsg) {
        let task = self.app.update(Message::Agent(message));
        self.driver.settle(&mut self.app, task).unwrap();
    }

    fn edits(&self) -> Report<EditEntry> {
        let path = self
            .dir
            .path()
            .join(".signex")
            .join(Journal::Edits.file_name());
        read_journal(&path).unwrap()
    }

    fn access(&self) -> Report<AccessEntry> {
        let path = self
            .dir
            .path()
            .join(".signex")
            .join(Journal::Access.file_name());
        read_journal(&path).unwrap()
    }

    fn revision(&self, file: &str) -> u64 {
        self.app.document_state.engines[&self.dir.path().join(file)].revision()
    }

    fn journal_text(&self) -> String {
        let mut all = String::new();
        for journal in [Journal::Edits, Journal::Access] {
            let path = self.dir.path().join(".signex").join(journal.file_name());
            all.push_str(&std::fs::read_to_string(path).unwrap_or_default());
        }
        all
    }
}

fn color(doc: &str, rgb: &str) -> Value {
    json!({"document": doc, "at": [5, 0], "color": rgb})
}

#[test]
fn an_agent_command_is_one_edit_line_that_replays_at_the_same_revision_only() {
    let mut f = fixture();
    let doc = f.doc("b.snxsch");
    let before = f.revision("b.snxsch");

    let reply = f.ask("color_net", color(&doc, "#336699"));

    assert_eq!(reply["status"], "ran", "{reply}");
    let report = f.edits();
    assert_eq!(report.entries.len(), 1);
    let entry = &report.entries[0];
    assert_eq!(entry.v, 1);
    assert_eq!(entry.command, "color_net");
    assert_eq!(entry.pid, Some(4242));
    assert_eq!(entry.outcome, "ran");
    assert_eq!(entry.path.as_deref(), Some(doc.as_str()));
    assert!(entry.document.is_some(), "the document's persistent id");
    assert_eq!(entry.revision_before, Some(before));
    assert_eq!(entry.revision_after, Some(f.revision("b.snxsch")));
    assert!(entry.revision_after > entry.revision_before);
    assert!(entry.group.is_some(), "the undo group of the step");
    assert_eq!(entry.operands["document"], doc);
    assert_eq!(entry.operands["color"], "#336699");
    assert!(entry.replayable && !entry.truncated);

    assert!(entry.replay_request(before).is_ok());
    assert!(entry.replay_request(before + 1).is_err());
}

#[test]
fn an_apply_edits_line_carries_the_edit_list_and_its_group() {
    let mut f = fixture();
    let doc = f.doc("b.snxsch");
    let revision = f.revision("b.snxsch");

    let reply = f.ask(
        "apply_edits",
        json!({
            "document": doc,
            "expect_revision": revision.to_string(),
            "edits": [{"id": "place_junction", "args": {"at": [20, 20]}}],
            "summary": "add a junction",
        }),
    );

    assert_eq!(reply["status"], "answered", "{reply}");
    let entry = f.edits().entries.remove(0);
    assert_eq!(entry.command, "apply_edits");
    assert_eq!(entry.group, reply["data"]["group"].as_u64());
    assert_eq!(entry.revision_before, Some(revision));
    assert_eq!(entry.revision_after, reply["data"]["revision"].as_u64());
    assert!(entry.operands["edits"].contains("place_junction"));
    assert_eq!(entry.operands["summary"], "add a junction");
    assert!(entry.replayable);
}

#[test]
fn a_refused_command_is_a_line_with_its_reason_and_a_query_is_none() {
    let mut f = fixture();
    let doc = f.doc("b.snxsch");

    // No colours to clear: the handler refuses.
    let refused = f.ask("clear_all_net_colors", json!({"document": doc}));
    // Queries are covered by the access record, not logged one by one.
    f.ask("list_documents", json!({}));
    f.ask("read_document", json!({"document": doc}));

    assert_eq!(refused["status"], "refused", "{refused}");
    let report = f.edits();
    assert_eq!(report.entries.len(), 1, "{:?}", report.entries);
    let entry = &report.entries[0];
    assert_eq!(entry.outcome, "refused");
    assert!(entry.reason.is_some());
    assert!(!entry.replayable);
    // The handler's refusal is an edit line, not an access event.
    assert!(f.access().entries.is_empty(), "{:?}", f.access().entries);
}

#[test]
fn the_persons_own_command_is_not_journaled() {
    let mut f = fixture();
    let id = crate::keymap::AppCommandId::new("color_net").unwrap();
    let operands = [
        "at=5,0".to_string(),
        "color=#336699".to_string(),
        format!("document={}", f.doc("b.snxsch")),
    ];
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = crate::app::command::CommandArgs::parse_operands(&id, &refs).unwrap();

    let (_outcome, task) = f.app.run_operands(&id, &args);
    f.driver.settle(&mut f.app, task).unwrap();

    assert!(!f.dir.path().join(".signex").exists());
}

#[test]
fn a_headless_agent_command_is_journaled_with_this_process_as_the_pid() {
    let mut f = fixture();
    let id = crate::keymap::AppCommandId::new("apply_edits").unwrap();
    let revision = f.revision("b.snxsch");
    let operands = [
        format!("document={}", f.doc("b.snxsch")),
        format!("expect_revision={revision}"),
        r#"edits=[{"id":"place_junction","args":{"at":[20,20]}}]"#.to_string(),
    ];
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = crate::app::command::CommandArgs::parse_operands(&id, &refs).unwrap();

    let (_outcome, task) = f.app.run_headless_command_in_test(&id, args);
    f.driver.settle(&mut f.app, task).unwrap();

    let entry = f.edits().entries.remove(0);
    assert_eq!(entry.pid, Some(std::process::id()));
    assert_eq!(entry.revision_before, Some(revision));
    assert!(entry.replayable);
}

#[test]
fn a_headless_command_that_changes_no_document_is_not_journaled() {
    let mut f = fixture();
    let id = crate::keymap::AppCommandId::new("select_all").unwrap();

    let (_outcome, task) = f
        .app
        .run_headless_command_in_test(&id, crate::app::command::CommandArgs::none());
    f.driver.settle(&mut f.app, task).unwrap();

    assert!(!f.dir.path().join(".signex").exists());
}

#[test]
fn with_journaling_off_nothing_is_created() {
    let mut f = fixture();
    f.app.agent.journals_on = false;
    let doc = f.doc("b.snxsch");

    f.ask("color_net", color(&doc, "#336699"));
    f.send(AgentMsg::ConnectionOpened { pid: Some(1) });

    assert!(!f.dir.path().join(".signex").exists());
}

#[test]
fn lines_land_in_the_order_the_events_happened() {
    let mut f = fixture();
    let doc = f.doc("b.snxsch");
    // Each colour is a new revision; ask without settling in between, so
    // several lines wait on the queue at once.
    let mut tasks = Vec::new();
    for n in 0..12_u32 {
        let text = line(&f.app, "color_net", color(&doc, &format!("#33669{n:x}")));
        let (reply, task) = ask_with_task(&mut f.app, Input::Line(text));
        assert_eq!(reply["status"], "ran", "{reply}");
        tasks.push(task);
    }
    for task in tasks {
        f.driver.settle(&mut f.app, task).unwrap();
    }

    let entries = f.edits().entries;

    assert_eq!(entries.len(), 12);
    let revisions: Vec<_> = entries.iter().filter_map(|e| e.revision_before).collect();
    assert!(revisions.windows(2).all(|w| w[0] < w[1]), "{revisions:?}");
    assert!(entries.windows(2).all(|w| w[0].ts <= w[1].ts));
    assert_eq!(f.app.agent.journal.outstanding(), 0);
}

#[test]
fn the_session_token_never_appears_in_either_journal() {
    let mut f = fixture();
    let token = f.app.agent.token().unwrap().as_str().to_string();
    let doc = f.doc("b.snxsch");

    f.ask("color_net", color(&doc, "#336699"));
    // An agent that puts the token into an operand (a summary) still leaves it
    // out of the journal.
    f.ask(
        "apply_edits",
        json!({
            "document": doc,
            "expect_revision": f.revision("b.snxsch").to_string(),
            "edits": [{"id": "place_junction", "args": {"at": [20, 20]}}],
            "summary": format!("token is {token}"),
        }),
    );
    // A wrong token, and a request that names a client and rights.
    f.ask_line(json!({"token": "wrong", "id": "color_net", "args": {}}).to_string());
    f.ask_line(
        json!({
            "token": token, "id": "list_documents", "args": {},
            "client": "signex-cli", "grants": ["read", "edit"],
        })
        .to_string(),
    );
    f.send(AgentMsg::ConnectionOpened { pid: Some(4242) });
    f.send(AgentMsg::ConnectionClosed { pid: Some(4242) });

    let text = f.journal_text();
    assert!(!text.is_empty());
    assert!(!text.contains(&token), "the token is in a journal");
    assert!(text.contains("[redacted]"), "the operand was redacted");
}

#[test]
fn connections_are_recorded_accepted_refused_and_closed_with_their_rights() {
    let mut f = fixture();
    let token = f.app.agent.token().unwrap().as_str().to_string();

    f.send(AgentMsg::ConnectionOpened { pid: Some(4242) });
    f.send(AgentMsg::PeerRefused {
        pid: Some(99),
        reason: "the peer is another user".into(),
    });
    let ok = |args: Value| {
        json!({"token": token, "id": "list_documents", "args": args,
               "client": "signex-cli", "grants": ["read", "edit", "save"]})
        .to_string()
    };
    // The same rights twice are recorded once.
    f.ask_line(ok(json!({})));
    f.ask_line(ok(json!({})));
    f.ask_line(json!({"token": "nope", "id": "list_documents", "args": {}}).to_string());
    f.send(AgentMsg::ConnectionClosed { pid: Some(4242) });

    let report = f.access();
    let events: Vec<_> = report.entries.iter().map(|e| e.event.as_str()).collect();
    assert_eq!(
        events,
        ["accepted", "refused", "grants", "refused", "closed"],
        "{:?}",
        report.entries
    );
    let refused = &report.entries[1];
    assert_eq!(refused.pid, Some(99));
    assert_eq!(refused.reason.as_deref(), Some("the peer is another user"));
    let grants = &report.entries[2];
    assert_eq!(grants.capabilities, ["read", "edit", "save"]);
    assert_eq!(grants.client.as_deref(), Some("signex-cli"));
    assert!(
        report.entries[3]
            .reason
            .as_deref()
            .is_some_and(|why| why.contains("token")),
        "{:?}",
        report.entries[3]
    );
    assert!(
        report
            .entries
            .iter()
            .all(|e| e.v == 1 && e.mode == "socket")
    );
}

#[test]
fn a_failing_journal_never_blocks_the_command_and_is_said_in_messages() {
    let mut f = fixture();
    // `.signex` is a file: no journal can be written.
    std::fs::write(f.dir.path().join(".signex"), "in the way").unwrap();
    let doc = f.doc("b.snxsch");
    let mark = crate::diagnostics::newest_entry_id();

    let reply = f.ask("color_net", color(&doc, "#336699"));

    assert_eq!(
        reply["status"], "ran",
        "the command answered as it would: {reply}"
    );
    assert!(f.revision("b.snxsch") > 0);
    let said: Vec<String> = crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .map(|entry| entry.message)
        .collect();
    assert!(
        said.iter()
            .any(|m| m.starts_with("Agent journal:") && m.contains(".signex")),
        "{said:?}"
    );
    // The queue moved on: the next line is not stuck behind the failure.
    assert_eq!(f.app.agent.journal.outstanding(), 0);
}

#[test]
fn the_projects_line_cap_is_the_cap_of_its_journals() {
    let mut f = fixture();
    f.app.document_state.projects[0]
        .data
        .agent_journal
        .max_lines = 100;
    let doc = f.doc("b.snxsch");
    for n in 0..110_u32 {
        let reply = f.ask("color_net", color(&doc, &format!("#33{:02x}99", n % 200)));
        assert_ne!(reply["status"], "failed", "{reply}");
    }

    let lines = std::fs::read_to_string(f.dir.path().join(".signex/agent-edits.jsonl"))
        .unwrap()
        .lines()
        .count();

    assert_eq!(lines, 100);
}
