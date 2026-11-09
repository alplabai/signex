//! #2199 — what a headless wire run pulls into its net besides its two ends.

use super::*;
use crate::build::{ExtraKind, RunEnd, WireIntent, wire_intent};

/// Parts with one pin on their origin, at the given points.
fn parts(points: &[(&str, (f64, f64))]) -> SchematicSheet {
    let mut sheet = empty_sheet();
    add_lib(
        &mut sheet,
        "P",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    for (reference, (x, y)) in points {
        place(&mut sheet, reference, "P", pt(*x, *y));
    }
    sheet
}

type Xy = (f64, f64);

/// `before` with the run's wires and junctions added, as the engine would.
fn run(before: &SchematicSheet, segments: &[(Xy, Xy)], junctions: &[(f64, f64)]) -> SchematicSheet {
    let mut after = before.clone();
    for (a, b) in segments {
        after.wires.push(wire(pt(a.0, a.1), pt(b.0, b.1)));
    }
    for (x, y) in junctions {
        after.junctions.push(junction(pt(*x, *y)));
    }
    after
}

fn intent(
    before: &SchematicSheet,
    after: &SchematicSheet,
    start: (f64, f64),
    end: (f64, f64),
) -> WireIntent {
    wire_intent(before, after, pt(start.0, start.1), pt(end.0, end.1))
}

fn identities(intent: &WireIntent, kind: ExtraKind) -> Vec<&str> {
    intent
        .extra
        .iter()
        .filter(|item| item.kind == kind)
        .map(|item| item.identity.as_str())
        .collect()
}

#[test]
fn a_clean_run_between_two_pins_is_the_intended_net() {
    let before = parts(&[("R1", (0.0, 0.0)), ("C1", (20.0, 0.0))]);
    let after = run(&before, &[((0.0, 0.0), (20.0, 0.0))], &[]);
    let got = intent(&before, &after, (0.0, 0.0), (20.0, 0.0));
    assert!(got.extra.is_empty(), "{got:?}");
    assert!(got.open.is_empty() && got.joined && got.names.is_empty());
    let net = got.net.expect("two pins make a net");
    assert_eq!(net.pins, ["C1.1", "R1.1"]);
}

#[test]
fn a_pin_on_the_path_is_an_extra_connection() {
    let before = parts(&[("R1", (0.0, 0.0)), ("C1", (20.0, 0.0)), ("R5", (10.0, 0.0))]);
    // A pin mid-wire is a terminal only where a junction sits (D5.3).
    let after = run(&before, &[((0.0, 0.0), (20.0, 0.0))], &[(10.0, 0.0)]);
    let got = intent(&before, &after, (0.0, 0.0), (20.0, 0.0));
    assert_eq!(identities(&got, ExtraKind::Pin), ["R5.1"]);
    assert_eq!(got.extra[0].position, pt(10.0, 0.0));
}

#[test]
fn a_bend_landing_on_a_pin_is_an_extra_connection() {
    let before = parts(&[
        ("R1", (0.0, 0.0)),
        ("C1", (20.0, 10.0)),
        ("R5", (20.0, 0.0)),
    ]);
    let after = run(
        &before,
        &[((0.0, 0.0), (20.0, 0.0)), ((20.0, 0.0), (20.0, 10.0))],
        &[],
    );
    let got = intent(&before, &after, (0.0, 0.0), (20.0, 10.0));
    assert_eq!(identities(&got, ExtraKind::Pin), ["R5.1"]);
}

#[test]
fn a_wire_end_touched_mid_path_is_an_extra_connection_only_with_a_junction() {
    let mut before = parts(&[("R1", (0.0, 0.0)), ("C1", (20.0, 0.0))]);
    before.wires.push(wire(pt(10.0, -5.0), pt(10.0, 0.0)));
    let stub = before.wires[0].uuid.to_string();
    let segments = [((0.0, 0.0), (20.0, 0.0))];

    // The engine mints the junction of a T; without one nothing connects.
    let joined = run(&before, &segments, &[(10.0, 0.0)]);
    let got = intent(&before, &joined, (0.0, 0.0), (20.0, 0.0));
    assert_eq!(identities(&got, ExtraKind::Wire), [stub.as_str()]);
    assert_eq!(identities(&got, ExtraKind::Junction).len(), 0);

    let crossing = run(&before, &segments, &[]);
    let got = intent(&before, &crossing, (0.0, 0.0), (20.0, 0.0));
    assert!(got.extra.is_empty(), "{got:?}");
}

#[test]
fn a_label_touched_mid_path_is_an_extra_connection() {
    let mut before = parts(&[("R1", (0.0, 0.0)), ("C1", (20.0, 0.0))]);
    before
        .labels
        .push(label("SENSE", pt(10.0, 0.0), LabelType::Net));
    let after = run(&before, &[((0.0, 0.0), (20.0, 0.0))], &[]);
    let got = intent(&before, &after, (0.0, 0.0), (20.0, 0.0));
    assert_eq!(identities(&got, ExtraKind::Label), ["SENSE"]);
}

#[test]
fn a_run_between_two_named_nets_is_a_short() {
    let mut before = parts(&[("R1", (0.0, 0.0)), ("C1", (20.0, 0.0))]);
    before.wires.push(wire(pt(0.0, 0.0), pt(0.0, 10.0)));
    before.wires.push(wire(pt(20.0, 0.0), pt(20.0, 10.0)));
    before
        .labels
        .push(label("VCC", pt(0.0, 10.0), LabelType::Net));
    before
        .labels
        .push(label("GND", pt(20.0, 10.0), LabelType::Net));
    let after = run(&before, &[((0.0, 0.0), (20.0, 0.0))], &[]);
    let got = intent(&before, &after, (0.0, 0.0), (20.0, 0.0));
    assert_eq!(got.names, ["VCC", "GND"]);
    // The two nets are the ends' own: nothing else was pulled in.
    assert!(got.extra.is_empty(), "{got:?}");
    assert!(got.joined);
}

#[test]
fn one_name_at_both_ends_is_not_a_short() {
    let mut before = parts(&[("R1", (0.0, 0.0)), ("C1", (20.0, 0.0))]);
    before.wires.push(wire(pt(0.0, 0.0), pt(0.0, 10.0)));
    before.wires.push(wire(pt(20.0, 0.0), pt(20.0, 10.0)));
    for at in [(0.0, 10.0), (20.0, 10.0)] {
        before
            .labels
            .push(label("VCC", pt(at.0, at.1), LabelType::Net));
    }
    let after = run(&before, &[((0.0, 0.0), (20.0, 0.0))], &[]);
    assert_eq!(
        intent(&before, &after, (0.0, 0.0), (20.0, 0.0)).names,
        ["VCC"]
    );
}

#[test]
fn a_start_on_a_wire_interior_is_intended() {
    let mut before = parts(&[
        ("R1", (0.0, 0.0)),
        ("R2", (20.0, 0.0)),
        ("C1", (10.0, 10.0)),
    ]);
    before.wires.push(wire(pt(0.0, 0.0), pt(20.0, 0.0)));
    let after = run(&before, &[((10.0, 0.0), (10.0, 10.0))], &[(10.0, 0.0)]);
    let got = intent(&before, &after, (10.0, 0.0), (10.0, 10.0));
    assert!(got.extra.is_empty(), "{got:?}");
    assert!(got.open.is_empty(), "{got:?}");
    assert_eq!(got.net.expect("net").pins, ["C1.1", "R1.1", "R2.1"]);
}

#[test]
fn an_end_with_nothing_under_it_is_open() {
    let before = parts(&[("R1", (0.0, 0.0))]);
    let after = run(&before, &[((0.0, 0.0), (20.0, 0.0))], &[]);
    let got = intent(&before, &after, (0.0, 0.0), (20.0, 0.0));
    assert_eq!(got.open, [RunEnd::End]);
    assert!(got.joined && got.extra.is_empty());
}

/// A child sheet named `Sub` with one pin `SDA` at `at`.
fn with_sheet_pin(sheet: &mut SchematicSheet, at: Xy) {
    use signex_types::schematic::{ChildSheet, FillType, SheetPin};
    sheet.child_sheets.push(ChildSheet {
        uuid: Uuid::now_v7(),
        name: "Sub".into(),
        filename: "sub.snxsch".into(),
        position: pt(at.0, at.1 + 20.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: Uuid::now_v7(),
            name: "SDA".into(),
            direction: "input".into(),
            position: pt(at.0, at.1),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: Vec::new(),
    });
}

#[test]
fn a_sheet_pin_on_the_path_is_an_extra_connection_with_no_junction() {
    // The project netlist joins a sheet pin to the wire under it, end or
    // interior, so the run pulls it in.
    let mut before = parts(&[("R1", (0.0, 0.0)), ("C1", (20.0, 0.0))]);
    with_sheet_pin(&mut before, (10.0, 0.0));
    let after = run(&before, &[((0.0, 0.0), (20.0, 0.0))], &[]);
    let got = intent(&before, &after, (0.0, 0.0), (20.0, 0.0));
    assert_eq!(identities(&got, ExtraKind::SheetPin), ["SDA of Sub"]);
    assert_eq!(got.extra[0].position, pt(10.0, 0.0));
}

#[test]
fn a_run_ending_on_a_sheet_pin_is_attached_and_names_nothing_extra() {
    let mut before = parts(&[("R1", (0.0, 0.0))]);
    with_sheet_pin(&mut before, (10.0, 0.0));
    let after = run(&before, &[((0.0, 0.0), (10.0, 0.0))], &[]);
    let got = intent(&before, &after, (0.0, 0.0), (10.0, 0.0));
    assert!(got.extra.is_empty(), "{got:?}");
    assert!(got.open.is_empty(), "{got:?}");
}

#[test]
fn a_run_that_was_not_applied_is_not_joined() {
    let before = parts(&[("R1", (0.0, 0.0)), ("C1", (20.0, 0.0))]);
    let got = intent(&before, &before, (0.0, 0.0), (20.0, 0.0));
    assert!(!got.joined);
    assert!(got.net.is_none());
}
