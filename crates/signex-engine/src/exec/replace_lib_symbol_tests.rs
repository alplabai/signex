//! `Command::ReplaceLibSymbol` (#912): replace + one undo step, and the
//! refusal when a connected pin would vanish or move.

use crate::test_support::test_sheet;
use crate::{Command, Engine, EngineError};
use signex_types::schematic::{
    Junction, LibPin, LibSymbol, NoConnect, Pin, PinDirection, PinShapeStyle, Point, Symbol, Wire,
};

const ID: &str = "L:S";

fn lib_pin(number: &str, x: f64) -> LibPin {
    LibPin {
        unit: 1,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 2.54,
            name: number.to_string(),
            number: number.to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

fn lib_symbol(description: &str, pins: Vec<LibPin>) -> LibSymbol {
    LibSymbol {
        id: ID.to_string(),
        reference: "U".into(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: description.to_string(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins,
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

/// A sheet with the symbol at (10, 10) and a wire on pin "2" (world 12.54, 10).
fn engine_with_wired_pin_2() -> Engine {
    let mut sheet = test_sheet();
    sheet.lib_symbols.insert(
        ID.into(),
        lib_symbol("old", vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]),
    );
    sheet.symbols.push(Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: ID.into(),
        reference: "U1".into(),
        position: Point::new(10.0, 10.0),
        unit: 1,
        ..Default::default()
    });
    sheet.wires.push(Wire {
        start: Point::new(12.54, 10.0),
        end: Point::new(20.0, 10.0),
        ..Default::default()
    });
    Engine::new(sheet).unwrap()
}

#[test]
fn replace_swaps_the_copy_and_one_undo_restores_it() {
    let mut engine = engine_with_wired_pin_2();
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]);
    let result = engine
        .execute(Command::ReplaceLibSymbol { lib_symbol: fresh })
        .unwrap();
    assert!(result.changed);
    assert_eq!(engine.document().lib_symbols[ID].description, "new");

    engine.undo().unwrap();
    assert_eq!(engine.document().lib_symbols[ID].description, "old");
    assert!(!engine.can_undo());
}

#[test]
fn replacing_with_an_identical_symbol_records_nothing() {
    let mut engine = engine_with_wired_pin_2();
    let same = lib_symbol("old", vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]);
    let result = engine
        .execute(Command::ReplaceLibSymbol { lib_symbol: same })
        .unwrap();
    assert!(!result.changed);
    assert!(!engine.can_undo());
}

#[test]
fn a_vanishing_unconnected_pin_is_allowed() {
    let mut engine = engine_with_wired_pin_2();
    let fresh = lib_symbol("new", vec![lib_pin("2", 2.54)]);
    assert!(
        engine
            .execute(Command::ReplaceLibSymbol { lib_symbol: fresh })
            .unwrap()
            .changed
    );
}

#[test]
fn a_vanishing_connected_pin_is_refused_and_nothing_changes() {
    let mut engine = engine_with_wired_pin_2();
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54)]);
    let err = engine
        .execute(Command::ReplaceLibSymbol { lib_symbol: fresh })
        .unwrap_err();
    match err {
        EngineError::LibSymbolPinsConnected { pins, .. } => {
            assert_eq!(pins, vec!["U1 pin 2".to_string()]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(engine.document().lib_symbols[ID].description, "old");
    assert!(!engine.can_undo());
}

#[test]
fn a_connected_pin_that_moves_is_refused() {
    let mut engine = engine_with_wired_pin_2();
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54), lib_pin("2", 5.08)]);
    assert!(
        engine
            .execute(Command::ReplaceLibSymbol { lib_symbol: fresh })
            .is_err()
    );
}

#[test]
fn a_label_on_a_pin_tip_counts_as_a_connection() {
    let mut sheet = engine_with_wired_pin_2().document().clone();
    // Pin "1" tip (7.46, 10) carries only a label, no wire.
    sheet.labels.push(signex_types::schematic::Label {
        unknown_columns: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        text: "N".to_string(),
        position: Point::new(7.46, 10.0),
        rotation: 0.0,
        label_type: signex_types::schematic::LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        justify: signex_types::schematic::HAlign::Left,
        justify_v: signex_types::schematic::VAlign::Bottom,
    });
    let mut engine = Engine::new(sheet).unwrap();
    let fresh = lib_symbol("new", vec![lib_pin("2", 2.54)]);
    assert!(
        engine
            .execute(Command::ReplaceLibSymbol { lib_symbol: fresh })
            .is_err()
    );
}

