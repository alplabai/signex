//! #2326 slice 6.P5: today's query answers, pinned on every surface that
//! asks, before slice 6 moves the handlers and slice 8 puts them behind
//! `core.query` (ADR-0006, "Driving ports").
//!
//! The surfaces: the agent socket, the headless entry the CLI runs, and the
//! person port the test adapter's `query` drives (`run_query_headless` as
//! the person, `test_adapter::TestAdapter::query`). A palette, key or menu
//! call of a query is refused. Every answer reads the documents alone.

use serde_json::{Value, json};
use signex_i18n::RenderTranslatable;

use super::{SHEET_A, app_at_work, app_in, call, snapshot};
use crate::app::Signex;
use crate::app::agent::Origin;
use crate::app::command::queries::tests::{WIRE_A, path, sheet, uid};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;

fn doc(file: &str) -> String {
    path(file).display().to_string()
}

/// `args` as the socket sends them, as `name=value` operands.
fn operands(args: &Value) -> Vec<String> {
    let object = args.as_object().expect("operands are an object");
    object
        .iter()
        .map(|(name, value)| match value {
            Value::String(text) => format!("{name}={text}"),
            other => format!("{name}={other}"),
        })
        .collect()
}

fn parsed(id: &str, args: &Value) -> (AppCommandId, CommandArgs) {
    let command = AppCommandId::new(id).expect("a valid id");
    let texts = operands(args);
    if texts.is_empty() {
        return (command, CommandArgs::none());
    }
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&command, &refs).expect("operands parse");
    (command, args)
}

/// The headless entry `signex-cli` runs.
fn by_cli(app: &mut Signex, id: &str, args: &Value) -> CommandOutcome {
    let (command, args) = parsed(id, args);
    app.run_headless_command_in_test(&command, args).0
}

/// The person port: what `TestAdapter::query` runs.
fn by_port(app: &mut Signex, id: &str, args: &Value) -> CommandOutcome {
    let (command, args) = parsed(id, args);
    app.with_origin(Origin::Person, |app| {
        app.run_query_headless(&command, &args)
    })
    .expect("a query")
    .0
}

fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

/// The en-US reason of a refusal, and whether it carries a catalog id.
fn refused(outcome: CommandOutcome) -> (String, bool) {
    match outcome {
        CommandOutcome::Disabled { reason } => (reason.render_en_us(), reason.id().is_some()),
        other => panic!("not refused: {other:?}"),
    }
}

/// The socket's answer, or its refusal reason, for `id`.
fn by_socket(app: &mut Signex, id: &str, args: &Value) -> Result<Value, String> {
    let reply = call(app, id, args.clone());
    assert_eq!(reply["id"], id, "{reply}");
    assert_eq!(
        reply["changed"],
        json!([]),
        "a query changes nothing: {reply}"
    );
    match reply["status"].as_str() {
        Some("answered") => Ok(reply["data"].clone()),
        Some("refused") => Err(reply["reason"].as_str().expect("a reason").to_string()),
        _ => panic!("neither answered nor refused: {reply}"),
    }
}

/// The answer, or the en-US refusal reason, as the socket says it.
fn said(outcome: CommandOutcome) -> Result<Value, String> {
    match outcome {
        CommandOutcome::Answered(value) => Ok(value),
        CommandOutcome::Disabled { reason } => Err(reason.render_en_us()),
        other => panic!("neither answered nor refused: {other:?}"),
    }
}

fn project() -> String {
    path("P.snxprj").display().to_string()
}

/// The queries every surface answers alike on the fixture, with their
/// operands. `query_pins` is refused alike: the fixture's hierarchy is not
/// read. `list_documents` and `project_settings` differ on the socket (see
/// `the_socket_hides_the_project_document_the_cli_and_the_person_port_read`).
fn asked() -> Vec<(&'static str, Value)> {
    vec![
        ("document_info", json!({"document": doc("a.snxsch")})),
        ("read_document", json!({"document": doc("a.snxsch")})),
        (
            "read_document",
            json!({"document": doc("b.snxsch"), "kinds": "wire"}),
        ),
        ("resolve", json!({"id": uid(WIRE_A).to_string()})),
        ("resolve", json!({"id": uid(SHEET_A).to_string()})),
        ("find_items", json!({"designator": "U3"})),
        (
            "find_items",
            json!({"document": doc("a.snxsch"), "net": "GND"}),
        ),
        (
            "query_pins",
            json!({"document": doc("a.snxsch"), "designator": "U3"}),
        ),
    ]
}

#[test]
fn every_surface_gets_the_same_answer_to_each_query() {
    let mut app = app_at_work();
    for (id, args) in asked() {
        let socket = by_socket(&mut app, id, &args);
        let cli = said(by_cli(&mut app, id, &args));
        let port = said(by_port(&mut app, id, &args));
        assert_eq!(socket, cli, "{id} {args}: socket and CLI");
        assert_eq!(cli, port, "{id} {args}: CLI and the person port");
    }
}

