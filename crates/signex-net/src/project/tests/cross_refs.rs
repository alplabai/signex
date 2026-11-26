//! Off-sheet cross references (#1273): the pages a Global label's net
//! continues on.
use super::*;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn key(name: &str) -> SheetKey {
    SheetKey::new(name)
}

fn global(text: &str, at: Point, uuid: u128) -> Label {
    Label {
        uuid: id(uuid),
        ..label(text, at, LabelType::Global)
    }
}

/// A sheet with a wire from `at` and a Global label `text` on its end.
fn sheet_with_global(text: &str, at: Point, uuid: u128) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(at, pt(at.x + 5.0, at.y)));
    sheet.labels.push(global(text, at, uuid));
    sheet
}

/// The cross references of `root` (under `root.sch`) and `children`.
fn cross_refs(root: &SchematicSheet, children: &[(&str, SchematicSheet)]) -> Vec<PortCrossRef> {
    let mut sheets: HashMap<SheetKey, SchematicSheet> =
        children.iter().map(|(k, v)| (key(k), v.clone())).collect();
    sheets.insert(key("root.sch"), root.clone());
    let resolved = sheets
        .iter()
        .map(|(k, s)| {
            let sub = s
                .child_sheets
                .iter()
                .map(|cs| (cs.filename.clone(), key(&cs.filename)))
                .collect();
            (k.clone(), sub)
        })
        .collect();
    let roots = [ProjectRoot {
        key: key("root.sch"),
    }];
    let graph = ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    };
    let (_, index) = build_project_netlist_indexed(&graph);
    let (numbering, _) = sheet_numbering(&graph, "");
    index.port_cross_refs(&numbering)
}

fn place(root: &mut SchematicSheet, file: &str, uuid: u128) {
    let name = format!("{file}-{uuid}");
    let mut cs = child_sheet(&name, file, vec![]);
    cs.uuid = id(uuid);
    root.child_sheets.push(cs);
}

fn of(refs: &[PortCrossRef], label: u128) -> Vec<&PortCrossRef> {
    refs.iter().filter(|r| r.label == id(label)).collect()
}

fn pages(r: &PortCrossRef) -> Vec<usize> {
    r.partners.iter().map(|p| p.page).collect()
}

#[test]
fn a_global_label_lists_the_page_its_net_continues_on() {
    let mut root = sheet_with_global("VCC", pt(10.0, 20.0), 1);
    place(&mut root, "a.sch", 500);
    let a = sheet_with_global("VCC", pt(30.0, 40.0), 2);
    let refs = cross_refs(&root, &[("a.sch", a)]);

    let from_root = of(&refs, 1);
    assert_eq!(from_root.len(), 1, "{refs:?}");
    assert_eq!(from_root[0].page, Some(1));
    assert_eq!(from_root[0].text, "VCC");
    assert_eq!(pages(from_root[0]), [2]);
    let partner = &from_root[0].partners[0];
    assert_eq!(partner.sheet, key("a.sch"));
    assert_eq!(partner.label, id(2));
    assert_eq!((partner.at.x, partner.at.y), (30.0, 40.0));

    let from_child = of(&refs, 2);
    assert_eq!(from_child[0].page, Some(2));
    assert_eq!(pages(from_child[0]), [1]);
}

/// A sheet placed twice is two pages: each copy points at the other copy and
/// at the root, and the root points at both.
#[test]
fn each_placement_of_a_sheet_is_its_own_page() {
    let mut root = sheet_with_global("SDA", pt(0.0, 0.0), 1);
    place(&mut root, "b.sch", 500);
    place(&mut root, "b.sch", 501);
    let b = sheet_with_global("SDA", pt(0.0, 0.0), 2);
    let refs = cross_refs(&root, &[("b.sch", b)]);

    assert_eq!(pages(of(&refs, 1)[0]), [2, 3]);
    let copies = of(&refs, 2);
    assert_eq!(copies.len(), 2, "one entry per placement: {refs:?}");
    let mut seen: Vec<(Option<usize>, Vec<usize>)> =
        copies.iter().map(|r| (r.page, pages(r))).collect();
    seen.sort();
    assert_eq!(seen, [(Some(2), vec![1, 3]), (Some(3), vec![1, 2])]);
}

/// Labels on the same page are not cross references, and a net that stays
/// on its page lists none.
#[test]
fn a_net_that_stays_on_its_page_lists_no_partner() {
    let mut root = sheet_with_global("EN", pt(0.0, 0.0), 1);
    root.wires.push(wire(pt(0.0, 10.0), pt(5.0, 10.0)));
    root.labels.push(global("EN", pt(0.0, 10.0), 3));
    place(&mut root, "a.sch", 500);
    let a = sheet_with_global("OTHER", pt(0.0, 0.0), 2);
    let refs = cross_refs(&root, &[("a.sch", a)]);

    assert!(of(&refs, 1)[0].partners.is_empty(), "{refs:?}");
    assert!(of(&refs, 3)[0].partners.is_empty(), "{refs:?}");
    assert!(of(&refs, 2)[0].partners.is_empty(), "{refs:?}");
}

/// Partners are listed by page, then left to right.
#[test]
fn partners_are_sorted_by_page_then_position() {
    let mut root = sheet_with_global("CLK", pt(0.0, 0.0), 1);
    place(&mut root, "a.sch", 500);
    let mut a = sheet_with_global("CLK", pt(50.0, 0.0), 2);
    a.wires.push(wire(pt(10.0, 30.0), pt(15.0, 30.0)));
    a.labels.push(global("CLK", pt(10.0, 30.0), 3));
    let refs = cross_refs(&root, &[("a.sch", a)]);

    let partners: Vec<Uuid> = of(&refs, 1)[0].partners.iter().map(|p| p.label).collect();
    assert_eq!(partners, [id(3), id(2)]);
}
