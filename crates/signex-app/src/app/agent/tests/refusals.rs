//! Each way a request can be refused: it is answered `refused` with a reason,
//! logged to Messages, and changes nothing a person or a document holds.

use serde_json::{Value, json};

use super::{Input, SHEET_A, app_at_work, ask, ask_raw, line, path, snapshot};
use crate::app::Signex;
use crate::app::command::queries::tests::sheet;
use crate::app::handlers::editing_commands::net_color_driver as fixture;

fn doc(file: &str) -> String {
    path(file).display().to_string()
}

/// An absolute path on every OS that no open project holds.
fn elsewhere(file: &str) -> String {
    std::env::temp_dir()
        .join("snx-1924-elsewhere")
        .join(file)
        .display()
        .to_string()
}

/// What a refused request must leave as it was.
fn held(app: &Signex) -> (String, crate::headless::ChangeSnapshot, usize, usize) {
    (
        snapshot(app),
        app.change_snapshot(),
        app.document_state.tabs.len(),
        app.document_state.projects.len(),
    )
}

/// Run `input`, and check the reply is a refusal naming `fragment`, that
/// Messages says so with the client's pid, and that nothing changed.
fn refused_with(app: &mut Signex, input: Input, named: &str, fragment: &str) -> Value {
    let needle = format!("Agent request (pid 4242): {named} -> refused: ");
    let logged = refusals_logged(&needle, fragment);
    let before = held(app);

    let reply = ask_raw(app, input);

    assert_eq!(reply["status"], "refused", "{fragment}: {reply}");
    let reason = reply["reason"].as_str().unwrap_or_default();
    assert!(reason.contains(fragment), "{fragment}: {reply}");
    assert!(
        reply.get("data").is_none(),
        "a refusal has no data: {reply}"
    );
    assert_eq!(reply["changed"], json!([]));
    assert_eq!(
        refusals_logged(&needle, fragment),
        logged + 1,
        "not logged: {needle}{fragment}"
    );
    assert_eq!(held(app), before, "{fragment} changed something");
    reply
}

/// How many Messages lines begin with `start` and say `fragment`. A line over
/// 160 bytes is compacted and ends in `...`; the reasons come first, so what
/// the person needs is still in it.
fn refusals_logged(start: &str, fragment: &str) -> usize {
    let _ = crate::diagnostics::init_logging();
    crate::diagnostics::recent_entries()
        .iter()
        .filter(|entry| entry.message.starts_with(start) && entry.message.contains(fragment))
        .count()
}

fn refuse_line(app: &mut Signex, text: String, named: &str, fragment: &str) -> Value {
    refused_with(app, Input::Line(text), named, fragment)
}

#[test]
fn while_the_setting_is_off_nothing_is_served_and_no_socket_file_exists() {
    let mut app = app_at_work();
    // Off in Preferences: the listener has no subscription and no files.
    app.ui_state.agent_connections_enabled = false;
    assert_eq!(app.agent_listener_subscription().units(), 0);
    let dir = app.agent.runtime_dir().expect("a directory").clone();
    let socket = super::super::transport::endpoint_name(&dir, "anything");
    assert!(!std::path::Path::new(&socket).exists());

    let request = line(&app, "list_documents", json!({}));
    refuse_line(
        &mut app,
        request.clone(),
        "(no command)",
        "agent connections are off",
    );
    // A listener that closed behind a connection that is still open.
    app.ui_state.agent_connections_enabled = true;
    app.agent.endpoint = None;
    refuse_line(
        &mut app,
        request,
        "(no command)",
        "agent connections are off",
    );
}

#[test]
fn a_missing_or_wrong_token_is_refused_and_the_command_is_not_even_named() {
    let mut app = app_at_work();
    let no_token = json!({"id": "list_documents", "args": {}}).to_string();
    refuse_line(
        &mut app,
        no_token,
        "(no command)",
        "malformed request: missing field `token`",
    );
    for wrong in ["", "nope", "0".repeat(64).as_str()] {
        let text = json!({"token": wrong, "id": "list_documents"}).to_string();
        refuse_line(
            &mut app,
            text,
            "(no command)",
            "the session token is missing or wrong",
        );
    }
    // The token of the right length with its last character changed.
    let real = app.agent.token().unwrap().as_str().to_string();
    let mut near = real.clone();
    near.replace_range(63.., if real.ends_with('0') { "1" } else { "0" });
    let text = json!({"token": near, "id": "list_documents"}).to_string();
    refuse_line(
        &mut app,
        text,
        "(no command)",
        "the session token is missing or wrong",
    );
}

