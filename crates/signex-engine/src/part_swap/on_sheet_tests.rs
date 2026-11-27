//! #1261 — a pin or gate swap started on the schematic: checked against the
//! part's swap groups, the sheet rewired as for a board swap, and the swap a
//! clicked pin or a selection of units asks for.

use signex_types::part_swap::{GATE_SWAP_GROUPS_FIELD, PIN_SWAP_GROUPS_FIELD};
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, Symbol, Wire,
};
use uuid::Uuid;

use super::*;
use crate::test_support::test_sheet;

const A: Uuid = Uuid::from_u128(0xa);
const B: Uuid = Uuid::from_u128(0xb);
const R: Uuid = Uuid::from_u128(0xc);

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
/// x = -5 and 5.
fn dual() -> LibSymbol {
    let mut lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": "Q" })).unwrap();
    lib.pins = vec![
        lib_pin(1, "1", -5.0),
        lib_pin(1, "2", 5.0),
        lib_pin(2, "3", -5.0),
        lib_pin(2, "4", 5.0),
    ];
    lib
}

fn unit(uuid: Uuid, unit: u32, y: f64, pins: &str) -> Symbol {
    let mut symbol = Symbol {
        uuid,
        lib_id: "Q".into(),
        reference: "U1".into(),
        position: Point::new(0.0, y),
        unit,
        ..Default::default()
    };
    symbol
        .fields
        .insert(PIN_SWAP_GROUPS_FIELD.to_string(), pins.to_string());
    symbol
        .fields
        .insert(GATE_SWAP_GROUPS_FIELD.to_string(), "1,2".to_string());
    symbol
}

fn wire(n: u128, from: (f64, f64), to: (f64, f64)) -> Wire {
    Wire {
        uuid: Uuid::from_u128(n),
        start: Point::new(from.0, from.1),
        end: Point::new(to.0, to.1),
        ..Default::default()
    }
}

/// U1 gate 1 at the origin and gate 2 at y = 20, pins 1 and 2 in one pin
/// group, a wire running up from each of pins 1 and 2.
fn sheet_with(pins: &str) -> SchematicSheet {
    let mut sheet = test_sheet();
    sheet.lib_symbols.insert("Q".into(), dual());
    sheet.symbols = vec![unit(A, 1, 0.0, pins), unit(B, 2, 20.0, pins)];
    sheet.wires = vec![
        wire(1, (-5.0, 0.0), (-5.0, -10.0)),
        wire(2, (5.0, 0.0), (5.0, -10.0)),
    ];
    sheet
}

fn sheet() -> SchematicSheet {
    sheet_with("1,2;3,4")
}

fn pins(a: &str, b: &str) -> SheetSwapRequest {
    SheetSwapRequest::Pins(a.into(), b.into())
}

fn start(sheet: &SchematicSheet, n: u128) -> (f64, f64) {
    let w = sheet
        .wires
        .iter()
        .find(|w| w.uuid == Uuid::from_u128(n))
        .unwrap();
    (w.start.x, w.start.y)
}

#[test]
fn two_pins_of_one_group_trade_their_wires() {
    let swap = swap_on_sheet(&sheet(), A, &pins("1", "2")).unwrap();
    assert_eq!(swap.designator, "U1");
    assert_eq!(swap.pairs, [("1".to_string(), "2".to_string())]);
    let after = swap.sheet.expect("the wires moved");
    assert_eq!(start(&after, 1), (5.0, 0.0), "pin 1's wire is on pin 2");
    assert_eq!(start(&after, 2), (-5.0, 0.0), "pin 2's wire is on pin 1");
}

#[test]
fn any_unit_of_the_part_names_it() {
    let by_b = swap_on_sheet(&sheet(), B, &pins("1", "2")).unwrap();
    let by_a = swap_on_sheet(&sheet(), A, &pins("1", "2")).unwrap();
    // `SchematicSheet` has no `==`: the sheets compare by what a swap moves.
    let moved = |swap: &SheetSwap| {
        let sheet = swap.sheet.as_ref();
        (
            swap.designator.clone(),
            swap.pairs.clone(),
            sheet.map(|s| s.wires.clone()),
            sheet.map(|s| s.symbols.clone()),
        )
    };
    assert_eq!(
        moved(&by_a),
        moved(&by_b),
        "gate 2's uuid names the same part"
    );
}

#[test]
fn pins_outside_one_group_and_a_part_without_groups_are_refused() {
    let refusal = SheetSwapRefusal::PinsNotGrouped {
        designator: "U1".into(),
        a: "1".into(),
        b: "3".into(),
    };
    assert_eq!(
        swap_on_sheet(&sheet(), A, &pins("1", "3")).unwrap_err(),
        refusal
    );
    assert_eq!(
        refusal.to_string(),
        "pins 1 and 3 of U1 are not in one pin swap group of its part",
        "the words a board swap refuses in"
    );
    let mut bare = sheet();
    for symbol in &mut bare.symbols {
        symbol.fields.clear();
    }
    assert!(matches!(
        swap_on_sheet(&bare, A, &pins("1", "2")),
        Err(SheetSwapRefusal::PinsNotGrouped { .. })
    ));
}

