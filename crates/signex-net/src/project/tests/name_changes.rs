//! #2199 — the names an edit gives a project's nets, seen across sheets.

use super::*;

/// A label with its own uuid: nets are told apart by the elements that name
/// them, so two labels of one sheet must not share one.
fn label(text: &str, pos: Point, ty: LabelType) -> Label {
    Label {
        uuid: Uuid::now_v7(),
        ..super::label(text, pos, ty)
    }
}

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
) -> Vec<ProjectNetChange> {
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

/// The names of the nets the edit needs `allow_merge` for.
fn refused(
    before: &SchematicSheet,
    after: &SchematicSheet,
    children: &HashMap<String, SchematicSheet>,
) -> Vec<Vec<String>> {
    changes(before, after, children)
        .into_iter()
        .filter(|change| change.needs_allow_merge)
        .map(|change| change.names)
        .collect()
}

/// A parent wire from the sheet pin `SIG` at (0,0) to (5,0); the child names
/// that net `SIG` too (a hierarchical label: an alias) and carries a `VCC`
/// power port on the same wire (its own name).
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

/// A child with each of `names` as a hierarchical label on a wire of its own
/// at y = 20 * index.
fn child_of(names: &[&str]) -> SchematicSheet {
    let mut child = empty_sheet();
    for (i, name) in names.iter().enumerate() {
        let y = 20.0 * i as f64;
        child.wires.push(wire(pt(0.0, y), pt(10.0, y)));
        child
            .labels
            .push(label(name, pt(0.0, y), LabelType::Hierarchical));
    }
    child
}

/// A parent with the sheet `U` (file `c.sch`) whose entries `pins` are at
/// y = 20 * index, each on a stub wire ending at (5, y).
fn parent_with(pins: &[&str]) -> SchematicSheet {
    let mut root = empty_sheet();
    let entries = pins
        .iter()
        .enumerate()
        .map(|(i, name)| sheet_pin(name, pt(0.0, 20.0 * i as f64)))
        .collect();
    root.child_sheets.push(child_sheet("U", "c.sch", entries));
    for i in 0..pins.len() {
        let y = 20.0 * i as f64;
        root.wires.push(wire(pt(0.0, y), pt(5.0, y)));
    }
    root
}

/// `sheet` with a wire from (5, 0) down to (5, 20): the stubs of its first two
/// entries joined.
fn joined(sheet: &SchematicSheet) -> SchematicSheet {
    let mut after = sheet.clone();
    after.wires.push(wire(pt(5.0, 0.0), pt(5.0, 20.0)));
    after
}

fn net_label(sheet: &mut SchematicSheet, text: &str, at: (f64, f64)) {
    sheet
        .labels
        .push(label(text, pt(at.0, at.1), LabelType::Net));
}

#[test]
fn an_own_name_that_differs_from_the_nets_own_names_is_a_rename() {
    let (root, children) = project();
    // The net's own name is VCC (the child's port); SIG is its alias. A GND
    // label is another own name; so is a SIG label, which is no alias then.
    let got = refused(&root, &with_label(&root, "GND", (5.0, 0.0)), &children);
    assert_eq!(got, [["GND", "SIG", "VCC"]]);
    let got = refused(&root, &with_label(&root, "SIG", (5.0, 0.0)), &children);
    assert_eq!(got, [["SIG", "VCC"]]);
}

#[test]
fn the_nets_own_name_and_an_unrelated_net_are_no_change() {
    let (root, children) = project();
    for after in [
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
fn a_wire_that_joins_two_nets_with_different_own_names_is_a_rename() {
    let (mut root, children) = project();
    // A second named net on the parent, then a wire between the two.
    root.wires.push(wire(pt(0.0, 20.0), pt(5.0, 20.0)));
    root.labels
        .push(label("OTHER", pt(5.0, 20.0), LabelType::Global));
    let got = refused(&root, &joined(&root), &children);
    assert_eq!(got, [["OTHER", "SIG", "VCC"]]);
}

#[test]
fn two_nets_named_by_the_child_only_with_different_names_are_a_rename() {
    // The entries SDA and SCL, each net named only by the child's hierarchical
    // label (an alias): joined, they are two signals made one.
    let root = parent_with(&["SDA", "SCL"]);
    let children = HashMap::from([("c.sch".to_string(), child_of(&["SDA", "SCL"]))]);
    let got = changes(&root, &joined(&root), &children);
    assert_eq!(got.len(), 1);
    assert!(got[0].needs_allow_merge);
    assert_eq!(got[0].names, ["SCL", "SDA"]);
}

#[test]
fn nets_with_own_names_that_differ_are_a_rename_and_the_same_own_name_is_free() {
    let children = HashMap::from([("c.sch".to_string(), child_of(&["A", "B"]))]);
    for (second, rename) in [("NA", false), ("NB", true)] {
        let mut root = parent_with(&["A", "B"]);
        net_label(&mut root, "NA", (5.0, 0.0));
        net_label(&mut root, second, (5.0, 20.0));
        // With one label name on both stubs the two labels already join them
        // on the sheet, so give the second net its own wire.
        let after = if rename { joined(&root) } else { root.clone() };
        let got = refused(&root, &after, &children);
        assert_eq!(!got.is_empty(), rename, "{second}");
    }
}

#[test]
fn a_parents_name_wired_to_an_entry_whose_child_label_differs_is_an_alias() {
    // I2C_SDA on a parent net, wired to the entry SDA: the child's SDA is an
    // alias, so the join is free and the answer reports it.
    let mut root = parent_with(&["SDA", "UNUSED"]);
    root.child_sheets[0].pins.truncate(1);
    root.wires.truncate(1);
    root.wires.push(wire(pt(0.0, 20.0), pt(5.0, 20.0)));
    net_label(&mut root, "I2C_SDA", (5.0, 20.0));
    let children = HashMap::from([("c.sch".to_string(), child_of(&["SDA"]))]);
    let got = changes(&root, &joined(&root), &children);
    assert_eq!(got.len(), 1);
    assert!(!got[0].needs_allow_merge);
    assert_eq!(got[0].aliases, ["SDA"]);
    assert_eq!(got[0].names, ["I2C_SDA", "SDA"]);
    // The same name on both sides adds no alias.
    let mut same = root.clone();
    same.labels.pop();
    net_label(&mut same, "SDA", (5.0, 20.0));
    let mut after = same.clone();
    after.wires.push(wire(pt(5.0, 0.0), pt(5.0, 20.0)));
    let got = changes(&same, &after, &children);
    assert!(
        got.iter()
            .all(|c| !c.needs_allow_merge && c.aliases.is_empty())
    );
    // A local label on a net that only reaches the entry is no rename either.
    let mut bare = parent_with(&["SDA"]);
    let mut labelled = bare.clone();
    net_label(&mut labelled, "I2C_SDA", (5.0, 0.0));
    bare.wires.truncate(1);
    assert!(refused(&bare, &labelled, &children).is_empty());
}

#[test]
fn two_placements_of_one_child_join_freely_on_the_same_entry_and_not_on_different_ones() {
    let children = HashMap::from([("c.sch".to_string(), child_of(&["SIG", "SCL"]))]);
    for (second, rename) in [("SIG", false), ("SCL", true)] {
        let mut root = empty_sheet();
        for (name, entry, y) in [("U1", "SIG", 0.0), ("U2", second, 20.0)] {
            let pins = vec![sheet_pin(entry, pt(0.0, y))];
            let mut placed = child_sheet(name, "c.sch", pins);
            placed.uuid = Uuid::now_v7();
            root.child_sheets.push(placed);
            root.wires.push(wire(pt(0.0, y), pt(5.0, y)));
        }
        // The same entry of both placements is a shared bus: no rename.
        let got = refused(&root, &joined(&root), &children);
        assert_eq!(!got.is_empty(), rename, "{second}");
    }
}

#[test]
fn an_unnamed_net_may_gain_a_name_or_join_a_named_one() {
    let (mut root, children) = project();
    root.wires.push(wire(pt(0.0, 20.0), pt(5.0, 20.0)));
    let named = with_label(&root, "GND", (5.0, 20.0));
    assert!(refused(&root, &named, &children).is_empty());
    // The free wire joins the SIG net: the only named net it meets.
    let mut joined = root.clone();
    joined.wires.push(wire(pt(5.0, 0.0), pt(5.0, 20.0)));
    assert!(refused(&root, &joined, &children).is_empty());
}
