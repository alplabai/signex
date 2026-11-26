//! #2470: the attached forms of the commands the demo videos (#2437) found
//! missing on the channel: annotation by named sheet, the board writers into
//! a path inside the project, Fit, and a dock panel by its key. Each runs
//! through the socket as an agent's request does.

use serde_json::{Value, json};

use super::{app_at_work, app_in, call, call_settled, fixture, path};
use crate::app::Signex;
use crate::panels::PanelKind;

fn doc(file: &str) -> String {
    path(file).display().to_string()
}

/// The fixture's project, in `dir`, with an empty board open behind `b`.
fn app_with_board(dir: &std::path::Path) -> Signex {
    let mut app = app_in(dir);
    let board = signex_types::project::SetupProfile::Standard
        .pcb(4)
        .new_board();
    app.open_pcb_tab(dir.join("P.snxpcb"), "P.snxpcb".to_string(), board);
    app.document_state.projects[0].data.pcb_file = Some("P.snxpcb".to_string());
    let project = app.document_state.projects[0].id;
    for tab in &mut app.document_state.tabs {
        tab.project_id = Some(project);
    }
    fixture::activate(&mut app, &dir.join("b.snxsch"));
    app
}

fn reason(reply: &Value) -> &str {
    reply["reason"].as_str().unwrap_or_default()
}

#[test]
fn annotate_quietly_runs_on_its_named_sheet_and_the_dialog_points_to_it() {
    let mut app = app_at_work();
    let active = app.document_state.active_tab;
    let reply = call_settled(
        &mut app,
        "annotate_schematic_quietly",
        json!({"document": doc("a.snxsch")}),
    );
    assert!(
        reply["status"] == "answered" || reply["status"] == "ran",
        "{reply}"
    );
    assert_eq!(app.document_state.active_tab, active, "the tab stays");

    let bare = call(&mut app, "annotate_schematic_quietly", json!({}));
    assert_eq!(bare["status"], "refused", "{bare}");
    assert!(reason(&bare).contains("explicit `document`"), "{bare}");

    let dialog = call(&mut app, "annotate_schematic", json!({}));
    assert_eq!(dialog["status"], "refused", "{dialog}");
    assert!(
        reason(&dialog).contains("`annotate_schematic_quietly`"),
        "{dialog}"
    );
    assert!(!app.ui_state.annotate_dialog_open, "no dialog opened");
}

#[test]
fn a_board_report_is_written_inside_the_project_and_refused_outside_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let outside = tempfile::tempdir().expect("temp dir");
    let mut app = app_with_board(dir.path());
    let board = dir.path().join("P.snxpcb").display().to_string();
    let inside = dir.path().join("bom.csv");
    let reply = call_settled(
        &mut app,
        "export_board_report",
        json!({"document": board, "kind": "bom", "path": inside.display().to_string()}),
    );
    assert_eq!(reply["status"], "answered", "{reply}");
    assert!(inside.is_file(), "the CSV is written in the project");

    let away = outside.path().join("bom.csv");
    let refused = call_settled(
        &mut app,
        "export_board_report",
        json!({"document": board, "kind": "bom", "path": away.display().to_string()}),
    );
    assert_eq!(refused["status"], "refused", "{refused}");
    assert!(
        reason(&refused).contains("not inside the project folder"),
        "{refused}"
    );
    assert!(!away.exists(), "nothing is written outside");
}

/// The gate reads the path's text; a folder inside the project that is a
/// link out of it is caught by the write itself, which writes nothing.
#[cfg(unix)]
#[test]
fn a_link_out_of_the_project_is_not_written_through() {
    let dir = tempfile::tempdir().expect("temp dir");
    let outside = tempfile::tempdir().expect("temp dir");
    let mut app = app_with_board(dir.path());
    std::os::unix::fs::symlink(outside.path(), dir.path().join("out")).expect("link");
    let board = dir.path().join("P.snxpcb").display().to_string();
    let target = dir.path().join("out").join("bom.csv");
    let _reply = call_settled(
        &mut app,
        "export_board_report",
        json!({"document": board, "kind": "bom", "path": target.display().to_string()}),
    );
    assert!(
        !outside.path().join("bom.csv").exists(),
        "the link is not followed out of the project"
    );
}

