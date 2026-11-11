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

    let session = f.app.agent.session.clone();
    assert_eq!(entry.session, session);
    assert!(entry.replay_request(&session, before).is_ok());
    assert!(entry.replay_request(&session, before + 1).is_err());
}

/// An engine's revision counter starts again in every process, so a line of
/// one session must not be replayed in another even at the number it was
/// recorded at (the reviewer's `a_later_session_does_not_reuse...`).
#[test]
fn a_line_of_session_one_is_refused_in_session_two_at_the_same_revision() {
    let mut one = fixture();
    let before = one.revision("b.snxsch");
    one.ask("color_net", color(&one.doc("b.snxsch"), "#336699"));
    let entry = one.edits().entries.remove(0);

    // A second app is another session. Across processes the counter restarts,
    // so the same number can come up again: offer exactly that number.
    let two = fixture();
    assert_ne!(one.app.agent.session, two.app.agent.session);

    let refused = entry
        .replay_request(&two.app.agent.session, before)
        .unwrap_err();
    assert!(refused.contains("another session"), "{refused}");
    assert!(entry.replay_request(&one.app.agent.session, before).is_ok());
}

#[test]
fn a_revert_line_is_journaled_and_not_replayable() {
    let mut f = fixture();
    let doc = f.doc("a.snxsch");
    f.ask("color_net", color(&doc, "#336699"));
    let group = f.edits().entries[0].group.expect("a group");

    let reply = f.ask("revert", json!({"document": doc, "group": group}));

    assert_eq!(reply["status"], "answered", "{reply}");
    let lines = f.edits().entries;
    let revert = lines
        .iter()
        .find(|e| e.command == "revert")
        .expect("a revert line");
    assert!(!revert.replayable);
    assert!(
        revert
            .replay_request(&f.app.agent.session, revert.revision_before.unwrap())
            .is_err()
    );
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

/// #2178: a call across sheets is one line per sheet it touched, each with its
/// own path and revisions and all with the one group; one line alone cannot
/// replay the all-or-nothing group, so none is replayable.
#[test]
fn an_apply_edits_across_sheets_is_one_line_per_sheet_sharing_its_group() {
    let mut f = fixture();
    let (a, b) = (f.doc("a.snxsch"), f.doc("b.snxsch"));
    let (before_a, before_b) = (f.revision("a.snxsch"), f.revision("b.snxsch"));

    let reply = f.ask(
        "apply_edits",
        json!({
            "document": a,
            "expect_revision": before_a.to_string(),
            "expect_revisions": {b.clone(): before_b},
            "edits": [
                {"id": "place_junction", "args": {"at": [20, 20]}},
                {"id": "place_junction", "args": {"at": [20, 20], "document": b}},
            ],
        }),
    );

    assert_eq!(reply["status"], "answered", "{reply}");
    let group = reply["data"]["group"].as_u64();
    assert!(group.is_some(), "{reply}");
    let entries = f.edits().entries;
    assert_eq!(entries.len(), 2, "one line per sheet");
    for (entry, (path, before, file)) in entries
        .iter()
        .zip([(&a, before_a, "a.snxsch"), (&b, before_b, "b.snxsch")])
    {
        assert_eq!(entry.command, "apply_edits");
        assert_eq!(entry.path.as_deref(), Some(path.as_str()));
        assert_eq!(entry.group, group);
        assert_eq!(entry.revision_before, Some(before));
        assert_eq!(entry.revision_after, Some(f.revision(file)));
        assert!(!entry.replayable, "{file}");
    }
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
    let doc = f.doc("b.snxsch");
    let ok = |args: Value| {
        json!({"token": token, "id": "document_info", "args": args,
               "client": "signex-cli", "grants": ["read", "edit", "save"]})
        .to_string()
    };
    // The same rights twice are recorded once, on the project the request names.
    f.ask_line(ok(json!({"document": doc})));
    f.ask_line(ok(json!({"document": doc})));
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

    // Never past the cap; cut to 90 % when it passed it.
    assert!((90..=100).contains(&lines), "{lines}");
}

/// A second open project, in a folder of its own, with one sheet loaded.
fn open_second_project(f: &mut Fixture) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let mut data = f.app.document_state.projects[0].data.clone();
    data.name = "Q".into();
    data.dir = dir.path().to_string_lossy().into_owned();
    data.document_id = Some(signex_types::document_id::DocumentId::new());
    let id = f.app.document_state.mint_project_id();
    f.app
        .document_state
        .projects
        .push(crate::app::state::LoadedProject {
            id,
            path: dir.path().join("Q.snxprj"),
            data,
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
    dir
}

/// `text` as it is written inside a JSON string (backslashes doubled).
fn json_escaped(text: &str) -> String {
    let quoted = serde_json::to_string(text).expect("a string encodes");
    quoted.trim_matches('"').to_string()
}

fn journals_text(dir: &std::path::Path) -> String {
    let mut all = String::new();
    for journal in [Journal::Edits, Journal::Access] {
        let path = dir.join(".signex").join(journal.file_name());
        all.push_str(&std::fs::read_to_string(path).unwrap_or_default());
    }
    all
}

/// The client's name is cut to 1,024 characters in both journals: a token that
/// straddles the cut must not survive as a prefix. The token is replaced before
/// the cut (the replace on the encoded line no longer holds the whole token
/// after it), whatever the padding puts at the boundary.
#[test]
fn a_token_straddling_the_length_cut_leaves_no_prefix_in_either_journal() {
    let mut f = fixture();
    let token = f.app.agent.token().unwrap().as_str().to_string();
    let doc = f.doc("b.snxsch");

    for pad in 990..1_030 {
        let client = format!("{}{token}", "z".repeat(pad));
        // An access line (the rights of this client) and an edit line.
        for (id, args) in [
            ("document_info", json!({"document": doc})),
            // A new colour each time, so the command runs and is journaled.
            ("color_net", color(&doc, &format!("#{:02x}6699", pad - 900))),
        ] {
            let reply = f.ask_line(
                json!({
                    "token": token, "id": id, "args": args,
                    "client": client, "grants": ["read"],
                })
                .to_string(),
            );
            assert_ne!(reply["status"], "refused", "{reply}");
        }
    }

    let text = f.journal_text();
    assert!(text.contains("zzzzzzzz"), "the lines were written");
    assert!(
        !text.contains(&token[..8]),
        "a prefix of the token is in a journal"
    );
}

/// The token is replaced once, on the encoded line, so no field of either
/// journal can carry it: not a refused operand echoed in the reason, not the
/// client's name, not a right.
#[test]
fn the_token_in_a_refused_operand_or_the_client_name_is_not_in_the_access_journal() {
    let mut f = fixture();
    let token = f.app.agent.token().unwrap().as_str().to_string();
    let doc = f.doc("b.snxsch");

    // `at` is not a point: the request is turned away and the reason echoes it.
    let reply = f.ask_line(
        json!({
            "token": token, "id": "color_net",
            "args": {"document": doc, "at": token, "color": "#336699"},
            "client": token, "grants": [token, "read"],
        })
        .to_string(),
    );

    assert_eq!(reply["status"], "refused", "{reply}");
    let access = f.access();
    let refused = access
        .entries
        .iter()
        .find(|e| e.event == "refused")
        .expect("a refused line");
    assert!(
        refused
            .reason
            .as_deref()
            .is_some_and(|why| why.contains("[redacted]")),
        "{refused:?}"
    );
    assert!(
        !f.journal_text().contains(&token),
        "the token is in a journal"
    );
}

/// Two open projects: an event that names one project, or carries a path,
/// never reaches the other.
#[test]
fn one_projects_journal_never_names_another_projects_place() {
    let mut f = fixture();
    let b = open_second_project(&mut f);
    let a_dir = f.dir.path().display().to_string();
    let token = f.app.agent.token().unwrap().as_str().to_string();
    let doc = f.doc("b.snxsch");
    let inside = format!("allow-path:{a_dir}/exports");

    f.send(AgentMsg::ConnectionOpened { pid: Some(4242) });
    f.send(AgentMsg::PeerRefused {
        pid: Some(99),
        reason: format!("the peer at {a_dir}/sock is another user"),
    });
    f.ask_line(
        json!({"token": token, "id": "document_info", "args": {"document": doc},
            "client": "signex-cli",
            "grants": ["read", "edit", inside, "allow-path:/somewhere/else"]})
        .to_string(),
    );
    f.send(AgentMsg::ConnectionClosed { pid: Some(4242) });

    // B saw the connection and nothing that is A's.
    let b_text = journals_text(b.path());
    // A journal is JSON: on Windows a path is written with doubled backslashes,
    // so look for the raw and the escaped form, and in the parsed entries too.
    for form in [a_dir.clone(), json_escaped(&a_dir)] {
        assert!(
            !b_text.contains(&form),
            "B names A's place ({form}): {b_text}"
        );
    }
    let b_access: Report<AccessEntry> =
        read_journal(&b.path().join(".signex").join(Journal::Access.file_name())).unwrap();
    assert!(
        b_access.entries.iter().all(|e| {
            let texts = e
                .reason
                .iter()
                .chain(e.client.iter())
                .chain(&e.capabilities);
            texts.into_iter().all(|text| !text.contains(&a_dir))
        }),
        "B's parsed entries name A's place: {:?}",
        b_access.entries
    );
    assert!(b_text.contains(r#""event":"accepted""#));
    assert!(b_text.contains(r#""event":"closed""#));
    assert!(
        !b_text.contains(r#""event":"grants""#),
        "rights go to the project named only"
    );
    // A got the rights, with the roots inside it and a count of the others.
    let grants = f
        .access()
        .entries
        .into_iter()
        .find(|e| e.event == "grants")
        .expect("A's grants");
    assert!(
        grants.capabilities.contains(&inside),
        "{:?}",
        grants.capabilities
    );
    assert!(
        !grants
            .capabilities
            .iter()
            .any(|c| c.contains("/somewhere/else")),
        "{:?}",
        grants.capabilities
    );
    assert!(
        grants
            .capabilities
            .iter()
            .any(|c| c.contains("1 outside this project not recorded")),
        "{:?}",
        grants.capabilities
    );
    // The peer's reason names A's folder in A's own journal, scrubbed in B's.
    assert!(
        f.access()
            .entries
            .iter()
            .any(|e| e.reason.as_deref().is_some_and(|why| why.contains(&a_dir))),
        "A's own journal lost the peer's reason"
    );
    assert!(b_text.contains("[another project]"), "{b_text}");
}

/// A project opened while a connection lasts has no `accepted` for it, so it
/// gets no orphan `closed`; one closed meanwhile is fine without its `closed`.
#[test]
fn a_project_opened_or_closed_during_a_connection_gets_no_orphan_line() {
    let mut f = fixture();
    f.send(AgentMsg::ConnectionOpened { pid: Some(7) });
    let b = open_second_project(&mut f);
    f.send(AgentMsg::ConnectionClosed { pid: Some(7) });

    let b_text = journals_text(b.path());
    assert!(
        !b_text.contains(r#""event":"closed""#),
        "an orphan closed: {b_text}"
    );
    let events: Vec<_> = f.access().entries.iter().map(|e| e.event.clone()).collect();
    assert_eq!(events, ["accepted", "closed"]);

    // Closed during the connection: the other still gets both, nothing breaks.
    f.send(AgentMsg::ConnectionOpened { pid: Some(8) });
    f.app.document_state.projects.pop();
    f.send(AgentMsg::ConnectionClosed { pid: Some(8) });
    let events: Vec<_> = f.access().entries.iter().map(|e| e.event.clone()).collect();
    assert_eq!(events, ["accepted", "closed", "accepted", "closed"]);
    assert!(
        f.app.agent.connections.is_empty(),
        "no connection left tracked"
    );
}

/// A queued line is on disk when the app quits: the quit path writes what had
/// not started and waits for what had. The exit drops every message after it,
/// so a line left to the usual continuation would be lost.
#[test]
fn a_line_still_queued_at_quit_is_on_disk_after_it() {
    let mut f = fixture();
    let doc = f.doc("b.snxsch");
    // The first request starts a write that has not run yet; the second waits
    // behind it on the queue.
    let mut tasks = Vec::new();
    for rgb in ["#336699", "#336698"] {
        let text = line(&f.app, "color_net", color(&doc, rgb));
        let (reply, task) = ask_with_task(&mut f.app, Input::Line(text));
        assert_eq!(reply["status"], "ran", "{reply}");
        tasks.push(task);
    }
    assert_eq!(f.app.agent.journal.outstanding(), 1, "one waits");

    tasks.push(f.app.flush_quit_writes_then(iced::exit()));
    // One run for all of them, as the runtime does: once the exit lands, the
    // messages still queued are dropped, so only the barrier can save a line.
    f.driver
        .settle(&mut f.app, iced::Task::batch(tasks))
        .unwrap();

    assert_eq!(f.edits().entries.len(), 2, "both lines are on disk");
    assert_eq!(f.app.agent.journal.outstanding(), 0);
}

/// A command that names no document, with two projects open, has nowhere to
/// go: said in Messages with the command and both projects.
#[test]
fn an_unplaceable_edit_line_names_the_command_and_both_projects() {
    let mut f = fixture();
    let b = open_second_project(&mut f);
    let id = crate::keymap::AppCommandId::new("color_net").unwrap();
    let mark = crate::diagnostics::newest_entry_id();

    let (_outcome, task) = f
        .app
        .with_origin(crate::app::agent::Origin::agent(Some(5)), |app| {
            app.journaled(&id, &crate::app::command::CommandArgs::none(), |_| {
                (crate::app::command::CommandOutcome::Ran, iced::Task::none())
            })
        });
    f.driver.settle(&mut f.app, task).unwrap();

    let said: Vec<String> = crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .map(|entry| entry.message)
        .collect();
    let line = said
        .iter()
        .find(|m| m.starts_with("Agent journal:") && m.contains("color_net"))
        .unwrap_or_else(|| panic!("{said:?}"));
    // Both projects by their file names (a whole path is longer than a
    // Messages line keeps).
    assert!(
        line.contains("P.snxprj") && line.contains("Q.snxprj"),
        "{line}"
    );
    drop(b);
    assert!(line.contains("2 projects are open"), "{line}");
}

/// Another project's folder is replaced by component: `/w/P` goes from
/// `/w/P/y`, not from `/w/P2/x`.
#[test]
fn another_projects_folder_is_replaced_by_path_component_not_by_prefix() {
    use crate::app::agent::journal::record::without_other_projects;
    let own = std::path::Path::new("/w/Own");
    let all = [std::path::PathBuf::from("/w/P/P.snxprj")];

    let out = without_other_projects("see /w/P2/x and /w/P/y and /w/P", own, &all);

    assert_eq!(
        out,
        "see /w/P2/x and [another project]/y and [another project]"
    );
}

/// A folder name does not hide behind punctuation: a trailing `.`, `,`, `)` or
/// quote ends the path and it is redacted; `.`, `-` and `_` continue a name
/// only when another name character follows.
#[test]
fn another_projects_folder_is_redacted_before_trailing_punctuation_only() {
    use crate::app::agent::journal::record::without_other_projects;
    let own = std::path::Path::new("/w/Own");
    let all = [std::path::PathBuf::from("/w/P/P.snxprj")];
    let redact = |text: &str| without_other_projects(text, own, &all);

    assert_eq!(redact("it is in /w/P."), "it is in [another project].");
    assert_eq!(redact(r#"said "/w/P""#), r#"said "[another project]""#);
    assert_eq!(
        redact("(/w/P), /w/P; /w/P_"),
        "([another project]), [another project]; [another project]_"
    );
    // Kept: a longer name that merely starts with it.
    assert_eq!(redact("/w/P2/x"), "/w/P2/x");
    assert_eq!(
        redact("/w/P-old/x and /w/P.bak/x and /w/P_v2"),
        "/w/P-old/x and /w/P.bak/x and /w/P_v2"
    );
}

/// Two open projects whose folders start alike: the longer is handled first.
#[test]
fn the_longer_folder_is_redacted_before_the_shorter_one() {
    use crate::app::agent::journal::record::without_other_projects;
    let own = std::path::Path::new("/w/Own");
    let all = [
        std::path::PathBuf::from("/w/P/P.snxprj"),
        std::path::PathBuf::from("/w/P2/Q.snxprj"),
    ];

    let out = without_other_projects("/w/P2/x then /w/P/y", own, &all);

    assert_eq!(out, "[another project]/x then [another project]/y");
}

/// A connection still open when the app quits ends with it: each project that
/// saw it open gets its `closed` line, with the reason.
#[test]
fn a_connection_open_at_quit_gets_a_closed_line() {
    let mut f = fixture();
    f.send(AgentMsg::ConnectionOpened { pid: Some(9) });

    let quit = f.app.flush_quit_writes_then(iced::exit());
    f.driver.settle(&mut f.app, quit).unwrap();

    let entries = f.access().entries;
    let events: Vec<_> = entries.iter().map(|e| e.event.as_str()).collect();
    assert_eq!(events, ["accepted", "closed"], "{entries:?}");
    assert_eq!(entries[1].reason.as_deref(), Some("app quit"));
    assert!(f.app.agent.connections.is_empty());
}

/// What a project may see of a client's rights.
#[test]
fn rights_within_keeps_inside_roots_and_counts_the_rest() {
    use crate::app::agent::journal::record::rights_within;
    let dir = std::path::Path::new("/work/P");
    let rights = [
        "read".to_string(),
        "allow-path:/work/P/out".into(),
        "allow-path:/etc".into(),
        "allow-path:/home/x".into(),
    ];
    assert_eq!(
        rights_within(&rights, dir),
        [
            "read",
            "allow-path:/work/P/out",
            "allow-path:(2 outside this project not recorded)"
        ]
    );
    // Nothing dropped, nothing counted.
    assert_eq!(rights_within(&["edit".to_string()], dir), ["edit"]);
}
