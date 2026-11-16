//! The schematic half of a board pin or gate swap (#2410): wire ends and
//! marks move between the two pin tips, a swap that would join nets or
//! cannot move a connection is refused, and a gate swap trades unit numbers
//! of units that draw the same pins.

use signex_types::schematic::{
    LibPin, LibSymbol, NoConnect, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, Symbol,
    Wire,
};
use uuid::Uuid;

use super::*;
use crate::test_support::test_sheet;

const A: Uuid = Uuid::from_u128(0xa);
const B: Uuid = Uuid::from_u128(0xb);

fn lib_pin(unit: u32, number: &str, x: f64) -> LibPin {
    LibPin {
        unit,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 2.54,
            name: String::new(),
            number: number.to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

/// A two-gate part: unit 1 has pins 1 and 2, unit 2 pins 3 and 4, drawn at
/// x = -5 and 5; pin 8 every unit shares, at the origin.
fn quad() -> LibSymbol {
    let mut lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": "Q" })).unwrap();
    lib.pins = vec![
        lib_pin(1, "1", -5.0),
        lib_pin(1, "2", 5.0),
        lib_pin(2, "3", -5.0),
        lib_pin(2, "4", 5.0),
        lib_pin(0, "8", 0.0),
    ];
    lib
}

fn unit(uuid: Uuid, unit: u32, y: f64) -> Symbol {
    Symbol {
        uuid,
        lib_id: "Q".into(),
        reference: "U1".into(),
        position: Point::new(0.0, y),
        unit,
        ..Default::default()
    }
}

fn wire(n: u128, from: (f64, f64), to: (f64, f64)) -> Wire {
    Wire {
        uuid: Uuid::from_u128(n),
        start: Point::new(from.0, from.1),
        end: Point::new(to.0, to.1),
        ..Default::default()
    }
}

/// Unit 1 of U1 at the origin, unit 2 at y = 20, with `wires`.
fn sheet(wires: Vec<Wire>) -> SchematicSheet {
    let mut sheet = test_sheet();
    sheet.lib_symbols.insert("Q".into(), quad());
    sheet.symbols = vec![unit(A, 1, 0.0), unit(B, 2, 20.0)];
    sheet.wires = wires;
    sheet
}

fn pin(symbol: Uuid, number: &str) -> PlacedPin<'_> {
    PlacedPin { symbol, number }
}

fn ends(sheet: &SchematicSheet, n: u128) -> ((f64, f64), (f64, f64)) {
    let w = sheet
        .wires
        .iter()
        .find(|w| w.uuid == Uuid::from_u128(n))
        .unwrap();
    ((w.start.x, w.start.y), (w.end.x, w.end.y))
}

#[test]
fn the_wire_ends_on_two_pins_trade_places() {
    let before = sheet(vec![
        wire(1, (-5.0, 0.0), (-5.0, -10.0)),
        wire(2, (5.0, 0.0), (5.0, -10.0)),
    ]);
    let after = rewire_pins(&before, pin(A, "1"), pin(A, "2"))
        .unwrap()
        .expect("the wires moved");
    assert_eq!(ends(&after, 1), ((5.0, 0.0), (-5.0, -10.0)));
    assert_eq!(ends(&after, 2), ((-5.0, 0.0), (5.0, -10.0)));
    assert_eq!(after.symbols, before.symbols, "the part stays where it is");
}

#[test]
fn a_mark_on_a_pin_moves_with_its_connection() {
    let mut before = sheet(vec![wire(1, (-5.0, 0.0), (-5.0, -10.0))]);
    before.no_connects.push(NoConnect {
        uuid: Uuid::from_u128(9),
        position: Point::new(5.0, 0.0),
    });
    let after = rewire_pins(&before, pin(A, "1"), pin(A, "2"))
        .unwrap()
        .expect("the wires moved");
    assert_eq!(ends(&after, 1), ((5.0, 0.0), (-5.0, -10.0)));
    let mark = after.no_connects[0].position;
    assert_eq!((mark.x, mark.y), (-5.0, 0.0));
}

#[test]
fn nothing_on_either_pin_leaves_the_sheet_as_it_was() {
    let before = sheet(Vec::new());
    let after = rewire_pins(&before, pin(A, "1"), pin(A, "2")).unwrap();
    assert!(after.is_none(), "nothing to move");
}

#[test]
fn a_rewire_that_would_join_nets_is_refused() {
    // The wire runs right from pin 2; moved to pin 1 it would run through
    // pin 8 and pin 2 itself.
    let before = sheet(vec![wire(1, (5.0, 0.0), (20.0, 0.0))]);
    assert_eq!(
        rewire_pins(&before, pin(A, "1"), pin(A, "2")).unwrap_err(),
        SheetSwapRefusal::WouldJoin {
            designator: "U1".into()
        }
    );
}

#[test]
fn a_pin_in_the_middle_of_a_wire_and_a_locked_wire_are_refused() {
    let through = sheet(vec![wire(1, (-5.0, -10.0), (-5.0, 10.0))]);
    assert_eq!(
        rewire_pins(&through, pin(A, "1"), pin(A, "2")).unwrap_err(),
        SheetSwapRefusal::Unmovable {
            designator: "U1".into(),
            pin: "1".into()
        }
    );
    let mut locked = wire(1, (5.0, 0.0), (5.0, -10.0));
    locked.locked = true;
    assert_eq!(
        rewire_pins(&sheet(vec![locked]), pin(A, "1"), pin(A, "2")).unwrap_err(),
        SheetSwapRefusal::LockedWire {
            designator: "U1".into(),
            pin: "2".into()
        }
    );
}

#[test]
fn a_missing_pin_is_refused() {
    assert_eq!(
        rewire_pins(&sheet(Vec::new()), pin(A, "1"), pin(A, "3")).unwrap_err(),
        SheetSwapRefusal::NoPin {
            designator: "U1".into(),
            pin: "3".into()
        },
        "pin 3 is on unit 2, not on the unit placed as A"
    );
}

#[test]
fn a_gate_swap_trades_unit_numbers_and_keeps_the_wires() {
    let before = sheet(vec![wire(1, (5.0, 0.0), (5.0, -10.0))]);
    let after = swap_units(&before, &[(A, 2), (B, 1)]).unwrap();
    assert_eq!(after.symbols[0].unit, 2);
    assert_eq!(after.symbols[1].unit, 1);
    assert_eq!(after.wires, before.wires);
    let mut locked = before.clone();
    locked.symbols[1].locked = true;
    assert_eq!(
        swap_units(&locked, &[(A, 2), (B, 1)]).unwrap_err(),
        SheetSwapRefusal::SymbolLocked("U1".into())
    );
}

#[test]
fn gate_pins_pair_by_where_they_are_drawn() {
    let lib = quad();
    assert_eq!(
        gate_pin_pairs(&lib, "U1", 1, 2).unwrap(),
        [
            ("1".to_string(), "3".to_string()),
            ("2".to_string(), "4".to_string())
        ]
    );
    let differ = SheetSwapRefusal::GatesDiffer {
        designator: "U1".into(),
        a: 1,
        b: 1,
    };
    assert_eq!(gate_pin_pairs(&lib, "U1", 1, 1), Err(differ));
    let mut moved = quad();
    moved.pins[3].pin.position = Point::new(7.0, 0.0);
    assert!(gate_pin_pairs(&moved, "U1", 1, 2).is_err());
    assert_eq!(unit_of_pin(&lib, "3"), Some(2));
    assert_eq!(unit_of_pin(&lib, "8"), Some(0));
    assert_eq!(unit_of_pin(&lib, "9"), None);
}
