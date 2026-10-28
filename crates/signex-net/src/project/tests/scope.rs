//! #1074 — net scope is fixed, with no setting (ADR-0002 D8): a net label is
//! local to its sheet, a Global label (Port) is project-wide, a sheet entry
//! (Hierarchical label) joins only through the parent's matching sheet pin.
//! These pin behaviour the stitcher already has; they guard it against drift
//! and fail on trunk only if connectivity regresses.
//!
//! Already covered elsewhere: local labels do not cross
//! (`local_net_labels_do_not_cross_sheets`), a Global spans parent and child
//! (`global_label_spans_two_sheets`), pin-to-label binding and the unmatched
//! pin (`sheet_pin_binds_hierarchical_child_label`,
//! `unmatched_sheet_pin_stays_local`), and Power merging
//! (`power_symbol_and_power_label_merge_across_sheets`).

use std::collections::HashMap;

use signex_types::schematic::{LabelType, SchematicSheet};

use super::{
    add_lib, child_sheet, empty_sheet, label, names, place, pt, sheet_pin, stitch, stitch_pages,
    wire,
};

/// A sheet holding one wire from `(0, 0)` to `(10, 0)`, a label of `ty` named
/// `name` on its start, and a resistor `reference` on its end.
fn labelled_page(name: &str, ty: LabelType, reference: &str) -> SchematicSheet {
    let mut s = empty_sheet();
    s.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    s.labels.push(label(name, pt(0.0, 0.0), ty));
    add_lib(&mut s, "R");
    place(&mut s, reference, "R", pt(10.0, 0.0));
    s
}

// A Port is global by name: the root, a child page and a flat sibling page all
// land on one net.
#[test]
fn global_joins_child_and_flat_sibling_pages() {
    let mut root = labelled_page("VCC", LabelType::Global, "R_ROOT");
    root.child_sheets
        .push(child_sheet("kid", "kid.snxsch", Vec::new()));

    let mut children = HashMap::new();
    children.insert(
        "kid.snxsch".to_string(),
        labelled_page("VCC", LabelType::Global, "R_KID"),
    );
    children.insert(
        "sib.snxsch".to_string(),
        labelled_page("VCC", LabelType::Global, "R_SIB"),
    );

    let p = stitch_pages(&root, "root", &children, &["sib.snxsch"]);
    assert_eq!(p.netlist.nets.len(), 1, "{:?}", names(&p.netlist));
    assert_eq!(p.netlist.nets[0].terminals.len(), 3);
}

// A sheet entry on a page nobody references has no parent pin to join through,
// so two such pages keep their `X` apart.
#[test]
fn hierarchical_label_on_a_flat_page_without_a_parent_stays_local() {
    let root = labelled_page("X", LabelType::Hierarchical, "R_ROOT");
    let mut children = HashMap::new();
    children.insert(
        "sib.snxsch".to_string(),
        labelled_page("X", LabelType::Hierarchical, "R_SIB"),
    );

    let p = stitch_pages(&root, "root", &children, &["sib.snxsch"]);
    assert_eq!(p.netlist.nets.len(), 2, "{:?}", names(&p.netlist));
    for net in &p.netlist.nets {
        assert_eq!(net.terminals.len(), 1);
    }
}

// One child file placed twice: each placement's `X` joins only its own parent
// net, so the parent's two nets stay two nets and are not shorted through the
// shared file.
#[test]
fn two_placements_of_one_child_keep_their_own_nets() {
    let mut root = empty_sheet();
    add_lib(&mut root, "R");
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    place(&mut root, "R_A", "R", pt(10.0, 0.0));
    root.wires.push(wire(pt(0.0, 20.0), pt(10.0, 20.0)));
    place(&mut root, "R_B", "R", pt(10.0, 20.0));
    root.child_sheets.push(child_sheet(
        "A",
        "kid.snxsch",
        vec![sheet_pin("X", pt(0.0, 0.0))],
    ));
    root.child_sheets.push(child_sheet(
        "B",
        "kid.snxsch",
        vec![sheet_pin("X", pt(0.0, 20.0))],
    ));

    let mut children = HashMap::new();
    children.insert(
        "kid.snxsch".to_string(),
        labelled_page("X", LabelType::Hierarchical, "R1"),
    );

    let p = stitch(&root, "root", &children);
    assert_eq!(p.netlist.nets.len(), 2, "{:?}", names(&p.netlist));
    for net in &p.netlist.nets {
        let mut refs: Vec<&str> = net.terminals.iter().map(|t| t.reference.as_str()).collect();
        refs.sort_unstable();
        assert_eq!(refs.len(), 2, "{refs:?}");
        assert!(refs.contains(&"R1"), "{refs:?}");
        assert!(refs.contains(&"R_A") ^ refs.contains(&"R_B"), "{refs:?}");
    }
}
