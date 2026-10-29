//! #1249 / #1251 — a planned Active Bar tool arms nothing and places
//! nothing, and Design > Generate BOM opens the working BOM dialog.

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg, PLANNED_TOOLS};
use signex_app::app::{Message, Signex, TabInfo, TabKind, Tool};
use signex_app::canvas::CanvasEvent;
use signex_app::diagnostics;
use signex_app::menu_bar::MenuMessage;
use std::path::PathBuf;

fn app_with_empty_sheet() -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = false;
    let path = PathBuf::from("planned-tools.snxsch");
    let engine =
        signex_engine::Engine::new(super::place_sheet_tool::empty_sheet()).expect("engine");
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "planned-tools".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

fn object_count(app: &Signex) -> usize {
    let doc = app
        .document_state
        .active_engine()
        .expect("engine")
        .document();
    doc.symbols.len()
        + doc.wires.len()
        + doc.labels.len()
        + doc.child_sheets.len()
        + doc.text_notes.len()
        + doc.drawings.len()
        + doc.no_erc_directives.len()
}

#[test]
fn a_planned_tool_arms_nothing_and_places_nothing() {
    let _ = diagnostics::init_logging();
    for (action, name) in PLANNED_TOOLS {
        let mut app = app_with_empty_sheet();
        let tool_before = app.interaction_state.current_tool;

        let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(action.clone())));
        // Two clicks: enough for any drag-to-size or click-to-place tool.
        for (x, y) in [(10.0, 10.0), (40.0, 30.0)] {
            let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
                world_x: x,
                world_y: y,
            }));
        }

        assert_eq!(
            app.interaction_state.current_tool, tool_before,
            "{name} must not switch tool"
        );
        assert_eq!(object_count(&app), 0, "{name} must not place anything");
        assert!(
            app.interaction_state
                .active_canvas_mut()
                .ghost_text
                .is_none(),
            "{name} must not arm a text ghost"
        );
        assert!(
            diagnostics::recent_entries().iter().any(|e| e
                .message
                .starts_with(&format!("Planned, not available yet: {name}"))),
            "{name} must say why it did nothing"
        );
    }
}

#[test]
fn diff_pair_no_longer_drops_a_diff_pair_note() {
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::PlaceDiffPair,
    )));
    assert_ne!(app.interaction_state.current_tool, Tool::Text);
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: 5.0,
        world_y: 5.0,
    }));
    let doc = app
        .document_state
        .active_engine()
        .expect("engine")
        .document();
    assert!(doc.text_notes.iter().all(|n| n.text != "DIFF_PAIR"));
}

#[test]
fn a_real_tool_still_arms() {
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::DrawRectangle,
    )));
    assert_eq!(app.interaction_state.current_tool, Tool::Rectangle);
}

#[test]
fn generate_bom_opens_the_bom_dialog() {
    // The Design > Generate BOM row sends `ExportBom` (pinned by the
    // menu_bar planned_tests source check); this drives that message.
    let mut app = super::bom_validation::app_with_faulty_bom();
    assert!(app.document_state.bom_preview.is_none());
    let _ = app.update(Message::Menu(MenuMessage::ExportBom));
    assert!(app.document_state.bom_preview.is_some());
}

/// #1251 — the Place and View menu rows that used to be inert now send
/// messages whose handlers work.
#[test]
fn place_menu_rows_arm_their_tools() {
    for (message, tool) in [
        (MenuMessage::PlaceText, Tool::Text),
        (MenuMessage::PlaceNoConnect, Tool::NoConnect),
    ] {
        let mut app = app_with_empty_sheet();
        let _ = app.update(Message::Menu(message.clone()));
        assert_eq!(app.interaction_state.current_tool, tool, "{message:?}");
    }
}

#[test]
fn place_power_port_row_arms_a_power_port() {
    let mut app = app_with_empty_sheet();
    assert!(app.interaction_state.pending_power.is_none());
    let _ = app.update(Message::Menu(MenuMessage::PlacePowerPort));
    let (net, lib_id) = app
        .interaction_state
        .pending_power
        .clone()
        .expect("a power port is armed");
    assert_eq!((net.as_str(), lib_id.as_str()), ("GND", "power:GND"));
}

#[test]
fn zoom_rows_change_the_active_canvas_scale() {
    // The View > Zoom In / Zoom Out rows reach the zoom handler and move
    // the active sheet's camera scale; the bridge test pins the command ids
    // to these same messages.
    let mut app = app_with_empty_sheet();
    let scale = |app: &mut Signex| app.interaction_state.active_canvas_mut().live_camera().2;
    let start = scale(&mut app);
    let _ = app.update(Message::Menu(MenuMessage::ZoomIn));
    let zoomed_in = scale(&mut app);
    assert!(zoomed_in > start, "Zoom In: {start} -> {zoomed_in}");
    let _ = app.update(Message::Menu(MenuMessage::ZoomOut));
    let zoomed_out = scale(&mut app);
    assert!(
        zoomed_out < zoomed_in,
        "Zoom Out: {zoomed_in} -> {zoomed_out}"
    );
}
