//! #430 — multi-root / flat-stitch traversal: every declared page the root's
//! hierarchy never reaches is walked as its own independent top-level page.
//! Split out of the parent module only to keep that file under the size cap.
//!
//! The fixture throughout is `Add Existing Sheet`'s routine flat topology:
//! several sibling pages, none referencing any other, handed to
//! [`build_project_netlist`](super::super::build_project_netlist) as extra
//! [`ProjectRoot`](super::super::ProjectRoot)s the root never points at.
//!
//! Under #466 the page list is the *caller's* to state — `assemble_project_sheets`
//! computes `pages_outside_the_hierarchy` as "declared, and not reachable
//! **from the root**" — so these tests pass the pages explicitly via
//! `stitch_pages` rather than leaving the stitcher to guess which sheets are
//! orphans. Test 5 is the case that makes the traversal's visited set
//! load-bearing: a page that another page reaches is still on the caller's
//! list, and must contribute exactly one occurrence.

use std::collections::HashMap;

use signex_types::schematic::{LabelType, SchematicSheet};

use super::super::{ProjectGraph, ProjectRoot, SheetKey, StitchIssue, build_project_netlist};
use super::{
    add_lib, child_sheet, empty_sheet, label, names, place, place_power, pt, sheet_pin,
    stitch_pages, wire,
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

    let p = stitch_pages(&root, "root", &children, &["b.snxsch"]);
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
        "unqualified — a Global label is project-wide, a flat page is a peer"
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

    let p = stitch_pages(&root, "root", &children, &["b.snxsch", "c.snxsch"]);
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

// 3 ── Local `Net` labels never cross sheets (rule 4) — two flat pages with
//      distinct local names stay two distinct nets.
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

    let p = stitch_pages(&root, "root", &children, &["b.snxsch"]);
    assert!(p.issues.is_empty(), "{:?}", p.issues);
    assert_eq!(p.netlist.nets.len(), 2, "no shared name, no merge");
    let ns = names(&p.netlist);
    assert!(
        ns.iter().any(|n| n.starts_with("SDA")),
        "root's own net present: {ns:?}"
    );
    assert!(
        ns.iter().any(|n| n.starts_with("SCL")),
        "the page's own net present: {ns:?}"
    );
}

// 4 ── Two flat pages that both happen to carry the SAME bare local `Net`
//      name must NOT merge (a local label is sheet-scoped) — they collide on
//      the *name* instead, exactly like two same-named nets on one sheet, and
//      dedup suffixes the second.
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

    let p = stitch_pages(&root, "root", &children, &["b.snxsch"]);
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
    assert_eq!(
        ns.iter().filter(|n| n.starts_with("SDA")).count(),
        2,
        "the second is suffixed, not silently dropped: {ns:?}"
    );
}

// 5 ── THE VISITED-SET CASE. A flat page can itself have its own child sheet
//      (a hierarchy hanging off a flat page). Because `pages_outside_the_hierarchy`
//      is "declared and not reachable **from the root**", that child is on the
//      caller's page list too — the root reaches neither. It must still be
//      visited exactly once, through its parent page's subtree, and NOT walked
//      a second time as a root in its own right.
//
//      A second visit would not merely be redundant: it duplicates the sheet's
//      terminals, raises a spurious `SharedReferenceAcrossInstances`, and
//      shifts every subsequent `NetId` (ids are positional).
#[test]
fn a_flat_pages_own_child_sheet_is_visited_once_not_walked_again_as_a_root() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    add_lib(&mut root, "R");
    place(&mut root, "R_ROOT", "R", pt(10.0, 0.0));

    // "b.snxsch" is a flat page (root never references it) that itself
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

    // Both are on the page list, exactly as the app would produce them.
    let p = stitch_pages(&root, "root", &children, &["b.snxsch", "c.snxsch"]);
    assert!(
        p.issues.is_empty(),
        "c.snxsch reached once through b's own subtree, not a second instance: {:?}",
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
        "the sheet-pin binding still works, and R_C appears exactly once"
    );
}

// 6 ── A flat page's own missing child is still reported.
#[test]
fn a_flat_pages_missing_child_is_reported() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    add_lib(&mut root, "R");
    place(&mut root, "R_ROOT", "R", pt(10.0, 0.0));

    let mut b = empty_sheet();
    b.child_sheets
        .push(child_sheet("gone", "gone.snxsch", Vec::new()));

    let mut children = HashMap::new();
    children.insert("b.snxsch".to_string(), b);

    let p = stitch_pages(&root, "root", &children, &["b.snxsch"]);
    assert!(
        p.issues.iter().any(|i| matches!(
            i,
            StitchIssue::MissingChild { filename, .. } if filename == "gone.snxsch"
        )),
        "missing child reported against the page that references it: {:?}",
        p.issues
    );
}

