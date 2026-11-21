//! Two wire ends that meet on a pin end get a junction dot, a power port's pin
//! as a part's (Caner 2026-11-21): in either order of placing, in the edit's
//! own undo step, and gone once the pin leaves.

use crate::rubber_band_tests::{add_wire, lib_pin, lib_symbol, mv};
use crate::test_support::test_sheet;
use crate::{Command, Engine, place_power_port};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Symbol, Wire};
use uuid::Uuid;

const CORNER: (f64, f64) = (50.0, 40.0);

fn dots(engine: &Engine) -> Vec<Point> {
    (engine.document().junctions.iter())
        .map(|j| j.position)
        .collect()
}

fn corner() -> Point {
    Point::new(CORNER.0, CORNER.1)
}

/// A GND power port whose pin is at `at`, placed as the app places one.
fn place_gnd(engine: &mut Engine, at: (f64, f64)) -> Uuid {
    let symbol = Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "power:GND".to_string(),
        reference: "#PWR1".to_string(),
        value: "GND".to_string(),
        is_power: true,
        position: Point::new(at.0, at.1),
        unit: 1,
        ..Default::default()
    };
    let id = symbol.uuid;
    engine.execute_batch(place_power_port(symbol)).unwrap();
    id
}

fn draw_wire(engine: &mut Engine, a: (f64, f64), b: (f64, f64)) {
    let wire = Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        ..Default::default()
    };
    engine
        .execute(Command::PlaceWireSegmentAsIs { wire })
        .unwrap();
}

/// A wire down onto the corner and one from it to the right.
fn corner_wires(sheet: &mut SchematicSheet) {
    add_wire(sheet, (50.0, 10.0), CORNER);
    add_wire(sheet, CORNER, (90.0, 40.0));
}

#[test]
fn a_power_port_placed_where_two_wires_end_gets_a_dot_its_undo_takes() {
    let mut sheet = test_sheet();
    corner_wires(&mut sheet);
    let mut engine = Engine::new(sheet).unwrap();
    assert!(dots(&engine).is_empty());
    place_gnd(&mut engine, CORNER);
    assert_eq!(dots(&engine), vec![corner()]);
    assert!(engine.document().junctions[0].minted, "an automatic dot");

    engine.undo().unwrap();
    assert!(dots(&engine).is_empty(), "the dot leaves with the port");
    assert!(engine.document().symbols.iter().all(|s| !s.is_power));
}

#[test]
fn wires_drawn_to_a_power_port_get_a_dot_once_two_end_there() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    place_gnd(&mut engine, CORNER);
    draw_wire(&mut engine, (50.0, 10.0), CORNER);
    assert!(
        dots(&engine).is_empty(),
        "one wire on the pin is no meeting"
    );
    draw_wire(&mut engine, CORNER, (90.0, 40.0));
    assert_eq!(dots(&engine), vec![corner()]);

    engine.undo().unwrap();
    assert!(dots(&engine).is_empty(), "the dot leaves with the wire");
}

#[test]
fn one_wire_to_a_power_port_gets_no_dot() {
    let mut sheet = test_sheet();
    add_wire(&mut sheet, (50.0, 10.0), CORNER);
    let mut engine = Engine::new(sheet).unwrap();
    place_gnd(&mut engine, CORNER);
    assert!(dots(&engine).is_empty());
}

#[test]
fn a_part_pin_where_two_wires_end_gets_a_dot() {
    let mut sheet = test_sheet();
    corner_wires(&mut sheet);
    sheet
        .lib_symbols
        .insert("TP".to_string(), lib_symbol("TP", vec![lib_pin("1", 0.0)]));
    let mut engine = Engine::new(sheet).unwrap();
    let part = Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "TP".to_string(),
        reference: "TP1".to_string(),
        position: corner(),
        unit: 1,
        ..Default::default()
    };
    engine
        .execute(Command::PlaceSymbol { symbol: part })
        .unwrap();
    assert_eq!(dots(&engine), vec![corner()]);
}

#[test]
fn moving_the_power_port_off_the_corner_drops_the_dot() {
    let mut sheet = test_sheet();
    corner_wires(&mut sheet);
    let mut engine = Engine::new(sheet).unwrap();
    let port = place_gnd(&mut engine, CORNER);
    assert_eq!(dots(&engine), vec![corner()]);
    let item = SelectedItem::new(port, SelectedKind::Symbol);
    mv(&mut engine, vec![item], 0.0, 20.0, false);
    assert!(
        dots(&engine).is_empty(),
        "the pin left: {:?}",
        dots(&engine)
    );
}
