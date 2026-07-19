//! Tests for single-sheet netlist building.
use super::*;
use signex_types::schematic::{
    Junction, Label, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Symbol, Wire,
};
use std::collections::HashMap;
use uuid::Uuid;

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
        justify: signex_types::schematic::HAlign::Left,
        justify_v: signex_types::schematic::VAlign::Bottom,
    }
}

fn lib_pin(number: &str, local: Point, dir: PinDirection) -> LibPin {
    LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: dir,
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

/// Register a library symbol keyed by `lib_id`, whose pins are given in
/// **local** coordinates.
fn add_lib(sheet: &mut SchematicSheet, lib_id: &str, pins: Vec<LibPin>) {
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
            pins,
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        },
    );
}

/// Place a symbol instance of `lib_id` at world `origin` (no rotation /
/// mirror). With that transform a local pin `(lx, ly)` lands at world
/// `(origin.x + lx, origin.y - ly)`.
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

fn place_power(
    sheet: &mut SchematicSheet,
    reference: &str,
    lib_id: &str,
    value: &str,
    origin: Point,
) {
    place(sheet, reference, lib_id, origin);
    let sym = sheet.symbols.last_mut().unwrap();
    sym.is_power = true;
    sym.value = value.to_string();
}

#[test]
fn pin_on_a_junction_mid_wire_is_a_terminal() {
    // A pin tapping a wire mid-span where a junction sits is a terminal
    // (D5.3). The gate used to check only wire endpoints, so this pin was
    // silently dropped (and ERC flagged it unconnected).
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.junctions.push(junction(pt(5.0, 0.0)));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(0.0, 0.0));
    place(&mut sheet, "R2", "R", pt(5.0, 0.0));

    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets.len(), 1);
    assert_eq!(
        netlist.nets[0].terminals.len(),
        2,
        "the mid-wire junction pin is a terminal"
    );
}

#[test]
fn pin_on_a_bare_bus_endpoint_forms_no_phantom_net() {
    // A bus is a bundle, never unioned; gating on a bus endpoint used to
    // mint a one-terminal phantom net (D5.4). Now it does not connect.
    let mut sheet = empty_sheet();
    sheet.buses.push(signex_types::schematic::Bus {
        uuid: Uuid::nil(),
        start: pt(0.0, 0.0),
        end: pt(10.0, 0.0),
    });
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(0.0, 0.0));

    let netlist = build_netlist(&sheet);
    assert!(
        netlist.nets.is_empty(),
        "a bus-only pin forms no phantom net"
    );
}

#[test]
fn power_port_symbol_names_its_net() {
    // A power-port symbol (is_power, value "GND") names its net GND with no
    // GND label — new in #156; build_netlist used to ignore power ports for
    // naming.
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    add_lib(
        &mut sheet,
        "PWR",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(10.0, 0.0));
    place_power(&mut sheet, "#PWR01", "PWR", "GND", pt(0.0, 0.0));

    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets.len(), 1);
    assert_eq!(netlist.nets[0].name, "GND");
    assert_eq!(netlist.nets[0].terminals.len(), 2);
}

#[test]
fn point_on_segment_detects_interior_and_rejects_off_segment() {
    let a = (0, 0);
    let b = (10_000, 0);
    assert!(point_on_segment((5_000, 0), a, b), "interior point");
    assert!(point_on_segment((0, 0), a, b), "endpoint");
    assert!(!point_on_segment((5_000, 1_000), a, b), "off the line");
    assert!(
        !point_on_segment((11_000, 0), a, b),
        "collinear but past the end"
    );
}

#[test]
fn point_on_segment_is_exact_on_a_diagonal_wire() {
    // D5.5 decision: collinearity is exact in the bucket space, diagonals
    // included. A point exactly on a 45° wire is detected; one a single
    // bucket off the integer line is not (no ±1-bucket tolerance — that is
    // deferred to the integer-nm coordinate migration).
    let a = (0, 0);
    let b = (10_000, 10_000);
    assert!(
        point_on_segment((5_000, 5_000), a, b),
        "exactly on diagonal"
    );
    assert!(
        !point_on_segment((5_000, 5_001), a, b),
        "one bucket off the line is rejected"
    );
}

