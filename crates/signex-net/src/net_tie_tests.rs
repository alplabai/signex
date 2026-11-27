//! #1277: the netlist keeps the two nets of a net tie, and
//! `signex_types::net_tie::net_ties` reads which nets each tie joins off it.

use signex_types::net_tie::{NetTieStatus, net_ties};
use signex_types::schematic::{
    InstancePath, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point,
    SchematicSheet, Symbol,
};
use uuid::Uuid;

use crate::build_netlist;
use crate::fixtures::{empty_sheet, label, pt, wire};

const TIE: Uuid = Uuid::from_u128(0x1277_0001);

fn lib_pin(number: &str, at: Point) -> LibPin {
    LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: at,
            rotation: 0.0,
            length: 0.0,
            name: String::new(),
            number: number.to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

/// A two-pin body, pin 1 at its origin and pin 2 5 mm to its right.
fn two_pin_body(sheet: &mut SchematicSheet, lib_id: &str) {
    sheet.lib_symbols.insert(
        lib_id.to_string(),
        LibSymbol {
            id: lib_id.to_string(),
            reference: String::new(),
            value: String::new(),
            footprint: String::new(),
            datasheet: String::new(),
            description: String::new(),
            keywords: String::new(),
            fp_filters: String::new(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: Vec::new(),
            pins: vec![lib_pin("1", pt(0.0, 0.0)), lib_pin("2", pt(5.0, 0.0))],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        },
    );
}

fn tie_at_origin() -> Symbol {
    Symbol {
        uuid: TIE,
        lib_id: "NT".to_string(),
        reference: "NT1".to_string(),
        position: pt(0.0, 0.0),
        net_tie: true,
        ..Default::default()
    }
}

/// NT1 at the origin: pin 1 on `GND`, pin 2 on `AGND`, each named by a
/// local label at the end of a wire.
fn star_ground() -> SchematicSheet {
    let mut sheet = empty_sheet();
    two_pin_body(&mut sheet, "NT");
    sheet.symbols.push(tie_at_origin());
    sheet.wires.push(wire(pt(0.0, 0.0), pt(0.0, 10.0)));
    sheet.wires.push(wire(pt(5.0, 0.0), pt(5.0, 10.0)));
    sheet
        .labels
        .push(label("GND", pt(0.0, 10.0), LabelType::Net));
    sheet
        .labels
        .push(label("AGND", pt(5.0, 10.0), LabelType::Net));
    sheet
}

fn root(sheet: &SchematicSheet) -> Vec<(InstancePath, &SchematicSheet)> {
    vec![(InstancePath::root(), sheet)]
}

#[test]
fn a_tie_keeps_its_two_nets_and_records_that_it_joins_them() {
    let sheet = star_ground();
    let netlist = build_netlist(&sheet);
    let names: Vec<&str> = netlist.nets.iter().map(|n| n.name.as_str()).collect();
    assert!(
        names.contains(&"GND") && names.contains(&"AGND"),
        "{names:?}"
    );

    let ties = net_ties(&netlist, &root(&sheet));
    assert_eq!(ties.len(), 1);
    assert_eq!(ties[0].symbol, TIE);
    assert_eq!(ties[0].designator, "NT1");
    assert_eq!(ties[0].nets(), ["AGND", "GND"]);
    assert_eq!(ties[0].status(), NetTieStatus::Joins);
    let pins: Vec<(&str, &str)> = ties[0]
        .pins
        .iter()
        .map(|p| (p.pin.as_str(), p.net.as_str()))
        .collect();
    assert_eq!(pins, [("1", "GND"), ("2", "AGND")]);
}

/// The tie flag changes no connectivity: the same sheet with the flag off
/// has the same nets.
#[test]
fn the_flag_leaves_the_netlist_as_it_is() {
    let tied = star_ground();
    let mut plain = tied.clone();
    plain.symbols[0].net_tie = false;
    assert_eq!(build_netlist(&tied), build_netlist(&plain));
    assert!(net_ties(&build_netlist(&plain), &root(&plain)).is_empty());
}

#[test]
fn a_tie_with_both_pins_on_one_net_joins_nothing() {
    let mut sheet = star_ground();
    sheet.wires.push(wire(pt(0.0, 10.0), pt(5.0, 10.0)));
    sheet.labels.pop();
    let ties = net_ties(&build_netlist(&sheet), &root(&sheet));
    assert_eq!(ties[0].status(), NetTieStatus::OneNet);
    assert_eq!(ties[0].nets(), ["GND"]);
}

#[test]
fn a_tie_with_an_unwired_pin_is_open() {
    let mut sheet = empty_sheet();
    two_pin_body(&mut sheet, "NT");
    sheet.symbols.push(tie_at_origin());
    let ties = net_ties(&build_netlist(&sheet), &root(&sheet));
    assert_eq!(ties[0].status(), NetTieStatus::Open);
    assert!(ties[0].pins.is_empty());
    assert_eq!(ties[0].designator, "NT1", "the placed designator");
}
