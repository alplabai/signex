//! Mockup rows 42 (Part), 45 (Power port) and 50 (Pin of a placed part).

use super::support::{Bench, bench, bench_as_is, empty_sheet, part};
use signex_app::app::Message;
use signex_app::canvas::CanvasEvent;
use signex_app::panels::net_inspector::NetInspectorTab;
use signex_app::panels::{PanelMsg, PartPanelMsg, PlacedEditMsg, PlacedField};
use signex_types::format::SnxSchematic;
use signex_types::schematic::{
    PowerPortStyle, SCHEMATIC_PT_TO_MM, SelectedItem, SelectedKind, Symbol, power_port_lib_symbol,
};
use uuid::Uuid;

fn part_bench() -> (Bench, Uuid, SelectedItem) {
    let mut sheet = empty_sheet();
    sheet.symbols.push(part("R1", "10k", (50.0, 50.0)));
    let id = sheet.symbols[0].uuid;
    (
        bench(sheet),
        id,
        SelectedItem::new(id, SelectedKind::Symbol),
    )
}

fn sym(b: &Bench) -> &Symbol {
    &b.sheet().symbols[0]
}

#[test]
fn m42_editing_the_value_dirties_the_sheet_and_undo_restores_it() {
    let (mut b, id, _) = part_bench();
    b.panel(PanelMsg::EditSymbolValue(id, "4k7".into()));
    assert_eq!(sym(&b).value, "4k7");
    assert!(b.dirty());
    b.undo();
    assert_eq!(sym(&b).value, "10k");
    assert!(!b.dirty());
    b.redo();
    assert_eq!(sym(&b).value, "4k7");
}

#[test]
fn m42_editing_the_designator_renames_the_part() {
    let (mut b, id, _) = part_bench();
    b.panel(PanelMsg::EditSymbolDesignator(id, "R9".into()));
    assert_eq!(sym(&b).reference, "R9");
    b.undo();
    assert_eq!(sym(&b).reference, "R1");
}

#[test]
fn m42_editing_the_footprint_changes_it_and_undoes() {
    let (mut b, id, _) = part_bench();
    b.panel(PanelMsg::EditSymbolFootprint(id, "R_0805".into()));
    assert_eq!(sym(&b).footprint, "R_0805");
    b.undo();
    assert_eq!(sym(&b).footprint, "R_0603");
}

#[test]
fn m42_mirror_x_and_mirror_y_each_set_their_own_flag() {
    let (mut b, id, _) = part_bench();
    b.panel(PanelMsg::ToggleSymbolMirrorX(id));
    assert!(sym(&b).mirror_x && !sym(&b).mirror_y);
    b.undo();
    assert!(!sym(&b).mirror_x);
    b.panel(PanelMsg::ToggleSymbolMirrorY(id));
    assert!(sym(&b).mirror_y && !sym(&b).mirror_x);
}

#[test]
fn m42_dnp_toggles_and_undoes() {
    let (mut b, id, _) = part_bench();
    b.panel(PanelMsg::ToggleSymbolDnp(id));
    assert!(sym(&b).dnp);
    assert!(b.dirty());
    b.undo();
    assert!(!sym(&b).dnp);
}

#[test]
fn m42_a_locked_part_refuses_value_footprint_move_and_turn_but_takes_dnp() {
    let (mut b, id, item) = part_bench();
    b.panel(PanelMsg::ToggleSymbolLocked(id));
    assert!(sym(&b).locked);
    let at = sym(&b).position;

    b.panel(PanelMsg::EditSymbolValue(id, "4k7".into()));
    b.panel(PanelMsg::EditSymbolFootprint(id, "R_0805".into()));
    b.enter(item, PlacedField::X, "80");
    b.panel(PanelMsg::PlacedEdit(PlacedEditMsg::RotationPicked {
        item,
        degrees: 90.0,
    }));
    assert_eq!(sym(&b).value, "10k");
    assert_eq!(sym(&b).footprint, "R_0603");
    assert_eq!(sym(&b).position, at);
    assert_eq!(sym(&b).rotation, 0.0);

    b.panel(PanelMsg::ToggleSymbolDnp(id));
    assert!(sym(&b).dnp, "the lock leaves DNP open");
}

