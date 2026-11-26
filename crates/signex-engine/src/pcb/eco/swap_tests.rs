//! #1261 — a pin or gate swap started on the schematic reaches the board
//! through Update PCB: the sheet is swapped (`crate::part_swap`), the
//! netlist is built from it, and the change list moves the pads' nets.

use signex_types::net::Netlist;
use signex_types::part_swap::{GATE_SWAP_GROUPS_FIELD, PIN_SWAP_GROUPS_FIELD};
use signex_types::pcb::PcbBoard;
use signex_types::schematic::{
    Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet,
    Symbol, Wire,
};

use super::fixture::*;
use super::net_tests::{execute, net_of};
use super::*;
use crate::part_swap::{SheetSwapRequest, swap_on_sheet};
use crate::test_support::test_sheet;

const UNIT_A: u128 = 0xa;
const UNIT_B: u128 = 0xb;

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

fn unit(id: u128, unit: u32, y: f64) -> Symbol {
    let mut symbol = Symbol {
        uuid: u(id),
        lib_id: "DUAL".into(),
        reference: "U1".into(),
        position: Point::new(0.0, y),
        unit,
        ..Default::default()
    };
    symbol
        .fields
        .insert(PIN_SWAP_GROUPS_FIELD.to_string(), "1,2;3,4".to_string());
    symbol
        .fields
        .insert(GATE_SWAP_GROUPS_FIELD.to_string(), "1,2".to_string());
    symbol
}

/// U1, a dual gate: gate 1 at the origin with pins 1 and 2 at x = -5 and 5,
/// gate 2 at y = 40 with pins 3 and 4. A wire runs up 10 mm from pins 1 and
/// 2 to the net labels A and B.
fn sheet() -> SchematicSheet {
    let mut lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": "DUAL" })).unwrap();
    lib.pins = vec![
        lib_pin(1, "1", -5.0),
        lib_pin(1, "2", 5.0),
        lib_pin(2, "3", -5.0),
        lib_pin(2, "4", 5.0),
    ];
    let mut sheet = test_sheet();
    sheet.lib_symbols.insert("DUAL".into(), lib);
    sheet.symbols = vec![unit(UNIT_A, 1, 0.0), unit(UNIT_B, 2, 40.0)];
    let stub = |n: u128, x: f64| Wire {
        uuid: u(n),
        start: Point::new(x, 0.0),
        end: Point::new(x, -10.0),
        ..Default::default()
    };
    sheet.wires = vec![stub(0x101, -5.0), stub(0x102, 5.0)];
    sheet.labels = vec![
        Label::new("A", Point::new(-5.0, -10.0), LabelType::Net),
        Label::new("B", Point::new(5.0, -10.0), LabelType::Net),
    ];
    sheet
}

fn dual_footprint() -> signex_types::part::ResolvedFootprint {
    resolved("DUAL", 0x8, "1.0.0", &["1", "2", "3", "4"])
}

/// The design of `sheet` against `board`, as Update PCB reads it.
fn design_of(sheet: &SchematicSheet, board: PcbBoard) -> Design {
    let sheets = [(root(), sheet)];
    let parts = design_eco_parts(&sheets, |_, _| Ok(dual_footprint()));
    let netlist: Netlist = signex_net::build_netlist(sheet);
    Design::new(board, parts, netlist)
}

/// The board after a first Update PCB from the unswapped sheet.
fn synced_board() -> PcbBoard {
    let placed = board(vec![board_fp(
        0xf0,
        "U1",
        "",
        &[UNIT_A, UNIT_B],
        &dual_footprint(),
        (20.0, 15.0),
    )]);
    let mut design = design_of(&sheet(), placed);
    let list = design.diff();
    execute(&mut design, &list);
    assert_eq!(net_of(&design.board, "U1", "1"), Some("A"));
    assert_eq!(net_of(&design.board, "U1", "2"), Some("B"));
    design.board
}

fn swapped(request: &SheetSwapRequest) -> SchematicSheet {
    swap_on_sheet(&sheet(), u(UNIT_A), request)
        .expect("the swap is allowed")
        .sheet
        .expect("the sheet changed")
}

#[test]
fn a_pin_swap_on_the_schematic_moves_the_pad_nets_on_update_pcb() {
    let request = SheetSwapRequest::Pins("1".into(), "2".into());
    let mut design = design_of(&swapped(&request), synced_board());

    let list = design.diff();
    assert!(
        list.rows.iter().all(|row| row.group == EcoGroup::PadNets),
        "only pad nets change: {:?}",
        list.rows
    );
    assert_eq!(list.rows.len(), 2, "pads 1 and 2");
    execute(&mut design, &list);

    assert_eq!(net_of(&design.board, "U1", "1"), Some("B"));
    assert_eq!(net_of(&design.board, "U1", "2"), Some("A"));
    assert!(design.diff().rows.is_empty(), "the board is in step again");
}

#[test]
fn a_gate_swap_on_the_schematic_moves_the_gate_nets_on_update_pcb() {
    let mut design = design_of(&swapped(&SheetSwapRequest::Gates(1, 2)), synced_board());

    let list = design.diff();
    execute(&mut design, &list);

    assert_eq!(net_of(&design.board, "U1", "3"), Some("A"));
    assert_eq!(net_of(&design.board, "U1", "4"), Some("B"));
    assert_ne!(net_of(&design.board, "U1", "1"), Some("A"));
    assert_ne!(net_of(&design.board, "U1", "2"), Some("B"));
    assert!(design.diff().rows.is_empty(), "the board is in step again");
}