// 7 ── Two flat pages that reference each other (neither reachable from root)
//      close a cycle instead of hanging the traversal.
#[test]
fn two_flat_pages_referencing_each_other_is_a_cycle_not_a_hang() {
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
    let p = stitch_pages(&root, "root", &children, &["b.snxsch", "c.snxsch"]);
    assert!(
        p.issues
            .iter()
            .any(|i| matches!(i, StitchIssue::SheetCycle { .. })),
        "the mutual reference is reported as a cycle: {:?}",
        p.issues
    );
}

// 8 ── Determinism: the result must not depend on the `sheets` map's hash
//      order. Root order is the caller's to fix — `ProjectGraph.roots` is an
//      ordered slice precisely so it is never left to a hash — and what this
//      pins is that nothing *else* in the traversal leaks map order.
#[test]
fn flat_page_traversal_is_deterministic_across_map_insertion_order() {
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
        stitch_pages(&root, "root", &children, &["b.snxsch", "c.snxsch"])
    };
    assert_eq!(build(true), build(false));
}

// 9 ── What `ProjectRoot.name` is for, and why the app leaves it `None`.
//      A page seeded with a name qualifies its own sheet-scoped labels with
//      that chain, so an identically-named local net on two pages stays two
//      *distinguishably named* nets instead of two suffixed ones. Global and
//      Power labels are project-wide by definition and cross regardless — the
//      seed does not change what merges, only what the sheet-scoped result is
//      called. #430 ships `None` (a page is a peer of the root, not nested
//      under it); this pins the other setting so the field cannot rot into a
//      no-op unnoticed.
#[test]
fn name_seeded_page_qualifies_its_own_labels() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    root.labels.push(label("SDA", pt(0.0, 0.0), LabelType::Net));
    add_lib(&mut root, "R");
    place(&mut root, "R_ROOT", "R", pt(10.0, 0.0));

    let mut page = empty_sheet();
    page.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    page.labels.push(label("SDA", pt(0.0, 0.0), LabelType::Net));
    add_lib(&mut page, "R");
    place(&mut page, "R_PAGE", "R", pt(10.0, 0.0));

    let mut sheets: HashMap<SheetKey, SchematicSheet> = HashMap::new();
    sheets.insert(SheetKey::new("root"), root);
    sheets.insert(SheetKey::new("b.snxsch"), page);
    let resolved = HashMap::new();

    let roots = [
        ProjectRoot {
            key: SheetKey::new("root"),
            name: None,
        },
        ProjectRoot {
            key: SheetKey::new("b.snxsch"),
            name: Some("page2".to_string()),
        },
    ];
    let p = build_project_netlist(&ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    });

    assert_eq!(p.netlist.nets.len(), 2, "{:?}", names(&p.netlist));
    let ns = names(&p.netlist);
    assert!(
        ns.contains(&"SDA"),
        "the unseeded root keeps the bare name: {ns:?}"
    );
    assert!(
        ns.iter().any(|n| n.contains("page2")),
        "the seeded page's own net carries its chain: {ns:?}"
    );
    assert!(
        !p.issues
            .iter()
            .any(|i| matches!(i, StitchIssue::NameCollision { .. })),
        "distinct names, so no collision suffix was needed: {:?}",
        p.issues
    );
}

// 10 ── #466 × #430: a page listed twice contributes one occurrence, not two.
//      The caller is not supposed to do this, but a page list assembled from
//      two sources could; double-stitching would corrupt the netlist quietly
//      rather than fail loudly, so the visited-set skip covers it too.
#[test]
fn a_page_listed_twice_still_contributes_one_occurrence() {
    let root = empty_sheet();

    let mut b = empty_sheet();
    b.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    b.labels.push(label("VCC", pt(0.0, 0.0), LabelType::Global));
    add_lib(&mut b, "R");
    place(&mut b, "R_B", "R", pt(10.0, 0.0));

    let mut children = HashMap::new();
    children.insert("b.snxsch".to_string(), b);

    let once = stitch_pages(&root, "root", &children, &["b.snxsch"]);
    let twice = stitch_pages(&root, "root", &children, &["b.snxsch", "b.snxsch"]);
    assert_eq!(
        once, twice,
        "a repeated root is skipped, not stitched a second time"
    );
    assert!(
        !twice
            .issues
            .iter()
            .any(|i| matches!(i, StitchIssue::SharedReferenceAcrossInstances { .. })),
        "and it is not mistaken for a genuine second instance: {:?}",
        twice.issues
    );
}
