//! #2420, #1281: what the sheet's directives do to the netlist.
use super::*;
use signex_types::diff_pair_class::DiffPairClass;
use signex_types::net::NetClassId;
use signex_types::net_class::NetClass;
use signex_types::schematic::{BlanketPour, NetRules, SchDirective};

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
    edge_blanket(a, b, class, false)
}

fn edge_blanket(a: Point, b: Point, class: Option<&str>, include_edge_nets: bool) -> SchDirective {
    SchDirective::Blanket {
        uuid: id(91),
        start: a,
        end: b,
        net_class: class.map(str::to_string),
        erc_exclusions: Vec::new(),
        track_width: None,
        pour: BlanketPour::Normal,
        include_edge_nets,
    }
}

fn diff_pair(n: u128, at: Point, class: Option<&str>) -> SchDirective {
    SchDirective::DiffPair {
        uuid: id(n),
        position: at,
        class: class.map(str::to_string),
    }
}

fn net_parameters(at: Point, class: Option<&str>, rules: NetRules) -> SchDirective {
    SchDirective::NetParameters {
        uuid: id(96),
        position: at,
        net_class: class.map(str::to_string),
        diff_pair_class: None,
        rules,
        show_on_flag: false,
    }
}

fn class(name: &str) -> Option<NetClassId> {
    Some(NetClassId(name.to_string()))
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
    assert_eq!(netlist.nets[0].class, class("HighSpeed"));
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
fn a_blanket_catches_a_net_crossing_its_edge_only_when_told_to() {
    let region = (pt(-1.0, -1.0), pt(5.0, 1.0));
    for (include, want) in [(false, None), (true, class("HighSpeed"))] {
        let mut sheet = two_nets("A", "B");
        let directive = edge_blanket(region.0, region.1, Some("HighSpeed"), include);
        sheet.directives.push(directive.clone());
        let netlist = build_netlist(&sheet);
        assert_eq!(netlist.nets[0].class, want, "include_edge_nets {include}");
        let listed = crate::blanket_nets(&sheet, &netlist, &directive);
        let catch = if include {
            crate::BlanketCatch::EdgeIncluded
        } else {
            crate::BlanketCatch::EdgeOnly
        };
        assert_eq!(
            listed,
            [crate::BlanketNet {
                net: netlist.nets[0].id,
                name: "A".to_string(),
                catch,
            }]
        );
        assert_eq!(listed[0].catch.is_caught(), include);
    }
}

#[test]
fn a_blanket_lists_the_nets_inside_it_and_none_out_of_reach() {
    let mut sheet = two_nets("A", "B");
    let directive = blanket(pt(-1.0, -1.0), pt(11.0, 1.0), None);
    sheet.directives.push(directive.clone());
    let netlist = build_netlist(&sheet);
    let listed = crate::blanket_nets(&sheet, &netlist, &directive);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "A");
    assert_eq!(listed[0].catch, crate::BlanketCatch::Inside);
}

#[test]
fn a_blanket_never_catches_a_power_net() {
    let mut sheet = two_nets("GND", "B");
    sheet.labels[0].label_type = LabelType::Power;
    let directive = blanket(pt(-1.0, -1.0), pt(11.0, 21.0), Some("SW_NODE"));
    sheet.directives.push(directive.clone());
    let netlist = build_netlist(&sheet);
    let gnd = netlist.nets.iter().find(|n| n.name == "GND").expect("GND");
    let b = netlist.nets.iter().find(|n| n.name == "B").expect("B");
    assert_eq!(gnd.class, None);
    assert_eq!(b.class, class("SW_NODE"));
    let listed = crate::blanket_nets(&sheet, &netlist, &directive);
    assert!(listed.iter().all(|n| n.name != "GND"), "{listed:?}");
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
    assert_eq!(netlist.nets[0].class, class("First"));
}

#[test]
fn net_parameters_give_their_net_its_class_and_rules() {
    let mut sheet = two_nets("VBUS", "B");
    let rules = NetRules {
        width_mm: Some(0.8),
        clearance_mm: Some(0.3),
        ..NetRules::default()
    };
    sheet.directives.push(net_parameters(
        pt(5.0, 0.0),
        Some("POWER_IN"),
        rules.clone(),
    ));
    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets[0].class, class("POWER_IN"));
    assert_eq!(netlist.nets[1].class, None);
    let on = crate::net_parameters(&sheet, &netlist);
    assert_eq!(on.len(), 1);
    assert_eq!(on[0].net_name, "VBUS");
    assert_eq!(on[0].net, netlist.nets[0].id);
    assert_eq!(on[0].rules, rules);
}

#[test]
fn net_parameters_off_a_wire_name_no_net() {
    let mut sheet = two_nets("A", "B");
    sheet.directives.push(net_parameters(
        pt(5.0, 50.0),
        Some("X"),
        NetRules::default(),
    ));
    let netlist = build_netlist(&sheet);
    assert!(crate::net_parameters(&sheet, &netlist).is_empty());
    assert!(netlist.nets.iter().all(|n| n.class.is_none()));
}

