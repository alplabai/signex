//! #2199 — what a point placement (label, power port, junction, no-connect)
//! does to net and bus names and to the pin's own net.

use signex_types::schematic::{Bus, ChildSheet, FillType, SheetPin};

use super::*;
use crate::build::{EndSpec, ExtraKind, placement_intent};

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

fn pin_ref(sheet: &SchematicSheet, reference: &str) -> EndSpec {
    let sym = sheet
        .symbols
        .iter()
        .find(|s| s.reference == reference)
        .expect("a part");
    EndSpec {
        at: sym.position,
        pin: Some((sym.uuid, "1".into())),
        ..EndSpec::at(sym.position)
    }
}

fn with_label(sheet: &SchematicSheet, text: &str, at: (f64, f64)) -> SchematicSheet {
    let mut after = sheet.clone();
    after
        .labels
        .push(label(text, pt(at.0, at.1), LabelType::Net));
    after
}

fn with_junction(sheet: &SchematicSheet, at: (f64, f64)) -> SchematicSheet {
    let mut after = sheet.clone();
    after.junctions.push(junction(pt(at.0, at.1)));
    after
}

/// A VCC wire along y = 0 whose tip at (10, 0) also carries a GND port that
/// no junction ties yet; R1 hangs on the wire's start.
fn vcc_with_a_loose_gnd_port() -> SchematicSheet {
    let mut sheet = parts(&[("R1", (0.0, 0.0))]);
    sheet.wires.push(wire(pt(0.0, 0.0), pt(20.0, 0.0)));
    sheet
        .labels
        .push(label("VCC", pt(0.0, 0.0), LabelType::Net));
    place_power(&mut sheet, "#PWR1", "P", "GND", pt(10.0, 0.0));
    sheet
}

#[test]
fn a_junction_or_label_that_ties_a_loose_port_renames_the_net() {
    let before = vcc_with_a_loose_gnd_port();
    for after in [
        with_junction(&before, (10.0, 0.0)),
        with_label(&before, "VCC", (10.0, 0.0)),
    ] {
        let got = placement_intent(&before, &after, None);
        assert_eq!(got.changes.len(), 1, "{got:?}");
        assert!(got.changes[0].after.contains(&"GND".to_string()));
    }
}

#[test]
fn the_same_name_an_alias_and_an_unnamed_net_are_no_change() {
    let mut before = parts(&[("R1", (0.0, 0.0))]);
    before.wires.push(wire(pt(0.0, 0.0), pt(20.0, 0.0)));
    // Unnamed: a name is gained freely.
    assert!(
        placement_intent(&before, &with_label(&before, "SIG", (10.0, 0.0)), None)
            .changes
            .is_empty()
    );
    before
        .labels
        .push(label("VCC", pt(0.0, 0.0), LabelType::Net));
    before
        .labels
        .push(label("X", pt(20.0, 0.0), LabelType::Net));
    for name in ["VCC", "X"] {
        let after = with_label(&before, name, (10.0, 0.0));
        assert!(placement_intent(&before, &after, None).changes.is_empty());
    }
    let after = with_label(&before, "GND", (10.0, 0.0));
    assert_eq!(placement_intent(&before, &after, None).changes.len(), 1);
}

