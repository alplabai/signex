//! Mockup row 41: the pre-placement form a placement tool opens on Tab.

use super::support::{Bench, bench, empty_sheet};
use signex_app::app::{InputMsg, Message, Tool};
use signex_app::panels::{PanelMsg, PositionField, PrePlacementPositionMsg};
use signex_app::toolbar::ToolMessage;
use signex_types::schematic::{HAlign, Point, SCHEMATIC_PT_TO_MM, VAlign, WireWidthPreset};
use signex_types::theme::Color;

const RED: Color = Color::new(255, 0, 0, 255);

fn paused_in(tool: Tool) -> Bench {
    let mut b = bench(empty_sheet());
    let _ = b.app.update(Message::Tool(ToolMessage::SelectTool(tool)));
    let _ = b.app.update(Message::Tool(ToolMessage::PrePlacementTab));
    assert!(b.app.interaction_state.active_canvas().placement_paused);
    b
}

fn type_field(b: &mut Bench, field: PositionField, text: &str) {
    b.panel(PanelMsg::PrePlacementPosition(
        PrePlacementPositionMsg::FieldTyped(field, text.to_string()),
    ));
}

fn type_point(b: &mut Bench, x: &str, y: &str) {
    type_field(b, PositionField::X, x);
    type_field(b, PositionField::Y, y);
}

fn esc(b: &mut Bench) {
    let _ = b
        .app
        .update(Message::Input(InputMsg::EscapePressed { window: None }));
}

fn stored_y(b: &Bench, shown: f64) -> f64 {
    b.app.document_state.panel_ctx.shown_y(shown)
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

#[test]
fn m41_a_typed_position_and_ok_place_the_label_there_and_undo_removes_it() {
    let mut b = paused_in(Tool::Label);
    type_point(&mut b, "101.6", "76.2");
    assert!(b.sheet().labels.is_empty(), "typing places nothing");
    b.panel(PanelMsg::ConfirmPrePlacement);
    assert_eq!(b.sheet().labels.len(), 1);
    let want = Point::new(101.6, stored_y(&b, 76.2));
    assert!(near(b.sheet().labels[0].position, want));
    assert!(b.dirty());
    assert!(!b.app.interaction_state.active_canvas().placement_paused);
    b.undo();
    assert!(b.sheet().labels.is_empty());
    assert!(!b.dirty());
}

#[test]
fn m41_the_form_values_reach_the_placed_label() {
    let mut b = paused_in(Tool::Label);
    b.panel(PanelMsg::SetPrePlacementText("SDA".into()));
    b.panel(PanelMsg::SetPrePlacementRotation(90.0));
    b.panel(PanelMsg::SetPrePlacementFontSize(12));
    b.panel(PanelMsg::PrePlacementPosition(
        PrePlacementPositionMsg::AnchorPicked(HAlign::Right, VAlign::Top),
    ));
    type_point(&mut b, "50.8", "50.8");
    b.panel(PanelMsg::ConfirmPrePlacement);
    let placed = &b.sheet().labels[0];
    assert_eq!(placed.text, "SDA");
    assert_eq!(placed.rotation, 90.0);
    assert_eq!(placed.font_size, 12.0 * SCHEMATIC_PT_TO_MM);
    assert_eq!(
        (placed.justify, placed.justify_v),
        (HAlign::Right, VAlign::Top)
    );
}

#[test]
fn m41_esc_drops_the_edits_places_nothing_and_goes_back_to_placing() {
    let mut b = paused_in(Tool::Label);
    let form = b.app.document_state.panel_ctx.pre_placement.as_ref();
    let rotation_at_tab = form.expect("Tab opens the form").rotation;
    type_point(&mut b, "101.6", "76.2");
    b.panel(PanelMsg::SetPrePlacementRotation(90.0));
    esc(&mut b);
    assert!(b.sheet().labels.is_empty());
    assert!(!b.dirty());
    assert_eq!(b.app.interaction_state.current_tool, Tool::Label);
    assert!(!b.app.interaction_state.active_canvas().placement_paused);
    // Decision 2026-11-20 (#2529): back to how it was at the Tab, here no form.
    assert!(b.app.document_state.panel_ctx.pre_placement.is_none());

    let _ = b.app.update(Message::Tool(ToolMessage::PrePlacementTab));
    let form = b.app.document_state.panel_ctx.pre_placement.as_ref();
    let form = form.expect("the next Tab opens the form");
    assert_eq!(form.position.value(PositionField::X), None);
    assert_eq!(form.rotation, rotation_at_tab, "the dropped edit is gone");
}

#[test]
fn m41_ok_with_an_unreadable_position_is_refused_and_stays_paused() {
    let mut b = paused_in(Tool::Label);
    type_point(&mut b, "50", "12a");
    b.panel(PanelMsg::ConfirmPrePlacement);
    assert!(b.sheet().labels.is_empty());
    assert!(!b.dirty());
    assert!(b.app.interaction_state.active_canvas().placement_paused);
}

#[test]
fn m41_typed_wire_start_and_end_place_one_wire_with_the_form_style() {
    let mut b = paused_in(Tool::Wire);
    b.panel(PanelMsg::SetPrePlacementWireWidth(WireWidthPreset::Large));
    b.panel(PanelMsg::SetPrePlacementWireColor(Some(RED)));
    type_point(&mut b, "88.9", "63.5");
    type_field(&mut b, PositionField::EndX, "114.3");
    type_field(&mut b, PositionField::EndY, "63.5");
    b.panel(PanelMsg::ConfirmPrePlacement);

    let wires = b.sheet().wires.clone();
    assert_eq!(wires.len(), 1);
    let y = stored_y(&b, 63.5);
    assert!(near(wires[0].start, Point::new(88.9, y)));
    assert!(near(wires[0].end, Point::new(114.3, y)));
    assert_eq!(wires[0].stroke_width, WireWidthPreset::Large.stroke_mm());
    assert_eq!(wires[0].color, Some(RED));
    b.undo();
    assert!(b.sheet().wires.is_empty(), "one undo step");
}

#[test]
fn m41_a_tool_with_no_other_options_still_places_at_the_typed_position() {
    let mut b = paused_in(Tool::Junction);
    type_point(&mut b, "114.3", "76.2");
    b.panel(PanelMsg::ConfirmPrePlacement);
    assert_eq!(b.sheet().junctions.len(), 1);
    let want = Point::new(114.3, stored_y(&b, 76.2));
    assert!(near(b.sheet().junctions[0].position, want));
}
