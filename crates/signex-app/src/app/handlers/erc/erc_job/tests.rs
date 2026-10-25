//! The project-wide `NetMultipleNames` pass of the ERC job (#1061): which
//! per-sheet findings it replaces, where its findings are filed, and how the
//! severity overrides apply to them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_erc::{RuleId, RuleKind, Severity, Violation};
use signex_types::schematic::{
    ChildSheet, FillType, HAlign, Label, LabelType, Point, SchematicSheet, SheetPin, VAlign, Wire,
};
use uuid::Uuid;

use super::{ViolationsByPath, add_project_net_names};
use crate::app::project_sheets::project_graph;
use crate::app::state::scope::CaseFold;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn label(text: &str, at: Point, label_type: LabelType) -> Label {
    Label {
        unknown_columns: Default::default(),
        uuid: Uuid::new_v4(),
        text: text.to_string(),
        position: at,
        rotation: 0.0,
        label_type,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
    }
}

/// A pin-less wire `(0,0)-(10,0)` carrying the given labels at its two ends.
fn named_wire(left: (&str, LabelType), right: (&str, LabelType)) -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet.wires.push(Wire {
        start: pt(0.0, 0.0),
        end: pt(10.0, 0.0),
        ..Default::default()
    });
    sheet.labels.push(label(left.0, pt(0.0, 0.0), left.1));
    sheet.labels.push(label(right.0, pt(10.0, 0.0), right.1));
    sheet
}

fn child_with_pin(filename: &str, pin: &str) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::nil(),
        name: "child".to_string(),
        filename: filename.to_string(),
        position: pt(0.0, 0.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: Uuid::nil(),
            name: pin.to_string(),
            direction: String::new(),
            position: pt(0.0, 0.0),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: Vec::new(),
    }
}

fn finding(rule: RuleKind, severity: Severity, message: &str) -> Violation {
    Violation::new(RuleId::builtin(rule), rule, severity, message, pt(0.0, 0.0))
}

fn multi_names(list: &[Violation]) -> Vec<&Violation> {
    list.iter()
        .filter(|v| v.builtin_kind() == Some(RuleKind::NetMultipleNames))
        .collect()
}

struct Fixture {
    dir: PathBuf,
    graph: crate::app::project_sheets::AssembledGraph,
    by_path: ViolationsByPath,
}

impl Fixture {
    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn run(&mut self, roots: &[&str], overrides: &HashMap<RuleKind, Severity>) {
        let roots: Vec<signex_net::ProjectRoot> = roots
            .iter()
            .map(|r| signex_net::ProjectRoot {
                key: signex_net::SheetKey::new(*r),
            })
            .collect();
        add_project_net_names(
            &mut self.by_path,
            &self.graph,
            &roots,
            Some(&self.dir),
            CaseFold::host_default(),
            overrides,
        );
    }
}

/// `root.snxsch` (Global `G`, sheet pin `SDA` to `child.snxsch`), a child that
/// calls its side Hierarchical `SDA`, and `lone.snxsch`, which the root never
/// reaches. Each sheet starts with a per-sheet `NetMultipleNames` finding and
/// an Info finding of another rule.
fn fixture() -> Fixture {
    let dir = Path::new("/proj").to_path_buf();
    let mut root = named_wire(("G", LabelType::Global), ("G", LabelType::Global));
    root.child_sheets
        .push(child_with_pin("child.snxsch", "SDA"));
    let child = named_wire(
        ("SDA", LabelType::Hierarchical),
        ("SDA", LabelType::Hierarchical),
    );
    let lone = named_wire(
        ("A", LabelType::Hierarchical),
        ("B", LabelType::Hierarchical),
    );
    let sheets: HashMap<PathBuf, SchematicSheet> = [
        (dir.join("root.snxsch"), root),
        (dir.join("child.snxsch"), child),
        (dir.join("lone.snxsch"), lone),
    ]
    .into_iter()
    .collect();
    let graph = project_graph(&sheets, Some(&dir), Some(&dir.join("root.snxsch")));
    let by_path = sheets
        .keys()
        .map(|p| {
            (
                p.clone(),
                vec![
                    finding(RuleKind::UnusedPin, Severity::Info, "other"),
                    finding(RuleKind::NetMultipleNames, Severity::Warning, "per-sheet"),
                ],
            )
        })
        .collect();
    Fixture {
        dir,
        graph,
        by_path,
    }
}

#[test]
fn a_sheet_outside_the_roots_keeps_its_own_findings() {
    let mut f = fixture();
    f.run(&["root.snxsch"], &HashMap::new());

    let lone = &f.by_path[&f.path("lone.snxsch")];
    let kept = multi_names(lone);
    assert_eq!(kept.len(), 1, "{lone:?}");
    assert_eq!(kept[0].message, "per-sheet");
}

#[test]
fn a_reached_sheet_trades_its_findings_for_the_project_ones_filed_under_the_winner() {
    let mut f = fixture();
    // `G` lands on the root wire; the child's own `SDA` labels are one name.
    // Make the root wire carry a second name so the net has two.
    let key = signex_net::SheetKey::new("root.snxsch");
    f.graph.sheets.get_mut(&key).unwrap().labels.push(label(
        "H",
        pt(5.0, 0.0),
        LabelType::Hierarchical,
    ));
    f.run(&["root.snxsch"], &HashMap::new());

    let root = multi_names(&f.by_path[&f.path("root.snxsch")]);
    assert_eq!(root.len(), 1, "{root:?}");
    assert!(root[0].message.starts_with("Net has multiple names"));
    assert!(root[0].message.contains("the netlist uses 'G'"));
    assert!(
        multi_names(&f.by_path[&f.path("child.snxsch")]).is_empty(),
        "the child's per-sheet finding is replaced, not duplicated"
    );
}

#[test]
fn the_appended_findings_are_sorted_in() {
    let mut f = fixture();
    f.run(&["root.snxsch"], &HashMap::new());

    // The appended finding sorts ahead of the UnusedPin finding that was first.
    let root = &f.by_path[&f.path("root.snxsch")];
    assert_eq!(root[0].builtin_kind(), Some(RuleKind::NetMultipleNames));
    assert_eq!(root[1].builtin_kind(), Some(RuleKind::UnusedPin));
}

#[test]
fn a_severity_override_applies_to_project_findings() {
    let mut f = fixture();
    let off = HashMap::from([(RuleKind::NetMultipleNames, Severity::Off)]);
    f.run(&["root.snxsch"], &off);
    assert!(multi_names(&f.by_path[&f.path("root.snxsch")]).is_empty());

    let mut f = fixture();
    let error = HashMap::from([(RuleKind::NetMultipleNames, Severity::Error)]);
    f.run(&["root.snxsch"], &error);
    let root = &f.by_path[&f.path("root.snxsch")];
    assert!(
        multi_names(root)
            .iter()
            .all(|v| v.severity == Severity::Error)
    );
    assert_eq!(root[0].severity, Severity::Error, "errors sort first");
}

#[test]
fn no_roots_leaves_every_per_sheet_finding_alone() {
    let mut f = fixture();
    f.run(&[], &HashMap::new());
    for list in f.by_path.values() {
        assert_eq!(multi_names(list).len(), 1);
    }
}
