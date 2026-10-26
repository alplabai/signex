//! #1049 — a bus carried by a sheet pin, a Port or a hierarchical label joins
//! its members across sheets, member by member.
use std::collections::HashMap;

use signex_types::schematic::{Bus, BusEntry, LabelType};
use uuid::Uuid;

use super::super::ProjectNetlist;
use super::{
    SchematicSheet, add_lib, child_sheet, empty_sheet, label, place, pt, sheet_pin, stitch,
    stitch_pages, wire,
};

/// A vertical bus `(0,0)-(0,40)` named `bus_name` at its top end, with one
/// entry per `(member, reference)`; each entry feeds a wire to a resistor of
/// that reference, and the wire carries a local label of the member's name.
fn bus_sheet(
    bus_name: &str,
    bus_label_type: LabelType,
    members: &[(&str, &str)],
) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.buses.push(Bus {
        uuid: Uuid::new_v4(),
        start: pt(0.0, 0.0),
        end: pt(0.0, 40.0),
    });
    sheet
        .labels
        .push(label(bus_name, pt(0.0, 40.0), bus_label_type));
    add_lib(&mut sheet, "R");
    for (i, (member, reference)) in members.iter().enumerate() {
        let y = 5.0 + 10.0 * i as f64;
        sheet.bus_entries.push(BusEntry {
            uuid: Uuid::new_v4(),
            position: pt(0.0, y),
            size: (2.54, 2.54),
        });
        sheet
            .wires
            .push(wire(pt(2.54, y + 2.54), pt(12.54, y + 2.54)));
        place(&mut sheet, reference, "R", pt(12.54, y + 2.54));
        sheet
            .labels
            .push(label(member, pt(6.0, y + 2.54), LabelType::Net));
    }
    sheet
}

/// The references on the net that carries `reference`, sorted.
fn net_of(p: &ProjectNetlist, reference: &str) -> Vec<String> {
    let net = p
        .netlist
        .nets
        .iter()
        .find(|n| n.terminals.iter().any(|t| t.reference == reference))
        .unwrap_or_else(|| panic!("no net carries {reference}"));
    let mut refs: Vec<String> = net.terminals.iter().map(|t| t.reference.clone()).collect();
    refs.sort();
    refs
}

/// A parent whose bus is named `pin_name` by a sheet pin on its top end (the
/// bus carries no label of its own), and one child file `child.sch` whose bus
/// is named by a `child_label` of kind `ty`.
fn two_level(
    pin_name: &str,
    parent_members: &[(&str, &str)],
    child_label: &str,
    ty: LabelType,
    child_members: &[(&str, &str)],
) -> (SchematicSheet, HashMap<String, SchematicSheet>) {
    let mut root = bus_sheet("unused", LabelType::Net, parent_members);
    // Drop the bus-name label: the sheet pin names this bus.
    root.labels.remove(0);
    root.child_sheets.push(child_sheet(
        "Sub",
        "child.sch",
        vec![sheet_pin(pin_name, pt(0.0, 40.0))],
    ));
    let child = bus_sheet(child_label, ty, child_members);
    (root, HashMap::from([("child.sch".to_string(), child)]))
}

#[test]
fn a_bus_through_a_sheet_pin_joins_its_members_one_by_one() {
    let (root, map) = two_level(
        "D[0..3]",
        &[("D1", "RP1"), ("D2", "RP2")],
        "D[0..3]",
        LabelType::Hierarchical,
        &[("D1", "RC1"), ("D2", "RC2")],
    );
    let p = stitch(&root, "root", &map);
    assert_eq!(net_of(&p, "RP1"), ["RC1", "RP1"]);
    assert_eq!(net_of(&p, "RP2"), ["RC2", "RP2"]);
    assert_eq!(p.netlist.nets.len(), 2, "members stay separate nets");
}

#[test]
fn a_member_missing_on_the_narrower_side_stays_local() {
    let (root, map) = two_level(
        "D[0..3]",
        &[("D1", "RP1"), ("D3", "RP3")],
        "D[0..1]",
        LabelType::Hierarchical,
        &[("D1", "RC1")],
    );
    let p = stitch(&root, "root", &map);
    assert_eq!(net_of(&p, "RP1"), ["RC1", "RP1"], "D1 is on both buses");
    assert_eq!(net_of(&p, "RP3"), ["RP3"], "D3 is not on the child's bus");
}

#[test]
fn a_local_bus_label_never_leaves_its_sheet() {
    let (root, map) = two_level(
        "D[0..3]",
        &[("D1", "RP1")],
        "D[0..3]",
        LabelType::Net,
        &[("D1", "RC1")],
    );
    let p = stitch(&root, "root", &map);
    assert_eq!(net_of(&p, "RP1"), ["RP1"]);
}

#[test]
fn same_named_global_buses_join_member_by_member() {
    let a = bus_sheet(
        "D[0..3]",
        LabelType::Global,
        &[("D1", "RA1"), ("D2", "RA2")],
    );
    let b = bus_sheet(
        "D[0..3]",
        LabelType::Global,
        &[("D2", "RB2"), ("D3", "RB3")],
    );
    let map = HashMap::from([("b.sch".to_string(), b)]);
    let p = stitch_pages(&a, "root", &map, &["b.sch"]);
    assert_eq!(net_of(&p, "RA2"), ["RA2", "RB2"]);
    assert_eq!(net_of(&p, "RA1"), ["RA1"]);
    assert_eq!(net_of(&p, "RB3"), ["RB3"]);
}

#[test]
fn a_group_bus_joins_through_a_sheet_pin() {
    let (root, map) = two_level(
        "{SDA SCL}",
        &[("SDA", "RP1")],
        "{SDA SCL}",
        LabelType::Hierarchical,
        &[("SDA", "RC1")],
    );
    let p = stitch(&root, "root", &map);
    assert_eq!(net_of(&p, "RP1"), ["RC1", "RP1"]);
}