#[test]
fn m42_a_typed_position_moves_the_part_in_one_undo_step() {
    let (mut b, _, item) = part_bench();
    b.type_in(item, PlacedField::X, "80");
    assert_eq!(sym(&b).position.x, 50.0, "typing alone moves nothing");
    b.panel(PanelMsg::PlacedEdit(PlacedEditMsg::Submitted {
        item,
        field: PlacedField::X,
    }));
    assert_eq!(sym(&b).position.x, 80.0);
    assert!(!b.has_draft());
    b.undo();
    assert_eq!(sym(&b).position.x, 50.0);
    assert!(!b.dirty());
}

#[test]
fn m42_a_position_that_is_not_a_number_is_refused_and_the_text_stays() {
    let (mut b, _, item) = part_bench();
    b.enter(item, PlacedField::X, "12a");
    assert_eq!(sym(&b).position.x, 50.0);
    assert!(b.has_draft(), "the user can fix what was typed");
    assert!(!b.dirty());
}

#[test]
fn m42_the_rotation_control_turns_the_part() {
    let (mut b, _, item) = part_bench();
    b.panel(PanelMsg::PlacedEdit(PlacedEditMsg::RotationPicked {
        item,
        degrees: 90.0,
    }));
    assert_eq!(sym(&b).rotation, 90.0);
    b.undo();
    assert_eq!(sym(&b).rotation, 0.0);
}

#[test]
fn m42_a_parameter_value_is_set_and_undoes() {
    let (mut b, _, item) = part_bench();
    let field = PlacedField::Parameter("MPN".into());
    b.enter(item, field, "RC0603FR-0710KL");
    assert_eq!(sym(&b).fields["MPN"], "RC0603FR-0710KL");
    b.undo();
    assert!(!sym(&b).fields.contains_key("MPN"));
}

#[test]
fn m42_a_parameter_named_after_a_fixed_row_is_refused() {
    let (mut b, _, item) = part_bench();
    b.enter(item, PlacedField::Parameter("Value".into()), "1k");
    assert_eq!(sym(&b).value, "10k");
    assert!(!sym(&b).fields.contains_key("Value"));
    assert!(b.has_draft());
    assert!(!b.dirty());
}

#[test]
fn m42_the_more_menu_opens_and_closes_without_touching_the_sheet() {
    let (mut b, id, _) = part_bench();
    b.panel(PanelMsg::Part(PartPanelMsg::MenuToggled(id)));
    assert_eq!(b.app.document_state.panel_ctx.part_panel.menu_for, Some(id));
    b.panel(PanelMsg::Part(PartPanelMsg::MenuToggled(id)));
    assert_eq!(b.app.document_state.panel_ctx.part_panel.menu_for, None);
    assert!(!b.dirty());
}

// ── Row 45: power port ──

fn port_bench() -> (Bench, Uuid) {
    let lib_id = format!("power:{}", PowerPortStyle::Bar.lib_tag());
    let mut sheet = empty_sheet();
    let body = power_port_lib_symbol(&lib_id);
    sheet.lib_symbols.insert(body.id.clone(), body);
    let mut port = part("#PWR1", "+3V3", (60.0, 60.0));
    port.lib_id = lib_id;
    port.is_power = true;
    let id = port.uuid;
    sheet.symbols.push(port);
    (bench(sheet), id)
}

fn earth(id: Uuid) -> PanelMsg {
    PanelMsg::EditPowerPortStyle {
        symbol_id: id,
        new_lib_id: format!("power:{}", PowerPortStyle::Earth.lib_tag()),
        rotation_degrees: 180.0,
    }
}

#[test]
fn m45_picking_a_style_changes_the_glyph_and_turn_in_one_undo_step() {
    let (mut b, id) = port_bench();
    let before = sym(&b).lib_id.clone();
    b.panel(earth(id));
    assert_eq!(PowerPortStyle::of(&sym(&b).lib_id), PowerPortStyle::Earth);
    assert_eq!(sym(&b).rotation, 180.0);
    assert!(b.dirty());
    b.undo();
    assert_eq!(sym(&b).lib_id, before);
    assert_eq!(sym(&b).rotation, 0.0);
    assert!(!b.dirty());
}

