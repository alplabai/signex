//! `Command::ChangeComponent` (#1260): the swap, its single undo step, and
//! the preview of what it does to connected pins.

use super::change_component::{ComponentReplacement, DataChange, change_component_preview};
use crate::test_support::test_sheet;
use crate::{Command, Engine};
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, Symbol, Wire,
};
use uuid::Uuid;

const OLD: &str = "L:OLD";
const NEW: &str = "L:NEW";

fn lib_pin(number: &str, name: &str, x: f64) -> LibPin {
    LibPin {
        unit: 1,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 2.54,
            name: name.to_string(),
            number: number.to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

fn lib_symbol(id: &str, pins: Vec<LibPin>) -> LibSymbol {
    let mut lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": id })).unwrap();
    lib.pins = pins;
    lib
}

fn replacement(pins: Vec<LibPin>) -> ComponentReplacement {
    ComponentReplacement {
        lib_symbol: lib_symbol(NEW, pins),
        library_id: Some(Uuid::from_u128(0xA)),
        row_id: Some(Uuid::from_u128(0xB)),
        library_version: "2.0".into(),
        value: "NEW-MPN".into(),
        footprint: "L:FP-NEW".into(),
        datasheet: "https://new.example/d.pdf".into(),
        fields: vec![
            ("MPN".into(), "NEW-MPN".into()),
            ("Manufacturer".into(), String::new()),
        ],
    }
}

/// U1 at (10, 10) drawn as OLD (pins "1" at x=-2.54 and "2" at x=2.54),
/// with a wire on pin "2" (world 12.54, 10).
fn engine_with_wired_pin_2() -> (Engine, Uuid) {
    let mut sheet = test_sheet();
    sheet.lib_symbols.insert(
        OLD.into(),
        lib_symbol(
            OLD,
            vec![lib_pin("1", "IN", -2.54), lib_pin("2", "OUT", 2.54)],
        ),
    );
    let id = Uuid::from_u128(1);
    let mut fields = std::collections::HashMap::new();
    fields.insert("Tolerance".to_string(), "1%".to_string());
    sheet.symbols.push(Symbol {
        uuid: id,
        lib_id: OLD.into(),
        reference: "U1".into(),
        value: "OLD-MPN".into(),
        footprint: "R_0603".into(),
        position: Point::new(10.0, 10.0),
        unit: 1,
        fields,
        library_id: Some(Uuid::from_u128(0x1)),
        row_id: Some(Uuid::from_u128(0x2)),
        library_version: "1.0".into(),
        ..Default::default()
    });
    sheet.wires.push(Wire {
        start: Point::new(12.54, 10.0),
        end: Point::new(20.0, 10.0),
        ..Default::default()
    });
    (Engine::new(sheet).unwrap(), id)
}

fn swap_pins() -> Vec<LibPin> {
    vec![lib_pin("1", "IN", -2.54), lib_pin("2", "OUT", 2.54)]
}

#[test]
fn the_swap_re_keys_the_symbol_and_replaces_part_owned_data() {
    let (mut engine, id) = engine_with_wired_pin_2();
    let result = engine
        .execute(Command::ChangeComponent {
            symbol_id: id,
            replacement: replacement(swap_pins()),
        })
        .unwrap();
    assert!(result.changed);
    let doc = engine.document();
    let sym = &doc.symbols[0];
    assert_eq!(sym.lib_id, NEW);
    assert!(doc.lib_symbols.contains_key(NEW));
    assert_eq!(sym.library_id, Some(Uuid::from_u128(0xA)));
    assert_eq!(sym.row_id, Some(Uuid::from_u128(0xB)));
    assert_eq!(sym.library_version, "2.0");
    assert_eq!(sym.value, "NEW-MPN");
    assert_eq!(sym.reference, "U1", "the designator stays");
    assert_eq!(sym.footprint, "L:FP-NEW", "the footprint is the new part's");
    assert_eq!(sym.datasheet, "https://new.example/d.pdf");
    assert_eq!(sym.fields.get("MPN").map(String::as_str), Some("NEW-MPN"));
    assert!(
        !sym.fields.contains_key("Manufacturer"),
        "an identity field the new part leaves empty is cleared, not left stale"
    );
    assert_eq!(
        sym.fields.get("Tolerance").map(String::as_str),
        Some("1%"),
        "a user parameter the new part does not define stays"
    );
    assert_eq!(sym.position, Point::new(10.0, 10.0));
}

#[test]
fn the_whole_swap_is_one_undo_step() {
    let (mut engine, id) = engine_with_wired_pin_2();
    engine
        .execute(Command::ChangeComponent {
            symbol_id: id,
            replacement: replacement(swap_pins()),
        })
        .unwrap();
    engine.undo().unwrap();
    let doc = engine.document();
    assert_eq!(doc.symbols[0].lib_id, OLD);
    assert_eq!(doc.symbols[0].value, "OLD-MPN");
    assert_eq!(doc.symbols[0].row_id, Some(Uuid::from_u128(0x2)));
    assert!(
        !doc.lib_symbols.contains_key(NEW),
        "the new drawing is gone"
    );
    assert!(!engine.can_undo(), "one undo undid all of it");
}

#[test]
fn a_copy_of_the_new_part_the_sheet_already_holds_wins() {
    let (mut engine, id) = engine_with_wired_pin_2();
    let mut held = lib_symbol(NEW, swap_pins());
    held.description = "held".into();
    let mut doc = engine.document().clone();
    doc.lib_symbols.insert(NEW.into(), held);
    engine.set_document(doc);
    engine
        .execute(Command::ChangeComponent {
            symbol_id: id,
            replacement: replacement(swap_pins()),
        })
        .unwrap();
    assert_eq!(engine.document().lib_symbols[NEW].description, "held");
}

#[test]
fn an_unknown_symbol_changes_nothing() {
    let (mut engine, _) = engine_with_wired_pin_2();
    let result = engine
        .execute(Command::ChangeComponent {
            symbol_id: Uuid::from_u128(99),
            replacement: replacement(swap_pins()),
        })
        .unwrap();
    assert!(!result.changed);
    assert!(!engine.can_undo());
}

#[test]
fn pin_uuids_follow_the_mapping() {
    let (mut engine, id) = engine_with_wired_pin_2();
    let (a, b) = (Uuid::from_u128(0xA1), Uuid::from_u128(0xB1));
    let mut doc = engine.document().clone();
    doc.symbols[0].pin_uuids.insert("1".into(), a);
    doc.symbols[0].pin_uuids.insert("2".into(), b);
    engine.set_document(doc);
    // The new part numbers the old pin "2" (OUT) as "9" and has no pin "1".
    engine
        .execute(Command::ChangeComponent {
            symbol_id: id,
            replacement: replacement(vec![lib_pin("9", "OUT", 2.54)]),
        })
        .unwrap();
    let uuids = &engine.document().symbols[0].pin_uuids;
    assert_eq!(uuids.get("9"), Some(&b), "a matched pin keeps its uuid");
    assert!(!uuids.contains_key("1") && !uuids.contains_key("2"));
}

#[test]
fn pin_uuids_of_pins_the_unit_does_not_show_are_carried_over() {
    let (mut engine, id) = engine_with_wired_pin_2();
    let hidden = Uuid::from_u128(0xC1);
    let mut doc = engine.document().clone();
    doc.symbols[0].pin_uuids.insert("77".into(), hidden);
    engine.set_document(doc);
    engine
        .execute(Command::ChangeComponent {
            symbol_id: id,
            replacement: replacement(swap_pins()),
        })
        .unwrap();
    assert_eq!(
        engine.document().symbols[0].pin_uuids.get("77"),
        Some(&hidden)
    );
}

/// U1 unit 1 (pin "1") and unit 2 (pin "2") of one two-unit OLD part, plus an
/// unrelated R2; a wire on each unit's pin.
fn engine_with_two_units() -> (Engine, Uuid, Uuid) {
    let (engine, first) = engine_with_wired_pin_2();
    let mut doc = engine.document().clone();
    let mut old = lib_symbol(
        OLD,
        vec![lib_pin("1", "IN", -2.54), lib_pin("2", "OUT", 2.54)],
    );
    old.pins[0].unit = 1;
    old.pins[1].unit = 2;
    doc.lib_symbols.insert(OLD.into(), old);
    let second = Uuid::from_u128(2);
    let mut unit2 = doc.symbols[0].clone();
    unit2.uuid = second;
    unit2.unit = 2;
    unit2.position = Point::new(40.0, 10.0);
    doc.symbols.push(unit2);
    let mut other = doc.symbols[0].clone();
    other.uuid = Uuid::from_u128(3);
    other.reference = "R2".into();
    doc.symbols.push(other);
    doc.wires.push(Wire {
        start: Point::new(42.54, 10.0),
        end: Point::new(50.0, 10.0),
        ..Default::default()
    });
    let engine = Engine::new(doc).unwrap();
    (engine, first, second)
}

fn two_unit_replacement() -> ComponentReplacement {
    let mut r = replacement(swap_pins());
    r.lib_symbol.pins[0].unit = 1;
    r.lib_symbol.pins[1].unit = 2;
    r
}

#[test]
fn every_placed_unit_of_the_part_is_swapped_in_one_undo_step() {
    let (mut engine, first, _second) = engine_with_two_units();
    engine
        .execute(Command::ChangeComponent {
            symbol_id: first,
            replacement: two_unit_replacement(),
        })
        .unwrap();
    let doc = engine.document();
    for sym in doc.symbols.iter().filter(|s| s.reference == "U1") {
        assert_eq!(sym.lib_id, NEW, "unit {} was left behind", sym.unit);
        assert_eq!(sym.row_id, Some(Uuid::from_u128(0xB)));
        assert_eq!(sym.value, "NEW-MPN");
        assert_eq!(sym.library_version, "2.0");
    }
    let r2 = doc.symbols.iter().find(|s| s.reference == "R2").unwrap();
    assert_eq!(r2.lib_id, OLD, "another designator is not touched");

    engine.undo().unwrap();
    assert!(engine.document().symbols.iter().all(|s| s.lib_id == OLD));
    assert!(!engine.can_undo(), "one undo undid every unit");
}

#[test]
fn the_preview_covers_the_pins_of_every_unit() {
    let (engine, first, _second) = engine_with_two_units();
    // The new part drops pin "2", which unit 2 has wired.
    let mut r = replacement(vec![lib_pin("1", "IN", -2.54)]);
    r.lib_symbol.pins[0].unit = 1;
    let p = change_component_preview(engine.document(), first, &r);
    assert_eq!(p.unmatched_connected, vec!["2".to_string()]);
}

#[test]
fn a_new_part_without_a_placed_unit_is_refused_and_nothing_changes() {
    let (mut engine, first, _second) = engine_with_two_units();
    let before = format!("{:?}", engine.document());
    // Only unit 1 exists on the new part; unit 2 is placed.
    let mut r = replacement(vec![lib_pin("1", "IN", -2.54)]);
    r.lib_symbol.pins[0].unit = 1;
    let err = engine
        .execute(Command::ChangeComponent {
            symbol_id: first,
            replacement: r,
        })
        .unwrap_err();
    assert!(err.to_string().contains("U1 unit 2"), "{err}");
    assert_eq!(format!("{:?}", engine.document()), before);
    assert!(!engine.can_undo() || engine.document().symbols[0].lib_id == OLD);
}

#[test]
fn the_preview_lists_what_is_replaced_and_what_is_kept() {
    let (engine, id) = engine_with_wired_pin_2();
    let p = change_component_preview(engine.document(), id, &replacement(swap_pins()));
    for replaced in ["Value", "Footprint", "Datasheet", "MPN"] {
        assert!(
            p.replaced.iter().any(|r| r == replaced),
            "{replaced}: {:?}",
            p.replaced
        );
    }
    assert!(p.kept.iter().any(|k| k == "Designator"));
    assert!(p.kept.iter().any(|k| k == "Tolerance"), "{:?}", p.kept);
}

#[test]
fn preview_of_a_faithful_swap_is_clean() {
    let (engine, id) = engine_with_wired_pin_2();
    let p = change_component_preview(engine.document(), id, &replacement(swap_pins()));
    assert!(p.is_clean(), "{p:?}");
    assert_eq!(p.mapping.pairs.len(), 2);
}

#[test]
fn preview_reports_a_connected_pin_with_no_match() {
    let (engine, id) = engine_with_wired_pin_2();
    // The new part has no pin "2" / "OUT".
    let p = change_component_preview(
        engine.document(),
        id,
        &replacement(vec![lib_pin("1", "IN", -2.54)]),
    );
    assert_eq!(p.unmatched_connected, vec!["2".to_string()]);
    assert!(p.moved_connected.is_empty());
}

#[test]
fn preview_ignores_an_unmatched_pin_nothing_is_connected_to() {
    let (engine, id) = engine_with_wired_pin_2();
    // Pin "1" is unwired, so losing it costs no connection.
    let p = change_component_preview(
        engine.document(),
        id,
        &replacement(vec![lib_pin("2", "OUT", 2.54)]),
    );
    assert!(p.is_clean(), "{p:?}");
    assert_eq!(p.mapping.unmatched, vec!["1".to_string()]);
}

#[test]
fn preview_reports_a_matched_pin_whose_tip_moved() {
    let (engine, id) = engine_with_wired_pin_2();
    // Pin "2" exists on the new part but 5.08 mm further out.
    let p = change_component_preview(
        engine.document(),
        id,
        &replacement(vec![lib_pin("1", "IN", -2.54), lib_pin("2", "OUT", 7.62)]),
    );
    assert_eq!(p.moved_connected, vec!["2".to_string()]);
    assert!(p.unmatched_connected.is_empty());
}

#[test]
fn preview_reports_a_new_pin_landing_on_a_foreign_connection() {
    let (engine, id) = engine_with_wired_pin_2();
    // The old pin "2" is matched by name to "9" elsewhere, while a new
    // pin "3" lands on the wire's end where old pin "2" used to be.
    let p = change_component_preview(
        engine.document(),
        id,
        &replacement(vec![lib_pin("9", "OUT", 9.0), lib_pin("3", "X", 2.54)]),
    );
    assert_eq!(p.gained, vec!["3".to_string()]);
}

#[test]
fn preview_reports_a_unit_the_new_part_lacks() {
    let (mut engine, id) = engine_with_wired_pin_2();
    let mut doc = engine.document().clone();
    doc.symbols[0].unit = 3;
    doc.lib_symbols.insert(OLD.into(), lib_symbol(OLD, vec![]));
    engine.set_document(doc);
    let p = change_component_preview(engine.document(), id, &replacement(swap_pins()));
    assert_eq!(p.missing_unit, Some(3));
}

fn change(label: &str, old: &str, new: &str) -> DataChange {
    DataChange {
        label: label.into(),
        old: old.into(),
        new: new.into(),
    }
}

#[test]
fn the_preview_names_the_old_value_of_part_data_that_is_cleared_or_changed() {
    let (engine, id) = engine_with_wired_pin_2();
    let mut r = replacement(swap_pins());
    r.footprint = String::new();
    let p = change_component_preview(engine.document(), id, &r);
    assert!(
        p.changed_data.contains(&change("Footprint", "R_0603", "")),
        "{:?}",
        p.changed_data
    );
    assert!(
        p.changed_data
            .contains(&change("Value", "OLD-MPN", "NEW-MPN"))
    );
    assert!(
        p.changed_data.iter().all(|c| !c.old.is_empty()),
        "filling an empty field loses nothing: {:?}",
        p.changed_data
    );
}

#[test]
fn the_part_data_preview_covers_every_unit() {
    let (mut engine, first, _second) = engine_with_two_units();
    let mut doc = engine.document().clone();
    doc.symbols[1].footprint = "FP-OF-UNIT-2".into();
    engine.set_document(doc);
    let mut r = two_unit_replacement();
    r.footprint = String::new();
    let p = change_component_preview(engine.document(), first, &r);
    assert!(p.changed_data.contains(&change("Footprint", "R_0603", "")));
    assert!(
        p.changed_data
            .contains(&change("Footprint", "FP-OF-UNIT-2", ""))
    );
}
