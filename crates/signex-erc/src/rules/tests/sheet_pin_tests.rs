//! #1836: sheet-pin rules, one positive and one negative case each, and the
//! unexposed child port the cross-sheet rule must keep reporting.

use super::*;
use crate::context::{ErcBus, ErcChildSheet, ErcSheetPin};
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
        connected: false,
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
fn a_sheet_pin_on_a_wire_end_or_interior_or_a_bus_end_is_connected() {
    let mut c = with_sheets(
        vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))],
        vec![sheet(
            "S",
            vec![
                pin("END", pt(0.0, 0.0), "input"),
                pin("MID", pt(5.0, 0.0), "input"),
                pin("BUS", pt(0.0, 20.0), "input"),
            ],
        )],
    );
    c.buses.push(ErcBus {
        uuid: Uuid::now_v7(),
        start: pt(0.0, 20.0),
        end: pt(10.0, 20.0),
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

// ---- connectivity contract: labels and junctions on a pin ------------------

/// A snapshot with one sheet symbol `S` whose pin `P` sits at (5, 5), plus
/// whatever `extra` adds, run through the real projection.
fn pin_sheet(extra: impl FnOnce(&mut signex_types::schematic::SchematicSheet)) -> ErcContext {
    use signex_types::schematic::{ChildSheet, FillType, SchematicSheet, SheetPin};
    let mut sheet = SchematicSheet {
        child_sheets: vec![ChildSheet {
            uuid: Uuid::now_v7(),
            name: "S".into(),
            filename: "s.snxsch".into(),
            position: Point::new(5.0, 0.0),
            size: (10.0, 10.0),
            stroke_width: 0.0,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: vec![SheetPin {
                uuid: Uuid::now_v7(),
                name: "P".into(),
                direction: "input".into(),
                position: Point::new(5.0, 5.0),
                rotation: 0.0,
                auto_generated: false,
                user_moved: false,
            }],
            instances: Vec::new(),
        }],
        ..Default::default()
    };
    extra(&mut sheet);
    ErcContext::from_snapshot(&sheet)
}

fn schematic_label(text: &str, kind: LabelType, at: Point) -> signex_types::schematic::Label {
    use signex_types::schematic::{HAlign, Label, VAlign};
    Label {
        unknown_columns: Default::default(),
        uuid: Uuid::now_v7(),
        text: text.into(),
        position: at,
        rotation: 0.0,
        label_type: kind,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked: false,
    }
}

#[test]
fn a_label_or_junction_on_the_pin_connects_it_but_a_bare_pin_does_not() {
    let bare = pin_sheet(|_| {});
    assert_eq!(found(&bare, RuleKind::SheetPinUnconnected).len(), 1);
    for kind in [LabelType::Net, LabelType::Global, LabelType::Hierarchical] {
        let c = pin_sheet(|s| {
            s.labels
                .push(schematic_label("N", kind, Point::new(5.0, 5.0)))
        });
        assert!(
            found(&c, RuleKind::SheetPinUnconnected).is_empty(),
            "{kind:?} label on the pin"
        );
    }
    let c = pin_sheet(|s| {
        s.junctions.push(signex_types::schematic::Junction {
            uuid: Uuid::now_v7(),
            position: Point::new(5.0, 5.0),
            diameter: 0.0,
            ..Default::default()
        })
    });
    assert!(found(&c, RuleKind::SheetPinUnconnected).is_empty());
    let c = pin_sheet(|s| {
        s.no_connects.push(signex_types::schematic::NoConnect {
            uuid: Uuid::now_v7(),
            position: Point::new(5.0, 5.0),
        })
    });
    assert!(found(&c, RuleKind::SheetPinUnconnected).is_empty());
}

#[test]
fn a_pin_in_the_middle_of_a_bus_is_not_connected_but_a_bus_end_is() {
    let mut mid = pin_sheet(|_| {});
    mid.buses.push(ErcBus {
        uuid: Uuid::now_v7(),
        start: pt(0.0, 5.0),
        end: pt(10.0, 5.0),
    });
    assert_eq!(found(&mid, RuleKind::SheetPinUnconnected).len(), 1);
    let mut end = pin_sheet(|_| {});
    end.buses.push(ErcBus {
        uuid: Uuid::now_v7(),
        start: pt(5.0, 5.0),
        end: pt(10.0, 5.0),
    });
    assert!(found(&end, RuleKind::SheetPinUnconnected).is_empty());
}

// ---- waiver identity of the unexposed-port finding ------------------------

/// A parent placing `child.snxsch` once per entry of `symbols`, and the child
/// holding Hierarchical `INX` (uuid `label`), run through the full engine.
fn unexposed(symbols: &[&str], label: Uuid) -> Vec<Diagnostic> {
    let mut child = ctx(
        vec![],
        vec![],
        vec![hier_label("INX", pt(1.0, 1.0))],
        vec![],
    );
    child.labels[0].uuid = label;
    let mut parent = with_sheets(vec![], symbols.iter().map(|n| sheet(n, vec![])).collect());
    parent
        .children
        .insert("child.snxsch".into(), std::sync::Arc::new(child));
    found(&parent, RuleKind::BadHierSheetPin)
        .into_iter()
        .filter(|d| d.message.contains("is not exposed"))
        .collect()
}

fn child_sheet_with_label(label: Uuid) -> signex_types::schematic::SchematicSheet {
    let mut l = schematic_label("INX", LabelType::Hierarchical, Point::new(1.0, 1.0));
    l.uuid = label;
    signex_types::schematic::SchematicSheet {
        labels: vec![l],
        ..Default::default()
    }
}

#[test]
fn a_single_placement_keeps_the_waiver_identity_it_had_before() {
    use crate::Violation;
    use crate::waivers::violation::{new_waiver, split_waived, stamp_keys, waiver_key};
    let label = Uuid::now_v7();
    let sheet = child_sheet_with_label(label);
    let diagnostics = unexposed(&["a"], label);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].tag, None, "one placement stays untagged");

    // The identity before the finding was ever tagged: rule id + the label.
    let mut old = Violation::from(diagnostics[0].clone());
    old.tag = None;
    let key = waiver_key(&old, &sheet).expect("waivable");
    assert_eq!(key.objects, [label.to_string()]);
    let waiver = new_waiver(&key, &old.message, "by design", "me", "2026-11-01").unwrap();

    let mut list = vec![Violation::from(diagnostics[0].clone())];
    stamp_keys(&mut list, &sheet);
    assert!(split_waived(list, &[waiver]).kept.is_empty());
}

#[test]
fn two_placements_get_distinct_waiver_identities() {
    use crate::Violation;
    use crate::waivers::violation::waiver_key;
    let label = Uuid::now_v7();
    let sheet = child_sheet_with_label(label);
    let diagnostics = unexposed(&["a", "b"], label);
    assert_eq!(diagnostics.len(), 2);
    let keys: Vec<_> = diagnostics
        .into_iter()
        .map(|d| waiver_key(&Violation::from(d), &sheet).expect("waivable"))
        .collect();
    assert_ne!(keys[0].objects, keys[1].objects);
}
