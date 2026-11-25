//! #2420, #1281, #1489, #1282: what the sheet's directives do to the netlist.
use super::*;
use signex_types::diff_pair_class::DiffPairClass;
use signex_types::net::NetClassId;
use signex_types::net_class::NetClass;
use signex_types::schematic::{BlanketPour, DirectiveKind, NetRules, SchDirective};

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
            pair_by_suffix,
            ..DiffPairClass::named("USB")
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

/// `directive` with the uuid `n`, so two of one kind stay apart.
fn with_id(mut directive: SchDirective, n: u128) -> SchDirective {
    directive.set_uuid(id(n));
    directive
}

/// #2656: two Net Parameters directives on one net naming different classes.
/// The first in sheet order wins, and the conflict names the other.
#[test]
fn two_net_parameters_on_one_net_conflict_and_the_first_wins() {
    let mut sheet = two_nets("A", "B");
    let first = with_id(
        net_parameters(pt(3.0, 0.0), Some("POWER"), NetRules::default()),
        101,
    );
    let second = with_id(
        net_parameters(pt(7.0, 0.0), Some("SIGNAL"), NetRules::default()),
        102,
    );
    sheet.directives.extend([first, second]);
    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets[0].class, class("POWER"));
    let conflicts = crate::net_class_conflicts(&sheet, &netlist);
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    let c = &conflicts[0];
    assert_eq!(c.net_name, "A");
    assert_eq!(
        (c.winner.directive, c.winner.class.as_str()),
        (id(101), "POWER")
    );
    assert_eq!(c.winner.at, pt(3.0, 0.0));
    let lost: Vec<(Uuid, &str)> = c
        .overridden
        .iter()
        .map(|a| (a.directive, a.class.as_str()))
        .collect();
    assert_eq!(lost, [(id(102), "SIGNAL")]);
}

/// Swapping the two directives swaps the winner: the answer follows sheet
/// order and nothing else.
#[test]
fn the_resolution_follows_sheet_order_only() {
    let mut sheet = two_nets("A", "B");
    sheet.directives.extend([
        with_id(
            net_parameters(pt(7.0, 0.0), Some("SIGNAL"), NetRules::default()),
            102,
        ),
        with_id(
            net_parameters(pt(3.0, 0.0), Some("POWER"), NetRules::default()),
            101,
        ),
    ]);
    let netlist = build_netlist(&sheet);
    assert_eq!(netlist.nets[0].class, class("SIGNAL"));
    let conflicts = crate::net_class_conflicts(&sheet, &netlist);
    assert_eq!(conflicts[0].winner.directive, id(102));
    assert_eq!(conflicts, crate::net_class_conflicts(&sheet, &netlist));
}

#[test]
fn a_net_parameters_class_over_a_blanket_class_is_a_conflict() {
    let mut sheet = two_nets("A", "B");
    sheet
        .directives
        .push(blanket(pt(-1.0, -1.0), pt(11.0, 21.0), Some("SW_NODE")));
    sheet.directives.push(net_parameters(
        pt(5.0, 0.0),
        Some("POWER_IN"),
        NetRules::default(),
    ));
    let netlist = build_netlist(&sheet);
    let conflicts = crate::net_class_conflicts(&sheet, &netlist);
    // Only A: B takes the blanket's class from the blanket alone.
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    let c = &conflicts[0];
    assert_eq!(c.net_name, "A");
    assert_eq!(c.winner.kind, DirectiveKind::NetParameters);
    assert_eq!(c.overridden.len(), 1);
    assert_eq!(c.overridden[0].kind, DirectiveKind::Blanket);
    assert_eq!(c.overridden[0].class, "SW_NODE");
    // A blanket's anchor is its centre, inside it.
    assert_eq!(c.overridden[0].at, pt(5.0, 10.0));
}

#[test]
fn two_blankets_with_different_classes_on_one_net_conflict() {
    let mut sheet = two_nets("A", "B");
    let region = (pt(-1.0, -1.0), pt(11.0, 1.0));
    sheet.directives.extend([
        with_id(blanket(region.0, region.1, Some("First")), 111),
        with_id(blanket(region.0, region.1, Some("Second")), 112),
    ]);
    let netlist = build_netlist(&sheet);
    let conflicts = crate::net_class_conflicts(&sheet, &netlist);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].winner.directive, id(111));
    assert_eq!(conflicts[0].overridden[0].directive, id(112));
}

#[test]
fn directives_naming_the_same_class_do_not_conflict() {
    let mut sheet = two_nets("A", "B");
    sheet
        .directives
        .push(blanket(pt(-1.0, -1.0), pt(11.0, 1.0), Some("POWER")));
    sheet.directives.push(net_parameters(
        pt(5.0, 0.0),
        Some(" POWER "),
        NetRules::default(),
    ));
    let netlist = build_netlist(&sheet);
    assert!(crate::net_class_conflicts(&sheet, &netlist).is_empty());
    let all = crate::class_assignments(&sheet, &netlist);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].1.len(), 2, "both assignments are listed");
}

/// A Compile Mask hides a stub wire whose end sits under the Net Parameters
/// flag. The netlist classes the net through the wire left under the flag,
/// so the conflict check reads the masked sheet too and reports it.
#[test]
fn a_conflict_is_judged_on_the_masked_sheet_the_netlist_reads() {
    let mut sheet = two_nets("A", "B");
    sheet
        .wires
        .insert(0, wire_id(pt(5.0, 0.0), pt(6.0, 0.0), id(3)));
    sheet.directives.extend([
        mask(pt(4.0, -1.0), pt(7.0, 1.0)),
        blanket(pt(-1.0, -1.0), pt(11.0, 1.0), Some("SW_NODE")),
        net_parameters(pt(5.0, 0.0), Some("POWER_IN"), NetRules::default()),
    ]);
    let netlist = build_netlist(&sheet);
    let a = netlist.nets.iter().find(|n| n.name == "A").expect("net A");
    assert_eq!(a.class, class("POWER_IN"));
    let conflicts = crate::net_class_conflicts(&sheet, &netlist);
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    assert_eq!(conflicts[0].winner.class, "POWER_IN");
    assert_eq!(conflicts[0].overridden[0].class, "SW_NODE");
}

/// The netlist's classes are the first assignment of each net, so the
/// conflict list and `Net::class` can never disagree.
#[test]
fn every_net_takes_the_winner_of_its_assignments() {
    let mut sheet = two_nets("A", "B");
    sheet
        .directives
        .push(blanket(pt(-1.0, -1.0), pt(11.0, 21.0), Some("SW_NODE")));
    sheet.directives.push(net_parameters(
        pt(5.0, 20.0),
        Some("POWER_IN"),
        NetRules::default(),
    ));
    let netlist = build_netlist(&sheet);
    for (net, list) in crate::class_assignments(&sheet, &netlist) {
        let n = netlist.nets.iter().find(|n| n.id == net).unwrap();
        assert_eq!(n.class, class(&list[0].class), "{}", n.name);
    }
}
