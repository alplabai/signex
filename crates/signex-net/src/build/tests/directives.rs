//! #2420: what the sheet's directives do to the netlist.
use super::*;
use signex_types::net::NetClassId;
use signex_types::schematic::SchDirective;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// Two one-wire nets, each with a resistor on both ends: the first from
/// (0,0) to (10,0) named `first`, the second from (0,20) to (10,20) named
/// `second`.
fn two_nets(first: &str, second: &str) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet
        .wires
        .push(wire_id(pt(0.0, 0.0), pt(10.0, 0.0), id(1)));
    sheet
        .wires
        .push(wire_id(pt(0.0, 20.0), pt(10.0, 20.0), id(2)));
    sheet
        .labels
        .push(label(first, pt(0.0, 0.0), LabelType::Net));
    sheet
        .labels
        .push(label(second, pt(0.0, 20.0), LabelType::Net));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(0.0, 0.0));
    place(&mut sheet, "R2", "R", pt(10.0, 0.0));
    place(&mut sheet, "R3", "R", pt(0.0, 20.0));
    place(&mut sheet, "R4", "R", pt(10.0, 20.0));
    sheet
}

fn mask(a: Point, b: Point) -> SchDirective {
    SchDirective::CompileMask {
        uuid: id(90),
        start: a,
        end: b,
    }
}

fn blanket(a: Point, b: Point, class: Option<&str>) -> SchDirective {
    SchDirective::Blanket {
        uuid: id(91),
        start: a,
        end: b,
        net_class: class.map(str::to_string),
        erc_exclusions: Vec::new(),
    }
}

fn names(netlist: &Netlist) -> Vec<&str> {
    netlist.nets.iter().map(|n| n.name.as_str()).collect()
}

#[test]
fn a_sheet_without_a_mask_is_read_as_it_is() {
    let sheet = two_nets("A", "B");
    assert!(matches!(
        crate::compile_masked(&sheet),
        std::borrow::Cow::Borrowed(_)
    ));
}

#[test]
fn a_compile_mask_takes_the_net_it_covers_out_of_the_netlist() {
    let mut sheet = two_nets("A", "B");
    assert_eq!(names(&build_netlist(&sheet)), ["A", "B"]);
    sheet.directives.push(mask(pt(-1.0, -1.0), pt(11.0, 1.0)));
    assert_eq!(names(&build_netlist(&sheet)), ["B"]);
}

#[test]
fn a_mask_over_one_end_drops_only_what_lies_inside() {
    let mut sheet = two_nets("A", "B");
    // R2 and the wire's far end are inside; the wire's near end is not, so
    // the wire stays and R1 keeps the net.
    sheet.directives.push(mask(pt(9.0, -1.0), pt(11.0, 1.0)));
    let netlist = build_netlist(&sheet);
    let a = netlist.nets.iter().find(|n| n.name == "A").expect("net A");
    let refs: Vec<&str> = a.terminals.iter().map(|t| t.reference.as_str()).collect();
    assert_eq!(refs, ["R1"]);
}

#[test]
fn a_blanket_gives_the_nets_inside_it_its_class() {
    let mut sheet = two_nets("A", "B");
    sheet
        .directives
        .push(blanket(pt(-1.0, -1.0), pt(11.0, 1.0), Some(" HighSpeed ")));
    let netlist = build_netlist(&sheet);
    assert_eq!(
        netlist.nets[0].class,
        Some(NetClassId("HighSpeed".to_string()))
    );
    assert_eq!(netlist.nets[1].class, None);
}

#[test]
fn a_blanket_over_part_of_a_wire_or_without_a_class_sets_none() {
    let mut sheet = two_nets("A", "B");
    sheet
        .directives
        .push(blanket(pt(-1.0, -1.0), pt(5.0, 1.0), Some("HighSpeed")));
    sheet
        .directives
        .push(blanket(pt(-1.0, 19.0), pt(11.0, 21.0), None));
    let netlist = build_netlist(&sheet);
    assert!(netlist.nets.iter().all(|n| n.class.is_none()));
}

#[test]
fn the_first_blanket_in_sheet_order_wins() {
    let mut sheet = two_nets("A", "B");
    let region = (pt(-1.0, -1.0), pt(11.0, 1.0));
    sheet
        .directives
        .push(blanket(region.0, region.1, Some("First")));
    sheet
        .directives
        .push(blanket(region.0, region.1, Some("Second")));
    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets[0].class, Some(NetClassId("First".to_string())));
}

#[test]
fn a_diff_pair_directive_pairs_its_net_with_the_partner() {
    let mut sheet = two_nets("USB_D_P", "USB_D_N");
    sheet.directives.push(SchDirective::DiffPair {
        uuid: id(92),
        position: pt(5.0, 0.0),
    });
    let netlist = build_netlist(&sheet);
    let pairs = crate::diff_pairs(&sheet, &netlist);
    assert_eq!(
        pairs,
        [crate::DiffPair {
            directive: id(92),
            name: "USB_D".to_string(),
            positive: Some("USB_D_P".to_string()),
            negative: Some("USB_D_N".to_string()),
        }]
    );
    assert!(pairs[0].is_complete());
}

#[test]
fn a_diff_pair_directive_off_a_wire_or_on_an_unsuffixed_net_pairs_nothing() {
    let mut sheet = two_nets("CLK", "USB_D_N");
    sheet.directives.push(SchDirective::DiffPair {
        uuid: id(93),
        position: pt(5.0, 0.0),
    });
    sheet.directives.push(SchDirective::DiffPair {
        uuid: id(94),
        position: pt(5.0, 50.0),
    });
    let netlist = build_netlist(&sheet);
    assert!(crate::diff_pairs(&sheet, &netlist).is_empty());
}

#[test]
fn a_half_pair_is_reported_incomplete() {
    let mut sheet = two_nets("USB_D_N", "OTHER");
    sheet.directives.push(SchDirective::DiffPair {
        uuid: id(95),
        position: pt(5.0, 0.0),
    });
    let netlist = build_netlist(&sheet);
    let pairs = crate::diff_pairs(&sheet, &netlist);
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].positive, None);
    assert_eq!(pairs[0].negative, Some("USB_D_N".to_string()));
    assert!(!pairs[0].is_complete());
}
