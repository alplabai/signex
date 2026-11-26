//! #2656: `NetClassConflict` over the stitched project netlist, so a class a
//! directive on another sheet gives the net is reported.

use std::collections::HashMap;

use signex_net::{ProjectGraph, ProjectRoot, SheetKey};
use signex_types::project::ChannelNaming;
use signex_types::schematic::{
    Label, LabelType, LibPin, LibSymbol, NoErcDirective, Pin, PinDirection, PinShapeStyle, Point,
    SchDirective, SchematicSheet, Symbol, Wire,
};
use uuid::Uuid;

use super::{ProjectClassFindings, project_net_class_conflicts_named};
use crate::fixtures::{net_blanket, net_parameters};
use crate::{RuleKind, Violation};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn resistor() -> LibSymbol {
    LibSymbol {
        id: "R".to_string(),
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
        pins: vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: pt(0.0, 0.0),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

/// A global `VBUS` wire from (0,0) to (10,0) with the uuid `wire`, resistor
/// `reference` on its right end, and `directives`.
fn vbus_sheet(wire: u128, reference: &str, directives: Vec<SchDirective>) -> SchematicSheet {
    let mut s = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    s.wires.push(Wire {
        uuid: Uuid::from_u128(wire),
        start: pt(0.0, 0.0),
        end: pt(10.0, 0.0),
        ..Default::default()
    });
    s.labels
        .push(Label::new("VBUS", pt(0.0, 0.0), LabelType::Global));
    s.lib_symbols.insert("R".to_string(), resistor());
    s.symbols.push(Symbol {
        uuid: Uuid::from_u128(wire + 1000),
        lib_id: "R".to_string(),
        reference: reference.to_string(),
        position: pt(10.0, 0.0),
        ..Default::default()
    });
    s.directives = directives;
    s
}

/// The project pass over the root pages `sheets`, in the order given.
fn check(sheets: Vec<(&str, SchematicSheet)>) -> ProjectClassFindings {
    let roots: Vec<ProjectRoot> = sheets
        .iter()
        .map(|(k, _)| ProjectRoot {
            key: SheetKey::new(*k),
        })
        .collect();
    let sheets: HashMap<SheetKey, SchematicSheet> = sheets
        .into_iter()
        .map(|(k, s)| (SheetKey::new(k), s))
        .collect();
    let resolved = sheets.keys().map(|k| (k.clone(), HashMap::new())).collect();
    project_net_class_conflicts_named(
        &ProjectGraph {
            sheets: &sheets,
            resolved: &resolved,
            roots: &roots,
        },
        &ChannelNaming::default(),
    )
}

fn only(found: &ProjectClassFindings) -> (&str, &Violation) {
    assert_eq!(found.findings.len(), 1, "{:?}", found.findings);
    let (key, v) = &found.findings[0];
    (key.as_str(), v)
}

#[test]
fn a_class_another_sheet_overrides_is_reported_on_the_overridden_sheet() {
    let a = vbus_sheet(1, "R1", vec![net_parameters(12, pt(5.0, 0.0), "POWER")]);
    let b = vbus_sheet(2, "R2", vec![net_parameters(13, pt(5.0, 0.0), "SIGNAL")]);
    let found = check(vec![("a", a), ("b", b)]);
    let (sheet, v) = only(&found);
    assert_eq!(sheet, "b");
    assert_eq!(v.rule, RuleKind::NetClassConflict);
    assert_eq!(
        v.message,
        "Net VBUS takes class POWER from a Net Parameters directive on sheet a; \
         class SIGNAL of this Net Parameters directive is overridden"
    );
    assert_eq!(v.location, pt(5.0, 0.0));
    assert_eq!(v.primary.map(|p| p.uuid), Some(Uuid::from_u128(13)));
    assert_eq!(v.peer, None, "the winner is on another canvas");
    for (key, n) in [("a", 12), ("b", 13)] {
        assert!(
            found
                .covered
                .contains(&(SheetKey::new(key), Uuid::from_u128(n))),
            "{key}"
        );
    }
}

/// On one sheet the project pass gives the per-sheet rule's finding, peer
/// included, so replacing that one loses nothing.
#[test]
fn a_conflict_on_one_sheet_keeps_its_peer() {
    let found = check(vec![(
        "a",
        vbus_sheet(
            1,
            "R1",
            vec![
                net_blanket(11, pt(-1.0, -1.0), pt(11.0, 1.0), "SIGNAL"),
                net_parameters(12, pt(5.0, 0.0), "POWER"),
            ],
        ),
    )]);
    let (sheet, v) = only(&found);
    assert_eq!(sheet, "a");
    assert_eq!(
        v.message,
        "Net VBUS takes class POWER from a Net Parameters directive; \
         class SIGNAL of this Net Blanket is overridden"
    );
    assert_eq!(v.primary.map(|p| p.uuid), Some(Uuid::from_u128(11)));
    assert_eq!(v.peer.map(|p| p.uuid), Some(Uuid::from_u128(12)));
}

#[test]
fn a_no_erc_directive_on_the_overridden_directive_silences_it() {
    let mut b = vbus_sheet(2, "R2", vec![net_parameters(13, pt(5.0, 0.0), "SIGNAL")]);
    b.no_erc_directives
        .push(NoErcDirective::new(Uuid::from_u128(99), pt(5.0, 0.0)));
    let a = vbus_sheet(1, "R1", vec![net_parameters(12, pt(5.0, 0.0), "POWER")]);
    let found = check(vec![("a", a), ("b", b)]);
    assert!(found.findings.is_empty(), "{:?}", found.findings);
    assert_eq!(found.covered.len(), 2, "still judged");
}

/// #2961: the project answer replaces a sheet's own `NetClassConflict`
/// finding only on a directive it judged and a net it carries; a finding on
/// another net, or of another rule, the sheet keeps.
#[test]
fn the_project_answer_replaces_a_sheet_finding_on_a_judged_directive_and_carried_net() {
    let found = check(vec![(
        "a",
        vbus_sheet(
            1,
            "R1",
            vec![
                net_blanket(11, pt(-1.0, -1.0), pt(11.0, 1.0), "SIGNAL"),
                net_parameters(12, pt(5.0, 0.0), "POWER"),
            ],
        ),
    )]);
    let (_, project) = only(&found);
    let a = SheetKey::new("a");
    let on_net = |net: &str| Violation {
        tag: Some(format!("net:{net}")),
        ..project.clone()
    };
    assert!(found.replaces(&a, &on_net("VBUS")));
    assert!(
        !found.replaces(&a, &on_net("OTHER")),
        "a net it does not carry"
    );
    assert!(
        !found.replaces(&SheetKey::new("b"), &on_net("VBUS")),
        "a sheet it did not judge"
    );
    let other_rule = Violation {
        rule: RuleKind::NetMultipleNames,
        rule_id: crate::RuleId::builtin(RuleKind::NetMultipleNames),
        ..on_net("VBUS")
    };
    assert!(!found.replaces(&a, &other_rule), "another rule");
}
