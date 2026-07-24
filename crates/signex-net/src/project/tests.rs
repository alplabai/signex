//! Tests for project-level netlist stitching.
use super::*;
use crate::build_netlist;
use signex_types::schematic::{
    ChildSheet, FillType, HAlign, Junction, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle,
    Point, SheetPin, Symbol, VAlign, Wire,
};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::nil(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        symbols: Vec::new(),
        wires: Vec::new(),
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: HashMap::new(),
        lib_symbols: HashMap::new(),
    }
}

fn wire(a: Point, b: Point) -> Wire {
    Wire {
        uuid: Uuid::nil(),
        start: a,
        end: b,
        stroke_width: 0.0,
    }
}

fn junction(pos: Point) -> Junction {
    Junction {
        uuid: Uuid::nil(),
        position: pos,
        diameter: 0.0,
        minted: false,
    }
}

fn label(text: &str, pos: Point, ty: LabelType) -> Label {
    Label {
        uuid: Uuid::nil(),
        text: text.to_string(),
        position: pos,
        rotation: 0.0,
        label_type: ty,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
    }
}

fn lib_pin(number: &str, local: Point) -> LibPin {
    LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: local,
            rotation: 0.0,
            length: 0.0,
            name: String::new(),
            number: number.to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

/// A one-pin library symbol whose pin sits at local `(0, 0)`.
fn add_lib(sheet: &mut SchematicSheet, lib_id: &str) {
    sheet.lib_symbols.insert(
        lib_id.to_string(),
        LibSymbol {
            id: lib_id.to_string(),
            reference: String::new(),
            value: String::new(),
            footprint: String::new(),
            datasheet: String::new(),
            description: String::new(),
            keywords: String::new(),
            fp_filters: String::new(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: Vec::new(),
            pins: vec![lib_pin("1", pt(0.0, 0.0))],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        },
    );
}

fn place(sheet: &mut SchematicSheet, reference: &str, lib_id: &str, origin: Point) {
    sheet.symbols.push(Symbol {
        uuid: Uuid::nil(),
        lib_id: lib_id.to_string(),
        reference: reference.to_string(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        position: origin,
        rotation: 0.0,
        mirror_x: false,
        mirror_y: false,
        unit: 1,
        is_power: false,
        ref_text: None,
        val_text: None,
        fields_autoplaced: false,
        fields_user_placed: false,
        dnp: false,
        in_bom: true,
        on_board: true,
        exclude_from_sim: false,
        locked: false,
        fields: HashMap::new(),
        custom_properties: Vec::new(),
        pin_uuids: HashMap::new(),
        instances: Vec::new(),
        library_id: None,
        row_id: None,
        library_version: String::new(),
    });
}

fn place_power(sheet: &mut SchematicSheet, reference: &str, lib_id: &str, value: &str, at: Point) {
    place(sheet, reference, lib_id, at);
    let s = sheet.symbols.last_mut().unwrap();
    s.is_power = true;
    s.value = value.to_string();
}

fn place_xform(
    sheet: &mut SchematicSheet,
    reference: &str,
    lib_id: &str,
    origin: Point,
    rotation: f64,
    mirror_x: bool,
    mirror_y: bool,
) {
    place(sheet, reference, lib_id, origin);
    let s = sheet.symbols.last_mut().unwrap();
    s.rotation = rotation;
    s.mirror_x = mirror_x;
    s.mirror_y = mirror_y;
}

fn sheet_pin(name: &str, pos: Point) -> SheetPin {
    SheetPin {
        uuid: Uuid::nil(),
        name: name.to_string(),
        direction: String::new(),
        position: pos,
        rotation: 0.0,
        auto_generated: false,
        user_moved: false,
    }
}

fn child_sheet(name: &str, filename: &str, pins: Vec<SheetPin>) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::nil(),
        name: name.to_string(),
        filename: filename.to_string(),
        position: pt(0.0, 0.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins,
        instances: Vec::new(),
    }
}

fn names(nl: &Netlist) -> Vec<&str> {
    nl.nets.iter().map(|n| n.name.as_str()).collect()
}

// 1 ── Equivalence gate: build_project_netlist(root, &{}, None).netlist is
//      byte-for-byte build_netlist(root), incl. rotated/mirrored symbols.
fn assert_equiv(sheet: &SchematicSheet) {
    let p = build_project_netlist(sheet, &HashMap::new(), None);
    assert!(
        p.issues.is_empty(),
        "single sheet has no issues: {:?}",
        p.issues
    );
    assert_eq!(
        p.netlist,
        build_netlist(sheet),
        "project(root, &{{}}) must equal build_netlist(root)"
    );
}

#[test]
fn equivalence_gate_root_only() {
    // (a) two pins on one wire.
    let mut a = empty_sheet();
    a.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    add_lib(&mut a, "R");
    place(&mut a, "R1", "R", pt(0.0, 0.0));
    place(&mut a, "R2", "R", pt(10.0, 0.0));
    assert_equiv(&a);

    // (b) T-junction (issue #107) with a label.
    let mut b = empty_sheet();
    b.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    b.wires.push(wire(pt(5.0, 0.0), pt(5.0, 5.0)));
    b.junctions.push(junction(pt(5.0, 0.0)));
    b.labels.push(label("SDA", pt(0.0, 0.0), LabelType::Net));
    add_lib(&mut b, "R");
    place(&mut b, "R1", "R", pt(0.0, 0.0));
    place(&mut b, "R2", "R", pt(5.0, 5.0));
    assert_equiv(&b);

    // (c) rotated (90°) + mirrored symbol: its pin projects to (0, 2.54);
    //     a wire there gives it a terminal. Both paths use one transform.
    let mut c = empty_sheet();
    c.wires.push(wire(pt(0.0, 2.54), pt(10.0, 2.54)));
    sheet_add_pin_lib(&mut c, "U", pt(2.54, 0.0));
    place_xform(&mut c, "U1", "U", pt(0.0, 0.0), 90.0, true, false);
    place(&mut c, "R1", "R", pt(10.0, 2.54));
    add_lib(&mut c, "R");
    assert_equiv(&c);

    // (d) power-port naming.
    let mut d = empty_sheet();
    d.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    add_lib(&mut d, "R");
    add_lib(&mut d, "PWR");
    place(&mut d, "R1", "R", pt(10.0, 0.0));
    place_power(&mut d, "#PWR01", "PWR", "GND", pt(0.0, 0.0));
    assert_equiv(&d);
}

/// Library whose single pin sits at `local` (for the rotated-symbol case).
fn sheet_add_pin_lib(sheet: &mut SchematicSheet, lib_id: &str, local: Point) {
    sheet.lib_symbols.insert(
        lib_id.to_string(),
        LibSymbol {
            id: lib_id.to_string(),
            reference: String::new(),
            value: String::new(),
            footprint: String::new(),
            datasheet: String::new(),
            description: String::new(),
            keywords: String::new(),
            fp_filters: String::new(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: Vec::new(),
            pins: vec![lib_pin("1", local)],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        },
    );
}

// A parent with one child; the child has a labelled pin net. Returns
// (root, children map) for the binding tests.
fn parent_child(
    pin_name: &str,
    pin_pos: Point,
    child_label: &str,
    child_label_type: LabelType,
) -> (SchematicSheet, HashMap<String, SchematicSheet>) {
    let mut root = empty_sheet();
    // A resistor on the root net that the sheet pin sits on.
    root.wires
        .push(wire(pin_pos, pt(pin_pos.x + 5.0, pin_pos.y)));
    add_lib(&mut root, "R");
    place(&mut root, "RP", "R", pt(pin_pos.x + 5.0, pin_pos.y));
    root.child_sheets.push(child_sheet(
        "U1",
        "child.sch",
        vec![sheet_pin(pin_name, pin_pos)],
    ));

    let mut child = empty_sheet();
    child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    child
        .labels
        .push(label(child_label, pt(0.0, 0.0), child_label_type));
    add_lib(&mut child, "R");
    place(&mut child, "RC", "R", pt(10.0, 0.0));

    let mut map = HashMap::new();
    map.insert("child.sch".to_string(), child);
    (root, map)
}

// 2 ── SheetPin ↔ child label binding (Hierarchical and Global); an
//      unmatched pin leaves the parent net local.
#[test]
fn sheet_pin_binds_hierarchical_child_label() {
    let (root, map) = parent_child("BUS", pt(0.0, 0.0), "BUS", LabelType::Hierarchical);
    let p = build_project_netlist(&root, &map, None);
    assert!(p.issues.is_empty(), "{:?}", p.issues);
    // RP (root) and RC (child) are one net through the pin↔label binding.
    assert_eq!(p.netlist.nets.len(), 1);
    assert_eq!(p.netlist.nets[0].terminals.len(), 2);
}

#[test]
fn sheet_pin_binds_global_child_label() {
    let (root, map) = parent_child("VCC", pt(0.0, 0.0), "VCC", LabelType::Global);
    let p = build_project_netlist(&root, &map, None);
    assert_eq!(p.netlist.nets.len(), 1);
    assert_eq!(p.netlist.nets[0].terminals.len(), 2);
}

#[test]
fn unmatched_sheet_pin_stays_local() {
    // Pin "BUS" but the child's label is "OTHER" → no binding: two nets.
    let (root, map) = parent_child("BUS", pt(0.0, 0.0), "OTHER", LabelType::Hierarchical);
    let p = build_project_netlist(&root, &map, None);
    assert_eq!(p.netlist.nets.len(), 2, "unbound pin keeps nets separate");
}

// 3 ── Global name spans sheets; power-port symbol + Power label merge.
#[test]
fn global_label_spans_two_sheets() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    root.labels
        .push(label("NET5V", pt(0.0, 0.0), LabelType::Global));
    add_lib(&mut root, "R");
    place(&mut root, "R1", "R", pt(10.0, 0.0));
    root.child_sheets
        .push(child_sheet("A", "a.sch", Vec::new()));

    let mut child = empty_sheet();
    child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    child
        .labels
        .push(label("NET5V", pt(0.0, 0.0), LabelType::Global));
    add_lib(&mut child, "R");
    place(&mut child, "R2", "R", pt(10.0, 0.0));

    let mut map = HashMap::new();
    map.insert("a.sch".to_string(), child);
    let p = build_project_netlist(&root, &map, None);
    assert_eq!(p.netlist.nets.len(), 1, "same-name Global spans sheets");
    assert_eq!(p.netlist.nets[0].name, "NET5V");
    assert_eq!(p.netlist.nets[0].terminals.len(), 2);
}

#[test]
fn membership_aggregates_across_sheet_occurrences() {
    // D3.1 cross-sheet: a net stitched from a Global label on two sheets
    // carries the wire uuids from *both* occurrences, so the net-flood
    // highlights the whole net project-wide — not just the clicked sheet.
    let rw = Uuid::from_u128(0x11);
    let cw = Uuid::from_u128(0x22);

    let mut root = empty_sheet();
    root.wires.push(Wire {
        uuid: rw,
        start: pt(0.0, 0.0),
        end: pt(10.0, 0.0),
        stroke_width: 0.0,
    });
    root.labels
        .push(label("NET5V", pt(0.0, 0.0), LabelType::Global));
    add_lib(&mut root, "R");
    place(&mut root, "R1", "R", pt(10.0, 0.0));
    root.child_sheets
        .push(child_sheet("A", "a.sch", Vec::new()));

    let mut child = empty_sheet();
    child.wires.push(Wire {
        uuid: cw,
        start: pt(0.0, 0.0),
        end: pt(10.0, 0.0),
        stroke_width: 0.0,
    });
    child
        .labels
        .push(label("NET5V", pt(0.0, 0.0), LabelType::Global));
    add_lib(&mut child, "R");
    place(&mut child, "R2", "R", pt(10.0, 0.0));

    let mut map = HashMap::new();
    map.insert("a.sch".to_string(), child);
    let p = build_project_netlist(&root, &map, None);
    assert_eq!(p.netlist.nets.len(), 1, "same-name Global spans sheets");
    let net = &p.netlist.nets[0];
    assert!(
        net.wires.contains(&rw) && net.wires.contains(&cw),
        "both occurrences' wires belong to the net: {:?}",
        net.wires
    );
}

#[test]
fn power_symbol_and_power_label_merge_across_sheets() {
    // Root: GND power-port symbol. Child: GND Power label. One net.
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    add_lib(&mut root, "R");
    add_lib(&mut root, "PWR");
    place(&mut root, "R1", "R", pt(10.0, 0.0));
    place_power(&mut root, "#PWR01", "PWR", "GND", pt(0.0, 0.0));
    root.child_sheets
        .push(child_sheet("A", "a.sch", Vec::new()));

    let mut child = empty_sheet();
    child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    child
        .labels
        .push(label("GND", pt(0.0, 0.0), LabelType::Power));
    add_lib(&mut child, "R");
    place(&mut child, "R2", "R", pt(10.0, 0.0));

    let mut map = HashMap::new();
    map.insert("a.sch".to_string(), child);
    let p = build_project_netlist(&root, &map, None);
    assert_eq!(p.netlist.nets.len(), 1, "GND port + GND label are one net");
    assert_eq!(p.netlist.nets[0].name, "GND");
    // R1 + the #PWR01 power-port pin (root) + R2 (child).
    assert_eq!(p.netlist.nets[0].terminals.len(), 3);
}

// 4 ── Local Net labels never cross; the two are distinct, qualified nets.
#[test]
fn local_net_labels_do_not_cross_sheets() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    root.labels.push(label("SDA", pt(0.0, 0.0), LabelType::Net));
    add_lib(&mut root, "R");
    place(&mut root, "R1", "R", pt(10.0, 0.0));
    root.child_sheets
        .push(child_sheet("charger", "a.sch", Vec::new()));

    let mut child = empty_sheet();
    child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    child
        .labels
        .push(label("SDA", pt(0.0, 0.0), LabelType::Net));
    add_lib(&mut child, "R");
    place(&mut child, "R2", "R", pt(10.0, 0.0));

    let mut map = HashMap::new();
    map.insert("a.sch".to_string(), child);
    let p = build_project_netlist(&root, &map, None);
    assert_eq!(p.netlist.nets.len(), 2, "local Net labels stay per-sheet");
    // Root SDA stays bare; the child SDA is qualified by its sheet name.
    let ns = names(&p.netlist);
    assert!(ns.contains(&"SDA"), "root net bare: {ns:?}");
    assert!(ns.contains(&"charger/SDA"), "child net qualified: {ns:?}");
}

// 5 ── One child file instantiated twice: instances not shorted; refdes
//      collision reported; terminals per occurrence.
#[test]
fn same_child_instantiated_twice_is_not_shorted() {
    let mut root = empty_sheet();
    root.child_sheets.push(child_sheet(
        "A",
        "child.sch",
        vec![sheet_pin("P", pt(0.0, 0.0))],
    ));
    root.child_sheets.push(child_sheet(
        "B",
        "child.sch",
        vec![sheet_pin("P", pt(20.0, 0.0))],
    ));

    let mut child = empty_sheet();
    child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    child
        .labels
        .push(label("P", pt(0.0, 0.0), LabelType::Hierarchical));
    add_lib(&mut child, "R");
    place(&mut child, "R1", "R", pt(10.0, 0.0));

    let mut map = HashMap::new();
    map.insert("child.sch".to_string(), child);
    let p = build_project_netlist(&root, &map, None);
    assert_eq!(
        p.netlist.nets.len(),
        2,
        "instances are not shorted together"
    );
    for net in &p.netlist.nets {
        assert_eq!(net.terminals.len(), 1, "one R1 per instance");
        assert_eq!(net.terminals[0].reference, "R1");
    }
    assert!(
        p.issues
            .contains(&StitchIssue::SharedReferenceAcrossInstances {
                filename: "child.sch".to_string(),
                reference: "R1".to_string(),
            }),
        "refdes collision reported: {:?}",
        p.issues
    );
}

// 6 ── Missing child file → issue, parent net stays local, deterministic.
#[test]
fn missing_child_reported_and_local() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    add_lib(&mut root, "R");
    place(&mut root, "R1", "R", pt(10.0, 0.0));
    root.child_sheets.push(child_sheet(
        "A",
        "gone.sch",
        vec![sheet_pin("P", pt(0.0, 0.0))],
    ));

    let p = build_project_netlist(&root, &HashMap::new(), None);
    assert_eq!(p.netlist.nets.len(), 1, "root net survives");
    assert!(
        p.issues.iter().any(|i| matches!(
            i,
            StitchIssue::MissingChild { filename, .. } if filename == "gone.sch"
        )),
        "missing child reported: {:?}",
        p.issues
    );
    // Deterministic.
    assert_eq!(p, build_project_netlist(&root, &HashMap::new(), None));
}

