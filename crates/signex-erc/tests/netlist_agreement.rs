//! ERC must read the same net model as `build_netlist` (ADR-0002, issue #675).
//!
//! One shared fixture is run through both the authoritative netlist and the
//! ERC context; they must agree on how many nets exist, what the supply net is
//! called, and which pins count as connected.

use signex_erc::ErcContext;
use signex_net::build_netlist;
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, Symbol, Wire,
};
use uuid::Uuid;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn lib(id: &str, dir: PinDirection) -> LibSymbol {
    LibSymbol {
        id: id.to_string(),
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
        pins: vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: dir,
                shape_style: PinShapeStyle::Plain,
                position: pt(0.0, 0.0),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

fn place(sheet: &mut SchematicSheet, reference: &str, lib_id: &str, at: Point, power: &str) {
    sheet.symbols.push(Symbol {
        uuid: Uuid::new_v4(),
        lib_id: lib_id.to_string(),
        reference: reference.to_string(),
        position: at,
        is_power: !power.is_empty(),
        value: power.to_string(),
        ..Default::default()
    });
}

fn wire(a: Point, b: Point) -> Wire {
    Wire {
        uuid: Uuid::new_v4(),
        start: a,
        end: b,
        ..Default::default()
    }
}

/// Wire A: source pin + GND port. Wire B: load pin + GND port (disjoint from A,
/// joined only by the shared port name). Plus a capacitor pin dropped straight
/// onto a third GND port, no wire.
fn fixture() -> SchematicSheet {
    let mut s = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    for (id, dir) in [
        ("SRC", PinDirection::PowerOutput),
        ("LOAD", PinDirection::PowerInput),
        ("GND", PinDirection::PowerInput),
        ("C", PinDirection::Passive),
    ] {
        s.lib_symbols.insert(id.to_string(), lib(id, dir));
    }
    s.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    s.wires.push(wire(pt(0.0, 20.0), pt(10.0, 20.0)));
    place(&mut s, "U1", "SRC", pt(0.0, 0.0), "");
    place(&mut s, "#PWR1", "GND", pt(10.0, 0.0), "GND");
    place(&mut s, "U2", "LOAD", pt(0.0, 20.0), "");
    place(&mut s, "#PWR2", "GND", pt(10.0, 20.0), "GND");
    place(&mut s, "C1", "C", pt(50.0, 50.0), "");
    place(&mut s, "#PWR3", "GND", pt(50.0, 50.0), "GND");
    s
}

#[test]
fn erc_and_netlist_agree_on_net_count_and_supply_name() {
    let sheet = fixture();
    let netlist = build_netlist(&sheet);
    let ctx = ErcContext::from_snapshot(&sheet);

    // The wired GND rail and the tip-to-tip C1/#PWR3 pair are two physical
    // islands joined only by the shared port name: one net.
    assert_eq!(netlist.nets.len(), 1, "{netlist:?}");
    assert_eq!(netlist.nets[0].name, "GND");
    assert_eq!(ctx.nets.len(), netlist.nets.len(), "{:?}", ctx.nets);
    assert_eq!(ctx.nets[0].name, "GND");
}

#[test]
fn erc_sees_the_driver_on_the_merged_supply_net() {
    let ctx = ErcContext::from_snapshot(&fixture());
    let gnd = ctx.nets.iter().find(|n| n.name == "GND").expect("GND net");
    assert!(gnd.has_driver, "U1's PowerOutput drives the joined GND net");
}

#[test]
fn erc_marks_tip_to_tip_pins_connected() {
    let ctx = ErcContext::from_snapshot(&fixture());
    let c1 = ctx.symbols.iter().find(|s| s.reference == "C1").unwrap();
    assert!(
        c1.pins.iter().all(|p| p.connected),
        "C1.1 touches the GND port tip"
    );
}
