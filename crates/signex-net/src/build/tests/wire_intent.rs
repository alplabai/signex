//! #2199 — what a headless wire run pulls into its net besides its two ends.

use super::*;
use crate::build::{EndSpec, ExtraKind, RunEnd, WireIntent, wire_intent};

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
    wire_intent(
        before,
        after,
        EndSpec::at(pt(start.0, start.1)),
        EndSpec::at(pt(end.0, end.1)),
    )
}

/// The end `reference`.1 names, as a pin reference: its tip and pin.
fn pin_ref(sheet: &SchematicSheet, reference: &str) -> EndSpec {
    let sym = sheet
        .symbols
        .iter()
        .find(|s| s.reference == reference)
        .expect("a part");
    EndSpec {
        at: sym.position,
        pin: Some((sym.uuid, "1".into())),
    }
}

#[test]
fn a_wire_passing_under_a_referenced_pin_is_not_joined_silently() {
    // R1.1 sits on the interior of a GND wire with no end or junction there.
    let mut before = parts(&[("R1", (0.0, 0.0)), ("C1", (20.0, 0.0))]);
    before.wires.push(wire(pt(0.0, -10.0), pt(0.0, 10.0)));
    before
        .labels
        .push(label("GND", pt(0.0, 10.0), LabelType::Net));
    // The engine mints the T junction at the run's end on the passing wire.
    let after = run(&before, &[((0.0, 0.0), (20.0, 0.0))], &[(0.0, 0.0)]);
    for start in [pin_ref(&before, "R1"), EndSpec::at(pt(0.0, 0.0))] {
        let got = wire_intent(&before, &after, start, pin_ref(&before, "C1"));
        assert!(
            !identities(&got, ExtraKind::Label).is_empty(),
            "the GND net is pulled in: {got:?}"
        );
    }
}

#[test]
fn a_power_port_touching_a_referenced_pin_is_extra() {
    // A GND port butted on R1.1 pin to pin is no connection (ADR-0002 D5.1);
    // the run to R1.1 would make it one.
    let mut before = parts(&[("R1", (0.0, 0.0)), ("C1", (20.0, 0.0))]);
    place_power(&mut before, "#PWR1", "P", "GND", pt(0.0, 0.0));
    let after = run(&before, &[((0.0, 0.0), (20.0, 0.0))], &[]);
    let got = wire_intent(
        &before,
        &after,
        pin_ref(&before, "R1"),
        pin_ref(&before, "C1"),
    );
    assert_eq!(identities(&got, ExtraKind::Pin), ["#PWR1.1"], "{got:?}");

    // With a stub wire on the tip the port and R1.1 are already on one net,
    // which the run wanted: the port is no surprise.
    before.wires.push(wire(pt(0.0, 0.0), pt(0.0, -5.0)));
    let after = run(&before, &[((0.0, 0.0), (20.0, 0.0))], &[]);
    let got = wire_intent(
        &before,
        &after,
        pin_ref(&before, "R1"),
        pin_ref(&before, "C1"),
    );
    assert!(got.extra.is_empty(), "{got:?}");
}

#[test]
fn a_label_on_a_referenced_pin_under_a_passing_wire_would_join_that_wire() {
    use crate::build::{TipAnchor, joins_at_pin};
    // An unnamed wire carrying C1.1 passes under R1.1's tip: no end or
    // junction there, so R1.1 is on no net until a label anchors the tip.
    let mut before = parts(&[("R1", (0.0, 0.0)), ("C1", (0.0, 10.0))]);
    before.wires.push(wire(pt(0.0, 10.0), pt(0.0, -10.0)));
    for anchor in [TipAnchor::Label, TipAnchor::Junction] {
        let extra = joins_at_pin(&before, pin_ref(&before, "R1"), anchor);
        assert!(
            extra
                .iter()
                .any(|i| i.kind == ExtraKind::Pin && i.identity == "C1.1"),
            "{extra:?}"
        );
    }
    // On a tip nothing passes under, nothing is pulled in.
    let alone = parts(&[("R1", (0.0, 0.0))]);
    assert!(joins_at_pin(&alone, pin_ref(&alone, "R1"), TipAnchor::Junction).is_empty());
}

#[test]
fn a_no_connect_on_a_pin_butted_on_another_pin_would_join_them() {
    use crate::build::{TipAnchor, joins_at_pin};
    let before = parts(&[("R1", (0.0, 0.0)), ("C2", (0.0, 0.0))]);
    let extra = joins_at_pin(&before, pin_ref(&before, "R1"), TipAnchor::NoConnect);
    assert_eq!(identities_of(&extra, ExtraKind::Pin), ["C2.1"], "{extra:?}");
    // Alone on its tip the flag joins nothing.
    let alone = parts(&[("R1", (0.0, 0.0))]);
    assert!(joins_at_pin(&alone, pin_ref(&alone, "R1"), TipAnchor::NoConnect).is_empty());
}

fn identities_of(extra: &[crate::build::ExtraItem], kind: ExtraKind) -> Vec<&str> {
    extra
        .iter()
        .filter(|i| i.kind == kind)
        .map(|i| i.identity.as_str())
        .collect()
}

#[test]
fn a_no_connect_flag_is_no_connection_anchor() {
    use crate::build::has_connection_anchor;
    let mut sheet = parts(&[("R1", (0.0, 0.0))]);
    sheet.no_connects.push(signex_types::schematic::NoConnect {
        uuid: Uuid::nil(),
        position: pt(0.0, 0.0),
    });
    assert!(!has_connection_anchor(&sheet, pt(0.0, 0.0)));
    sheet.wires.push(wire(pt(0.0, 0.0), pt(0.0, 5.0)));
    assert!(has_connection_anchor(&sheet, pt(0.0, 0.0)));
    assert!(
        !has_connection_anchor(&sheet, pt(0.0, 2.0)),
        "interior is no anchor"
    );
}

