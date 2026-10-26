//! #1061 — `multi_named_nets`: every net with two or more different names,
//! including nets with no component pin, reported once with the exported name.

use super::*;

/// `multi_named_nets` over `root` (key `root`) and `children` (keyed by
/// filename), walked from `root` only.
fn multi(
    root: &SchematicSheet,
    children: &HashMap<String, SchematicSheet>,
) -> (Vec<MultiNamedNet>, ProjectNetlist) {
    let mut sheets: HashMap<SheetKey, SchematicSheet> = children
        .iter()
        .map(|(k, v)| (SheetKey::new(k.clone()), v.clone()))
        .collect();
    sheets.insert(SheetKey::new("root"), root.clone());
    let resolved: HashMap<SheetKey, HashMap<String, SheetKey>> = sheets
        .iter()
        .map(|(key, sheet)| {
            let submap = sheet
                .child_sheets
                .iter()
                .map(|cs| (cs.filename.clone(), SheetKey::new(cs.filename.clone())))
                .collect();
            (key.clone(), submap)
        })
        .collect();
    let roots = [ProjectRoot {
        key: SheetKey::new("root"),
    }];
    let graph = ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    };
    (multi_named_nets(&graph), build_project_netlist(&graph))
}

fn source_names(net: &MultiNamedNet) -> Vec<&str> {
    net.sources.iter().map(|s| s.name.as_str()).collect()
}

/// A wire `(0,0)-(10,0)` with a resistor `reference` on its left end and a
/// `Net` label `a` there plus a `Hierarchical` label `b` on the right end.
fn named_wire(reference: &str, a: &str, b: &str) -> SchematicSheet {
    let mut s = empty_sheet();
    s.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    s.labels.push(label(a, pt(0.0, 0.0), LabelType::Net));
    s.labels
        .push(label(b, pt(10.0, 0.0), LabelType::Hierarchical));
    add_lib(&mut s, "R");
    place(&mut s, reference, "R", pt(0.0, 0.0));
    s
}

#[test]
fn a_bare_wire_with_two_labels_and_no_pin_is_reported_once() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    root.labels
        .push(label("A", pt(0.0, 0.0), LabelType::Hierarchical));
    root.labels
        .push(label("B", pt(10.0, 0.0), LabelType::Hierarchical));

    let (found, netlist) = multi(&root, &HashMap::new());

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].exported, "A");
    assert_eq!(source_names(&found[0]), ["A", "B"]);
    assert!(
        netlist.netlist.nets.is_empty(),
        "no pin, so no exported net"
    );
}

#[test]
fn a_power_port_only_rail_with_a_second_name_is_reported_with_the_symbol() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    root.labels
        .push(label("RAIL", pt(10.0, 0.0), LabelType::Hierarchical));
    add_lib(&mut root, "PWR");
    place_power(&mut root, "#PWR1", "PWR", "VCC", pt(0.0, 0.0));
    let port = Uuid::from_u128(7);
    root.symbols[0].uuid = port;

    let (found, _) = multi(&root, &HashMap::new());

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].exported, "VCC", "a power name outranks a label");
    let power = found[0]
        .sources
        .iter()
        .find(|s| s.label_type.is_none())
        .expect("the power-port source");
    assert_eq!((power.name.as_str(), power.uuid), ("VCC", port));
}

#[test]
fn a_suffixed_exported_name_is_reported_as_the_export_shows_it() {
    // Two sibling children named `charger`: each net is `charger/ALT` and the
    // later one is suffixed by the export's dedup pass.
    let mut root = empty_sheet();
    root.child_sheets
        .push(child_sheet("charger", "a.sch", Vec::new()));
    root.child_sheets
        .push(child_sheet("charger", "b.sch", Vec::new()));
    let mut map = HashMap::new();
    map.insert("a.sch".to_string(), named_wire("RA", "SDA", "ALT"));
    map.insert("b.sch".to_string(), named_wire("RB", "SDA", "ALT"));

    let (found, netlist) = multi(&root, &map);

    let mut exported: Vec<&str> = found.iter().map(|n| n.exported.as_str()).collect();
    exported.sort();
    let mut netlist_names: Vec<&str> = names(&netlist.netlist);
    netlist_names.sort();
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(exported, netlist_names, "same names as the export");
    assert!(exported.iter().any(|n| n.starts_with("charger/ALT_")));
}

#[test]
fn a_child_placed_twice_lists_each_name_once_per_net() {
    // Global names join every instance into one net; the shared child's labels
    // must not show up once per instance.
    let mut root = empty_sheet();
    root.child_sheets
        .push(child_sheet("U1", "c.sch", Vec::new()));
    root.child_sheets
        .push(child_sheet("U2", "c.sch", Vec::new()));
    let mut c = empty_sheet();
    c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    c.labels.push(label("A", pt(0.0, 0.0), LabelType::Global));
    c.labels.push(label("B", pt(10.0, 0.0), LabelType::Global));
    let mut map = HashMap::new();
    map.insert("c.sch".to_string(), c);

    let (found, _) = multi(&root, &map);

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(source_names(&found[0]), ["A", "B"]);
}

#[test]
fn the_winner_is_carried_out_even_when_the_export_suffixes_it() {
    // `ZED` (hierarchical) beats `ALT` (local) but sorts after it; the second
    // `charger/ZED` is suffixed, so its exported text matches no source.
    let mut root = empty_sheet();
    root.child_sheets
        .push(child_sheet("charger", "a.sch", Vec::new()));
    root.child_sheets
        .push(child_sheet("charger", "b.sch", Vec::new()));
    let mut map = HashMap::new();
    map.insert("a.sch".to_string(), named_wire("RA", "ALT", "ZED"));
    map.insert("b.sch".to_string(), named_wire("RB", "ALT", "ZED"));

    let (found, _) = multi(&root, &map);

    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found.iter().any(|n| n.exported.starts_with("charger/ZED_")));
    for net in &found {
        assert_eq!(net.sources[net.winner].name, "ZED", "{net:?}");
        assert!(net.has_terminals);
    }
}

#[test]
fn a_terminal_less_group_is_marked_as_not_exported() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    root.labels
        .push(label("B", pt(0.0, 0.0), LabelType::Hierarchical));
    root.labels
        .push(label("A", pt(10.0, 0.0), LabelType::Hierarchical));

    let (found, _) = multi(&root, &HashMap::new());

    assert!(!found[0].has_terminals);
    assert_eq!(found[0].sources[found[0].winner].name, "A");
}
