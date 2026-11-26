//! #2470: the attached forms of the commands the demo videos (#2437) found
//! missing on the channel: annotation by named sheet, Fit, and a dock panel
//! by its key (the board writers are `channel_writers`). Each runs through
//! the socket as an agent's request does.

use serde_json::{Value, json};

use super::{SHEET_B, app_at_work, app_in, call, call_settled, fixture, sheet, sheet_entry, uid};
use crate::app::Signex;
use crate::panels::PanelKind;

fn reason(reply: &Value) -> &str {
    reply["reason"].as_str().unwrap_or_default()
}

/// The reference of the part `symbol` on the open sheet `sheet`.
fn reference_on(app: &Signex, sheet: &std::path::Path, symbol: uuid::Uuid) -> String {
    let engine = &app.document_state.engines[sheet];
    let part = engine.document().symbols.iter().find(|s| s.uuid == symbol);
    part.map(|s| s.reference.clone()).expect("the part")
}

#[test]
fn annotate_quietly_numbers_its_named_sheet_and_the_dialog_points_to_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut app = app_in(dir.path());
    // A third sheet of the project with an unnumbered part.
    let c = dir.path().join("c.snxsch");
    let part = uid(0x2470);
    let mut unnumbered = sheet(SHEET_B + 7);
    unnumbered.symbols[0].uuid = part;
    unnumbered.symbols[0].reference = "U?".into();
    fixture::add_sheet_with(&mut app, &c, unnumbered);
    app.document_state.mark_document_clean(&c);
    app.document_state.projects[0]
        .data
        .sheets
        .push(sheet_entry("c"));
    fixture::activate(&mut app, &dir.path().join("b.snxsch"));
    let active = app.document_state.active_tab;

    let reply = call_settled(
        &mut app,
        "annotate_schematic_quietly",
        json!({"document": c.display().to_string()}),
    );
    assert!(
        reply["status"] == "answered" || reply["status"] == "ran",
        "{reply}"
    );
    let now = reference_on(&app, &c, part);
    assert!(
        now.starts_with('U') && !now.contains('?'),
        "numbered: {now}, {reply}"
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