#[test]
fn two_pins_on_one_wire_share_a_net_with_both_terminals() {
    // A single wire from (0,0) to (10,0). R1 pin 1 sits at (0,0),
    // R2 pin 1 at (10,0). Both land on the wire's endpoints, so they
    // belong to one net carrying two terminals.
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(0.0, 0.0));
    place(&mut sheet, "R2", "R", pt(10.0, 0.0));

    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets.len(), 1);
    let net = &netlist.nets[0];
    assert_eq!(net.terminals.len(), 2);
    assert_eq!(net.terminals[0].reference, "R1");
    assert_eq!(net.terminals[1].reference, "R2");
    assert_eq!(net.id, NetId(1));
    assert_eq!(net.name, "N$1", "unlabelled net gets an auto name");
}

#[test]
fn t_junction_merges_wire_ending_on_another_wires_interior() {
    // Horizontal wire (0,0)-(10,0); a vertical wire (5,0)-(5,5) ends on
    // its interior; a junction dot at (5,0). A pin at each far end must
    // land on ONE net once the junction connects the two wires.
    // Regression for issue #107.
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire(pt(5.0, 0.0), pt(5.0, 5.0)));
    sheet.junctions.push(junction(pt(5.0, 0.0)));
    add_lib(
        &mut sheet,
        "U",
        vec![lib_pin("A", pt(0.0, 0.0), PinDirection::Output)],
    );
    // Local pin (0,0) at origin (0,0) → world (0,0).
    place(&mut sheet, "U1", "U", pt(0.0, 0.0));
    // Local pin (0,0) at origin (5,5) → world (5,5).
    place(&mut sheet, "U2", "U", pt(5.0, 5.0));

    let netlist = build_netlist(&sheet);
    assert_eq!(
        netlist.nets.len(),
        1,
        "a T-junction must merge both wires into one net"
    );
    assert_eq!(
        netlist.nets[0].terminals.len(),
        2,
        "both pins belong to the merged net"
    );
}

#[test]
fn t_intersection_without_junction_stays_two_nets() {
    // Same geometry, no junction dot: the connection is not asserted, so
    // the wires remain two separate nets.
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire(pt(5.0, 0.0), pt(5.0, 5.0)));
    add_lib(
        &mut sheet,
        "U",
        vec![lib_pin("A", pt(0.0, 0.0), PinDirection::Output)],
    );
    place(&mut sheet, "U1", "U", pt(0.0, 0.0));
    place(&mut sheet, "U2", "U", pt(5.0, 5.0));

    let netlist = build_netlist(&sheet);
    assert_eq!(
        netlist.nets.len(),
        2,
        "without a junction the T is two separate nets"
    );
}

#[test]
fn label_names_the_net_by_priority() {
    // Two labels on the same wire: a plain Net label and a Global label.
    // The Global one wins.
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet
        .labels
        .push(label("local", pt(0.0, 0.0), LabelType::Net));
    sheet
        .labels
        .push(label("VBUS", pt(10.0, 0.0), LabelType::Global));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(0.0, 0.0));

    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets.len(), 1);
    assert_eq!(netlist.nets[0].name, "VBUS");
}

#[test]
fn class_is_left_to_project_rules() {
    // The builder no longer derives a class from the name prefix (D3.2):
    // class resolution is a project-rules concern layered on connectivity.
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet
        .labels
        .push(label("I2C_SDA", pt(0.0, 0.0), LabelType::Net));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(0.0, 0.0));

    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets[0].name, "I2C_SDA");
    assert!(netlist.nets[0].class.is_none());
}