/// Today's split, pinned so slice 6 does not move it silently: the socket
/// keeps only documents that belong to a project (`document_entries` under
/// `agent.serving`), which drops the `.snxprj` row itself. So the socket
/// lists no project and refuses `project_settings`, which the CLI and the
/// person port answer.
#[test]
fn the_socket_hides_the_project_document_the_cli_and_the_person_port_read() {
    let mut app = app_at_work();
    let kinds = |listed: &Value| -> Vec<String> {
        let documents = listed["documents"].as_array().expect("documents");
        documents
            .iter()
            .map(|doc| doc["kind"].to_string())
            .collect()
    };
    let socket = by_socket(&mut app, "list_documents", &json!({})).expect("listed");
    let cli = answered(by_cli(&mut app, "list_documents", &json!({})));
    let port = answered(by_port(&mut app, "list_documents", &json!({})));
    assert_eq!(kinds(&socket), ["\"schematic\"", "\"schematic\""]);
    assert_eq!(
        kinds(&cli),
        ["\"schematic\"", "\"schematic\"", "\"project\""]
    );
    assert_eq!(cli, port, "CLI and the person port");

    let args = json!({"document": project()});
    let socket = by_socket(&mut app, "project_settings", &args).expect_err("refused");
    assert_eq!(socket, "P is not part of a project open in this app");
    let cli = answered(by_cli(&mut app, "project_settings", &args));
    let port = answered(by_port(&mut app, "project_settings", &args));
    assert_eq!(cli["project"], project().as_str());
    assert_eq!(cli, port, "CLI and the person port");
}

#[test]
fn asking_on_any_surface_leaves_the_persons_work_and_the_documents_as_they_were() {
    let mut app = app_at_work();
    let before = snapshot(&app);
    let revisions = |app: &Signex| {
        ["a.snxsch", "b.snxsch"].map(|file| {
            let engine = &app.document_state.engines[&path(file)];
            (
                engine.revision(),
                app.document_state.is_document_dirty(&path(file)),
            )
        })
    };
    let documents = revisions(&app);
    for (id, args) in asked() {
        let _ = by_socket(&mut app, id, &args);
        let _ = by_cli(&mut app, id, &args);
        let _ = by_port(&mut app, id, &args);
    }
    assert_eq!(snapshot(&app), before, "selection, tool, gesture, palette");
    assert_eq!(revisions(&app), documents, "no edit, no dirty mark");
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(path("b.snxsch").as_path())
    );
}

/// The person's palette, key and menu route: no GUI surface shows a query's
/// data, so each is refused with today's legacy English (no catalog id yet).
#[test]
fn a_palette_key_or_menu_call_of_any_query_is_refused_with_todays_reason() {
    let mut app = app_at_work();
    for id in crate::app::command::queries::query_command_ids() {
        let command = AppCommandId::new(id).expect("a valid id");
        let (outcome, _task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
        let (reason, has_id) = refused(outcome);
        assert_eq!(
            reason,
            format!("`{id}` is for agents and the CLI: it answers with data")
        );
        assert!(!has_id, "{id}: legacy English until #2644");
    }
}

#[test]
fn a_document_that_is_not_open_is_refused_with_one_reason_on_every_surface() {
    let mut app = app_at_work();
    let gone = doc("gone.snxsch");
    for id in ["document_info", "read_document"] {
        let args = json!({"document": gone});
        let socket = by_socket(&mut app, id, &args).expect_err("the socket refuses");
        let (cli, cli_has_id) = refused(by_cli(&mut app, id, &args));
        let (port, port_has_id) = refused(by_port(&mut app, id, &args));
        assert_eq!(socket, cli, "{id}: socket and CLI");
        assert_eq!(cli, port, "{id}: CLI and the person port");
        assert!(cli.contains("gone.snxsch"), "{id}: names it: {cli}");
        assert!(
            !cli_has_id && !port_has_id,
            "{id}: legacy English until #2644"
        );
    }
}

/// A missing required operand: the CLI and the person port reach the query,
/// which names the operand; the socket refuses at the operand parse first.
#[test]
fn a_query_missing_its_document_is_refused_on_every_surface() {
    let mut app = app_at_work();
    for id in ["document_info", "read_document", "query_pins"] {
        let want = format!("`{id}` needs the operand `document`");
        let (cli, _) = refused(by_cli(&mut app, id, &json!({})));
        let (port, _) = refused(by_port(&mut app, id, &json!({})));
        assert_eq!(cli, want, "{id}: CLI");
        assert_eq!(port, want, "{id}: the person port");
        let socket = by_socket(&mut app, id, &json!({})).expect_err("the socket refuses");
        assert!(
            socket.contains("document"),
            "{id}: socket names it: {socket}"
        );
    }
}

/// A project sheet that is not loaded is read for the answer and dropped:
/// no engine, tab or id stays, and every surface reads the same sheet.
#[test]
fn an_unloaded_project_sheet_is_answered_alike_and_leaves_no_engine_behind() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(dir.path());
    let unloaded = dir.path().join("c.snxsch");
    let text = signex_types::format::SnxSchematic::new(sheet(102))
        .write_string()
        .expect("encode the sheet");
    std::fs::write(&unloaded, text).unwrap();
    app.document_state.projects[0]
        .data
        .sheets
        .push(super::sheet_entry("c"));
    let engines = app.document_state.engines.len();
    let tabs = app.document_state.tabs.len();
    let before = snapshot(&app);

    let args = json!({"document": unloaded.display().to_string()});
    let socket = by_socket(&mut app, "read_document", &args).expect("the socket answers");
    let cli = answered(by_cli(&mut app, "read_document", &args));
    let port = answered(by_port(&mut app, "read_document", &args));

    assert_eq!(socket, cli, "socket and CLI");
    assert_eq!(cli, port, "CLI and the person port");
    assert!(cli.to_string().contains(&uid(WIRE_A).to_string()), "{cli}");
    assert!(!app.document_state.engines.contains_key(&unloaded));
    assert_eq!(app.document_state.engines.len(), engines);
    assert_eq!(app.document_state.tabs.len(), tabs);
    assert_eq!(snapshot(&app), before);
}
