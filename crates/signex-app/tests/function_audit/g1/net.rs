//! Mockup rows 43 (Wire / bus) and 44 (Net label / port).

use super::support::{Bench, bench, empty_sheet, label, wire};
use signex_app::panels::{PanelMsg, PlacedEditMsg, PlacedField, WireColorPicker};
use signex_types::schematic::{
    Bus, HAlign, LabelType, Point, SCHEMATIC_PT_TO_MM, SelectedItem, SelectedKind, WireWidthPreset,
};
use signex_types::theme::Color;
use uuid::Uuid;

const RED: Color = Color::new(255, 0, 0, 255);

fn wire_bench() -> (Bench, Uuid, SelectedItem) {
    let mut sheet = empty_sheet();
    sheet.wires.push(wire((10.0, 20.0), (30.0, 20.0), 0.0));
    let id = sheet.wires[0].uuid;
    (bench(sheet), id, SelectedItem::new(id, SelectedKind::Wire))
}

#[test]
fn m43_a_width_preset_sets_the_wire_and_undo_restores_it() {
    let (mut b, id, _) = wire_bench();
    b.panel(PanelMsg::EditWireWidth(id, WireWidthPreset::Large));
    assert_eq!(
        b.sheet().wires[0].stroke_width,
        WireWidthPreset::Large.stroke_mm()
    );
    assert!(b.dirty());
    b.undo();
    assert_eq!(b.sheet().wires[0].stroke_width, 0.0);
    assert!(!b.dirty());
}

#[test]
fn m43_a_colour_is_set_and_reset_to_default_and_closes_the_menu() {
    let (mut b, id, _) = wire_bench();
    b.panel(PanelMsg::ToggleWireColorPicker);
    b.panel(PanelMsg::EditWireColor(id, Some(RED)));
    assert_eq!(b.sheet().wires[0].color, Some(RED));
    assert!(matches!(
        b.app.document_state.panel_ctx.wire_color_picker,
        WireColorPicker::Closed
    ));
    b.panel(PanelMsg::EditWireColor(id, None));
    assert_eq!(b.sheet().wires[0].color, None);
}

#[test]
fn m43_cancelling_the_colour_menu_leaves_the_wire() {
    let (mut b, _, _) = wire_bench();
    b.panel(PanelMsg::ToggleWireColorPicker);
    assert!(matches!(
        b.app.document_state.panel_ctx.wire_color_picker,
        WireColorPicker::Palette
    ));
    b.panel(PanelMsg::CancelWireColorPicker);
    assert!(matches!(
        b.app.document_state.panel_ctx.wire_color_picker,
        WireColorPicker::Closed
    ));
    assert_eq!(b.sheet().wires[0].color, None);
    assert!(!b.dirty());
}

#[test]
fn m43_start_and_end_each_stretch_their_own_end() {
    let (mut b, _, item) = wire_bench();
    b.enter(item, PlacedField::X, "5");
    assert_eq!(b.sheet().wires[0].start, Point::new(5.0, 20.0));
    assert_eq!(b.sheet().wires[0].end, Point::new(30.0, 20.0));
    b.enter(item, PlacedField::EndX, "40");
    assert_eq!(b.sheet().wires[0].start, Point::new(5.0, 20.0));
    assert_eq!(b.sheet().wires[0].end, Point::new(40.0, 20.0));
    b.undo();
    assert_eq!(b.sheet().wires[0].end, Point::new(30.0, 20.0));
    b.undo();
    assert_eq!(b.sheet().wires[0].start, Point::new(10.0, 20.0));
    assert!(!b.dirty());
}

#[test]
fn m43_an_end_typed_onto_the_start_is_refused() {
    let (mut b, _, item) = wire_bench();
    b.enter(item, PlacedField::EndX, "10");
    assert_eq!(b.sheet().wires[0].end, Point::new(30.0, 20.0));
    assert!(b.has_draft(), "the text stays to fix");
    assert!(!b.dirty());
}

#[test]
fn m43_an_end_that_is_not_a_number_is_refused() {
    let (mut b, _, item) = wire_bench();
    b.enter(item, PlacedField::EndX, "far");
    assert_eq!(b.sheet().wires[0].end, Point::new(30.0, 20.0));
    assert!(!b.dirty());
}

#[test]
fn m43_a_locked_wire_refuses_width_colour_and_ends_until_unlocked() {
    let (mut b, id, item) = wire_bench();
    b.panel(PanelMsg::ToggleItemLocked(item));
    assert!(b.sheet().wires[0].locked);

    b.panel(PanelMsg::EditWireWidth(id, WireWidthPreset::Large));
    b.panel(PanelMsg::EditWireColor(id, Some(RED)));
    b.enter(item, PlacedField::EndX, "40");
    assert_eq!(b.sheet().wires[0].stroke_width, 0.0);
    assert_eq!(b.sheet().wires[0].color, None);
    assert_eq!(b.sheet().wires[0].end, Point::new(30.0, 20.0));

    b.panel(PanelMsg::ToggleItemLocked(item));
    b.panel(PanelMsg::EditWireWidth(id, WireWidthPreset::Large));
    assert_eq!(
        b.sheet().wires[0].stroke_width,
        WireWidthPreset::Large.stroke_mm()
    );
}