#[test]
fn a_locked_unit_and_a_symbol_not_on_the_sheet_are_refused() {
    let mut locked = sheet();
    locked.symbols[1].locked = true;
    assert_eq!(
        swap_on_sheet(&locked, A, &pins("1", "2")).unwrap_err(),
        SheetSwapRefusal::SymbolLocked("U1".into())
    );
    assert_eq!(
        swap_on_sheet(&sheet(), R, &pins("1", "2")).unwrap_err(),
        SheetSwapRefusal::NoSymbol(R)
    );
}

#[test]
fn two_gates_trade_unit_numbers_and_name_their_pin_pairs() {
    let swap = swap_on_sheet(&sheet(), A, &SheetSwapRequest::Gates(1, 2)).unwrap();
    assert_eq!(
        swap.pairs,
        [
            ("1".to_string(), "3".to_string()),
            ("2".to_string(), "4".to_string())
        ]
    );
    let after = swap.sheet.expect("the units traded");
    let units: Vec<(Uuid, u32)> = after.symbols.iter().map(|s| (s.uuid, s.unit)).collect();
    assert_eq!(units, [(A, 2), (B, 1)]);
    assert_eq!(after.wires, sheet().wires, "the wires stay where they are");
}

#[test]
fn a_gate_not_placed_on_the_sheet_and_gates_outside_one_group_are_refused() {
    let mut one = sheet();
    one.symbols.truncate(1);
    assert_eq!(
        swap_on_sheet(&one, A, &SheetSwapRequest::Gates(1, 2)).unwrap_err(),
        SheetSwapRefusal::GateNotPlaced {
            designator: "U1".into(),
            unit: 2
        }
    );
    let mut ungrouped = sheet();
    for symbol in &mut ungrouped.symbols {
        symbol.fields.remove(GATE_SWAP_GROUPS_FIELD);
    }
    let refusal = swap_on_sheet(&ungrouped, A, &SheetSwapRequest::Gates(1, 2)).unwrap_err();
    assert_eq!(
        refusal.to_string(),
        "gates 1 and 2 of U1 are not in one gate swap group of its part"
    );
}

#[test]
fn a_clicked_pin_trades_with_the_one_other_pin_of_its_group() {
    assert_eq!(
        pins_of_clicked_pin(&sheet(), A, "2").unwrap(),
        pins("2", "1")
    );
    assert_eq!(
        pins_of_clicked_pin(&sheet_with("1,2,4"), A, "1").unwrap_err(),
        SheetSwapRefusal::ManyPartners {
            designator: "U1".into(),
            member: "pin 1".into(),
            others: "2, 4".into(),
        },
        "a group of three asks which pin"
    );
    assert_eq!(
        pins_of_clicked_pin(&sheet_with("3,4"), A, "1").unwrap_err(),
        SheetSwapRefusal::PinInNoGroup {
            designator: "U1".into(),
            pin: "1".into()
        }
    );
}

#[test]
fn a_selection_of_units_names_the_gates_to_swap() {
    let doc = sheet();
    assert_eq!(
        gates_of_selection(&doc, &[B, A]).unwrap(),
        (B, SheetSwapRequest::Gates(2, 1)),
        "two selected units trade with each other"
    );
    assert_eq!(
        gates_of_selection(&doc, &[A]).unwrap(),
        (A, SheetSwapRequest::Gates(1, 2)),
        "one unit trades with the one other placed gate of its group"
    );
    assert_eq!(
        gates_of_selection(&doc, &[]).unwrap_err(),
        SheetSwapRefusal::NotOnePart
    );
    let mut two_parts = sheet();
    two_parts.symbols.push(Symbol {
        uuid: R,
        reference: "U2".into(),
        ..unit(R, 1, 40.0, "1,2")
    });
    assert_eq!(
        gates_of_selection(&two_parts, &[A, R]).unwrap_err(),
        SheetSwapRefusal::NotOnePart,
        "units of two parts are no gate swap"
    );
    let mut alone = sheet();
    alone.symbols.truncate(1);
    assert_eq!(
        gates_of_selection(&alone, &[A]).unwrap_err(),
        SheetSwapRefusal::GateInNoGroup {
            designator: "U1".into(),
            unit: 1
        },
        "its partner is not placed here"
    );
}

#[test]
fn the_unit_holding_a_pin_is_its_gate() {
    let doc = sheet();
    let units: Vec<&Symbol> = doc.symbols.iter().collect();
    let lib = dual();
    assert_eq!(unit_holding(&units, &lib, "3", "U1").unwrap(), B);
    assert_eq!(
        unit_holding(&units, &lib, "9", "U1")
            .unwrap_err()
            .to_string(),
        "U1 has no pin 9"
    );
    assert_eq!(
        unit_holding(&units[..1], &lib, "3", "U1")
            .unwrap_err()
            .to_string(),
        "the gate of pin 3 of U1 is not placed on its sheet"
    );
}
