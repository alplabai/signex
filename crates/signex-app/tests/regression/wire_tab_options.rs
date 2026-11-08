//! The Wire tool's Tab form offers Width and Color: they apply to the wire
//! being routed and the ones after it, Resume carries on with them, and the
//! same two fields edit a placed wire through undoable commands.

use signex_app::app::{EditMsg, Message, Signex, Tool};
use signex_app::dock::DockMessage;
use signex_app::panels::{PanelMsg, PrePlacementKind};
use signex_app::toolbar::ToolMessage;
use signex_types::schematic::{Wire, WireWidthPreset};
use signex_types::theme::Color;

use crate::library_place_symbol::{app_with_empty_sheet, click, doc};

const RED: Color = Color::new(255, 0, 0, 255);

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn tool(app: &mut Signex, message: ToolMessage) {
    let _ = app.update(Message::Tool(message));
}

fn wires(app: &Signex) -> Vec<Wire> {
    doc(app).wires.clone()
}

#[test]
fn tab_in_the_wire_tool_opens_a_form_that_styles_this_and_the_following_wires() {
    let mut app = app_with_empty_sheet();
    tool(&mut app, ToolMessage::SelectTool(Tool::Wire));
    tool(&mut app, ToolMessage::PrePlacementTab);

    let pre = app
        .document_state
        .panel_ctx
        .pre_placement
        .as_ref()
        .expect("Tab opens the pre-placement form");
    assert_eq!(pre.kind, PrePlacementKind::Wire);
    assert_eq!(pre.wire_width, WireWidthPreset::Small, "default choice");
    assert_eq!(pre.wire_color, None);
    assert!(app.interaction_state.active_canvas().placement_paused);

    panel(
        &mut app,
        PanelMsg::SetPrePlacementWireWidth(WireWidthPreset::Large),
    );
    panel(&mut app, PanelMsg::SetPrePlacementWireColor(Some(RED)));

    // Paused: a click places nothing.
    click(&mut app, (10.0, 20.0));
    assert!(wires(&app).is_empty());

    tool(&mut app, ToolMessage::ResumePlacement);
    click(&mut app, (10.0, 20.0));
    click(&mut app, (30.0, 20.0));
    let placed = wires(&app);
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0].stroke_width, WireWidthPreset::Large.stroke_mm());
    assert_eq!(placed[0].color, Some(RED));

    // The next wire of the session keeps the choice.
    click(&mut app, (30.0, 40.0));
    let placed = wires(&app);
    assert_eq!(placed.len(), 2);
    assert_eq!(placed[1].stroke_width, 1.0);
    assert_eq!(placed[1].color, Some(RED));
}

#[test]
fn a_wire_placed_without_the_form_keeps_the_default_look() {
    let mut app = app_with_empty_sheet();
    tool(&mut app, ToolMessage::SelectTool(Tool::Wire));
    click(&mut app, (10.0, 20.0));
    click(&mut app, (30.0, 20.0));
    let placed = wires(&app);
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0].stroke_width, 0.0);
    assert_eq!(placed[0].color, None);
}

#[test]
fn width_and_colour_of_a_placed_wire_edit_and_undo() {
    let mut app = app_with_empty_sheet();
    tool(&mut app, ToolMessage::SelectTool(Tool::Wire));
    click(&mut app, (10.0, 20.0));
    click(&mut app, (30.0, 20.0));
    let id = wires(&app)[0].uuid;

    panel(
        &mut app,
        PanelMsg::EditWireWidth(id, WireWidthPreset::Medium),
    );
    panel(&mut app, PanelMsg::EditWireColor(id, Some(RED)));
    let wire = wires(&app)[0].clone();
    assert_eq!(wire.stroke_width, 0.5);
    assert_eq!(wire.color, Some(RED));

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(wires(&app)[0].color, None, "colour undoes first");
    assert_eq!(wires(&app)[0].stroke_width, 0.5);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(wires(&app)[0].stroke_width, 0.0);

    let _ = app.update(Message::Edit(EditMsg::Redo));
    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(wires(&app)[0].color, Some(RED));

    // Back to the theme colour is an edit too.
    panel(&mut app, PanelMsg::EditWireColor(id, None));
    assert_eq!(wires(&app)[0].color, None);
}
