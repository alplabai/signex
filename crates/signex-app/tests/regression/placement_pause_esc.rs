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

/// #2769: a second Tab goes on placing with the edits, and leaves no pause
/// for a later Esc to restore.
#[test]
fn a_second_tab_resumes_placing_with_the_edits() {
    let mut app = app_with_empty_sheet();
    app.ui_state.first_run_tour_open = false;
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
    let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::SetPrePlacementWireWidth(WireWidthPreset::Large),
    )));
    let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));

    assert!(!app.interaction_state.active_canvas().placement_paused);
    assert!(app.interaction_state.placement_pause.is_none());
    assert_eq!(app.interaction_state.current_tool, Tool::Wire);
    let form = app.document_state.panel_ctx.pre_placement.as_ref();
    assert_eq!(form.map(|f| f.wire_width), Some(WireWidthPreset::Large));
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

/// A position typed into the paused form is one of its edits: Esc drops it
/// and places nothing, and the form goes back to what it held at the Tab.
/// OK with a field that does not read keeps the pause, so Esc still
/// restores.
#[test]
fn esc_after_typing_a_position_places_nothing_and_restores_the_form() {
    use signex_app::panels::{PositionField, PrePlacementPositionMsg};
    let mut app = app_with_empty_sheet();
    app.ui_state.first_run_tour_open = false;
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Junction)));
    let before_tab = app.document_state.panel_ctx.pre_placement.clone();
    let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));
    assert!(app.interaction_state.active_canvas().placement_paused);
    let typed = |app: &mut Signex, field, text: &str| {
        let msg = PrePlacementPositionMsg::FieldTyped(field, text.to_owned());
        let _ = app.update(Message::Dock(DockMessage::Panel(
            PanelMsg::PrePlacementPosition(msg),
        )));
    };
    typed(&mut app, PositionField::X, "50");
    typed(&mut app, PositionField::Y, "12a");
    // OK with Y unreadable: refused, still paused, nothing placed.
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::ConfirmPrePlacement,
    )));
    assert!(app.interaction_state.active_canvas().placement_paused);
    assert!(doc(&app).junctions.is_empty());

    esc(&mut app);
    assert!(!app.interaction_state.active_canvas().placement_paused);
    assert_eq!(app.interaction_state.current_tool, Tool::Junction);
    assert!(doc(&app).junctions.is_empty(), "Esc places nothing");
    let form = &app.document_state.panel_ctx.pre_placement;
    let shape = |pp: &Option<signex_app::panels::PrePlacementData>| {
        pp.as_ref()
            .map(|pp| (pp.rotation, pp.label_text.clone(), pp.position.clone()))
    };
    assert_eq!(
        shape(form),
        shape(&before_tab),
        "the form is what it was at the Tab"
    );
    assert!(
        form.as_ref().is_none_or(|pp| !pp.position.is_typed()),
        "the typed position is gone"
    );
}
