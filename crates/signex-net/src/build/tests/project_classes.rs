//! #2656: net class conflicts between the sheets of a project, ranked as the
//! stitched netlist ranks them.
use std::collections::HashMap;

use super::directives::{blanket, id, net_parameters, with_id};
use super::*;
use signex_types::net::NetClassId;
use signex_types::project::ChannelNaming;
use signex_types::schematic::{NetRules, SchDirective};

use crate::{ProjectClassConflicts, ProjectGraph, ProjectRoot, SheetKey};

/// One sheet holding a global `VBUS` wire from (0,0) to (10,0) with the
/// uuid `wire`, a resistor on each end (`refs`), and `directives`.
fn vbus_sheet(wire: u128, refs: [&str; 2], directives: Vec<SchDirective>) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet
        .wires
        .push(wire_id(pt(0.0, 0.0), pt(10.0, 0.0), id(wire)));
    sheet
        .labels
        .push(label("VBUS", pt(0.0, 0.0), LabelType::Global));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, refs[0], "R", pt(0.0, 0.0));
    place(&mut sheet, refs[1], "R", pt(10.0, 0.0));
    sheet.directives = directives;
    sheet
}

fn parameters(n: u128, class: &str) -> SchDirective {
    with_id(
        net_parameters(pt(5.0, 0.0), Some(class), NetRules::default()),
        n,
    )
}

/// Two root pages `a` and `b`, so `VBUS` joins them through its global label.
fn two_pages(a: Vec<SchDirective>, b: Vec<SchDirective>) -> HashMap<SheetKey, SchematicSheet> {
    [
        (SheetKey::new("a"), vbus_sheet(201, ["R1", "R2"], a)),
        (SheetKey::new("b"), vbus_sheet(202, ["R3", "R4"], b)),
    ]
    .into()
}

fn check(sheets: &HashMap<SheetKey, SchematicSheet>) -> (ProjectClassConflicts, Option<String>) {
    let resolved: HashMap<SheetKey, HashMap<String, SheetKey>> =
        sheets.keys().map(|k| (k.clone(), HashMap::new())).collect();
    let roots = [
        ProjectRoot {
            key: SheetKey::new("a"),
        },
        ProjectRoot {
            key: SheetKey::new("b"),
        },
    ];
    let graph = ProjectGraph {
        sheets,
        resolved: &resolved,
        roots: &roots,
    };
    let naming = ChannelNaming::default();
    let netlist = crate::build_project_netlist_named(&graph, &naming).netlist;
    let vbus = netlist
        .nets
        .iter()
        .find(|n| n.name == "VBUS")
        .expect("VBUS");
    let class = vbus.class.clone().map(|NetClassId(c)| c);
    (crate::project_net_class_conflicts(&graph, &naming), class)
}

/// (sheet, directive, class) of an assignment.
fn who(a: &crate::SheetClassAssignment) -> (&str, Uuid, &str) {
    (
        a.sheet.as_str(),
        a.assignment.directive,
        a.assignment.class.as_str(),
    )
}

#[test]
fn net_parameters_on_two_sheets_naming_two_classes_conflict() {
    let sheets = two_pages(
        vec![parameters(301, "POWER")],
        vec![parameters(302, "SIGNAL")],
    );
    // Each sheet on its own sees one directive and no conflict.
    for sheet in sheets.values() {
        assert!(crate::net_class_conflicts(sheet, &build_netlist(sheet)).is_empty());
    }
    let (found, class) = check(&sheets);
    assert_eq!(class.as_deref(), Some("POWER"), "sheet key order: a first");
    assert_eq!(found.conflicts.len(), 1, "{found:?}");
    let c = &found.conflicts[0];
    assert_eq!(c.net_name, "VBUS");
    assert_eq!(who(&c.winner), ("a", id(301), "POWER"));
    let lost: Vec<_> = c.overridden.iter().map(who).collect();
    assert_eq!(lost, [("b", id(302), "SIGNAL")]);
    assert!(found.judged.contains(&(SheetKey::new("a"), id(301))));
    assert!(found.judged.contains(&(SheetKey::new("b"), id(302))));
}

/// The winner is the netlist's class: moving the classes between the sheets
/// moves the winner with them.
#[test]
fn the_winner_follows_the_sheet_key_order_the_netlist_reads() {
    let sheets = two_pages(
        vec![parameters(301, "SIGNAL")],
        vec![parameters(302, "POWER")],
    );
    let (found, class) = check(&sheets);
    assert_eq!(class.as_deref(), Some("SIGNAL"));
    assert_eq!(who(&found.conflicts[0].winner), ("a", id(301), "SIGNAL"));
}

