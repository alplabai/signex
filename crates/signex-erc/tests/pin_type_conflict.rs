//! `PinTypeConflict` is driven off the pin connection matrix (issue #348):
//! baseline cells decide by default, a per-cell override replaces the cell, and
//! a finding names both symbols so the Messages panel can locate it.

use std::collections::HashMap;

use signex_erc::{ErcConfig, RuleKind, Severity, Violation};
use signex_types::schematic::{
    LibPin, LibSymbol, NoConnect, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet,
    SelectedKind, Symbol, Wire,
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

/// A single-pin symbol whose pin sits at `x`.
struct Part {
    reference: &'static str,
    direction: PinDirection,
    is_power: bool,
    value: &'static str,
    x: f64,
}

fn plain(reference: &'static str, direction: PinDirection, x: f64) -> Part {
    Part {
        reference,
        direction,
        is_power: false,
        value: "",
        x,
    }
}

fn flag(reference: &'static str, x: f64, value: &'static str) -> Part {
    Part {
        reference,
        direction: PinDirection::PowerOutput,
        is_power: true,
        value,
        x,
    }
}

/// An ordinary (non-flag) power port whose single pin is a `PowerOutput`.
fn port(reference: &'static str, x: f64, value: &'static str) -> Part {
    Part {
        reference,
        direction: PinDirection::PowerOutput,
        is_power: true,
        value,
        x,
    }
}

fn sheet_of(parts: &[Part]) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    for part in parts {
        let lib_id = if part.reference.starts_with("#FLG") {
            "power:PWR_FLAG".to_string()
        } else if part.reference.starts_with("#STY") {
            "power:arrow".to_string()
        } else {
            format!("lib_{}", part.reference)
        };
        sheet
            .lib_symbols
            .insert(lib_id.clone(), lib_with_one_pin(&lib_id, part.direction));
        sheet.symbols.push(Symbol {
            uuid: Uuid::new_v4(),
            lib_id,
            reference: part.reference.to_string(),
            value: part.value.to_string(),
            is_power: part.is_power,
            position: Point::new(part.x, 0.0),
            ..Default::default()
        });
    }
    sheet
}

fn wire(sheet: &mut SchematicSheet, from: f64, to: f64) {
    sheet.wires.push(Wire {
        uuid: Uuid::new_v4(),
        start: Point::new(from, 0.0),
        end: Point::new(to, 0.0),
        ..Default::default()
    });
}

#[test]
fn a_row_severity_does_not_flatten_a_warning_cell() {
    let (sheet, _, _) = two_pins_on_one_wire(PinDirection::Output, PinDirection::Output);
    let found =
        signex_erc::run_with_config(&sheet, &with_override((OUTPUT, OUTPUT), Severity::Warning));
    // The row is on (Error is its stored "on" level); the cell's Warning stays.
    let row_on = HashMap::from([(RuleKind::PinTypeConflict, Severity::Error)]);

    let after = conflicts(signex_erc::apply_overrides(found, &row_on));

    assert_eq!(after.len(), 1);
    assert_eq!(after[0].severity, Severity::Warning);
}

#[test]
fn a_row_set_off_still_silences_the_rule() {
    let (sheet, _, _) = two_pins_on_one_wire(PinDirection::Output, PinDirection::Output);
    let row_off = HashMap::from([(RuleKind::PinTypeConflict, Severity::Off)]);

    let after = signex_erc::apply_overrides(signex_erc::run(&sheet), &row_off);

    assert!(conflicts(after).is_empty());
}

#[test]
fn the_row_normalizes_any_on_level_to_the_default() {
    let rule = RuleKind::PinTypeConflict;
    assert_eq!(rule.normalize_override(Severity::Info), Severity::Error);
    assert_eq!(rule.normalize_override(Severity::Off), Severity::Off);
    assert_eq!(
        RuleKind::UnusedPin.normalize_override(Severity::Info),
        Severity::Info
    );
}

#[test]
fn three_outputs_on_one_net_yield_one_finding() {
    let mut sheet = sheet_of(&[
        plain("U1", PinDirection::Output, 0.0),
        plain("U2", PinDirection::Output, 10.0),
        plain("U3", PinDirection::Output, 20.0),
    ]);
    wire(&mut sheet, 0.0, 10.0);
    wire(&mut sheet, 10.0, 20.0);

    let found = conflicts(signex_erc::run(&sheet));

    assert_eq!(found.len(), 1, "{found:?}");
    for reference in ["U1", "U2", "U3"] {
        assert!(found[0].message.contains(reference), "{}", found[0].message);
    }
}