#[test]
fn an_unknown_field_a_bad_argument_and_text_that_is_not_json_are_refused() {
    let mut app = app_at_work();
    let token = app.agent.token().unwrap().as_str().to_string();
    for (text, fragment) in [
        (
            json!({"token": token, "id": "list_documents", "extra": 1}).to_string(),
            "malformed request: unknown field `extra`",
        ),
        (
            json!({"token": token, "id": "list_documents", "args": [1]}).to_string(),
            "malformed request",
        ),
        (
            json!({"token": token, "id": "list_documents", "args": {"at": [1]}}).to_string(),
            "operand `at` must be [x, y]",
        ),
        ("not json".to_string(), "malformed request"),
        (String::new(), "malformed request"),
    ] {
        refuse_line(&mut app, text, "(no command)", fragment);
    }
    refused_with(
        &mut app,
        Input::NotUtf8,
        "(no command)",
        "the request is not UTF-8 text",
    );
}

#[test]
fn an_oversized_request_is_refused() {
    let mut app = app_at_work();
    refused_with(
        &mut app,
        Input::Oversized,
        "(no command)",
        "the request is larger than",
    );
}

#[test]
fn undo_tool_view_and_window_only_ids_and_a_path_writing_save_are_refused() {
    let mut app = app_at_work();
    let a = doc("a.snxsch");
    for (id, args, fragment) in [
        ("undo", json!({"document": a}), "is the person's own"),
        ("redo", json!({"document": a}), "is the person's own"),
        (
            "place_wire",
            json!({"start": [0, 0], "end": [1, 1]}),
            "does not name a document",
        ),
        ("next_grid", json!({}), "only works in a window"),
        ("select_all", json!({}), "does not name a document"),
        ("open_erc_dialog", json!({}), "only works in a window"),
        (
            "save_document_as",
            json!({"document": a, "path": elsewhere("elsewhere.snxsch")}),
            "takes a file path",
        ),
        (
            "open_document",
            json!({"path": elsewhere("x.snxsch")}),
            "takes a file path",
        ),
        ("not_a_command", json!({}), "unknown command"),
    ] {
        let named = format!("`{id}`");
        let request = line(&app, id, args);
        refuse_line(&mut app, request, &named, fragment);
    }
}

#[test]
fn a_document_that_is_not_in_an_open_project_is_refused() {
    let mut app = app_at_work();
    // A loose sheet the person opened without its project.
    fixture::add_sheet_with(&mut app, &path("loose.snxsch"), sheet(SHEET_A + 50));
    fixture::activate(&mut app, &path("b.snxsch"));
    app.refresh_panel_ctx();

    for (document, fragment) in [
        (
            elsewhere("not-open.snxsch"),
            "not an open or referenced document",
        ),
        (
            doc("loose.snxsch"),
            "is not part of a project open in this app",
        ),
        (
            "00000000-0000-4000-8000-0000000000ff".to_string(),
            "no document has the id",
        ),
    ] {
        for id in ["read_document", "document_info", "color_net"] {
            let args = if id == "color_net" {
                json!({"document": document, "at": [0, 0], "color": "#336699"})
            } else {
                json!({"document": document})
            };
            let named = format!("`{id}`");
            let request = line(&app, id, args);
            refuse_line(&mut app, request, &named, fragment);
        }
    }
    // A document-less command that needs one.
    let request = line(&app, "clear_all_net_colors", json!({}));
    let reply = ask(&mut app, request);
    assert_eq!(reply["status"], "refused", "{reply}");
}

