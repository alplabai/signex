//! Signal harness connectivity (#1280): bundles, entry joins, compiled names,
//! nesting, the cross-sheet join and the golden netlist of the fixture.

use super::*;
use crate::build_netlist;
use crate::harness_fixture::{
    connector, connectors, fixture_netlist, golden_text, harness_wire, mcu, place_part, sensor,
    stitch, wire,
};
use signex_types::schematic::{Label, LabelType, SchematicSheet};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Two `T` connectors with entries `a` and `b`, tips joined by a harness
/// wire, each entry 0 wired to a part's pin.
fn two_connectors(a: &[&str], b: &[&str]) -> SchematicSheet {
    let mut s = SchematicSheet::default();
    let r1 = place_part(&mut s, "R1", 1, pt(0.0, 0.0));
    let r2 = place_part(&mut s, "R2", 1, pt(100.0, 0.0));
    let left = connector("T", pt(20.0, 0.0), a);
    let right = connector("T", pt(60.0, 40.0), b);
    s.wires.push(wire(r1[0], left.entry_point(0)));
    s.wires.push(wire(r2[0], right.entry_point(0)));
    s.harness
        .wires
        .push(harness_wire(left.tip_point(), right.tip_point()));
    s.harness.connectors.extend([left, right]);
    s
}

#[test]
fn the_fixture_netlist_matches_its_golden_netlist() {
    let stitched = fixture_netlist();
    assert!(stitched.issues.is_empty(), "{:?}", stitched.issues);
    assert_eq!(
        golden_text(&stitched.netlist),
        include_str!("testdata/harness_netlist.golden.tsv")
    );
}

#[test]
fn same_named_entries_of_one_bundle_are_one_net_named_type_dot_entry() {
    let netlist = build_netlist(&two_connectors(&["A"], &["A"]));
    assert_eq!(netlist.nets.len(), 1, "{netlist:?}");
    assert_eq!(netlist.nets[0].name, "T.A");
    assert_eq!(netlist.nets[0].terminals.len(), 2);
}

#[test]
fn entries_of_different_names_stay_apart() {
    let netlist = build_netlist(&two_connectors(&["A"], &["B"]));
    assert_eq!(netlist.nets.len(), 2, "{netlist:?}");
    let mut names: Vec<&str> = netlist.nets.iter().map(|n| n.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["T.A", "T.B"]);
}

#[test]
fn a_net_label_outranks_the_compiled_name() {
    let mut s = two_connectors(&["A"], &["A"]);
    let at = s.wires[0].start;
    s.labels.push(Label::new("CLK", at, LabelType::Net));
    let netlist = build_netlist(&s);
    assert_eq!(netlist.nets.len(), 1);
    assert_eq!(netlist.nets[0].name, "CLK");
}

#[test]
fn a_nested_entry_compiles_to_type_entry_sub_entry() {
    let stitched = stitch(mcu(), vec![]);
    let mut names: Vec<String> = stitched
        .netlist
        .nets
        .iter()
        .map(|n| n.name.clone())
        .collect();
    names.sort();
    assert!(names.contains(&"DEBUG.UART.TX".to_string()), "{names:?}");
    assert!(names.contains(&"DEBUG.SWDIO".to_string()), "{names:?}");
    // The nested UART bundle is not a top-level harness of its own.
    assert!(!names.iter().any(|n| n.starts_with("UART.")), "{names:?}");
}

#[test]
fn a_typed_sheet_entry_joins_the_childs_port_entry_by_entry() {
    let stitched = stitch(mcu(), vec![("sensor.snxsch", sensor())]);
    let sck = stitched
        .netlist
        .nets
        .iter()
        .find(|n| n.name == "SPI_BUS.SCK")
        .expect("SPI_BUS.SCK");
    let pins: Vec<String> = sck
        .terminals
        .iter()
        .map(|t| format!("{}.{}", t.reference, t.pin))
        .collect();
    assert_eq!(pins, ["U1.1", "U3.1"]);
}

