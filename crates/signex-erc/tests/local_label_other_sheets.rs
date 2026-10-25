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