// 7 ── Cycles: A→B→A and child-instantiates-root → SheetCycle, no hang.
#[test]
fn cycles_are_reported_without_hanging() {
    let mut a = empty_sheet();
    a.child_sheets.push(child_sheet("toB", "b.sch", Vec::new()));
    let mut b = empty_sheet();
    b.child_sheets.push(child_sheet("toA", "a.sch", Vec::new()));

    let mut map = HashMap::new();
    map.insert("a.sch".to_string(), a.clone());
    map.insert("b.sch".to_string(), b);
    // Root is a.sch; root_filename lets the B→A edge be seen as a cycle.
    let p = build_project_netlist(&a, &map, Some("a.sch"));
    assert!(
        p.issues
            .iter()
            .any(|i| matches!(i, StitchIssue::SheetCycle { .. })),
        "cycle reported: {:?}",
        p.issues
    );
}

// 8 ── Sheet pin anchored on a wire *interior* on the parent.
#[test]
fn sheet_pin_anchors_to_wire_interior() {
    let mut root = empty_sheet();
    // Wire 0..10; the sheet pin sits mid-span at (5,0), and RP at (10,0).
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    add_lib(&mut root, "R");
    place(&mut root, "RP", "R", pt(0.0, 0.0));
    root.child_sheets.push(child_sheet(
        "A",
        "a.sch",
        vec![sheet_pin("BUS", pt(5.0, 0.0))],
    ));

    let mut child = empty_sheet();
    child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    child
        .labels
        .push(label("BUS", pt(0.0, 0.0), LabelType::Hierarchical));
    add_lib(&mut child, "R");
    place(&mut child, "RC", "R", pt(10.0, 0.0));

    let mut map = HashMap::new();
    map.insert("a.sch".to_string(), child);
    let p = build_project_netlist(&root, &map, None);
    assert_eq!(
        p.netlist.nets.len(),
        1,
        "interior-anchored pin binds the net"
    );
    assert_eq!(p.netlist.nets[0].terminals.len(), 2);
}