#[test]
fn the_pdf_and_fabrication_writers_keep_their_path_in_the_project() {
    let dir = tempfile::tempdir().expect("temp dir");
    let outside = tempfile::tempdir().expect("temp dir");
    let mut app = app_with_board(dir.path());
    let board = dir.path().join("P.snxpcb").display().to_string();
    for (id, name) in [
        ("export_board_pdf", "board.pdf"),
        ("fabrication_outputs", "fab"),
    ] {
        let away = outside.path().join(name).display().to_string();
        let refused = call_settled(&mut app, id, json!({"document": board, "path": away}));
        assert_eq!(refused["status"], "refused", "{id}: {refused}");
        assert!(
            reason(&refused).contains("not inside the project folder"),
            "{id}: {refused}"
        );
        let inside = dir.path().join(name).display().to_string();
        let reply = call_settled(&mut app, id, json!({"document": board, "path": inside}));
        assert!(
            !reason(&reply).contains("not inside the project folder")
                && !reason(&reply).contains("takes a file path"),
            "{id}: {reply}"
        );
    }
    assert_eq!(
        std::fs::read_dir(outside.path()).expect("read").count(),
        0,
        "nothing is written outside"
    );
}

#[test]
fn fit_answers_with_the_document_in_view_and_keeps_the_tab() {
    let mut app = app_at_work();
    let active = app.document_state.active_tab;
    let reply = call(&mut app, "zoom_to_fit", json!({}));
    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(reply["data"]["fitted"]["name"], "b", "{reply}");
    assert_eq!(app.document_state.active_tab, active, "the tab stays");
}

fn dock(app: &mut Signex, panel: &str, state: &str) -> Value {
    call(
        app,
        "set_dock_panel",
        json!({"panel": panel, "state": state}),
    )
}

#[test]
fn a_dock_panel_is_opened_collapsed_expanded_and_closed_by_its_key() {
    let mut app = app_at_work();
    let active = app.document_state.active_tab;
    let opened = dock(&mut app, "messages", "open");
    assert_eq!(opened["status"], "answered", "{opened}");
    assert_eq!(opened["data"]["open"], true, "{opened}");
    assert!(app.document_state.dock.is_frontmost(PanelKind::Messages));

    let collapsed = dock(&mut app, "messages", "collapse");
    assert_eq!(collapsed["data"]["collapsed"], true, "{collapsed}");
    assert!(!app.document_state.dock.is_frontmost(PanelKind::Messages));
    // A state, not a toggle: collapsing again leaves it collapsed.
    let again = dock(&mut app, "messages", "collapse");
    assert_eq!(again["data"]["collapsed"], true, "{again}");
    let expanded = dock(&mut app, "messages", "expand");
    assert_eq!(expanded["data"]["collapsed"], false, "{expanded}");

    let closed = dock(&mut app, "messages", "close");
    assert_eq!(closed["data"]["open"], false, "{closed}");
    assert_eq!(closed["data"]["place"], Value::Null, "{closed}");
    assert!(!app.is_panel_open(PanelKind::Messages));
    assert_eq!(app.document_state.active_tab, active, "the tab stays");
}

#[test]
fn an_unknown_panel_or_state_and_a_closed_panels_collapse_are_refused() {
    let mut app = app_at_work();
    for (panel, state, wanted) in [
        ("nope", "open", "`panel` is one of"),
        ("sch_library", "open", "`panel` is one of"),
        ("messages", "toggle", "`state` is one of"),
    ] {
        let reply = dock(&mut app, panel, state);
        assert_eq!(reply["status"], "refused", "{panel} {state}: {reply}");
        assert!(reason(&reply).contains(wanted), "{reply}");
    }
    let _closed = dock(&mut app, "messages", "close");
    let reply = dock(&mut app, "messages", "collapse");
    assert_eq!(reply["status"], "refused", "{reply}");
    assert!(reason(&reply).contains("is not docked"), "{reply}");
}
