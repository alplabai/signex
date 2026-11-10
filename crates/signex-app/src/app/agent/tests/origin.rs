//! #2253: a command knows whether the person or an agent runs it, and every
//! agent command that changes a document is one labelled undo step the
//! person's Ctrl+Z takes and `revert` can take back.

use serde_json::{Value, json};

use super::{app_at_work, call, call_as, path};
use crate::app::Signex;
use crate::app::agent::Origin;
use crate::app::handlers::editing_commands::net_color_driver as fixture;

const PID: u32 = 7771;

fn doc(file: &str) -> String {
    path(file).display().to_string()
}

fn color_args(file: &str) -> Value {
    json!({"document": doc(file), "at": [5, 0], "color": "#336699"})
}

fn net_colors(app: &Signex, file: &str) -> usize {
    app.document_state.engines[&path(file)]
        .document()
        .net_colors
        .len()
}

fn undo_position(app: &Signex, file: &str) -> usize {
    app.document_state.engines[&path(file)].undo_position()
}

fn top_group(app: &Signex, file: &str) -> Option<signex_engine::UndoGroupId> {
    app.document_state.engines[&path(file)].next_undo_group()
}

fn said_since(mark: Option<u64>) -> Vec<String> {
    crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .map(|entry| entry.message)
        .collect()
}

#[test]
fn an_agents_color_net_is_one_labelled_step_the_persons_undo_takes() {
    let mut app = app_at_work();
    let depth = undo_position(&app, "b.snxsch");

    let reply = call_as(&mut app, PID, "color_net", color_args("b.snxsch"));

    assert_eq!(reply["status"], "ran", "{reply}");
    assert!(net_colors(&app, "b.snxsch") > 0);
    assert_eq!(undo_position(&app, "b.snxsch"), depth + 1, "one step");
    let group = top_group(&app, "b.snxsch").expect("the step has a group");
    let edits = &app.document_state.agent_edits;
    assert_eq!(
        edits.labels.get(&group).map(String::as_str),
        Some("Agent: color_net on b.snxsch")
    );
    assert_eq!(edits.origins.get(&group), Some(&Origin::agent(Some(PID))));
    // The reply says the group, so the agent can name it in `revert`.
    assert!(
        reply["messages"]
            .to_string()
            .contains(&format!("undo group {}", group.get())),
        "{reply}"
    );

    // The person's Ctrl+Z takes exactly that step, and Messages says whose.
    let mark = crate::diagnostics::newest_entry_id();
    assert!(app.apply_engine_undo(false));
    assert_eq!(net_colors(&app, "b.snxsch"), 0);
    assert_eq!(undo_position(&app, "b.snxsch"), depth);
    assert!(
        said_since(mark)
            .iter()
            .any(|m| m == "Undone: Agent: color_net on b.snxsch (agent, pid 7771)"),
        "{:?}",
        said_since(mark)
    );
}

#[test]
fn revert_takes_back_a_single_command_step_and_refuses_what_is_not_the_top_agent_step() {
    // Sheet `a`: the person is drawing on `b`, which would hold a revert.
    const FILE: &str = "a.snxsch";
    let mut app = app_at_work();
    call(&mut app, "color_net", color_args(FILE));
    let group = top_group(&app, FILE).expect("a group");

    let reply = call(
        &mut app,
        "revert",
        json!({"document": doc(FILE), "group": group.get()}),
    );

    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(reply["data"]["reverted"], true);
    assert_eq!(net_colors(&app, FILE), 0);
    assert!(app.document_state.agent_edits.labels.is_empty());
    assert!(app.document_state.agent_edits.origins.is_empty());

    // The person's step is not an agent's: it has no group to name, and an
    // agent step under it is no longer the newest.
    call(&mut app, "color_net", color_args(FILE));
    let buried = top_group(&app, FILE).expect("a group");
    app.execute_on_sheet(
        &path(FILE),
        signex_engine::Command::SetNetColors {
            colors: Default::default(),
        },
    )
    .expect("the person clears the colours");
    assert_eq!(top_group(&app, FILE), None, "the person's step");
    let depth = undo_position(&app, FILE);
    for group in [buried.get(), 424_242] {
        let refused = call(
            &mut app,
            "revert",
            json!({"document": doc(FILE), "group": group}),
        );
        assert_eq!(refused["status"], "refused", "{refused}");
    }
    assert_eq!(undo_position(&app, FILE), depth, "nothing moved");
}