#[test]
fn net_records_its_wire_and_junction_membership() {
    // D3.1: a net carries the wire + junction uuids it occupies — the
    // membership the net-flood highlights and the ratsnest reads.
    let mut sheet = empty_sheet();
    let w1 = Uuid::from_u128(1);
    let w2 = Uuid::from_u128(2);
    let jn = Uuid::from_u128(3);
    sheet.wires.push(wire_id(pt(0.0, 0.0), pt(10.0, 0.0), w1));
    sheet.wires.push(wire_id(pt(5.0, 0.0), pt(5.0, 5.0), w2));
    sheet.junctions.push(Junction {
        uuid: jn,
        position: pt(5.0, 0.0),
        diameter: 0.0,
    });
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(0.0, 0.0));
    place(&mut sheet, "R2", "R", pt(5.0, 5.0));

    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets.len(), 1);
    let net = &netlist.nets[0];
    assert_eq!(net.wires.len(), 2, "both wires belong to the net");
    assert!(net.wires.contains(&w1) && net.wires.contains(&w2));
    assert_eq!(net.junctions, vec![jn]);
}

#[test]
fn unconnected_pins_form_no_net() {
    // A symbol with a pin, but no wire/label anywhere near it: nothing
    // lands on the tip, so it contributes no terminal and no net.
    let mut sheet = empty_sheet();
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(0.0, 0.0));

    let netlist = build_netlist(&sheet);
    assert!(netlist.nets.is_empty());
}

/// Two physically separate one-wire groups, each with a pin and a same-name
/// label of `ty`. Returns the built netlist so a test can assert whether
/// they were merged by name.
fn two_labelled_groups(text: &str, ty: LabelType) -> Netlist {
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire(pt(50.0, 0.0), pt(60.0, 0.0)));
    sheet.labels.push(label(text, pt(0.0, 0.0), ty));
    sheet.labels.push(label(text, pt(50.0, 0.0), ty));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(0.0, 0.0));
    place(&mut sheet, "R2", "R", pt(50.0, 0.0));
    build_netlist(&sheet)
}

#[test]
fn global_labels_of_the_same_name_merge_separate_groups() {
    // Same-name Global labels are one electrical net across the whole
    // design, even with no wire between the two groups.
    let netlist = two_labelled_groups("VBUS", LabelType::Global);
    assert_eq!(netlist.nets.len(), 1, "same-name globals merge");
    assert_eq!(netlist.nets[0].name, "VBUS");
    assert_eq!(netlist.nets[0].terminals.len(), 2);
}

#[test]
fn power_labels_of_the_same_name_merge() {
    // Every `GND` power port is the same net.
    let netlist = two_labelled_groups("GND", LabelType::Power);
    assert_eq!(netlist.nets.len(), 1, "same-name power nets merge");
    assert_eq!(netlist.nets[0].name, "GND");
    assert_eq!(netlist.nets[0].terminals.len(), 2);
}

#[test]
fn local_net_labels_of_the_same_name_merge_on_one_sheet() {
    // Same-name local labels connect within a single sheet.
    let netlist = two_labelled_groups("SDA", LabelType::Net);
    assert_eq!(netlist.nets.len(), 1, "same-name local labels connect");
    assert_eq!(netlist.nets[0].name, "SDA");
    assert_eq!(netlist.nets[0].terminals.len(), 2);
}

#[test]
fn hierarchical_labels_of_the_same_name_do_not_merge_on_one_sheet() {
    // Hierarchical labels connect to a parent sheet's pins, not to
    // same-name peers — that stitching is the cross-sheet increment's job,
    // so two same-name hierarchical groups stay separate here.
    let netlist = two_labelled_groups("BUS", LabelType::Hierarchical);
    assert_eq!(
        netlist.nets.len(),
        2,
        "hierarchical same-name is not merged on one sheet"
    );
}

