//! #430 — multi-root / flat-stitch traversal: a `children` entry the root's
//! hierarchy never reaches is stitched in as its own independent top-level
//! page. Split out of the parent module only to keep that file under the
//! size cap.
//!
//! The fixture throughout is `Add Existing Sheet`'s routine flat topology:
//! several sibling pages, none referencing any other, passed to
//! [`build_project_netlist`] purely as extra `children` entries the root
//! never points at.

use std::collections::HashMap;

use signex_types::schematic::LabelType;

use super::super::{StitchIssue, build_project_netlist};
use super::{
    add_lib, child_sheet, empty_sheet, label, names, place, place_power, pt, sheet_pin, wire,
};

// 1 ── A page nobody references contributes its own net, and a shared
//      project-wide Global label merges it with the root's.
#[test]
fn flat_sibling_with_shared_global_label_merges_into_one_net() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    root.labels
        .push(label("VCC", pt(0.0, 0.0), LabelType::Global));
    add_lib(&mut root, "R");
    place(&mut root, "R_ROOT", "R", pt(10.0, 0.0));
    // Deliberately NOT referenced by any child_sheets entry anywhere.

    let mut sibling = empty_sheet();
    sibling.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sibling
        .labels
        .push(label("VCC", pt(0.0, 0.0), LabelType::Global));
    add_lib(&mut sibling, "R");
    place(&mut sibling, "R_SIB", "R", pt(10.0, 0.0));

    let mut children = HashMap::new();
    children.insert("b.snxsch".to_string(), sibling);

    let p = build_project_netlist(&root, &children, None);
    assert!(
        p.issues.is_empty(),
        "an unreferenced sibling is routine, not a structural problem: {:?}",
        p.issues
    );
    assert_eq!(
        p.netlist.nets.len(),
        1,
        "the shared Global label merges the two pages into one net: {:?}",
        names(&p.netlist)
    );
    let net = &p.netlist.nets[0];
    assert_eq!(
        net.name, "VCC",
        "unqualified — a flat sibling is a peer, not nested"
    );
    let mut refs: Vec<&str> = net.terminals.iter().map(|t| t.reference.as_str()).collect();
    refs.sort_unstable();
    assert_eq!(
        refs,
        vec!["R_ROOT", "R_SIB"],
        "both pages' terminals land on the merged net"
    );
}

// 2 ── Two flat siblings (neither referenced by root nor by each other) merge
//      with each other by a shared Power label, while the root's own,
//      unrelated net stays untouched.
#[test]
fn two_flat_siblings_merge_by_shared_power_label_root_stays_separate() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    add_lib(&mut root, "R");
    place(&mut root, "R_ROOT", "R", pt(10.0, 0.0));

    let mut b = empty_sheet();
    b.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    add_lib(&mut b, "R");
    add_lib(&mut b, "PWR");
    place(&mut b, "R_B", "R", pt(10.0, 0.0));
    place_power(&mut b, "#PWR01", "PWR", "GND", pt(0.0, 0.0));

    let mut c = empty_sheet();
    c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    add_lib(&mut c, "R");
    place(&mut c, "R_C", "R", pt(10.0, 0.0));
    c.labels.push(label("GND", pt(0.0, 0.0), LabelType::Power));

    let mut children = HashMap::new();
    children.insert("b.snxsch".to_string(), b);
    children.insert("c.snxsch".to_string(), c);

    let p = build_project_netlist(&root, &children, None);
    assert!(p.issues.is_empty(), "{:?}", p.issues);
    assert_eq!(
        p.netlist.nets.len(),
        2,
        "root's own net + the GND net merged across the two siblings: {:?}",
        names(&p.netlist)
    );
    let gnd = p
        .netlist
        .nets
        .iter()
        .find(|n| n.name == "GND")
        .expect("GND net present");
    let mut refs: Vec<&str> = gnd.terminals.iter().map(|t| t.reference.as_str()).collect();
    refs.sort_unstable();
    assert_eq!(
        refs,
        vec!["#PWR01", "R_B", "R_C"],
        "both sibling pages contribute (R_B and #PWR01 share sibling b's own wire, \
         C joins across pages by the shared GND label)"
    );
    let root_net = p
        .netlist
        .nets
        .iter()
        .find(|n| n.name != "GND")
        .expect("root's own net present");
    assert_eq!(root_net.terminals.len(), 1);
    assert_eq!(root_net.terminals[0].reference, "R_ROOT");
}