#[test]
fn clear_all_net_colors_is_a_labelled_step_like_color_net() {
    let mut app = app_at_work();
    call(&mut app, "color_net", color_args("b.snxsch"));
    let depth = undo_position(&app, "b.snxsch");

    let reply = call(
        &mut app,
        "clear_all_net_colors",
        json!({"document": doc("b.snxsch")}),
    );

    assert_eq!(reply["status"], "ran", "{reply}");
    assert_eq!(net_colors(&app, "b.snxsch"), 0);
    assert_eq!(undo_position(&app, "b.snxsch"), depth + 1);
    let group = top_group(&app, "b.snxsch").expect("a group");
    assert_eq!(
        app.document_state
            .agent_edits
            .labels
            .get(&group)
            .map(String::as_str),
        Some("Agent: clear_all_net_colors on b.snxsch")
    );
    assert!(app.apply_engine_undo(false));
    assert!(net_colors(&app, "b.snxsch") > 0, "the colour is back");
}

#[test]
fn a_command_that_makes_no_undo_step_gets_no_group() {
    let mut app = app_at_work();
    let depth = undo_position(&app, "b.snxsch");
    // A refused command leaves nothing behind either.
    let refused = call(
        &mut app,
        "clear_all_net_colors",
        json!({"document": doc("b.snxsch")}),
    );
    assert_eq!(refused["status"], "refused", "{refused}");
    let saved = call(
        &mut app,
        "save_document",
        json!({"document": doc("b.snxsch")}),
    );
    assert_eq!(saved["status"], "ran", "{saved}");
    assert_eq!(undo_position(&app, "b.snxsch"), depth);
    assert!(app.document_state.agent_edits.labels.is_empty());
}

#[test]
fn the_origin_is_the_agent_only_while_its_command_runs_and_always_put_back() {
    let mut app = app_at_work();
    assert_eq!(app.agent.origin, Origin::Person);

    // A refusal, a command that ran and a query each leave the person.
    let unknown = json!({"document": "no-such.snxsch", "at": [0, 0], "color": "#336699"});
    for (id, args, status) in [
        ("color_net", unknown, "refused"),
        ("color_net", color_args("b.snxsch"), "ran"),
        ("list_documents", json!({}), "answered"),
    ] {
        let reply = call_as(&mut app, PID, id, args);
        assert_eq!(reply["status"], status, "{reply}");
        assert_eq!(app.agent.origin, Origin::Person, "after `{id}`");
    }
    // A request that is not even a request, too.
    let reply = super::ask_raw_as(&mut app, PID, super::Input::NotUtf8);
    assert_eq!(reply["status"], "refused");
    assert_eq!(app.agent.origin, Origin::Person);

    // What runs inside sees the agent, and the previous origin comes back.
    let inside = app.with_origin(Origin::agent(Some(5)), |app| app.agent.origin.clone());
    assert_eq!(inside, Origin::agent(Some(5)));
    assert_eq!(app.agent.origin, Origin::Person);
}

#[test]
fn the_persons_own_command_makes_no_agent_label() {
    let mut app = app_at_work();
    let id = crate::keymap::AppCommandId::new("color_net").unwrap();
    let operands = [
        "at=5,0".to_string(),
        "color=#336699".to_string(),
        format!("document={}", doc("b.snxsch")),
    ];
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = crate::app::command::CommandArgs::parse_operands(&id, &refs).unwrap();

    let (outcome, _task) = app.run_operands(&id, &args);

    assert_eq!(outcome, crate::app::command::CommandOutcome::Ran);
    assert!(net_colors(&app, "b.snxsch") > 0);
    assert_eq!(top_group(&app, "b.snxsch"), None, "an ungrouped step");
    assert!(app.document_state.agent_edits.labels.is_empty());
    assert!(app.document_state.agent_edits.origins.is_empty());
}