#[test]
fn a_net_reaches_other_sheets_by_a_sheet_pin_a_global_or_hierarchical_label_or_a_port() {
    use crate::build::net_reaches_other_sheets;
    let base = || {
        let mut sheet = empty_sheet();
        sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        sheet
    };
    let mut plain = base();
    plain
        .labels
        .push(label("LOCAL", pt(0.0, 0.0), LabelType::Net));
    assert!(!net_reaches_other_sheets(&plain, pt(5.0, 0.0)));
    for kind in [LabelType::Global, LabelType::Hierarchical] {
        let mut sheet = base();
        sheet.labels.push(label("X", pt(10.0, 0.0), kind));
        assert!(net_reaches_other_sheets(&sheet, pt(5.0, 0.0)), "{kind:?}");
    }
    let mut ported = base();
    add_lib(
        &mut ported,
        "P",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place_power(&mut ported, "#PWR1", "P", "VCC", pt(10.0, 0.0));
    assert!(net_reaches_other_sheets(&ported, pt(5.0, 0.0)));
    let mut pinned = base();
    with_sheet_pin(&mut pinned, (0.0, 0.0));
    assert!(net_reaches_other_sheets(&pinned, pt(5.0, 0.0)));
}

#[test]
fn a_junction_merges_named_nets_only_when_their_names_differ() {
    let mut before = empty_sheet();
    before.wires.push(wire(pt(0.0, 0.0), pt(20.0, 0.0)));
    before.wires.push(wire(pt(10.0, -5.0), pt(10.0, 5.0)));
    before
        .labels
        .push(label("VCC", pt(0.0, 0.0), LabelType::Net));
    // One named, one unnamed: what a junction is for.
    let after = with_junction(&before, (10.0, 0.0));
    assert!(placement_intent(&before, &after, None).changes.is_empty());
    before
        .labels
        .push(label("GND", pt(10.0, 5.0), LabelType::Net));
    let after = with_junction(&before, (10.0, 0.0));
    let got = placement_intent(&before, &after, None);
    assert_eq!(got.changes.len(), 1);
    assert_eq!(got.changes[0].before.len(), 2);
}

fn with_sheet_pin(sheet: &mut SchematicSheet, at: (f64, f64)) {
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
fn a_placement_on_a_referenced_pin_names_what_it_pulls_onto_the_pins_net() {
    // A wire carrying C1.1 passes under R1.1's tip.
    let mut passing = parts(&[("R1", (0.0, 0.0)), ("C1", (0.0, 10.0))]);
    passing.wires.push(wire(pt(0.0, 10.0), pt(0.0, -10.0)));
    for after in [
        with_label(&passing, "SIG", (0.0, 0.0)),
        with_junction(&passing, (0.0, 0.0)),
    ] {
        let got = placement_intent(&passing, &after, Some(pin_ref(&passing, "R1")));
        assert!(
            got.extra
                .iter()
                .any(|i| i.kind == ExtraKind::Pin && i.identity == "C1.1"),
            "{got:?}"
        );
    }
    // A coordinate has no extra check: the caller's own T.
    let after = with_junction(&passing, (0.0, 0.0));
    assert!(placement_intent(&passing, &after, None).extra.is_empty());

    // A pin butted on the tip.
    let butted = parts(&[("R1", (0.0, 0.0)), ("C2", (0.0, 0.0))]);
    let mut after = butted.clone();
    after.no_connects.push(signex_types::schematic::NoConnect {
        uuid: Uuid::nil(),
        position: pt(0.0, 0.0),
    });
    let got = placement_intent(&butted, &after, Some(pin_ref(&butted, "R1")));
    assert!(got.extra.iter().any(|i| i.identity == "C2.1"), "{got:?}");

    // A sheet pin on the tip.
    let mut sheet_pin = parts(&[("R1", (0.0, 0.0))]);
    with_sheet_pin(&mut sheet_pin, (0.0, 0.0));
    let after = with_label(&sheet_pin, "X", (0.0, 0.0));
    let got = placement_intent(&sheet_pin, &after, Some(pin_ref(&sheet_pin, "R1")));
    assert!(
        got.extra
            .iter()
            .any(|i| i.kind == ExtraKind::SheetPin && i.identity == "SDA of Sub"),
        "{got:?}"
    );

    // Alone on its tip a label pulls in nothing; the label keeps its text.
    let alone = parts(&[("R1", (0.0, 0.0))]);
    let after = with_label(&alone, "SIG", (0.0, 0.0));
    assert!(
        placement_intent(&alone, &after, Some(pin_ref(&alone, "R1")))
            .extra
            .is_empty()
    );
}

#[test]
fn a_sheet_entry_reference_names_only_that_sheet_pin() {
    use crate::build::wire_intent;
    // R1.1 and the sheet pin SDA share a tip with no wire on it.
    let mut before = parts(&[("R1", (0.0, 0.0))]);
    with_sheet_pin(&mut before, (0.0, 0.0));
    let mut after = before.clone();
    after.wires.push(wire(pt(0.0, 0.0), pt(20.0, 0.0)));
    let entry = EndSpec {
        sheet_pin: Some(before.child_sheets[0].pins[0].uuid),
        ..EndSpec::at(pt(0.0, 0.0))
    };
    let tail = EndSpec::at(pt(20.0, 0.0));
    let got = wire_intent(&before, &after, entry, tail.clone());
    let items: Vec<_> = got
        .extra
        .iter()
        .map(|i| (i.kind, i.identity.as_str()))
        .collect();
    // The named pin is intended; the part pin on its tip is not.
    assert_eq!(items, [(ExtraKind::Pin, "R1.1")], "{got:?}");
    // A coordinate there names both.
    let got = wire_intent(&before, &after, EndSpec::at(pt(0.0, 0.0)), tail);
    assert!(got.extra.is_empty(), "{got:?}");
}

#[test]
fn a_label_that_names_a_bus_run_another_name_is_a_change() {
    let bus = |a: f64, b: f64| Bus {
        uuid: Uuid::now_v7(),
        start: pt(a, 0.0),
        end: pt(b, 0.0),
    };
    let mut before = empty_sheet();
    before.buses.push(bus(0.0, 10.0));
    before.buses.push(bus(40.0, 50.0));
    before
        .labels
        .push(label("D[0..7]", pt(0.0, 0.0), LabelType::Net));
    before
        .labels
        .push(label("A[0..15]", pt(50.0, 0.0), LabelType::Net));
    let after = with_label(&before, "D[0..7]", (45.0, 0.0));
    let got = placement_intent(&before, &after, None);
    assert!(got.changes.iter().any(|c| c.bus), "{got:?}");
    // The same name on its own run is no change, and a junction tying the two
    // runs merges them.
    let after = with_label(&before, "A[0..15]", (45.0, 0.0));
    assert!(placement_intent(&before, &after, None).changes.is_empty());
    let mut tied = before.clone();
    tied.buses.push(bus(10.0, 40.0));
    let got = placement_intent(&before, &tied, None);
    assert!(got.changes.iter().any(|c| c.bus && c.before.len() == 2));
}
