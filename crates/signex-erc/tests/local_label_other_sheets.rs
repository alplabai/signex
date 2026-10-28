//! `LocalLabelNameOnOtherSheets`: a local net label whose name is also a local
//! net label on another sheet is a separate net, and the user is told so.

use std::collections::HashMap;

use signex_erc::{RuleKind, Violation, run_with_project};
use signex_net::SheetKey;
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet};
use uuid::Uuid;

type Resolutions = HashMap<SheetKey, HashMap<String, SheetKey>>;

fn label(text: &str, label_type: LabelType) -> Label {
    Label {
        uuid: Uuid::new_v4(),
        text: text.to_string(),
        position: Point { x: 10.0, y: 10.0 },
        rotation: 0.0,
        label_type,
        shape: String::new(),
        font_size: 0.0,
        font_family: None,
        justify: Default::default(),
        justify_v: signex_types::schematic::VAlign::Bottom,
        unknown_columns: Default::default(),
    }
}

fn sheet(labels: &[(&str, LabelType)]) -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".to_string(),
        labels: labels.iter().map(|(t, ty)| label(t, *ty)).collect(),
        ..Default::default()
    }
}

/// `(sheet key, labels, [(child filename, child key)])`; first entry is the
/// top sheet. Returns the rule's findings for the top sheet's run.
type Spec<'a> = [(&'a str, Vec<(&'a str, LabelType)>, Vec<(&'a str, &'a str)>)];

fn run_top(spec: &Spec) -> Vec<Violation> {
    let sheets: HashMap<SheetKey, SchematicSheet> = spec
        .iter()
        .map(|(k, labels, _)| (SheetKey::new(*k), sheet(labels)))
        .collect();
    let resolutions: Resolutions = spec
        .iter()
        .map(|(k, _, kids)| {
            (
                SheetKey::new(*k),
                kids.iter()
                    .map(|(f, c)| ((*f).to_string(), SheetKey::new(*c)))
                    .collect(),
            )
        })
        .collect();
    let top = SheetKey::new(spec[0].0);
    run_with_project(&sheets[&top], Some(&top), &resolutions, &sheets)
        .into_iter()
        .filter(|v| v.rule == RuleKind::LocalLabelNameOnOtherSheets)
        .collect()
}

use LabelType::{Global, Net};

#[test]
fn fires_for_local_sda_on_two_sheets() {
    let found = run_top(&[
        ("top", vec![("SDA", Net)], vec![("a.snxsch", "a")]),
        ("a", vec![("SDA", Net)], vec![]),
    ]);
    assert_eq!(found.len(), 1, "{found:?}");
    let m = &found[0].message;
    assert!(
        m.contains("'SDA'") && m.contains("this sheet") && m.contains("a.snxsch"),
        "{m}"
    );
    assert!(m.contains("Use a Port or a sheet entry"), "{m}");
}

#[test]
fn silent_when_a_port_carries_the_name() {
    let found = run_top(&[
        ("top", vec![("SDA", Net)], vec![("a.snxsch", "a")]),
        ("a", vec![("SDA", Net), ("SDA", Global)], vec![]),
    ]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn silent_for_two_local_labels_on_one_sheet() {
    let found = run_top(&[
        (
            "top",
            vec![("SDA", Net), ("SDA", Net)],
            vec![("a.snxsch", "a")],
        ),
        ("a", vec![("SCL", Net)], vec![]),
    ]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn silent_for_a_single_sheet() {
    let found = run_top(&[("top", vec![("SDA", Net)], vec![])]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_child_placed_twice_does_not_fire_against_itself() {
    let found = run_top(&[
        ("top", vec![], vec![("a1.snxsch", "a"), ("a2.snxsch", "a")]),
        ("a", vec![("SDA", Net)], vec![]),
    ]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn clash_between_grandchildren_is_reported_once_by_the_lowest_sheet() {
    let spec = [
        ("top", vec![], vec![("a.snxsch", "a")]),
        ("a", vec![], vec![("b.snxsch", "b"), ("c.snxsch", "c")]),
        ("b", vec![("SDA", Net)], vec![]),
        ("c", vec![("SDA", Net)], vec![]),
    ];
    assert!(run_top(&spec).is_empty(), "top must stay quiet");
}

/// Parent wire carries Net `SDA` and ends on sheet pin `SDA`; the child has one
/// wire carrying Hierarchical `SDA`. The netlist joins them through the sheet
/// entry, so the rule must not warn.
fn sheet_entry_findings(child_labels: &[(&str, LabelType)]) -> Vec<Violation> {
    use signex_types::schematic::{ChildSheet, SheetPin, Wire};
    let wire = |start, end| Wire {
        uuid: Uuid::new_v4(),
        start,
        end,
        stroke_width: 0.0,
        unknown_columns: Default::default(),
    };
    let pin_at = Point { x: 20.0, y: 10.0 };
    let mut top = sheet(&[("SDA", Net)]);
    top.wires.push(wire(Point { x: 10.0, y: 10.0 }, pin_at));
    top.child_sheets.push(ChildSheet {
        uuid: Uuid::new_v4(),
        name: "A".to_string(),
        filename: "a.snxsch".to_string(),
        position: Point { x: 20.0, y: 5.0 },
        size: (20.0, 10.0),
        stroke_width: 0.0,
        fill: Default::default(),
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: Uuid::new_v4(),
            name: "SDA".to_string(),
            direction: String::new(),
            position: pin_at,
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: Vec::new(),
    });
    let mut child = sheet(child_labels);
    child
        .wires
        .push(wire(Point { x: 0.0, y: 10.0 }, Point { x: 10.0, y: 10.0 }));
    let (tk, ck) = (SheetKey::new("top"), SheetKey::new("a"));
    let sheets: HashMap<SheetKey, SchematicSheet> = [(tk.clone(), top), (ck.clone(), child)]
        .into_iter()
        .collect();
    let resolutions: Resolutions = [
        (
            tk.clone(),
            [("a.snxsch".to_string(), ck.clone())].into_iter().collect(),
        ),
        (ck, HashMap::new()),
    ]
    .into_iter()
    .collect();
    let found = run_with_project(&sheets[&tk], Some(&tk), &resolutions, &sheets);
    found
        .into_iter()
        .filter(|v| v.rule == RuleKind::LocalLabelNameOnOtherSheets)
        .collect()
}

/// The exact repro: Hierarchical `SDA` alone in the child.
#[test]
fn silent_when_a_sheet_entry_joins_net_label_to_hierarchical_label() {
    let found = sheet_entry_findings(&[("SDA", LabelType::Hierarchical)]);
    assert!(found.is_empty(), "{found:?}");
}

/// The child wire also carries a local Net `SDA` next to the Hierarchical one.
/// The sheet entry joins it to the parent's `SDA`, so it is one net, not two.
#[test]
fn silent_when_the_child_wire_has_hierarchical_and_net_label() {
    let found = sheet_entry_findings(&[("SDA", LabelType::Hierarchical), ("SDA", Net)]);
    assert!(found.is_empty(), "{found:?}");
}

/// Without a sheet-entry join the same two Net labels still warn.
#[test]
fn fires_when_child_net_label_has_no_hierarchical_partner() {
    let found = sheet_entry_findings(&[("SDA", Net)]);
    assert_eq!(found.len(), 1, "{found:?}");
}