#[test]
fn a_headless_run_is_an_agent_with_this_process_as_its_pid() {
    let mut app = app_at_work();
    let id = crate::keymap::AppCommandId::new("apply_edits").unwrap();
    let revision = app.document_state.engines[&path("b.snxsch")].revision();
    let operands = [
        format!("document={}", doc("b.snxsch")),
        format!("expect_revision={revision}"),
        r#"edits=[{"id":"place_junction","args":{"at":[20,20]}}]"#.to_string(),
    ];
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = crate::app::command::CommandArgs::parse_operands(&id, &refs).unwrap();

    let (outcome, _task) = app.run_headless_command_in_test(&id, args);

    assert!(matches!(
        outcome,
        crate::app::command::CommandOutcome::Answered(_)
    ));
    let group = top_group(&app, "b.snxsch").expect("a group");
    assert_eq!(
        app.document_state.agent_edits.origins.get(&group),
        Some(&Origin::agent(Some(std::process::id())))
    );
    assert_eq!(app.agent.origin, Origin::Person, "put back");
}

/// The pick marker of a net-class assignment carries no group (#2171), so an
/// agent's `assign_net_class` is not an agent step yet: it has no label and
/// `revert` cannot name it. `docs/AGENT_EDITS.md`, "Known limits".
#[test]
fn an_agents_net_class_pick_stays_a_pick_with_no_group_or_label() {
    let mut app = app_at_work();
    app.document_state.projects[0].data.net_classes =
        vec![signex_types::net_class::NetClass::named("Power")];
    app.settle_net_class_styles_blocking();

    let reply = call(
        &mut app,
        "assign_net_class",
        json!({"document": doc("a.snxsch"), "net_name": "VCC", "class": "Power"}),
    );

    assert_eq!(reply["status"], "ran", "{reply}");
    assert_eq!(top_group(&app, "a.snxsch"), None);
    assert!(app.document_state.agent_edits.labels.is_empty());
    assert!(app.document_state.agent_edits.origins.is_empty());
}

/// The reason the person's own run of `name` on sheet `a` is refused with.
fn persons_refusal(app: &mut Signex, name: &str, operands: &[&str]) -> String {
    let id = crate::keymap::AppCommandId::new(name).unwrap();
    let document = format!("document={}", doc("a.snxsch"));
    let mut texts: Vec<&str> = operands.to_vec();
    texts.push(&document);
    let args = crate::app::command::CommandArgs::parse_operands(&id, &texts).unwrap();
    match app.run_operands(&id, &args).0 {
        crate::app::command::CommandOutcome::Disabled { reason } => reason,
        other => panic!("{name} was not refused: {other:?}"),
    }
}

/// A read-only sheet (written by a newer Signex) must refuse an agent exactly
/// as it refuses the person, and say so once in Messages (#1402): the
/// transaction must not hide the handler's own check.
#[test]
fn an_agent_gets_the_persons_read_only_reason_and_the_warning() {
    for (name, operands, args) in [
        (
            "color_net",
            vec!["at=5,0", "color=#336699"],
            json!({"document": doc("a.snxsch"), "at": [5, 0], "color": "#336699"}),
        ),
        (
            "clear_all_net_colors",
            vec![],
            json!({"document": doc("a.snxsch")}),
        ),
    ] {
        let mut app = app_at_work();
        let mut read_only = super::sheet(super::SHEET_A);
        read_only.unknown.newer_minor_source = Some("snxsch/1.7".into());
        fixture::add_sheet_with(&mut app, &path("a.snxsch"), read_only);

        // The agent first: the warning is said once per sheet.
        let reply = call(&mut app, name, args);
        let person = persons_refusal(&mut app, name, &operands);

        assert_eq!(reply["status"], "refused", "{reply}");
        let reason = reply["reason"].as_str().expect("a reason");
        assert_eq!(reason, person, "{name}");
        assert!(reason.contains("snxsch/1.7"), "{reason}");
        let said = reply["messages"].to_string();
        assert!(said.contains("snxsch/1.7"), "{name}: the warning: {said}");
        assert!(!app.document_state.engines[&path("a.snxsch")].in_transaction());
    }
}

#[test]
fn a_refused_command_leaves_no_transaction_open() {
    let mut app = app_at_work();
    // No net colours to clear: refused after the transaction began.
    let refused = call(
        &mut app,
        "clear_all_net_colors",
        json!({"document": doc("b.snxsch")}),
    );
    assert_eq!(refused["status"], "refused", "{refused}");
    assert!(!app.document_state.engines[&path("b.snxsch")].in_transaction());

    let ran = call(&mut app, "color_net", color_args("b.snxsch"));
    assert_eq!(ran["status"], "ran", "{ran}");
    assert!(top_group(&app, "b.snxsch").is_some());
}
