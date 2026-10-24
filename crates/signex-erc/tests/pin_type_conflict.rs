//! `PinTypeConflict` is driven off the pin connection matrix (issue #348):
//! baseline cells decide by default, a per-cell override replaces the cell, and
//! a finding names both symbols so the Messages panel can locate it.

use std::collections::HashMap;

use signex_erc::{ErcConfig, RuleKind, Severity, Violation};
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, SelectedKind,
    Symbol, Wire,
};
use uuid::Uuid;

/// Matrix rows / columns, in `PinAxis::ALL` order.
const INPUT: u8 = 0;
const OUTPUT: u8 = 1;
const POWER_OUT: u8 = 4;

fn lib_with_one_pin(id: &str, direction: PinDirection) -> LibSymbol {
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
                direction,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 0.0),
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

/// U1 at (0,0) and U2 at (10,0), one wire between their single pins.
fn two_pins_on_one_wire(left: PinDirection, right: PinDirection) -> (SchematicSheet, Uuid, Uuid) {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    sheet.wires.push(Wire {
        uuid: Uuid::new_v4(),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        ..Default::default()
    });
    let mut ids = Vec::new();
    for (reference, at, dir) in [("U1", 0.0, left), ("U2", 10.0, right)] {
        let lib_id = format!("lib_{reference}");
        sheet
            .lib_symbols
            .insert(lib_id.clone(), lib_with_one_pin(&lib_id, dir));
        let uuid = Uuid::new_v4();
        ids.push(uuid);
        sheet.symbols.push(Symbol {
            uuid,
            lib_id,
            reference: reference.to_string(),
            position: Point::new(at, 0.0),
            ..Default::default()
        });
    }
    (sheet, ids[0], ids[1])
}

fn conflicts(violations: Vec<Violation>) -> Vec<Violation> {
    violations
        .into_iter()
        .filter(|v| v.rule == RuleKind::PinTypeConflict)
        .collect()
}

fn with_override(cell: (u8, u8), severity: Severity) -> ErcConfig {
    ErcConfig::with_pin_matrix_overrides(&HashMap::from([(cell, severity)]))
}

#[test]
fn two_outputs_on_one_net_emit_one_locatable_conflict() {
    let (sheet, u1, u2) = two_pins_on_one_wire(PinDirection::Output, PinDirection::Output);

    let found = conflicts(signex_erc::run(&sheet));

    assert_eq!(found.len(), 1, "{found:?}");
    let v = &found[0];
    assert_eq!(v.severity, Severity::Error, "Output/Output baseline cell");
    assert_eq!((v.location.x, v.location.y), (0.0, 0.0));
    let primary = v.primary.as_ref().expect("primary symbol");
    let peer = v.peer.as_ref().expect("peer symbol");
    assert_eq!((primary.uuid, primary.kind), (u1, SelectedKind::Symbol));
    assert_eq!((peer.uuid, peer.kind), (u2, SelectedKind::Symbol));
}

#[test]
fn output_driving_an_input_emits_nothing() {
    let (sheet, _, _) = two_pins_on_one_wire(PinDirection::Output, PinDirection::Input);

    assert!(conflicts(signex_erc::run(&sheet)).is_empty());
}

#[test]
fn overriding_the_cell_to_off_suppresses_the_conflict() {
    let (sheet, _, _) = two_pins_on_one_wire(PinDirection::Output, PinDirection::Output);

    let found = conflicts(signex_erc::run_with_config(
        &sheet,
        &with_override((OUTPUT, OUTPUT), Severity::Off),
    ));

    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn overriding_the_cell_to_warning_downgrades_the_conflict() {
    let (sheet, _, _) = two_pins_on_one_wire(PinDirection::Output, PinDirection::Output);

    let found = conflicts(signex_erc::run_with_config(
        &sheet,
        &with_override((OUTPUT, OUTPUT), Severity::Warning),
    ));

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Warning);
}

#[test]
fn overriding_a_benign_cell_up_makes_it_fire() {
    let (sheet, _, _) = two_pins_on_one_wire(PinDirection::Output, PinDirection::Input);

    let found = conflicts(signex_erc::run_with_config(
        &sheet,
        &with_override((INPUT, OUTPUT), Severity::Info),
    ));

    assert_eq!(found.len(), 1, "Input/Output cell raised to Info");
    assert_eq!(found[0].severity, Severity::Info);
}

#[test]
fn a_pair_honours_the_override_on_either_mirror_cell() {
    let (sheet, _, _) = two_pins_on_one_wire(PinDirection::Output, PinDirection::PowerOutput);
    assert_eq!(
        conflicts(signex_erc::run(&sheet)).len(),
        1,
        "baseline Error"
    );

    let off_upper = with_override((OUTPUT, POWER_OUT), Severity::Off);
    let off_lower = with_override((POWER_OUT, OUTPUT), Severity::Off);

    assert!(conflicts(signex_erc::run_with_config(&sheet, &off_upper)).is_empty());
    assert!(conflicts(signex_erc::run_with_config(&sheet, &off_lower)).is_empty());
}

#[test]
fn the_no_config_path_is_the_baseline_matrix() {
    let (sheet, _, _) = two_pins_on_one_wire(PinDirection::Output, PinDirection::Output);

    let plain = conflicts(signex_erc::run(&sheet));
    let configured = conflicts(signex_erc::run_with_config(&sheet, &ErcConfig::default()));

    assert_eq!(plain.len(), configured.len());
    assert_eq!(plain[0].severity, configured[0].severity);
}

#[test]
fn shared_bus_drivers_and_passives_never_conflict() {
    for (a, b) in [
        (PinDirection::ThreeStatable, PinDirection::ThreeStatable),
        (PinDirection::OpenDrainLow, PinDirection::OpenDrainLow),
        (PinDirection::Passive, PinDirection::Output),
        (PinDirection::Unclassified, PinDirection::PowerOutput),
    ] {
        let (sheet, _, _) = two_pins_on_one_wire(a, b);
        assert!(
            conflicts(signex_erc::run(&sheet)).is_empty(),
            "{a:?} + {b:?} must not conflict"
        );
    }
}

#[test]
fn a_conflict_off_the_shared_net_is_not_reported() {
    // Same two outputs, but no wire joins them.
    let (mut sheet, _, _) = two_pins_on_one_wire(PinDirection::Output, PinDirection::Output);
    sheet.wires.clear();

    assert!(conflicts(signex_erc::run(&sheet)).is_empty());
}

#[test]
fn two_pins_of_one_symbol_on_one_net_do_not_conflict() {
    // An LDO with two PowerOutput VOUT pins strapped together: one driver.
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let mut lib = lib_with_one_pin("lib_U1", PinDirection::PowerOutput);
    let mut second = lib.pins[0].clone();
    second.pin.position = Point::new(10.0, 0.0);
    second.pin.number = "2".to_string();
    lib.pins.push(second);
    sheet.lib_symbols.insert("lib_U1".to_string(), lib);
    sheet.wires.push(Wire {
        uuid: Uuid::new_v4(),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        ..Default::default()
    });
    sheet.symbols.push(Symbol {
        uuid: Uuid::new_v4(),
        lib_id: "lib_U1".to_string(),
        reference: "U1".to_string(),
        position: Point::new(0.0, 0.0),
        ..Default::default()
    });

    let found = conflicts(signex_erc::run(&sheet));

    assert!(found.is_empty(), "{found:?}");
}
