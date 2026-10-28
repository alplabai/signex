//! `Command::ChangeComponent` (#1260): the swap, its single undo step, and
//! the preview of what it does to connected pins.

use super::change_component::{ComponentReplacement, change_component_preview};
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
fn the_swap_re_keys_the_symbol_and_keeps_designator_and_parameters() {
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
    assert_eq!(sym.footprint, "R_0603", "the chosen footprint stays");
    assert_eq!(sym.fields.get("Tolerance").map(String::as_str), Some("1%"));
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
fn preview_of_a_faithful_swap_is_clean() {
    let (engine, id) = engine_with_wired_pin_2();
    let p = change_component_preview(engine.document(), id, &lib_symbol(NEW, swap_pins()));
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
        &lib_symbol(NEW, vec![lib_pin("1", "IN", -2.54)]),
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
        &lib_symbol(NEW, vec![lib_pin("2", "OUT", 2.54)]),
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
        &lib_symbol(
            NEW,
            vec![lib_pin("1", "IN", -2.54), lib_pin("2", "OUT", 7.62)],
        ),
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
        &lib_symbol(NEW, vec![lib_pin("9", "OUT", 9.0), lib_pin("3", "X", 2.54)]),
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
    let p = change_component_preview(engine.document(), id, &lib_symbol(NEW, swap_pins()));
    assert_eq!(p.missing_unit, Some(3));
}
