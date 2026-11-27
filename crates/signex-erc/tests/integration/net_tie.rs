//! A net tie (#1277; Caner, 2026-11-25) joins two named nets on purpose:
//! its nets stay two, so ERC reports no short and no second name for it.
//! Joining the same rails with a wire is the short ERC does report.

use signex_erc::{RuleKind, Violation};
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, Symbol, Wire,
    power_port_lib_symbol,
};
use uuid::Uuid;

const PORT_LIB: &str = "power:GND";

fn passive(number: &str, x: f64) -> LibPin {
    LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
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

fn tie_body() -> LibSymbol {
    LibSymbol {
        id: "Device:NetTie".to_string(),
        reference: "NT".to_string(),
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
        pins: vec![passive("1", 0.0), passive("2", 5.0)],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

fn wire(a: (f64, f64), b: (f64, f64)) -> Wire {
    Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        ..Default::default()
    }
}

fn port(value: &str, x: f64) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: PORT_LIB.to_string(),
        reference: "#PWR".to_string(),
        value: value.to_string(),
        position: Point::new(x, 10.0),
        is_power: true,
        ..Default::default()
    }
}

/// NT1 at the origin, pin 1 wired up to a `GND` power port and pin 2 to an
/// `AGND` one: a star ground.
fn star_ground(net_tie: bool) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    sheet
        .lib_symbols
        .insert("Device:NetTie".to_string(), tie_body());
    sheet
        .lib_symbols
        .insert(PORT_LIB.to_string(), power_port_lib_symbol(PORT_LIB));
    sheet.symbols.push(Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:NetTie".to_string(),
        reference: "NT1".to_string(),
        value: "NetTie".to_string(),
        footprint: "NetTie_2".to_string(),
        position: Point::new(0.0, 0.0),
        net_tie,
        ..Default::default()
    });
    sheet.symbols.push(port("GND", 0.0));
    sheet.symbols.push(port("AGND", 5.0));
    sheet.wires.push(wire((0.0, 0.0), (0.0, 10.0)));
    sheet.wires.push(wire((5.0, 0.0), (5.0, 10.0)));
    sheet
}

fn rules(found: &[Violation]) -> Vec<RuleKind> {
    let mut rules: Vec<RuleKind> = found.iter().map(|v| v.rule).collect();
    rules.sort_by_key(|r| r.label());
    rules
}

const SHORT_OR_ALIAS: [RuleKind; 2] = [RuleKind::PowerPortShort, RuleKind::NetMultipleNames];

#[test]
fn a_tie_between_two_rails_is_no_short_and_no_second_name() {
    let found = signex_erc::run(&star_ground(true));
    assert!(
        found.iter().all(|v| !SHORT_OR_ALIAS.contains(&v.rule)),
        "{found:?}"
    );
}

/// The tie flag adds no finding and takes none away: the tie's pins are
/// passive, and its nets are what they are without it.
#[test]
fn the_tie_flag_changes_no_finding() {
    let tied = signex_erc::run(&star_ground(true));
    let plain = signex_erc::run(&star_ground(false));
    assert_eq!(rules(&tied), rules(&plain));
}

/// The control: the same rails joined by a wire are one net, a short.
#[test]
fn the_same_rails_joined_by_a_wire_are_a_short() {
    let mut sheet = star_ground(true);
    sheet.wires.push(wire((0.0, 10.0), (5.0, 10.0)));
    let found = signex_erc::run(&sheet);
    assert!(
        found.iter().any(|v| v.rule == RuleKind::PowerPortShort),
        "{found:?}"
    );
}