#[test]
fn a_run_ending_on_either_end_of_a_bus_entry_is_attached() {
    let mut before = parts(&[("R1", (0.0, 0.0))]);
    before.bus_entries.push(signex_types::schematic::BusEntry {
        uuid: Uuid::nil(),
        position: pt(20.0, 10.0),
        size: (-2.54, -2.54),
    });
    let after = run(&before, &[((0.0, 0.0), (17.46, 7.46))], &[]);
    let got = intent(&before, &after, (0.0, 0.0), (17.46, 7.46));
    assert!(got.open.is_empty(), "{got:?}");
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
    assert!(got.open.is_empty() && got.joined && !got.short());
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
    assert!(got.short());
    assert_eq!(got.merged_names(), ["VCC", "GND"]);
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
    let got = intent(&before, &after, (0.0, 0.0), (20.0, 0.0));
    assert!(!got.short());
    assert_eq!(got.merged_names(), ["VCC"]);
}

#[test]
fn nets_with_different_name_sets_are_a_short_even_with_a_shared_best_name() {
    // Both stubs carry the hierarchical label DATA (the best name of each),
    // and a net label of their own: two nets, P and Q.
    let mut before = parts(&[("R1", (0.0, 0.0)), ("C1", (20.0, 0.0))]);
    before.wires.push(wire(pt(0.0, 0.0), pt(0.0, 10.0)));
    before.wires.push(wire(pt(20.0, 0.0), pt(20.0, 10.0)));
    for (x, own) in [(0.0, "P"), (20.0, "Q")] {
        before
            .labels
            .push(label("DATA", pt(x, 10.0), LabelType::Hierarchical));
        before.labels.push(label(own, pt(x, 5.0), LabelType::Net));
    }
    let after = run(&before, &[((0.0, 0.0), (20.0, 0.0))], &[]);
    let got = intent(&before, &after, (0.0, 0.0), (20.0, 0.0));
    assert!(got.short(), "{got:?}");

    // The same names on both sides are no short.
    let mut same = parts(&[("R1", (0.0, 0.0)), ("C1", (20.0, 0.0))]);
    same.wires.push(wire(pt(0.0, 0.0), pt(0.0, 10.0)));
    same.wires.push(wire(pt(20.0, 0.0), pt(20.0, 10.0)));
    for x in [0.0, 20.0] {
        same.labels
            .push(label("DATA", pt(x, 10.0), LabelType::Hierarchical));
        same.labels.push(label("P", pt(x, 5.0), LabelType::Net));
    }
    let after = run(&same, &[((0.0, 0.0), (20.0, 0.0))], &[]);
    assert!(!intent(&same, &after, (0.0, 0.0), (20.0, 0.0)).short());
}

#[test]
fn a_junction_merges_named_nets_only_when_their_names_differ() {
    use crate::build::{junction_nets, names_short};
    // VCC along y = 0, GND across it with no junction.
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(20.0, 0.0)));
    sheet.wires.push(wire(pt(10.0, -5.0), pt(10.0, 5.0)));
    sheet
        .labels
        .push(label("VCC", pt(0.0, 0.0), LabelType::Net));
    // One named, one unnamed: a junction is what ties them, allowed.
    assert!(!names_short(&junction_nets(&sheet, pt(10.0, 0.0))));
    sheet
        .labels
        .push(label("GND", pt(10.0, 5.0), LabelType::Net));
    let nets = junction_nets(&sheet, pt(10.0, 0.0));
    assert_eq!(nets.len(), 2);
    assert!(names_short(&nets));
    // Nothing under the point, or a single wire: nothing merges.
    assert!(!names_short(&junction_nets(&sheet, pt(5.0, 0.0))));
    assert!(!names_short(&junction_nets(&sheet, pt(50.0, 50.0))));
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
fn the_net_names_at_a_point_are_every_name_best_first_or_none() {
    use crate::build::net_names_at;
    let mut sheet = parts(&[("R1", (0.0, 0.0))]);
    sheet.wires.push(wire(pt(0.0, 0.0), pt(0.0, 10.0)));
    assert!(net_names_at(&sheet, pt(0.0, 5.0)).is_empty(), "unnamed");
    sheet
        .labels
        .push(label("SIG", pt(0.0, 10.0), LabelType::Net));
    sheet
        .labels
        .push(label("VCC", pt(0.0, 10.0), LabelType::Global));
    // A wire interior and a pin tip both read the net's names, the best first.
    assert_eq!(net_names_at(&sheet, pt(0.0, 5.0)), ["VCC", "SIG"]);
    assert_eq!(net_names_at(&sheet, pt(0.0, 0.0)), ["VCC", "SIG"]);
    assert!(
        net_names_at(&sheet, pt(50.0, 50.0)).is_empty(),
        "nothing there"
    );
    // A power-port value is a name too.
    place_power(&mut sheet, "#PWR1", "P", "GND", pt(0.0, 10.0));
    assert!(net_names_at(&sheet, pt(0.0, 5.0)).contains(&"GND".to_string()));
}

#[test]
fn a_run_that_was_not_applied_is_not_joined() {
    let before = parts(&[("R1", (0.0, 0.0)), ("C1", (20.0, 0.0))]);
    let got = intent(&before, &before, (0.0, 0.0), (20.0, 0.0));
    assert!(!got.joined);
    assert!(got.net.is_none());
}