#[test]
fn a_new_pin_landing_on_an_existing_connection_is_refused() {
    let mut engine = engine_with_wired_pin_2();
    // The wire's far end (20, 10) is a connection no old pin occupied.
    let fresh = lib_symbol(
        "new",
        vec![lib_pin("1", -2.54), lib_pin("2", 2.54), lib_pin("3", 10.0)],
    );
    match engine
        .execute(Command::ReplaceLibSymbol { lib_symbol: fresh })
        .unwrap_err()
    {
        EngineError::LibSymbolPinsConnected { pins, .. } => {
            assert_eq!(pins, vec!["U1 pin 3".to_string()]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_removed_unit_is_refused() {
    let mut engine = engine_with_wired_pin_2();
    let mut p = lib_pin("1", -2.54);
    p.unit = 2;
    let fresh = lib_symbol("new", vec![p]);
    assert!(matches!(
        engine
            .execute(Command::ReplaceLibSymbol { lib_symbol: fresh })
            .unwrap_err(),
        EngineError::LibSymbolUnitRemoved { .. }
    ));
}

/// A sheet with the symbol at (10, 10) and no wire: pin "1" tip is (7.46, 10),
/// pin "2" tip is (12.54, 10).
fn engine_with_bare_symbol() -> Engine {
    let mut sheet = engine_with_wired_pin_2().document().clone();
    sheet.wires.clear();
    Engine::new(sheet).unwrap()
}

fn drop_pin_2(engine: &mut Engine) -> Result<crate::CommandResult, EngineError> {
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54)]);
    engine.execute(Command::ReplaceLibSymbol { lib_symbol: fresh })
}

#[test]
fn a_junction_alone_on_a_pin_tip_counts_as_a_connection() {
    let mut engine = engine_with_bare_symbol();
    assert!(drop_pin_2(&mut engine).unwrap().changed, "bare pin is free");

    let mut sheet = engine_with_bare_symbol().document().clone();
    sheet.junctions.push(Junction {
        uuid: uuid::Uuid::new_v4(),
        position: Point::new(12.54, 10.0),
        ..Default::default()
    });
    let mut engine = Engine::new(sheet).unwrap();
    assert!(matches!(
        drop_pin_2(&mut engine).unwrap_err(),
        EngineError::LibSymbolPinsConnected { .. }
    ));
}

#[test]
fn a_no_connect_on_a_pin_tip_counts_as_a_connection() {
    let mut sheet = engine_with_bare_symbol().document().clone();
    sheet.no_connects.push(NoConnect {
        uuid: uuid::Uuid::new_v4(),
        position: Point::new(12.54, 10.0),
    });
    let mut engine = Engine::new(sheet).unwrap();
    assert!(matches!(
        drop_pin_2(&mut engine).unwrap_err(),
        EngineError::LibSymbolPinsConnected { .. }
    ));
}

#[test]
fn a_wire_passing_through_a_pin_without_a_junction_counts_as_a_connection() {
    let mut sheet = engine_with_bare_symbol().document().clone();
    // Vertical wire through (12.54, 10): its endpoints are not on the pin.
    sheet.wires.push(Wire {
        start: Point::new(12.54, 5.0),
        end: Point::new(12.54, 15.0),
        ..Default::default()
    });
    assert!(sheet.junctions.is_empty());
    let mut engine = Engine::new(sheet).unwrap();
    match drop_pin_2(&mut engine).unwrap_err() {
        EngineError::LibSymbolPinsConnected { pins, .. } => {
            assert_eq!(pins, vec!["U1 pin 2".to_string()]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_new_pin_on_a_connection_is_refused_when_the_sheet_has_no_embedded_copy() {
    let mut sheet = engine_with_wired_pin_2().document().clone();
    sheet.lib_symbols.clear();
    let mut engine = Engine::new(sheet).unwrap();
    // Pin "2" lands on the wire end (12.54, 10); no old pin occupied it.
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]);
    match engine
        .execute(Command::ReplaceLibSymbol { lib_symbol: fresh })
        .unwrap_err()
    {
        EngineError::LibSymbolPinsConnected { pins, .. } => {
            assert_eq!(pins, vec!["U1 pin 2".to_string()]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert!(engine.document().lib_symbols.is_empty());
}

#[test]
fn a_replace_with_no_embedded_copy_succeeds_when_no_fresh_pin_is_connected() {
    let mut sheet = engine_with_bare_symbol().document().clone();
    sheet.lib_symbols.clear();
    let mut engine = Engine::new(sheet).unwrap();
    let fresh = lib_symbol("new", vec![lib_pin("1", -2.54), lib_pin("2", 2.54)]);
    assert!(
        engine
            .execute(Command::ReplaceLibSymbol { lib_symbol: fresh })
            .unwrap()
            .changed
    );
    assert_eq!(engine.document().lib_symbols[ID].description, "new");
}