// 9 ── Two same-name sheet pins on one ChildSheet merge through the child.
#[test]
fn two_same_name_sheet_pins_merge_through_child() {
    let mut root = empty_sheet();
    // Two separate root nets, each with a "BUS" sheet pin to the same child.
    root.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    root.wires.push(wire(pt(20.0, 0.0), pt(25.0, 0.0)));
    add_lib(&mut root, "R");
    place(&mut root, "RA", "R", pt(5.0, 0.0));
    place(&mut root, "RB", "R", pt(25.0, 0.0));
    root.child_sheets.push(child_sheet(
        "A",
        "a.sch",
        vec![
            sheet_pin("BUS", pt(0.0, 0.0)),
            sheet_pin("BUS", pt(20.0, 0.0)),
        ],
    ));

    let mut child = empty_sheet();
    child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    child
        .labels
        .push(label("BUS", pt(0.0, 0.0), LabelType::Hierarchical));
    add_lib(&mut child, "R");
    place(&mut child, "RC", "R", pt(10.0, 0.0));

    let mut map = HashMap::new();
    map.insert("a.sch".to_string(), child);
    let p = build_project_netlist(&root, &map, None);
    // Both root nets merge through the shared child BUS label → one net.
    assert_eq!(
        p.netlist.nets.len(),
        1,
        "same-name pins merge through child"
    );
    assert_eq!(p.netlist.nets[0].terminals.len(), 3);
}