/// A Net Parameters directive on a later sheet outranks a Net Blanket on an
/// earlier one, as it does on one sheet.
#[test]
fn net_parameters_on_a_later_sheet_outrank_a_blanket_on_an_earlier_one() {
    let cover = with_id(blanket(pt(-1.0, -1.0), pt(11.0, 1.0), Some("SW_NODE")), 311);
    let sheets = two_pages(vec![cover], vec![parameters(302, "POWER")]);
    let (found, class) = check(&sheets);
    assert_eq!(class.as_deref(), Some("POWER"));
    let c = &found.conflicts[0];
    assert_eq!(who(&c.winner), ("b", id(302), "POWER"));
    let lost: Vec<_> = c.overridden.iter().map(who).collect();
    assert_eq!(lost, [("a", id(311), "SW_NODE")]);
}

#[test]
fn two_sheets_naming_one_class_do_not_conflict() {
    let sheets = two_pages(
        vec![parameters(301, "POWER")],
        vec![parameters(302, "POWER")],
    );
    let (found, class) = check(&sheets);
    assert_eq!(class.as_deref(), Some("POWER"));
    assert!(found.conflicts.is_empty(), "{found:?}");
    assert_eq!(found.judged.len(), 2, "both are judged");
}

/// #2961: one page with local nets `N1` (wire uuid 401) and `N2` (402), a
/// part on each, under two overlapping Blankets: HS (411) first, PWR (412).
fn two_nets_under_two_blankets() -> SchematicSheet {
    let mut sheet = empty_sheet();
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    for (name, y, wire, reference) in [("N1", 0.0, 401, "R1"), ("N2", 5.0, 402, "R2")] {
        sheet.wires.push(wire_id(pt(0.0, y), pt(10.0, y), id(wire)));
        sheet.labels.push(label(name, pt(10.0, y), LabelType::Net));
        place(&mut sheet, reference, "R", pt(0.0, y));
    }
    sheet.directives = vec![
        with_id(blanket(pt(-1.0, -1.0), pt(11.0, 6.0), Some("HS")), 411),
        with_id(blanket(pt(-2.0, -2.0), pt(12.0, 7.0), Some("PWR")), 412),
    ];
    sheet
}

/// (winner, overridden) directives of a conflict, as ids.
fn ranked(winner: Uuid, overridden: impl Iterator<Item = Uuid>) -> (Uuid, Vec<Uuid>) {
    (winner, overridden.collect())
}

#[test]
fn two_nets_the_same_two_blankets_class_are_two_conflicts() {
    let sheet = two_nets_under_two_blankets();
    let local = crate::net_class_conflicts(&sheet, &build_netlist(&sheet));
    let sheets: HashMap<SheetKey, SchematicSheet> = [(SheetKey::new("a"), sheet)].into();
    let resolved: HashMap<SheetKey, HashMap<String, SheetKey>> =
        [(SheetKey::new("a"), HashMap::new())].into();
    let roots = [ProjectRoot {
        key: SheetKey::new("a"),
    }];
    let graph = ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    };
    let found = crate::project_net_class_conflicts(&graph, &ChannelNaming::default());

    assert_eq!(local.len(), 2, "per sheet: {local:?}");
    assert_eq!(found.conflicts.len(), 2, "project: {found:?}");
    let per_sheet: Vec<_> = local
        .iter()
        .map(|c| ranked(c.winner.directive, c.overridden.iter().map(|a| a.directive)))
        .collect();
    let project: Vec<_> = found
        .conflicts
        .iter()
        .map(|c| {
            let lost = c.overridden.iter().map(|a| a.assignment.directive);
            ranked(c.winner.assignment.directive, lost)
        })
        .collect();
    assert_eq!(per_sheet, project, "the two runs agree");
    assert_eq!(project[0], (id(411), vec![id(412)]));
    let mut names: Vec<&str> = found
        .conflicts
        .iter()
        .map(|c| c.net_name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names.len(), 2);
    assert_ne!(names[0], names[1], "one finding per net");
    for name in ["N1", "N2"] {
        assert!(
            found
                .carried
                .contains(&(SheetKey::new("a"), name.to_string())),
            "{name} is in the project netlist: {:?}",
            found.carried
        );
    }
}