#[test]
fn m45_the_name_edits_the_net_name_and_undoes() {
    let (mut b, id) = port_bench();
    b.panel(PanelMsg::EditSymbolValue(id, "+5V".into()));
    assert_eq!(sym(&b).value, "+5V");
    b.undo();
    assert_eq!(sym(&b).value, "+3V3");
}

#[test]
fn m45_the_font_size_is_stored_in_mm() {
    let (mut b, id) = port_bench();
    b.panel(PanelMsg::EditSymbolValueFontSizePt(id, 12));
    let size = sym(&b).val_text.as_ref().expect("value text").font_size;
    assert!((size - 12.0 * SCHEMATIC_PT_TO_MM).abs() < 1e-9, "{size}");
    b.undo();
    let size = sym(&b).val_text.as_ref().expect("value text").font_size;
    assert!((size - 1.27).abs() < 1e-9, "{size}");
}

#[test]
fn m45_a_locked_port_refuses_a_new_style_whole() {
    let (mut b, id) = port_bench();
    b.panel(PanelMsg::ToggleSymbolLocked(id));
    let before = sym(&b).lib_id.clone();
    b.panel(earth(id));
    assert_eq!(sym(&b).lib_id, before);
    assert_eq!(sym(&b).rotation, 0.0, "no half-applied style");
}

// ── Row 50: a pin of a placed part ──

/// U1's input pin of the sample sheet: the click lands on the pin, past the
/// wire end at 78.74 mm.
const U1_PIN: (f64, f64) = (80.01, 99.06);

fn sample_bench() -> Bench {
    let file = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/sample-project/linear-regulator.snxsch"
    );
    let text = std::fs::read_to_string(file).expect("the sample sheet");
    bench_as_is(SnxSchematic::parse(&text).expect("the sample parses").sheet)
}

fn click(b: &mut Bench, (world_x, world_y): (f64, f64)) {
    let _ = b.app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x,
        world_y,
    }));
}

#[test]
fn m50_a_click_on_a_pin_keeps_the_part_selected_and_focuses_the_pin() {
    let mut b = sample_bench();
    click(&mut b, U1_PIN);
    let canvas = b.app.interaction_state.active_canvas();
    assert_eq!(canvas.selected.len(), 1);
    assert_eq!(canvas.selected[0].kind, SelectedKind::Symbol);
    let (owner, _) = canvas.focused_pin.clone().expect("a focused pin");
    assert_eq!(owner, canvas.selected[0].uuid);
    let info = &b.app.document_state.panel_ctx.selection_info;
    assert!(info.iter().any(|(key, _)| key == "Focused Pin"), "{info:?}");
}

#[test]
fn m50_focusing_a_pin_edits_nothing() {
    let mut b = sample_bench();
    let before = b.sheet().clone();
    click(&mut b, U1_PIN);
    assert_eq!(b.sheet().symbols, before.symbols);
    assert!(!b.dirty());
}

#[test]
fn m50_a_click_on_empty_sheet_drops_the_pin_focus() {
    let mut b = sample_bench();
    click(&mut b, U1_PIN);
    click(&mut b, (5.0, 5.0));
    assert_eq!(b.app.interaction_state.active_canvas().focused_pin, None);
}

#[test]
fn m50_show_net_opens_the_net_inspector_on_that_net() {
    let mut b = sample_bench();
    click(&mut b, U1_PIN);
    b.panel(PanelMsg::Part(PartPanelMsg::PinNetShowRequested(
        "+5V".into(),
    )));
    let inspector = &b.app.ui_state.net_inspector;
    assert_eq!(inspector.tab, NetInspectorTab::ByNet);
    assert_eq!(inspector.filter, "+5V");
    assert!(inspector.expanded.contains("+5V"));
    assert!(!b.dirty());
}
