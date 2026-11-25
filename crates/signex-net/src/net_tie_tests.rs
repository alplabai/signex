use signex_types::net::{Net, NetId, Netlist, Terminal};
use signex_types::schematic::{
    InstancePath, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point,
    SchematicSheet, Symbol,
};
use uuid::Uuid;

use super::{NetTieStatus, net_ties};
use crate::build_netlist;
use crate::fixtures::{empty_sheet, label, pt, wire};

const TIE: Uuid = Uuid::from_u128(0x1277_0001);
const R1: Uuid = Uuid::from_u128(0x1277_0002);

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

fn part(uuid: Uuid, reference: &str, at: Point, net_tie: bool) -> Symbol {
    Symbol {
        uuid,
        lib_id: "NT".to_string(),
        reference: reference.to_string(),
        position: at,
        net_tie,
        ..Default::default()
    }
}

/// NT1 at the origin: pin 1 on `GND`, pin 2 on `AGND`, each named by a
/// local label at the end of a wire.
fn star_ground() -> SchematicSheet {
    let mut sheet = empty_sheet();
    two_pin_body(&mut sheet, "NT");
    sheet.symbols.push(part(TIE, "NT1", pt(0.0, 0.0), true));
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

#[test]
fn a_part_that_is_not_a_tie_is_not_listed() {
    let mut sheet = star_ground();
    sheet.symbols[0].net_tie = false;
    assert!(net_ties(&build_netlist(&sheet), &root(&sheet)).is_empty());
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
    sheet.symbols.push(part(TIE, "NT1", pt(0.0, 0.0), true));
    let ties = net_ties(&build_netlist(&sheet), &root(&sheet));
    assert_eq!(ties[0].status(), NetTieStatus::Open);
    assert!(ties[0].pins.is_empty());
    assert_eq!(ties[0].designator, "NT1", "the placed designator");
}

/// A tie placed in two occurrences of one sheet is two ties, each with the
/// nets of its own occurrence.
#[test]
fn each_occurrence_of_a_tie_is_its_own_tie() {
    let sheet = empty_sheet();
    let (a, b) = (
        InstancePath::root().child(Uuid::from_u128(1)),
        InstancePath::root().child(Uuid::from_u128(2)),
    );
    let mut with_tie = sheet.clone();
    with_tie.symbols.push(part(TIE, "NT1", pt(0.0, 0.0), true));
    let terminal = |path: &InstancePath, reference: &str, pin: &str| Terminal {
        symbol: TIE,
        reference: reference.to_string(),
        pin: pin.to_string(),
        occurrence: path.clone(),
    };
    let net = |id: u32, name: &str, terminals: Vec<Terminal>| Net {
        id: NetId(id),
        name: name.to_string(),
        class: None,
        wires: Vec::new(),
        junctions: Vec::new(),
        terminals,
    };
    let other = Terminal {
        symbol: R1,
        ..terminal(&a, "R1", "1")
    };
    let netlist = Netlist {
        nets: vec![
            net(1, "A/GND", vec![terminal(&a, "NT1_A", "1"), other]),
            net(2, "A/AGND", vec![terminal(&a, "NT1_A", "2")]),
            net(3, "B/GND", vec![terminal(&b, "NT1_B", "1")]),
            net(4, "B/AGND", vec![terminal(&b, "NT1_B", "2")]),
        ],
    };
    let ties = net_ties(&netlist, &[(a, &with_tie), (b, &with_tie)]);
    assert_eq!(ties.len(), 2);
    assert_eq!(ties[0].designator, "NT1_A");
    assert_eq!(ties[0].nets(), ["A/AGND", "A/GND"]);
    assert_eq!(ties[1].designator, "NT1_B");
    assert_eq!(ties[1].nets(), ["B/AGND", "B/GND"]);
}

#[test]
fn a_power_port_is_never_a_tie() {
    let mut sheet = star_ground();
    sheet.symbols[0].is_power = true;
    assert!(net_ties(&build_netlist(&sheet), &root(&sheet)).is_empty());
}

#[test]
fn the_status_names_are_stable() {
    assert_eq!(NetTieStatus::Joins.as_str(), "joins");
    assert_eq!(NetTieStatus::OneNet.as_str(), "one_net");
    assert_eq!(NetTieStatus::Open.as_str(), "open");
}
