//! #2199 — the connection rule of a part, sheet symbol or entry placed from
//! operands: what a new pin tip or entry lands on.

use signex_types::schematic::{ChildSheet, FillType, SheetPin};

use super::*;
use crate::build::{ExtraKind, placement_contacts};

/// A sheet with part `P` (pins 1 at x 0 and 2 at x 10) registered.
fn with_lib() -> SchematicSheet {
    let mut sheet = empty_sheet();
    add_lib(
        &mut sheet,
        "P",
        vec![
            lib_pin("1", pt(0.0, 0.0), PinDirection::Passive),
            lib_pin("2", pt(10.0, 0.0), PinDirection::Passive),
        ],
    );
    sheet
}

/// `before` plus `U9` placed at `at`.
fn placed(before: &SchematicSheet, at: (f64, f64)) -> SchematicSheet {
    let mut after = before.clone();
    place(&mut after, "U9", "P", pt(at.0, at.1));
    after
}

fn kinds(found: &[crate::ExtraItem]) -> Vec<(ExtraKind, String)> {
    found.iter().map(|i| (i.kind, i.identity.clone())).collect()
}

#[test]
fn a_part_in_free_space_lands_on_nothing() {
    let mut before = with_lib();
    before.wires.push(wire(pt(0.0, 50.0), pt(20.0, 50.0)));
    assert!(placement_contacts(&before, &placed(&before, (0.0, 0.0))).is_empty());
}

#[test]
fn a_pin_tip_on_a_wire_end_a_wire_run_a_junction_or_a_label_is_reported() {
    let mut before = with_lib();
    before.wires.push(wire(pt(-10.0, 0.0), pt(0.0, 0.0)));
    let found = placement_contacts(&before, &placed(&before, (0.0, 0.0)));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, ExtraKind::Wire);
    assert_eq!(found[0].position, pt(0.0, 0.0));

    // Pin 2 at (10, 0) on the interior of a vertical wire.
    let mut before = with_lib();
    before.wires.push(wire(pt(10.0, -5.0), pt(10.0, 5.0)));
    let found = placement_contacts(&before, &placed(&before, (0.0, 0.0)));
    assert_eq!(found[0].position, pt(10.0, 0.0), "touched where pin 2 is");

    let mut before = with_lib();
    before.junctions.push(junction(pt(10.0, 0.0)));
    before
        .labels
        .push(label("SDA", pt(0.0, 0.0), LabelType::Net));
    let found = placement_contacts(&before, &placed(&before, (0.0, 0.0)));
    assert_eq!(
        kinds(&found).iter().map(|(k, _)| *k).collect::<Vec<_>>(),
        [ExtraKind::Label, ExtraKind::Junction]
    );
    assert_eq!(found[0].identity, "SDA");
}

#[test]
fn a_pin_tip_on_another_pin_tip_is_reported_though_pin_to_pin_joins_nothing() {
    let mut before = with_lib();
    place(&mut before, "R1", "P", pt(10.0, 0.0));
    // U9's pin 2 lands on R1's pin 1.
    let found = placement_contacts(&before, &placed(&before, (0.0, 0.0)));
    assert_eq!(kinds(&found), [(ExtraKind::Pin, "R1.1".to_string())]);
}

fn symbol_with(pins: Vec<SheetPin>) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::now_v7(),
        name: "Charger".into(),
        filename: "charger.snxsch".into(),
        position: pt(20.0, 20.0),
        size: (20.0, 20.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins,
        instances: Vec::new(),
    }
}

fn entry(name: &str, at: Point) -> SheetPin {
    SheetPin {
        uuid: Uuid::now_v7(),
        name: name.into(),
        direction: "input".into(),
        position: at,
        rotation: 0.0,
        auto_generated: false,
        user_moved: false,
    }
}

#[test]
fn a_new_entry_on_a_wire_end_is_reported_and_an_old_one_is_not() {
    let mut before = with_lib();
    before.wires.push(wire(pt(10.0, 25.0), pt(20.0, 25.0)));
    before.child_sheets.push(symbol_with(Vec::new()));
    let mut after = before.clone();
    after.child_sheets[0]
        .pins
        .push(entry("SDA", pt(20.0, 25.0)));
    let found = placement_contacts(&before, &after);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, ExtraKind::Wire);

    // The same entry already on the sheet: nothing is new.
    assert!(placement_contacts(&after, &after).is_empty());
}

#[test]
fn a_part_pin_on_a_sheet_entry_names_the_entry() {
    let mut before = with_lib();
    before
        .child_sheets
        .push(symbol_with(vec![entry("SDA", pt(20.0, 25.0))]));
    let found = placement_contacts(&before, &placed(&before, (10.0, 25.0)));
    assert_eq!(
        kinds(&found),
        [(ExtraKind::SheetPin, "SDA of Charger".to_string())]
    );
}