// 10 ── Name collision via duplicate sibling ChildSheet.name → suffix + issue.
#[test]
fn duplicate_sibling_name_collision_is_suffixed() {
    // Two sibling children with the SAME name "charger", each a distinct
    // file, each with a local Net "SDA" → both qualify to "charger/SDA".
    let mut root = empty_sheet();
    root.child_sheets
        .push(child_sheet("charger", "a.sch", Vec::new()));
    root.child_sheets
        .push(child_sheet("charger", "b.sch", Vec::new()));

    let mut a = empty_sheet();
    a.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    a.labels.push(label("SDA", pt(0.0, 0.0), LabelType::Net));
    add_lib(&mut a, "R");
    place(&mut a, "RA", "R", pt(10.0, 0.0));
    let mut b = empty_sheet();
    b.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    b.labels.push(label("SDA", pt(0.0, 0.0), LabelType::Net));
    add_lib(&mut b, "R");
    place(&mut b, "RB", "R", pt(10.0, 0.0));

    let mut map = HashMap::new();
    map.insert("a.sch".to_string(), a);
    map.insert("b.sch".to_string(), b);
    let p = build_project_netlist(&root, &map, None);
    assert_eq!(p.netlist.nets.len(), 2, "two distinct qualified nets");
    let ns = names(&p.netlist);
    assert!(ns.contains(&"charger/SDA"), "first keeps the name: {ns:?}");
    assert!(
        ns.iter().any(|n| n.starts_with("charger/SDA_")),
        "second is suffixed: {ns:?}"
    );
    assert!(
        p.issues.contains(&StitchIssue::NameCollision {
            name: "charger/SDA".to_string()
        }),
        "collision reported: {:?}",
        p.issues
    );
}