#[test]
fn m43_a_bus_end_is_edited_like_a_wire_end() {
    let mut sheet = empty_sheet();
    let id = Uuid::now_v7();
    sheet.buses.push(Bus {
        uuid: id,
        start: Point::new(10.0, 40.0),
        end: Point::new(30.0, 40.0),
    });
    let mut b = bench(sheet);
    let item = SelectedItem::new(id, SelectedKind::Bus);
    b.enter(item, PlacedField::EndX, "50");
    assert_eq!(b.sheet().buses[0].end, Point::new(50.0, 40.0));
    assert_eq!(b.sheet().buses[0].start, Point::new(10.0, 40.0));
    b.undo();
    assert_eq!(b.sheet().buses[0].end, Point::new(30.0, 40.0));
}

// ── Row 44: net label and port ──

fn label_bench(kind: LabelType) -> (Bench, Uuid, SelectedItem) {
    let mut sheet = empty_sheet();
    let mut placed = label("SDA", (40.0, 40.0));
    placed.label_type = kind;
    let id = placed.uuid;
    sheet.labels.push(placed);
    (bench(sheet), id, SelectedItem::new(id, SelectedKind::Label))
}

#[test]
fn m44_the_net_name_is_edited_and_undoes() {
    let (mut b, id, _) = label_bench(LabelType::Net);
    b.panel(PanelMsg::EditLabelText(id, "SCL".into()));
    assert_eq!(b.sheet().labels[0].text, "SCL");
    assert!(b.dirty());
    b.undo();
    assert_eq!(b.sheet().labels[0].text, "SDA");
    assert!(!b.dirty());
}

#[test]
fn m44_a_port_name_is_edited_and_keeps_the_port_kind() {
    let (mut b, id, _) = label_bench(LabelType::Global);
    b.panel(PanelMsg::EditLabelText(id, "RESET".into()));
    assert_eq!(b.sheet().labels[0].text, "RESET");
    assert_eq!(b.sheet().labels[0].label_type, LabelType::Global);
}

#[test]
fn m44_the_justification_moves_the_text_anchor_and_undoes() {
    let (mut b, id, _) = label_bench(LabelType::Net);
    b.panel(PanelMsg::EditLabelJustifyH(id, HAlign::Right));
    assert_eq!(b.sheet().labels[0].justify, HAlign::Right);
    b.undo();
    assert_eq!(b.sheet().labels[0].justify, HAlign::Left);
}

#[test]
fn m44_a_direction_sets_rotation_and_justification_in_one_undo_step() {
    let (mut b, id, _) = label_bench(LabelType::Net);
    b.panel(PanelMsg::EditLabelDirection(id, 90.0, HAlign::Right));
    assert_eq!(b.sheet().labels[0].rotation, 90.0);
    assert_eq!(b.sheet().labels[0].justify, HAlign::Right);
    b.undo();
    assert_eq!(b.sheet().labels[0].rotation, 0.0);
    assert_eq!(b.sheet().labels[0].justify, HAlign::Left);
}

#[test]
fn m44_the_font_size_is_stored_in_mm() {
    let (mut b, id, _) = label_bench(LabelType::Net);
    b.panel(PanelMsg::EditLabelFontSizePt(id, 12));
    assert_eq!(b.sheet().labels[0].font_size, 12.0 * SCHEMATIC_PT_TO_MM);
}

#[test]
fn m44_the_rotation_control_turns_the_label() {
    let (mut b, _, item) = label_bench(LabelType::Net);
    b.panel(PanelMsg::PlacedEdit(PlacedEditMsg::RotationPicked {
        item,
        degrees: 90.0,
    }));
    assert_eq!(b.sheet().labels[0].rotation, 90.0);
}

#[test]
fn m44_a_position_that_is_not_a_number_is_refused() {
    let (mut b, _, item) = label_bench(LabelType::Net);
    b.enter(item, PlacedField::X, "left");
    assert_eq!(b.sheet().labels[0].position.x, 40.0);
    assert!(b.has_draft());
    assert!(!b.dirty());
}

#[test]
fn m44_a_locked_label_refuses_its_name_and_justification() {
    let (mut b, id, item) = label_bench(LabelType::Net);
    b.panel(PanelMsg::ToggleItemLocked(item));
    b.panel(PanelMsg::EditLabelText(id, "SCL".into()));
    b.panel(PanelMsg::EditLabelJustifyH(id, HAlign::Right));
    assert_eq!(b.sheet().labels[0].text, "SDA");
    assert_eq!(b.sheet().labels[0].justify, HAlign::Left);
}