#[test]
fn two_types_in_one_bundle_are_reported_as_a_conflict() {
    let mut s = two_connectors(&["A"], &["A"]);
    s.harness.connectors[1].harness_type = "U".into();
    let bundles = harness_bundles(&s);
    assert_eq!(bundles.len(), 1);
    assert_eq!(bundles[0].types, ["T", "U"]);
    // The net still joins, named after the first type.
    let netlist = build_netlist(&s);
    assert_eq!(netlist.nets.len(), 1);
    assert_eq!(netlist.nets[0].name, "T.A");
}

#[test]
fn bundles_list_ports_typed_entries_and_nesting() {
    let bundles = harness_bundles(&connectors());
    let top: Vec<&HarnessBundle> = bundles.iter().filter(|b| !b.nested).collect();
    assert_eq!(top.len(), 1);
    assert_eq!(top[0].ports.len(), 1);
    assert_eq!(top[0].ports[0].name, "DEBUG");
    assert_eq!(top[0].entry_names, ["SWDIO", "SWCLK", "UART"]);
    assert!(bundles.iter().any(|b| b.nested && b.types == ["UART"]));
    let root = harness_bundles(&mcu());
    let spi = root.iter().find(|b| b.types == ["SPI_BUS"]).unwrap();
    assert_eq!(spi.sheet_entries.len(), 1);
    assert_eq!(spi.sheet_entries[0].name, "SPI_BUS");
}

#[test]
fn a_bundle_lists_its_leaves_with_nested_entries_expanded() {
    let bundles = harness_bundles(&connectors());
    let top = bundles.iter().find(|b| !b.nested).unwrap();
    let paths: Vec<String> = top.leaves.iter().map(HarnessLeaf::dotted).collect();
    assert_eq!(paths, ["SWDIO", "SWCLK", "UART.TX", "UART.RX"]);
    assert!(top.leaves[0].is_plain());
    assert!(!top.leaves[2].is_plain());
    assert_eq!(top.leaves[2].connector, top.connectors[0]);
}

#[test]
fn a_sheet_without_harness_is_left_exactly_as_it_was() {
    let mut s = sensor();
    s.harness = Default::default();
    s.labels.clear();
    let mut parent = crate::build::merged_sheet_parent(&s);
    let before = parent.clone();
    join_entry_nets(&s, &mut parent);
    assert_eq!(parent, before);
    let sampled = sample(&s, &mut parent);
    assert!(sampled.names.is_empty());
    assert!(sampled.ports.is_empty());
    assert!(harness_bundles(&s).is_empty());
    // Every net keeps its auto name.
    assert!(
        build_netlist(&s)
            .nets
            .iter()
            .all(|n| n.name.starts_with("N$"))
    );
}

#[test]
fn a_missing_far_entry_is_paired_with_the_one_this_side_has() {
    let id = Uuid::from_u128;
    let entries = vec![(id(1), "SCK".to_string()), (id(2), "CS".to_string())];
    let far = vec!["SCK".to_string(), "CS_ADC".to_string()];
    let found = harness_mismatches(id(9), &entries, &far);
    assert_eq!(
        found,
        [HarnessMismatch {
            connector: id(9),
            entry: Some(id(2)),
            has: Some("CS".into()),
            expects: "CS_ADC".into(),
        }]
    );
    assert!(harness_mismatches(id(9), &entries, &["SCK".into(), "CS".into()]).is_empty());
}

#[test]
fn a_point_on_a_harness_wire_is_on_the_harness() {
    let mut s = SchematicSheet::default();
    s.harness
        .wires
        .push(harness_wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    assert!(on_harness(&s, &pt(0.0, 0.0)));
    assert!(on_harness(&s, &pt(5.0, 0.0)));
    assert!(!on_harness(&s, &pt(5.0, 1.0)));
}