// 10b ── A bare single-sheet collision dedups through the same shared pass,
// so the root netlist stays byte-identical to build_netlist while the
// stitcher still surfaces the clash.
#[test]
fn single_sheet_name_collision_matches_build_netlist_and_is_reported() {
    let mut root = empty_sheet();
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    root.wires.push(wire(pt(50.0, 0.0), pt(60.0, 0.0)));
    root.labels
        .push(label("BUS", pt(0.0, 0.0), LabelType::Hierarchical));
    root.labels
        .push(label("BUS", pt(50.0, 0.0), LabelType::Hierarchical));
    add_lib(&mut root, "R");
    place(&mut root, "R1", "R", pt(0.0, 0.0));
    place(&mut root, "R2", "R", pt(50.0, 0.0));

    let p = build_project_netlist(&root, &HashMap::new(), None);
    assert_eq!(
        p.netlist,
        build_netlist(&root),
        "dedup keeps both paths byte-identical"
    );
    let ns = names(&p.netlist);
    assert!(ns.contains(&"BUS"), "one keeps the bare name: {ns:?}");
    assert!(
        ns.iter().any(|n| n.starts_with("BUS_")),
        "the other is suffixed: {ns:?}"
    );
    assert!(
        p.issues.contains(&StitchIssue::NameCollision {
            name: "BUS".to_string()
        }),
        "collision reported: {:?}",
        p.issues
    );
}