#[test]
fn same_name_nets_on_one_sheet_get_unique_names() {
    // Two non-merging hierarchical labels of one name leave two electrically
    // distinct nets both wanting "BUS". A netlist with two nets of one name
    // is ambiguous to every downstream consumer, so names must stay unique:
    // the first keeps "BUS", the second is deterministically suffixed (D5.6).
    let netlist = two_labelled_groups("BUS", LabelType::Hierarchical);
    assert_eq!(netlist.nets.len(), 2);
    let names: Vec<&str> = netlist.nets.iter().map(|n| n.name.as_str()).collect();
    assert_ne!(
        names[0], names[1],
        "the two nets must not share a name: {names:?}"
    );
    assert!(names.contains(&"BUS"), "one keeps the bare name: {names:?}");
    assert!(
        names.iter().any(|n| n.starts_with("BUS_")),
        "the other is suffixed: {names:?}"
    );
}

#[test]
fn auto_name_never_collides_with_a_user_label() {
    // Bug #6: an unlabelled net auto-names "N$k"; the first net here (root at
    // the origin) takes "N$1". A user then spelled the *other* net's label
    // "N$1" too — the two must not resolve to one name.
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire(pt(50.0, 0.0), pt(60.0, 0.0)));
    sheet
        .labels
        .push(label("N$1", pt(50.0, 0.0), LabelType::Global));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(0.0, 0.0));
    place(&mut sheet, "R2", "R", pt(50.0, 0.0));

    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets.len(), 2);
    let names: Vec<&str> = netlist.nets.iter().map(|n| n.name.as_str()).collect();
    assert_ne!(
        names[0], names[1],
        "names stay unique despite the clash: {names:?}"
    );
}

#[test]
fn differently_named_labels_stay_separate() {
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire(pt(50.0, 0.0), pt(60.0, 0.0)));
    sheet
        .labels
        .push(label("NET_A", pt(0.0, 0.0), LabelType::Global));
    sheet
        .labels
        .push(label("NET_B", pt(50.0, 0.0), LabelType::Global));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(0.0, 0.0));
    place(&mut sheet, "R2", "R", pt(50.0, 0.0));

    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets.len(), 2, "different names are different nets");
}

#[test]
fn label_anchors_to_a_wire_interior_not_just_endpoints() {
    // A label placed mid-segment — (5,0) on the (0,0)-(10,0) wire, not an
    // endpoint — must anchor to that wire's net so its name-merge joins the
    // wire's group. Without the interior on-segment anchoring the label
    // would be a stray singleton at (5,0), and the two groups would NOT
    // merge (R1 and R2 would land on separate nets). Regression guard for
    // the point_on_segment anchoring path.
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire(pt(50.0, 0.0), pt(60.0, 0.0)));
    // First VBUS label sits on the *interior* of wire A, not an endpoint.
    sheet
        .labels
        .push(label("VBUS", pt(5.0, 0.0), LabelType::Global));
    sheet
        .labels
        .push(label("VBUS", pt(50.0, 0.0), LabelType::Global));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(0.0, 0.0));
    place(&mut sheet, "R2", "R", pt(50.0, 0.0));

    let netlist = build_netlist(&sheet);
    assert_eq!(
        netlist.nets.len(),
        1,
        "an interior-anchored label merges its wire's net"
    );
    assert_eq!(netlist.nets[0].name, "VBUS");
    assert_eq!(netlist.nets[0].terminals.len(), 2);
}

fn wire_id(a: Point, b: Point, id: Uuid) -> Wire {
    Wire {
        uuid: id,
        start: a,
        end: b,
        stroke_width: 0.0,
    }
}

#[test]
fn flood_returns_only_the_clicked_net() {
    // Two independent one-wire nets far apart. Clicking one must paint
    // only that wire — no spurious merge across the gap.
    let mut sheet = empty_sheet();
    let a = Uuid::from_u128(1);
    let b = Uuid::from_u128(2);
    sheet.wires.push(wire_id(pt(0.0, 0.0), pt(10.0, 0.0), a));
    sheet.wires.push(wire_id(pt(50.0, 0.0), pt(60.0, 0.0), b));

    let flood = flood_net_elements(&sheet, a).expect("clicked wire exists");
    assert_eq!(flood.wires, vec![a]);
    assert!(flood.junctions.is_empty());
}

