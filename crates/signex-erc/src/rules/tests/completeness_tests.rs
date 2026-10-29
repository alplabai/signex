//! #1286: the completeness rules, one positive and one negative case each.

use super::*;
use crate::diagnostic::Diagnostic;

/// Findings of `kind` from a full engine run over `c`.
fn found(c: &ErcContext, kind: RuleKind) -> Vec<Diagnostic> {
    crate::engine::run_all(c)
        .into_iter()
        .filter(|d| d.rule_kind == kind)
        .collect()
}

fn part(reference: &str) -> ErcSymbol {
    let mut s = symbol_with_pin(reference, pt(0.0, 0.0), false);
    s.pins.clear();
    s.uuid = Uuid::new_v4();
    s
}

fn pin_at(reference: &str, at: Point, dir: PinDirection) -> ErcSymbol {
    let mut s = symbol_with_pin(reference, at, true);
    s.uuid = Uuid::new_v4();
    s.pins[0].electrical_type = dir;
    s
}

fn on_wire(symbols: Vec<ErcSymbol>, labels: Vec<ErcLabel>) -> ErcContext {
    ctx(
        vec![wire(pt(0.0, 0.0), pt(12.7, 0.0))],
        vec![],
        labels,
        symbols,
    )
}

fn with_symbols(symbols: Vec<ErcSymbol>) -> ErcContext {
    ctx(vec![], vec![], vec![], symbols)
}

#[test]
fn part_without_footprint_flags_only_on_board_parts_with_no_unit_carrying_one() {
    assert_eq!(
        found(
            &with_symbols(vec![part("R1")]),
            RuleKind::PartWithoutFootprint
        )
        .len(),
        1
    );
    let mut fitted = part("R1");
    fitted.footprint = "R_0603".into();
    assert!(
        found(
            &with_symbols(vec![fitted.clone()]),
            RuleKind::PartWithoutFootprint
        )
        .is_empty()
    );
    let mut off_board = part("R1");
    off_board.on_board = false;
    assert!(
        found(
            &with_symbols(vec![off_board]),
            RuleKind::PartWithoutFootprint
        )
        .is_empty()
    );
    // Unit 2 of a part whose unit 1 carries the footprint passes.
    let mut unit2 = part("R1");
    unit2.unit = 2;
    assert!(
        found(
            &with_symbols(vec![fitted, unit2]),
            RuleKind::PartWithoutFootprint
        )
        .is_empty()
    );
}

#[test]
fn part_without_value_flags_empty_values_and_skips_power_ports() {
    assert_eq!(
        found(&with_symbols(vec![part("R1")]), RuleKind::PartWithoutValue).len(),
        1
    );
    let mut valued = part("R1");
    valued.value = "10k".into();
    let port = power_port("", pt(0.0, 0.0));
    assert!(
        found(
            &with_symbols(vec![valued, port]),
            RuleKind::PartWithoutValue
        )
        .is_empty()
    );
}

#[test]
fn part_without_mpn_reads_the_bom_mpn_fields_case_insensitively() {
    assert_eq!(
        found(&with_symbols(vec![part("R1")]), RuleKind::PartWithoutMpn).len(),
        1
    );
    let mut with_mpn = part("R1");
    with_mpn
        .attrs
        .insert("mpn".into(), "RC0603FR-0710KL".into());
    assert!(found(&with_symbols(vec![with_mpn]), RuleKind::PartWithoutMpn).is_empty());
    let mut blank = part("R1");
    blank.attrs.insert("MPN".into(), "  ".into());
    assert_eq!(
        found(&with_symbols(vec![blank]), RuleKind::PartWithoutMpn).len(),
        1
    );
}

#[test]
fn unannotated_designator_flags_question_marks_and_blanks_only() {
    let c = with_symbols(vec![
        part("R?"),
        part(""),
        part("R1"),
        power_port("+5V", pt(0.0, 0.0)),
    ]);
    assert_eq!(found(&c, RuleKind::UnannotatedDesignator).len(), 2);
}

#[test]
fn single_pin_net_flags_a_lone_pin_on_a_wire() {
    let c = on_wire(
        vec![pin_at("U1", pt(0.0, 0.0), PinDirection::Passive)],
        vec![],
    );
    let out = found(&c, RuleKind::SinglePinNet);
    assert_eq!(out.len(), 1, "{out:?}");
    assert!(out[0].message.contains("U1.1"), "{}", out[0].message);
}

#[test]
fn single_pin_net_skips_two_pins_off_sheet_nets_and_no_connects() {
    let two = on_wire(
        vec![
            pin_at("U1", pt(0.0, 0.0), PinDirection::Passive),
            pin_at("U2", pt(12.7, 0.0), PinDirection::Passive),
        ],
        vec![],
    );
    assert!(found(&two, RuleKind::SinglePinNet).is_empty());
    let global = on_wire(
        vec![pin_at("U1", pt(0.0, 0.0), PinDirection::Passive)],
        vec![global_label("SDA", pt(12.7, 0.0))],
    );
    assert!(found(&global, RuleKind::SinglePinNet).is_empty());
    let mut nc = on_wire(
        vec![pin_at("U1", pt(0.0, 0.0), PinDirection::Passive)],
        vec![],
    );
    nc.no_connects.push(crate::context::ErcNoConnect {
        position: pt(0.0, 0.0),
    });
    assert!(found(&nc, RuleKind::SinglePinNet).is_empty());
}