// 3 ── Local `Net` labels never cross sheets (rule 4) — two flat siblings with
//      distinct local names stay two distinct, unqualified nets.
#[test]
fn flat_siblings_with_distinct_local_labels_stay_separate() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    root.labels.push(label("SDA", pt(0.0, 0.0), LabelType::Net));
    add_lib(&mut root, "R");
    place(&mut root, "R_ROOT", "R", pt(10.0, 0.0));

    let mut sibling = empty_sheet();
    sibling.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sibling
        .labels
        .push(label("SCL", pt(0.0, 0.0), LabelType::Net));
    add_lib(&mut sibling, "R");
    place(&mut sibling, "R_SIB", "R", pt(10.0, 0.0));

    let mut children = HashMap::new();
    children.insert("b.snxsch".to_string(), sibling);

    let p = build_project_netlist(&root, &children, None);
    assert!(p.issues.is_empty(), "{:?}", p.issues);
    assert_eq!(p.netlist.nets.len(), 2, "no shared name, no merge");
    let ns = names(&p.netlist);
    assert!(ns.contains(&"SDA"), "root net unqualified: {ns:?}");
    assert!(ns.contains(&"SCL"), "sibling net unqualified: {ns:?}");
}

// 4 ── Two flat siblings that both happen to carry the SAME bare local `Net`
//      name must NOT merge (a local label is sheet-scoped, #430's own note:
//      "confirming hierarchical/local labels correctly do not merge across
//      siblings") — they collide on the *name* instead, exactly like two
//      same-named nets on one sheet, and dedup suffixes the second.
#[test]
fn flat_siblings_with_the_same_bare_local_name_collide_but_do_not_merge() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    root.labels.push(label("SDA", pt(0.0, 0.0), LabelType::Net));
    add_lib(&mut root, "R");
    place(&mut root, "R_ROOT", "R", pt(10.0, 0.0));

    let mut sibling = empty_sheet();
    sibling.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sibling
        .labels
        .push(label("SDA", pt(0.0, 0.0), LabelType::Net));
    add_lib(&mut sibling, "R");
    place(&mut sibling, "R_SIB", "R", pt(10.0, 0.0));

    let mut children = HashMap::new();
    children.insert("b.snxsch".to_string(), sibling);

    let p = build_project_netlist(&root, &children, None);
    assert_eq!(
        p.netlist.nets.len(),
        2,
        "same-named LOCAL labels still do not merge across pages: {:?}",
        names(&p.netlist)
    );
    // Each net keeps exactly its own page's terminal — proof they were never
    // unioned, only named alike.
    for net in &p.netlist.nets {
        assert_eq!(net.terminals.len(), 1);
    }
    let ns = names(&p.netlist);
    assert!(ns.contains(&"SDA"), "first keeps the bare name: {ns:?}");
    assert!(
        ns.iter().any(|n| n.starts_with("SDA_")),
        "second is suffixed, not silently dropped: {ns:?}"
    );
    assert!(
        p.issues.contains(&StitchIssue::NameCollision {
            name: "SDA".to_string()
        }),
        "the collision is reported, not silent: {:?}",
        p.issues
    );
}

