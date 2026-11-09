//! #2199 — the names an edit gives a project's nets, seen across sheets.

use super::*;

/// The project of `root` (key `root`) and `children` (keyed by filename).
fn graph_of(
    root: &SchematicSheet,
    children: &HashMap<String, SchematicSheet>,
) -> (
    HashMap<SheetKey, SchematicSheet>,
    HashMap<SheetKey, HashMap<String, SheetKey>>,
) {
    let mut sheets: HashMap<SheetKey, SchematicSheet> = children
        .iter()
        .map(|(k, v)| (SheetKey::new(k.clone()), v.clone()))
        .collect();
    sheets.insert(SheetKey::new("root"), root.clone());
    let resolved = sheets
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
    (sheets, resolved)
}

fn changes(
    before: &SchematicSheet,
    after: &SchematicSheet,
    children: &HashMap<String, SchematicSheet>,
) -> Vec<Vec<String>> {
    let (bs, br) = graph_of(before, children);
    let (as_, ar) = graph_of(after, children);
    let roots = [ProjectRoot {
        key: SheetKey::new("root"),
    }];
    project_name_changes(
        &ProjectGraph {
            sheets: &bs,
            resolved: &br,
            roots: &roots,
        },
        &ProjectGraph {
            sheets: &as_,
            resolved: &ar,
            roots: &roots,
        },
    )
}

/// A parent wire from the sheet pin `SIG` at (0,0) to (5,0); the child names
/// that net `SIG` too and carries a `VCC` power port on the same wire.
fn project() -> (SchematicSheet, HashMap<String, SchematicSheet>) {
    let mut root = empty_sheet();
    root.child_sheets.push(child_sheet(
        "U",
        "c.sch",
        vec![sheet_pin("SIG", pt(0.0, 0.0))],
    ));
    root.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    let mut child = empty_sheet();
    child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    child
        .labels
        .push(label("SIG", pt(0.0, 0.0), LabelType::Hierarchical));
    add_lib(&mut child, "PWR");
    place_power(&mut child, "#PWR1", "PWR", "VCC", pt(10.0, 0.0));
    (root, HashMap::from([("c.sch".to_string(), child)]))
}

fn with_label(sheet: &SchematicSheet, text: &str, at: (f64, f64)) -> SchematicSheet {
    let mut after = sheet.clone();
    after
        .labels
        .push(label(text, pt(at.0, at.1), LabelType::Global));
    after
}

#[test]
fn a_name_that_reaches_a_net_from_another_sheet_is_seen() {
    let (root, children) = project();
    // The net carries SIG and VCC (from the child); a GND label is a third.
    let got = changes(&root, &with_label(&root, "GND", (5.0, 0.0)), &children);
    assert_eq!(got, [["GND", "SIG", "VCC"]]);
}

#[test]
fn an_alias_the_net_already_carries_and_an_unrelated_net_are_no_change() {
    let (root, children) = project();
    for after in [
        with_label(&root, "SIG", (5.0, 0.0)),
        with_label(&root, "VCC", (5.0, 0.0)),
        // A label on a wire of its own, with nothing from the child on it.
        {
            let mut sheet = root.clone();
            sheet.wires.push(wire(pt(0.0, 20.0), pt(5.0, 20.0)));
            with_label(&sheet, "X", (5.0, 20.0))
        },
    ] {
        assert!(changes(&root, &after, &children).is_empty());
    }
}

#[test]
fn a_wire_that_joins_two_named_nets_across_the_project_is_seen() {
    let (mut root, children) = project();
    // A second named net on the parent, then a wire between the two.
    root.wires.push(wire(pt(0.0, 20.0), pt(5.0, 20.0)));
    root.labels
        .push(label("OTHER", pt(5.0, 20.0), LabelType::Global));
    let mut after = root.clone();
    after.wires.push(wire(pt(5.0, 0.0), pt(5.0, 20.0)));
    let got = changes(&root, &after, &children);
    assert_eq!(got, [["OTHER", "SIG", "VCC"]]);
}
