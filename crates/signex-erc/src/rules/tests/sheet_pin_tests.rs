//! #1836: sheet-pin rules, one positive and one negative case each, and the
//! unexposed child port the cross-sheet rule must keep reporting.

use super::*;
use crate::context::{ErcBus, ErcChildSheet, ErcNoConnect, ErcSheetPin};
use crate::diagnostic::Diagnostic;

fn found(c: &ErcContext, kind: RuleKind) -> Vec<Diagnostic> {
    crate::engine::run_all(c)
        .into_iter()
        .filter(|d| d.rule_kind == kind)
        .collect()
}

fn pin(name: &str, at: Point, direction: &str) -> ErcSheetPin {
    ErcSheetPin {
        uuid: Uuid::now_v7(),
        name: name.into(),
        position: at,
        direction: direction.into(),
    }
}

fn sheet(name: &str, pins: Vec<ErcSheetPin>) -> ErcChildSheet {
    ErcChildSheet {
        uuid: Uuid::now_v7(),
        name: name.into(),
        filename: "child.snxsch".into(),
        position: pt(0.0, 0.0),
        pins,
    }
}

fn with_sheets(wires: Vec<ErcWire>, sheets: Vec<ErcChildSheet>) -> ErcContext {
    let mut c = ctx(wires, vec![], vec![], vec![]);
    c.child_sheets = sheets;
    c
}

#[test]
fn a_sheet_pin_with_nothing_on_it_is_reported_with_the_floating_port() {
    let c = with_sheets(
        vec![],
        vec![sheet("Driver", vec![pin("IN", pt(5.0, 5.0), "input")])],
    );
    let hits = found(&c, RuleKind::SheetPinUnconnected);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(hits[0].message.contains("'IN'") && hits[0].message.contains("Driver"));
    assert_eq!(hits[0].severity, Severity::Warning);
}

#[test]
fn a_sheet_pin_on_a_wire_end_or_interior_a_bus_or_a_no_connect_is_connected() {
    let mut c = with_sheets(
        vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))],
        vec![sheet(
            "S",
            vec![
                pin("END", pt(0.0, 0.0), "input"),
                pin("MID", pt(5.0, 0.0), "input"),
                pin("BUS", pt(0.0, 20.0), "input"),
                pin("NC", pt(30.0, 30.0), "input"),
            ],
        )],
    );
    c.buses.push(ErcBus {
        uuid: Uuid::now_v7(),
        start: pt(0.0, 20.0),
        end: pt(10.0, 20.0),
    });
    c.no_connects.push(ErcNoConnect {
        position: pt(30.0, 30.0),
    });
    assert!(found(&c, RuleKind::SheetPinUnconnected).is_empty());
}

#[test]
fn two_output_sheet_pins_on_one_net_conflict_and_name_each_other() {
    let w = vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))];
    let c = with_sheets(
        w,
        vec![
            sheet("A", vec![pin("OUT", pt(0.0, 0.0), "output")]),
            sheet("B", vec![pin("OUT", pt(10.0, 0.0), "Output")]),
        ],
    );
    let hits = found(&c, RuleKind::SheetPinDirectionConflict);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(hits[0].primary.is_some() && hits[0].peer.is_some());
    assert_eq!(hits[0].severity, Severity::Warning);
}

#[test]
fn an_output_with_an_input_or_outputs_on_separate_nets_is_fine() {
    let two_nets = vec![
        wire(pt(0.0, 0.0), pt(10.0, 0.0)),
        wire(pt(0.0, 20.0), pt(10.0, 20.0)),
    ];
    let c = with_sheets(
        two_nets.clone(),
        vec![
            sheet("A", vec![pin("OUT", pt(0.0, 0.0), "output")]),
            sheet("B", vec![pin("OUT", pt(0.0, 20.0), "output")]),
        ],
    );
    assert!(found(&c, RuleKind::SheetPinDirectionConflict).is_empty());
    let c = with_sheets(
        vec![two_nets[0]],
        vec![
            sheet("A", vec![pin("OUT", pt(0.0, 0.0), "output")]),
            sheet("B", vec![pin("IN", pt(10.0, 0.0), "input")]),
        ],
    );
    assert!(found(&c, RuleKind::SheetPinDirectionConflict).is_empty());
}

#[test]
fn outputs_joined_by_a_global_label_conflict() {
    let mut c = with_sheets(
        vec![
            wire(pt(0.0, 0.0), pt(10.0, 0.0)),
            wire(pt(0.0, 20.0), pt(10.0, 20.0)),
        ],
        vec![
            sheet("A", vec![pin("OUT", pt(0.0, 0.0), "output")]),
            sheet("B", vec![pin("OUT", pt(0.0, 20.0), "output")]),
        ],
    );
    c.labels.push(global_label("SIG", pt(10.0, 0.0)));
    c.labels.push(global_label("SIG", pt(10.0, 20.0)));
    assert_eq!(found(&c, RuleKind::SheetPinDirectionConflict).len(), 1);
}

#[test]
fn a_child_port_the_parent_does_not_expose_is_reported_in_the_child() {
    let mut child = ctx(
        vec![],
        vec![],
        vec![hier_label("INX", pt(1.0, 1.0))],
        vec![],
    );
    child.key = Some(signex_net::SheetKey::new("child"));
    let mut parent = with_sheets(vec![], vec![sheet("S", vec![pin("IN", pt(0.0, 0.0), "")])]);
    parent
        .children
        .insert("child.snxsch".into(), std::sync::Arc::new(child));
    let hits: Vec<_> = found(&parent, RuleKind::BadHierSheetPin)
        .into_iter()
        .filter(|d| d.message.contains("is not exposed"))
        .collect();
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].child_sheet.as_deref(), Some("child.snxsch"));
}