#[test]
fn net_with_no_driver_flags_inputs_only() {
    let inputs = on_wire(
        vec![
            pin_at("U1", pt(0.0, 0.0), PinDirection::Input),
            pin_at("U2", pt(12.7, 0.0), PinDirection::Input),
        ],
        vec![],
    );
    assert_eq!(found(&inputs, RuleKind::NetWithNoDriver).len(), 1);
    for driver in [
        PinDirection::Output,
        PinDirection::Passive,
        PinDirection::Unclassified,
    ] {
        let driven = on_wire(
            vec![
                pin_at("U1", pt(0.0, 0.0), PinDirection::Input),
                pin_at("U2", pt(12.7, 0.0), driver),
            ],
            vec![],
        );
        assert!(
            found(&driven, RuleKind::NetWithNoDriver).is_empty(),
            "{driver:?}"
        );
    }
    let hier = on_wire(
        vec![pin_at("U1", pt(0.0, 0.0), PinDirection::Input)],
        vec![ErcLabel {
            label_type: LabelType::Hierarchical,
            ..net_label("IN", pt(12.7, 0.0))
        }],
    );
    assert!(found(&hier, RuleKind::NetWithNoDriver).is_empty());
}

fn port_on(value: &str, lib_id: &str, at: Point) -> ErcSymbol {
    let mut p = power_port(value, at);
    p.uuid = Uuid::new_v4();
    p.lib_id = lib_id.into();
    p.pins.push(ErcPin {
        world_pos: at,
        number: "1".into(),
        electrical_type: PinDirection::PowerInput,
        required: true,
        connected: true,
    });
    p
}

#[test]
fn power_input_not_driven_needs_a_source_on_the_net() {
    let bare = || {
        vec![
            pin_at("U1", pt(0.0, 0.0), PinDirection::PowerInput),
            pin_at("C1", pt(12.7, 0.0), PinDirection::Passive),
        ]
    };
    let out = found(&on_wire(bare(), vec![]), RuleKind::PowerInputNotDriven);
    assert_eq!(out.len(), 1, "{out:?}");

    let mut regulated = bare();
    regulated.push(pin_at("U2", pt(12.7, 0.0), PinDirection::PowerOutput));
    assert!(found(&on_wire(regulated, vec![]), RuleKind::PowerInputNotDriven).is_empty());

    for (value, lib_id) in [("+3V3", "power:+3V3"), ("PWR_FLAG", "power:PWR_FLAG")] {
        let mut ported = bare();
        ported.push(port_on(value, lib_id, pt(12.7, 0.0)));
        assert!(
            found(&on_wire(ported, vec![]), RuleKind::PowerInputNotDriven).is_empty(),
            "{lib_id}"
        );
    }
    // A port with no pins connects at its origin.
    let mut pinless = bare();
    pinless.push(power_port("+5V", pt(12.7, 0.0)));
    assert!(found(&on_wire(pinless, vec![]), RuleKind::PowerInputNotDriven).is_empty());
}

#[test]
fn labels_only_net_flags_a_local_label_with_no_pin() {
    let c = on_wire(vec![], vec![net_label("N", pt(0.0, 0.0))]);
    assert_eq!(found(&c, RuleKind::LabelsOnlyNet).len(), 1);
    let with_pin = on_wire(
        vec![pin_at("U1", pt(12.7, 0.0), PinDirection::Passive)],
        vec![net_label("N", pt(0.0, 0.0))],
    );
    assert!(found(&with_pin, RuleKind::LabelsOnlyNet).is_empty());
    let global = on_wire(vec![], vec![global_label("N", pt(0.0, 0.0))]);
    assert!(found(&global, RuleKind::LabelsOnlyNet).is_empty());
}

#[test]
fn units_not_placed_lists_the_missing_units() {
    let unit = |n: u32| {
        let mut s = part("U1");
        s.lib_id = "Amplifier:LM324".into();
        s.unit = n;
        s.unit_count = 3;
        s
    };
    let out = found(
        &with_symbols(vec![unit(1), unit(3)]),
        RuleKind::UnitsNotPlaced,
    );
    assert_eq!(out.len(), 1);
    assert!(out[0].message.ends_with("unit 2"), "{}", out[0].message);
    assert!(
        found(
            &with_symbols(vec![unit(1), unit(2), unit(3)]),
            RuleKind::UnitsNotPlaced
        )
        .is_empty()
    );
    let mut unannotated = unit(1);
    unannotated.reference = "U?".into();
    assert!(found(&with_symbols(vec![unannotated]), RuleKind::UnitsNotPlaced).is_empty());
}

#[test]
fn off_grid_flags_pins_and_wire_ends_in_the_micron_key_space() {
    let c = ctx(
        vec![wire(pt(0.0, 0.0), pt(1.0, 0.0))],
        vec![],
        vec![],
        vec![],
    );
    assert_eq!(found(&c, RuleKind::OffGrid).len(), 1);
    let symbol = pin_at("U1", pt(1.0, 0.0), PinDirection::Passive);
    assert_eq!(
        found(&with_symbols(vec![symbol]), RuleKind::OffGrid).len(),
        1
    );
    // 3 * 1.27 is 3.8100000000000005 in f64: on the grid, not a rounding miss.
    let on = ctx(
        vec![wire(pt(0.0, 0.0), pt(3.0 * 1.27, 2.54))],
        vec![],
        vec![],
        vec![],
    );
    assert!(found(&on, RuleKind::OffGrid).is_empty());
}