// 5 ── A flat sibling nobody references can itself have its own child sheet
//      (a hierarchy hanging off a flat page). It must be visited exactly once
//      through the sibling's own subtree, not a second time as a would-be
//      orphan of its own.
#[test]
fn a_flat_siblings_own_child_sheet_is_visited_once_not_promoted_again() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    add_lib(&mut root, "R");
    place(&mut root, "R_ROOT", "R", pt(10.0, 0.0));

    // "b.snxsch" is a flat sibling (root never references it) that itself
    // references "c.snxsch" via a named sheet pin.
    let mut b = empty_sheet();
    b.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    add_lib(&mut b, "R");
    place(&mut b, "R_B", "R", pt(5.0, 0.0));
    b.child_sheets.push(child_sheet(
        "leaf",
        "c.snxsch",
        vec![sheet_pin("BUS", pt(0.0, 0.0))],
    ));

    let mut c = empty_sheet();
    c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    c.labels
        .push(label("BUS", pt(0.0, 0.0), LabelType::Hierarchical));
    add_lib(&mut c, "R");
    place(&mut c, "R_C", "R", pt(10.0, 0.0));

    let mut children = HashMap::new();
    children.insert("b.snxsch".to_string(), b);
    children.insert("c.snxsch".to_string(), c);

    let p = build_project_netlist(&root, &children, None);
    assert!(
        p.issues.is_empty(),
        "c.snxsch reached once through b's own subtree, not a duplicate instance: {:?}",
        p.issues
    );
    // R_ROOT's net stays alone; B and C merge through the sheet-pin binding.
    assert_eq!(p.netlist.nets.len(), 2, "{:?}", names(&p.netlist));
    let bus = p
        .netlist
        .nets
        .iter()
        .find(|n| n.terminals.iter().any(|t| t.reference == "R_C"))
        .expect("C's net present");
    let mut refs: Vec<&str> = bus.terminals.iter().map(|t| t.reference.as_str()).collect();
    refs.sort_unstable();
    assert_eq!(
        refs,
        vec!["R_B", "R_C"],
        "the sheet-pin binding still works from inside a promoted subtree"
    );
}

// 6 ── A flat sibling's own missing child is still reported, parent_path
//      naming the sibling (not the project root).
#[test]
fn a_flat_siblings_missing_child_is_reported_against_the_sibling() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    add_lib(&mut root, "R");
    place(&mut root, "R_ROOT", "R", pt(10.0, 0.0));

    let mut b = empty_sheet();
    b.child_sheets
        .push(child_sheet("gone", "gone.snxsch", Vec::new()));

    let mut children = HashMap::new();
    children.insert("b.snxsch".to_string(), b);

    let p = build_project_netlist(&root, &children, None);
    assert!(
        p.issues.iter().any(|i| matches!(
            i,
            StitchIssue::MissingChild { parent_path, filename, .. }
                if parent_path == "b.snxsch" && filename == "gone.snxsch"
        )),
        "missing child reported against the sibling that references it: {:?}",
        p.issues
    );
}

// 7 ── Two flat siblings that reference each other (neither reachable from
//      root) close a cycle instead of hanging the traversal.
#[test]
fn two_flat_siblings_referencing_each_other_is_a_cycle_not_a_hang() {
    let root = empty_sheet();

    let mut b = empty_sheet();
    b.child_sheets
        .push(child_sheet("toC", "c.snxsch", Vec::new()));
    let mut c = empty_sheet();
    c.child_sheets
        .push(child_sheet("toB", "b.snxsch", Vec::new()));

    let mut children = HashMap::new();
    children.insert("b.snxsch".to_string(), b);
    children.insert("c.snxsch".to_string(), c);

    // Finishing at all (rather than looping forever) is half the assertion.
    let p = build_project_netlist(&root, &children, None);
    assert!(
        p.issues
            .iter()
            .any(|i| matches!(i, StitchIssue::SheetCycle { .. })),
        "the mutual reference is reported as a cycle: {:?}",
        p.issues
    );
}

// 8 ── Determinism: which flat sibling gets promoted "first" must not depend
//      on the `children` map's hash order — mirrors
//      `output_is_deterministic_across_map_order` for the orphan-promotion
//      path specifically.
#[test]
fn orphan_promotion_is_deterministic_across_map_insertion_order() {
    let build = |forward: bool| {
        let root = empty_sheet();
        let mk = |name: &str, reference: &str| {
            let mut s = empty_sheet();
            s.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
            s.labels.push(label(name, pt(0.0, 0.0), LabelType::Global));
            add_lib(&mut s, "R");
            place(&mut s, reference, "R", pt(10.0, 0.0));
            s
        };
        let mut children = HashMap::new();
        if forward {
            children.insert("b.snxsch".to_string(), mk("SHARED", "R_B"));
            children.insert("c.snxsch".to_string(), mk("SHARED", "R_C"));
        } else {
            children.insert("c.snxsch".to_string(), mk("SHARED", "R_C"));
            children.insert("b.snxsch".to_string(), mk("SHARED", "R_B"));
        }
        build_project_netlist(&root, &children, None)
    };
    assert_eq!(build(true), build(false));
}