#[test]
fn net_parameters_outrank_a_blanket_which_outranks_the_project_patterns() {
    let mut sheet = two_nets("A", "B");
    sheet
        .directives
        .push(blanket(pt(-1.0, -1.0), pt(11.0, 21.0), Some("SW_NODE")));
    sheet.directives.push(net_parameters(
        pt(5.0, 0.0),
        Some("POWER_IN"),
        NetRules::default(),
    ));
    sheet
        .wires
        .push(wire_id(pt(40.0, 0.0), pt(50.0, 0.0), id(3)));
    sheet.labels.push(label("C", pt(40.0, 0.0), LabelType::Net));
    place(&mut sheet, "R5", "R", pt(40.0, 0.0));
    let mut netlist = build_netlist(&sheet);
    let classes = [NetClass {
        name: "Pattern".to_string(),
        patterns: vec!["*".to_string()],
        ..NetClass::default()
    }];
    crate::fill_pattern_classes(&mut netlist, &classes);
    let of = |name: &str| {
        netlist
            .nets
            .iter()
            .find(|n| n.name == name)
            .and_then(|n| n.class.clone())
    };
    assert_eq!(of("A"), class("POWER_IN"));
    assert_eq!(of("B"), class("SW_NODE"));
    assert_eq!(of("C"), class("Pattern"));
}

#[test]
fn a_diff_pair_directive_pairs_its_net_with_the_partner() {
    let mut sheet = two_nets("USB_D_P", "USB_D_N");
    sheet.directives.push(diff_pair(92, pt(5.0, 0.0), None));
    let netlist = build_netlist(&sheet);
    let pairs = crate::diff_pairs(&sheet, &netlist);
    assert_eq!(
        pairs,
        [crate::DiffPair {
            directive: id(92),
            name: "USB_D".to_string(),
            positive: Some("USB_D_P".to_string()),
            negative: Some("USB_D_N".to_string()),
            class: None,
        }]
    );
    assert!(pairs[0].is_complete());
}

#[test]
fn a_pair_is_found_by_dp_dn_and_plus_minus_suffixes_too() {
    for (p, n, name) in [("USB_DP", "USB_DN", "USB_D"), ("CLK+", "CLK-", "CLK")] {
        let mut sheet = two_nets(p, n);
        sheet.directives.push(diff_pair(92, pt(5.0, 20.0), None));
        let netlist = build_netlist(&sheet);
        let pairs = crate::diff_pairs(&sheet, &netlist);
        assert_eq!(pairs.len(), 1, "{p}/{n}");
        assert_eq!(pairs[0].name, name);
        assert_eq!(pairs[0].positive.as_deref(), Some(p));
        assert_eq!(pairs[0].negative.as_deref(), Some(n));
    }
}

#[test]
fn a_pair_carries_its_class_and_the_class_decides_the_suffix_styles() {
    let mut sheet = two_nets("USB_DP", "USB_DN");
    sheet
        .directives
        .push(diff_pair(92, pt(5.0, 0.0), Some(" USB ")));
    let netlist = build_netlist(&sheet);
    let by_suffix = |pair_by_suffix: bool| {
        [DiffPairClass {
            name: "USB".to_string(),
            pair_by_suffix,
            ..DiffPairClass::default()
        }]
    };
    let pairs = crate::diff_pairs_with_classes(&sheet, &netlist, &by_suffix(true));
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].class.as_deref(), Some("USB"));
    assert!(pairs[0].is_complete());
    // Off: only `_P` / `_N` pair, so `USB_DP` names no pair.
    assert!(crate::diff_pairs_with_classes(&sheet, &netlist, &by_suffix(false)).is_empty());

    let members =
        crate::class_members(&[("power_in.snxsch".to_string(), &sheet)], &by_suffix(true));
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].sheet, "power_in.snxsch");
    assert_eq!(crate::pair_count(&members, "USB"), 1);
    assert_eq!(crate::pair_count(&members, "HDMI"), 0);
}

#[test]
fn a_diff_pair_directive_off_a_wire_or_on_an_unsuffixed_net_pairs_nothing() {
    let mut sheet = two_nets("CLK", "USB_D_N");
    sheet.directives.push(diff_pair(93, pt(5.0, 0.0), None));
    sheet.directives.push(diff_pair(94, pt(5.0, 50.0), None));
    let netlist = build_netlist(&sheet);
    assert!(crate::diff_pairs(&sheet, &netlist).is_empty());
}

#[test]
fn a_half_pair_is_reported_incomplete() {
    let mut sheet = two_nets("USB_D_N", "OTHER");
    sheet.directives.push(diff_pair(95, pt(5.0, 0.0), None));
    let netlist = build_netlist(&sheet);
    let pairs = crate::diff_pairs(&sheet, &netlist);
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].positive, None);
    assert_eq!(pairs[0].negative, Some("USB_D_N".to_string()));
    assert!(!pairs[0].is_complete());
}

#[test]
fn the_project_netlist_applies_blanket_and_net_parameters_classes() {
    let mut sheet = two_nets("A", "B");
    sheet
        .directives
        .push(blanket(pt(-1.0, 19.0), pt(11.0, 21.0), Some("SW_NODE")));
    sheet.directives.push(net_parameters(
        pt(5.0, 0.0),
        Some("POWER_IN"),
        NetRules::default(),
    ));
    let key = crate::SheetKey::new("root");
    let sheets: std::collections::HashMap<_, _> = [(key.clone(), sheet.clone())].into();
    let resolved: std::collections::HashMap<_, _> =
        [(key.clone(), std::collections::HashMap::new())].into();
    let roots = [crate::ProjectRoot { key }];
    let project = crate::build_project_netlist(&crate::ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    });
    let of = |name: &str| {
        project
            .netlist
            .nets
            .iter()
            .find(|n| n.name == name)
            .and_then(|n| n.class.clone())
    };
    assert_eq!(of("A"), class("POWER_IN"));
    assert_eq!(of("B"), class("SW_NODE"));
    // The one-sheet project still reads exactly as the sheet's own netlist.
    assert_eq!(project.netlist, build_netlist(&sheet));
}