#[test]
fn a_net_reports_its_worst_cell_severity() {
    // Output/Output is lowered to Warning, Output/PowerOut stays Error: the
    // net's single finding takes the worst of its pairs.
    let mut sheet = sheet_of(&[
        plain("U1", PinDirection::Output, 0.0),
        plain("U2", PinDirection::Output, 10.0),
        plain("U3", PinDirection::PowerOutput, 20.0),
    ]);
    wire(&mut sheet, 0.0, 10.0);
    wire(&mut sheet, 10.0, 20.0);

    let found = conflicts(signex_erc::run_with_config(
        &sheet,
        &with_override((OUTPUT, OUTPUT), Severity::Warning),
    ));

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].severity, Severity::Error);
}

#[test]
fn two_power_flags_sharing_a_value_do_not_conflict() {
    let mut sheet = sheet_of(&[flag("#FLG01", 0.0, "PWR"), flag("#FLG02", 10.0, "PWR")]);
    wire(&mut sheet, 0.0, 10.0);

    assert!(conflicts(signex_erc::run(&sheet)).is_empty());
}

#[test]
fn flags_on_different_rails_do_not_merge_the_rails() {
    // Both flags carry the active bar's value "PWR"; neither names a net, so
    // the VCC and GND rails stay apart and no flag-vs-flag conflict appears.
    let mut sheet = sheet_of(&[
        plain("U1", PinDirection::PowerOutput, 0.0),
        flag("#FLG01", 10.0, "PWR"),
        plain("U2", PinDirection::PowerOutput, 100.0),
        flag("#FLG02", 110.0, "PWR"),
    ]);
    wire(&mut sheet, 0.0, 10.0);
    wire(&mut sheet, 100.0, 110.0);

    assert!(conflicts(signex_erc::run(&sheet)).is_empty());
    let nets = signex_erc::ErcContext::from_snapshot(&sheet).nets;
    let rail_of = |reference: &str| {
        nets.iter()
            .position(|n| n.pins.iter().any(|p| p.reference == reference))
    };
    assert_ne!(rail_of("U1"), rail_of("U2"));
}

#[test]
fn a_flag_does_not_hide_a_genuine_second_driver() {
    let mut sheet = sheet_of(&[
        plain("U1", PinDirection::PowerOutput, 0.0),
        plain("U2", PinDirection::PowerOutput, 10.0),
        flag("#FLG01", 20.0, "PWR"),
    ]);
    wire(&mut sheet, 0.0, 10.0);
    wire(&mut sheet, 10.0, 20.0);

    assert_eq!(conflicts(signex_erc::run(&sheet)).len(), 1);
}

#[test]
fn no_erc_on_one_pin_silences_only_its_pairs() {
    let mut sheet = sheet_of(&[
        plain("U1", PinDirection::Output, 0.0),
        plain("U2", PinDirection::Output, 10.0),
        plain("U3", PinDirection::Output, 20.0),
    ]);
    wire(&mut sheet, 0.0, 10.0);
    wire(&mut sheet, 10.0, 20.0);
    sheet.no_erc_directives.push(NoConnect {
        uuid: Uuid::new_v4(),
        position: Point::new(0.0, 0.0),
    });

    let found = conflicts(signex_erc::run(&sheet));

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].message.contains("U2") && found[0].message.contains("U3"));
    assert!(!found[0].message.contains("U1"), "{}", found[0].message);
}

#[test]
fn two_genuine_power_outputs_are_still_flagged() {
    let mut sheet = sheet_of(&[
        plain("U1", PinDirection::PowerOutput, 0.0),
        plain("U2", PinDirection::PowerOutput, 10.0),
    ]);
    wire(&mut sheet, 0.0, 10.0);

    assert_eq!(conflicts(signex_erc::run(&sheet)).len(), 1);
}

#[test]
fn a_vcc_port_is_not_exempt_from_a_real_power_driver() {
    // Only a flag is exempt from PowerOut x PowerOut; an ordinary VCC port
    // driving the same rail as a regulator output is still reported.
    let mut sheet = sheet_of(&[
        plain("U1", PinDirection::PowerOutput, 0.0),
        port("#PWR01", 10.0, "VCC"),
    ]);
    wire(&mut sheet, 0.0, 10.0);

    assert_eq!(conflicts(signex_erc::run(&sheet)).len(), 1);
}

#[test]
fn styled_power_ports_with_one_value_still_merge_their_rails() {
    // Styled ports are ordinary ports (not flags): two sharing a value join
    // one net, so the drivers behind them conflict.
    let mut sheet = sheet_of(&[
        plain("U1", PinDirection::PowerOutput, 0.0),
        port("#STY01", 10.0, "+3V3"),
        plain("U2", PinDirection::PowerOutput, 100.0),
        port("#STY02", 110.0, "+3V3"),
    ]);
    wire(&mut sheet, 0.0, 10.0);
    wire(&mut sheet, 100.0, 110.0);

    assert_eq!(conflicts(signex_erc::run(&sheet)).len(), 1);
}