#[test]
fn flood_follows_a_t_junction_and_paints_the_dot() {
    // Horizontal wire (0,0)-(10,0); a vertical wire ends on its interior
    // at (5,0); a junction dot ties them. Clicking the horizontal wire
    // must paint both wires and the junction — the app's old inline flood
    // missed the interior T-merge and left the second wire uncoloured.
    let mut sheet = empty_sheet();
    let h = Uuid::from_u128(10);
    let v = Uuid::from_u128(11);
    let j = Uuid::from_u128(12);
    sheet.wires.push(wire_id(pt(0.0, 0.0), pt(10.0, 0.0), h));
    sheet.wires.push(wire_id(pt(5.0, 0.0), pt(5.0, 5.0), v));
    sheet.junctions.push(Junction {
        uuid: j,
        position: pt(5.0, 0.0),
        diameter: 0.0,
    });

    let mut flood = flood_net_elements(&sheet, h).expect("clicked wire exists");
    flood.wires.sort();
    assert_eq!(flood.wires, vec![h, v]);
    assert_eq!(flood.junctions, vec![j]);
}

#[test]
fn flood_respects_micron_precision_and_does_not_leak() {
    // Two wires whose nearest endpoints are 4 µm apart: (10,0) and
    // (10, 0.004). At 1 µm resolution these are distinct points, so the
    // wires are two separate nets. The app's old 0.01 mm bucket rounded
    // both to the same cell and merged them — the leak. Clicking one must
    // now paint only itself.
    let mut sheet = empty_sheet();
    let a = Uuid::from_u128(20);
    let b = Uuid::from_u128(21);
    sheet.wires.push(wire_id(pt(0.0, 0.0), pt(10.0, 0.0), a));
    sheet
        .wires
        .push(wire_id(pt(10.0, 0.004), pt(20.0, 0.004), b));

    let flood = flood_net_elements(&sheet, a).expect("clicked wire exists");
    assert_eq!(flood.wires, vec![a], "must not leak into the 4 µm-away net");
}

#[test]
fn flood_follows_a_same_name_label_merge() {
    // Regression for #404. Two physically disjoint wires, each carrying the
    // Net label `VCC`: `build_netlist` returns ONE net. A flood that derives
    // only the physical connectivity painted the clicked wire alone, so the
    // highlight contradicted the netlist it claims to colour.
    let mut sheet = empty_sheet();
    let a = Uuid::from_u128(30);
    let b = Uuid::from_u128(31);
    sheet.wires.push(wire_id(pt(0.0, 0.0), pt(10.0, 0.0), a));
    sheet.wires.push(wire_id(pt(50.0, 0.0), pt(60.0, 0.0), b));
    sheet
        .labels
        .push(label("VCC", pt(5.0, 0.0), LabelType::Net));
    sheet
        .labels
        .push(label("VCC", pt(55.0, 0.0), LabelType::Net));

    let mut flood = flood_net_elements(&sheet, a).expect("clicked wire exists");
    flood.wires.sort();
    assert_eq!(
        flood.wires,
        vec![a, b],
        "the highlight must paint the whole net the netlist derives"
    );
}

#[test]
fn flood_returns_none_for_an_unknown_wire() {
    let sheet = empty_sheet();
    assert!(flood_net_elements(&sheet, Uuid::from_u128(99)).is_none());
}

#[test]
fn net_numbering_is_deterministic() {
    // Two independent one-wire nets far apart. Building twice yields the
    // same ids, and ids are assigned 1..=N in sorted-root order.
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(1.0, 0.0)));
    sheet.wires.push(wire(pt(50.0, 0.0), pt(51.0, 0.0)));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(0.0, 0.0));
    place(&mut sheet, "R2", "R", pt(50.0, 0.0));

    let a = build_netlist(&sheet);
    let b = build_netlist(&sheet);
    assert_eq!(a, b, "deterministic across builds");
    assert_eq!(a.nets.len(), 2);
    assert_eq!(a.nets[0].id, NetId(1));
    assert_eq!(a.nets[1].id, NetId(2));
}
