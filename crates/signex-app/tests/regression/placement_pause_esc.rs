//! #2529 — Esc while placement is paused goes back to placing without the
//! panel edits: the next click places with the values the pause began with,
//! and a second Esc leaves the tool.

use signex_app::app::{InputMsg, Message, Signex, Tool};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_app::toolbar::ToolMessage;
use signex_types::schematic::WireWidthPreset;
use signex_types::theme::Color;

use crate::library_place_symbol::{app_with_empty_sheet, click, doc};

const RED: Color = Color::new(255, 0, 0, 255);

fn esc(app: &mut Signex) {
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
}

#[test]
fn tab_edit_esc_then_the_next_wire_places_with_the_original_values() {
    let mut app = app_with_empty_sheet();
    // The first-run tour claims Esc ahead of the placement pause.
    app.ui_state.first_run_tour_open = false;
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
    let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));
    assert!(app.interaction_state.active_canvas().placement_paused);
    for edit in [
        PanelMsg::SetPrePlacementWireWidth(WireWidthPreset::Large),
        PanelMsg::SetPrePlacementWireColor(Some(RED)),
    ] {
        let _ = app.update(Message::Dock(DockMessage::Panel(edit)));
    }

    esc(&mut app);
    assert!(!app.interaction_state.active_canvas().placement_paused);
    assert_eq!(app.interaction_state.current_tool, Tool::Wire);

    click(&mut app, (10.0, 20.0));
    click(&mut app, (30.0, 20.0));
    let placed = doc(&app).wires.clone();
    assert_eq!(placed.len(), 1, "placing goes on after the Esc");
    assert_eq!(placed[0].stroke_width, 0.0, "the default width, not Large");
    assert_eq!(placed[0].color, None, "the theme colour, not red");

    // No longer paused: Esc ends the wire in progress and the tool.
    esc(&mut app);
    esc(&mut app);
    assert_eq!(app.interaction_state.current_tool, Tool::Select);
}