#[test]
fn document_less_queries_never_show_a_loose_sheet_to_an_agent() {
    let mut app = app_at_work();
    let loose_id = uuid::Uuid::from_u128(SHEET_A + 50);
    fixture::add_sheet_with(&mut app, &path("loose.snxsch"), sheet(SHEET_A + 50));
    fixture::activate(&mut app, &path("b.snxsch"));
    app.refresh_panel_ctx();
    // A person's own view still has it.
    let loose = path("loose.snxsch");
    assert!(
        app.document_entries()
            .iter()
            .any(|e| e.path.as_deref() == Some(loose.as_path()))
    );
    let before = held(&app);

    for (id, args) in [
        ("list_documents", json!({})),
        ("find_items", json!({"kind": "wire"})),
        ("resolve", json!({"id": loose_id.to_string()})),
    ] {
        let reply = super::call(&mut app, id, args);
        let text = reply.to_string();
        assert!(!text.contains("loose.snxsch"), "{id}: {text}");
        if id == "resolve" {
            // The id is echoed back; what it names is not found.
            assert_eq!(reply["data"]["found"], false, "{text}");
        } else {
            assert!(!text.contains(&loose_id.to_string()), "{id}: {text}");
        }
        assert!(reply["status"] == "answered" || reply["status"] == "refused");
    }
    let listed = super::call(&mut app, "list_documents", json!({}));
    assert!(
        listed.to_string().contains("a.snxsch"),
        "project sheets stay"
    );
    assert_eq!(held(&app), before);
}

#[test]
fn a_long_path_does_not_push_the_reason_out_of_the_messages_line() {
    let mut app = app_at_work();
    let long = elsewhere(&format!("{}.snxsch", "n".repeat(200)));
    let request = line(&app, "read_document", json!({"document": long}));
    let reply = refuse_line(
        &mut app,
        request,
        "`read_document`",
        "not an open or referenced document",
    );
    // The line is compacted to 160 bytes and says why before it says where.
    assert!(
        crate::diagnostics::recent_entries().iter().any(|e| e
            .message
            .starts_with("Agent request (pid 4242): `read_document`")
            && e.message.contains("not an open or referenced document")
            && e.message.ends_with("...")),
        "{reply}"
    );
}

#[test]
fn a_file_a_project_names_outside_its_folder_is_not_read_for_an_agent_but_is_for_a_person() {
    let mut app = app_at_work();
    let outside = std::env::temp_dir().join("snx-1924-outside.snxsch");
    std::fs::write(&outside, "x").unwrap();
    let project = &app.document_state.projects[0];
    assert!(
        app.may_read_referenced(project, &outside),
        "a person opens it"
    );

    app.agent.serving = true;
    let project = &app.document_state.projects[0];
    assert!(
        !app.may_read_referenced(project, &outside),
        "an agent's query does not read it"
    );
    std::fs::remove_file(&outside).ok();
}

#[test]
fn closing_the_persons_active_document_is_refused_and_changes_nothing() {
    let mut app = app_at_work();
    let request = line(
        &app,
        "close_active_document",
        json!({"document": doc("b.snxsch")}),
    );
    refuse_line(
        &mut app,
        request,
        "`close_active_document`",
        "is the person's active document",
    );
    assert!(
        app.document_state
            .tabs
            .iter()
            .any(|t| t.path == path("b.snxsch"))
    );
}

#[test]
fn closing_a_sheet_the_person_is_typing_the_title_block_of_is_refused_and_the_draft_stays() {
    let mut app = app_at_work();
    app.document_state.drafts.set_title_block(
        &path("a.snxsch"),
        crate::panels::TitleBlockField::DocumentNumber,
        "TYPED",
    );
    let request = line(
        &app,
        "close_active_document",
        json!({"document": doc("a.snxsch")}),
    );
    refuse_line(
        &mut app,
        request,
        "`close_active_document`",
        "the person is editing this sheet's title block",
    );
    assert_eq!(app.document_state.drafts.title_blocks().len(), 1);
    assert!(
        app.document_state
            .tabs
            .iter()
            .any(|t| t.path == path("a.snxsch"))
    );
}

#[test]
fn closing_a_document_shown_in_a_window_of_its_own_is_refused_and_changes_nothing() {
    let mut app = app_at_work();
    app.ui_state.windows.insert(
        iced::window::Id::unique(),
        crate::app::state::WindowKind::UndockedTab {
            path: path("a.snxsch"),
            title: "a".to_string(),
        },
    );
    let request = line(
        &app,
        "close_active_document",
        json!({"document": doc("a.snxsch")}),
    );
    refuse_line(
        &mut app,
        request,
        "`close_active_document`",
        "is open in a window of its own",
    );
    assert!(
        app.document_state
            .tabs
            .iter()
            .any(|t| t.path == path("a.snxsch"))
    );
}