// 11 ── Determinism: children-map insertion order never changes the output.
#[test]
fn output_is_deterministic_across_map_order() {
    let build = |forward: bool| {
        let mut root = empty_sheet();
        root.labels
            .push(label("NET5V", pt(0.0, 0.0), LabelType::Global));
        root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        add_lib(&mut root, "R");
        place(&mut root, "R1", "R", pt(10.0, 0.0));
        root.child_sheets
            .push(child_sheet("A", "a.sch", Vec::new()));
        root.child_sheets
            .push(child_sheet("B", "b.sch", Vec::new()));

        let mk = |name: &str| {
            let mut c = empty_sheet();
            c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
            c.labels.push(label(name, pt(0.0, 0.0), LabelType::Global));
            add_lib(&mut c, "R");
            place(&mut c, "R2", "R", pt(10.0, 0.0));
            c
        };
        let mut map = HashMap::new();
        if forward {
            map.insert("a.sch".to_string(), mk("NET5V"));
            map.insert("b.sch".to_string(), mk("OTHER"));
        } else {
            map.insert("b.sch".to_string(), mk("OTHER"));
            map.insert("a.sch".to_string(), mk("NET5V"));
        }
        build_project_netlist(&root, &map, None)
    };
    assert_eq!(build(true), build(false));
}

#[test]
fn project_terminals_order_designators_naturally() {
    // Same rule as `build_netlist` — the project-level stitcher re-sorts
    // its own terminal vec, so it needs the natural comparator too or a
    // multi-sheet export reads R1, R10, R2 while a single-sheet one
    // reads R1, R2, R10.
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(100.0, 0.0)));
    add_lib(&mut sheet, "R");
    for n in 1..=10 {
        // Mid-span pins need a junction to count as terminals.
        sheet.junctions.push(junction(pt(n as f64, 0.0)));
        place(&mut sheet, &format!("R{n}"), "R", pt(n as f64, 0.0));
    }

    let project = build_project_netlist(&sheet, &HashMap::new(), None);
    assert_eq!(project.netlist.nets.len(), 1);
    let order: Vec<&str> = project.netlist.nets[0]
        .terminals
        .iter()
        .map(|t| t.reference.as_str())
        .collect();
    assert_eq!(
        order,
        vec!["R1", "R2", "R3", "R4", "R5", "R6", "R7", "R8", "R9", "R10"]
    );
}
