//! #1043 — a net with several names across sheets: the project netlist shows
//! the winner of the same rule as a single sheet, and either name still joins
//! sheets that use it. Rule: `docs/NET_NAMING.md`.

use super::*;

/// A one-wire sheet with a resistor `reference` and the given labels at its
/// two ends.
fn sheet_with(
    reference: &str,
    left: (&str, LabelType),
    right: Option<(&str, LabelType)>,
) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.labels.push(label(left.0, pt(0.0, 0.0), left.1));
    if let Some((text, ty)) = right {
        sheet.labels.push(label(text, pt(10.0, 0.0), ty));
    }
    add_lib(&mut sheet, "R");
    place(&mut sheet, reference, "R", pt(10.0, 0.0));
    sheet
}

#[test]
fn either_global_name_joins_a_child_and_the_smallest_text_wins() {
    // The root wire is named `SDA_B` and `SDA_A`; one child uses each name.
    let mut root = sheet_with(
        "R1",
        ("SDA_B", LabelType::Global),
        Some(("SDA_A", LabelType::Global)),
    );
    root.child_sheets
        .push(child_sheet("A", "a.sch", Vec::new()));
    root.child_sheets
        .push(child_sheet("B", "b.sch", Vec::new()));
    let mut map = HashMap::new();
    map.insert(
        "a.sch".to_string(),
        sheet_with("R2", ("SDA_A", LabelType::Global), None),
    );
    map.insert(
        "b.sch".to_string(),
        sheet_with("R3", ("SDA_B", LabelType::Global), None),
    );

    let p = stitch(&root, "root", &map);

    assert_eq!(names(&p.netlist), ["SDA_A"], "{:?}", p.netlist);
    assert_eq!(p.netlist.nets[0].terminals.len(), 3);
}

#[test]
fn power_name_wins_over_a_global_name_across_sheets() {
    let mut root = sheet_with(
        "R1",
        ("AAA", LabelType::Global),
        Some(("ZZZ", LabelType::Power)),
    );
    root.child_sheets
        .push(child_sheet("A", "a.sch", Vec::new()));
    let mut map = HashMap::new();
    map.insert(
        "a.sch".to_string(),
        sheet_with("R2", ("AAA", LabelType::Global), None),
    );

    let p = stitch(&root, "root", &map);

    assert_eq!(names(&p.netlist), ["ZZZ"], "{:?}", p.netlist);
    assert_eq!(p.netlist.nets[0].terminals.len(), 2, "alias AAA crossed");
}

/// Root wire at the origin carries R1; every listed child binds its `TX` pin to
/// it, and each child names its own wire `TX` with a hierarchical label.
fn sibling_tx(order: [&str; 2]) -> ProjectNetlist {
    let mut root = sheet_with("R1", ("UNUSED", LabelType::Net), None);
    root.labels.clear();
    let mut map = HashMap::new();
    for (i, n) in order.iter().enumerate() {
        let file = format!("{}.sch", n.to_lowercase());
        root.child_sheets
            .push(child_sheet(n, &file, vec![sheet_pin("TX", pt(0.0, 0.0))]));
        map.insert(
            file,
            sheet_with(
                &format!("R{}", i + 2),
                ("TX", LabelType::Hierarchical),
                None,
            ),
        );
    }
    stitch(&root, "root", &map)
}

#[test]
fn sibling_hierarchical_tie_picks_the_smallest_qualified_name_in_any_order() {
    for order in [["A", "B"], ["B", "A"]] {
        let p = sibling_tx(order);
        assert_eq!(names(&p.netlist), ["A/TX"], "{order:?}: {:?}", p.netlist);
        assert_eq!(p.netlist.nets[0].terminals.len(), 3);
    }
}

#[test]
fn nested_hierarchical_tie_picks_the_smallest_qualified_name() {
    // Root -> A -> B, `X` named hierarchically on both A and B: `A/B/X` sorts
    // before `A/X`.
    let mut root = sheet_with("R1", ("UNUSED", LabelType::Net), None);
    root.labels.clear();
    root.child_sheets.push(child_sheet(
        "A",
        "a.sch",
        vec![sheet_pin("X", pt(0.0, 0.0))],
    ));
    let mut a = sheet_with("R2", ("X", LabelType::Hierarchical), None);
    a.child_sheets.push(child_sheet(
        "B",
        "b.sch",
        vec![sheet_pin("X", pt(10.0, 0.0))],
    ));
    let mut map = HashMap::new();
    map.insert("a.sch".to_string(), a);
    map.insert(
        "b.sch".to_string(),
        sheet_with("R3", ("X", LabelType::Hierarchical), None),
    );

    let p = stitch(&root, "root", &map);

    assert_eq!(names(&p.netlist), ["A/B/X"], "{:?}", p.netlist);
}
