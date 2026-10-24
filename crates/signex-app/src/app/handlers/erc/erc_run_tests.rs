//! A child-sheet ERC finding carries child coordinates; it must be filed under
//! the child sheet, not drawn or panned to on the parent's canvas (#956).

use std::path::PathBuf;

use signex_types::format::SnxSchematic;
use signex_types::schematic::{HAlign, Label, LabelType, Point, VAlign};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};

const CHILD_AT: (f64, f64) = (150.0, 120.0);

/// `top.snxsch` is open and active with a sheet symbol for `child.snxsch`
/// (no pins); the on-disk child has a hierarchical label the parent does not
/// expose, at `CHILD_AT`.
fn parent_with_a_far_child_label() -> (Signex, PathBuf) {
    let dir = std::env::temp_dir().join(format!("signex-erc-child-space-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    let mut child = sheet_with_net("R_CHILD", "CHILD_NET", &[]);
    child.labels.push(Label {
        unknown_columns: Default::default(),
        uuid: Uuid::new_v4(),
        text: "ORPHAN".to_string(),
        position: Point::new(CHILD_AT.0, CHILD_AT.1),
        rotation: 0.0,
        label_type: LabelType::Hierarchical,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::default(),
        justify_v: VAlign::default(),
    });
    let text = SnxSchematic::new(child).write_string().expect("serialize");
    std::fs::write(dir.join("child.snxsch"), text).expect("write child");

    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    let top = dir.join("top.snxsch");
    let mut parent = sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]);
    parent.child_sheets[0].position = Point::new(10.0, 10.0);
    parent.child_sheets[0].size = (30.0, 30.0);
    open_with(&mut app.document_state, &top, parent);
    app.document_state.active_path = Some(top);
    (app, dir)
}

fn at_child_coordinate(x: f64, y: f64) -> bool {
    (x - CHILD_AT.0).abs() < 1e-6 && (y - CHILD_AT.1).abs() < 1e-6
}

#[test]
fn a_child_label_finding_draws_no_marker_on_the_parent_canvas() {
    let (mut app, dir) = parent_with_a_far_child_label();
    app.run_erc_blocking();

    let top = dir.join("top.snxsch");
    let parent_hit = app
        .ui_state
        .erc_violations_by_path
        .get(&top)
        .is_some_and(|v| {
            v.iter()
                .any(|v| at_child_coordinate(v.location.x, v.location.y))
        });
    let marker_hit = app
        .interaction_state
        .active_canvas()
        .erc_markers
        .iter()
        .any(|m| at_child_coordinate(m.x, m.y));
    std::fs::remove_dir_all(&dir).ok();
    assert!(!parent_hit, "child coordinate filed under the parent sheet");
    assert!(
        !marker_hit,
        "marker drawn on the parent canvas at child coordinates"
    );
}

#[test]
fn a_child_label_finding_is_filed_and_targeted_on_the_child_sheet() {
    let (mut app, dir) = parent_with_a_far_child_label();
    app.run_erc_blocking();

    let child = dir.join("child.snxsch");
    let filed = app.ui_state.erc_violations_by_path.get(&child).map(|v| {
        v.iter()
            .filter(|v| v.builtin_kind() == Some(signex_erc::RuleKind::BadHierSheetPin))
            .filter(|v| at_child_coordinate(v.location.x, v.location.y))
            .map(|v| v.location_space)
            .collect::<Vec<_>>()
    });
    let entries = app.build_erc_diagnostic_entries();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(filed, Some(vec![signex_erc::LocationSpace::ThisSheet]));
    let entry = entries
        .iter()
        .find(|e| {
            at_child_coordinate(e.world_x, e.world_y)
                && e.rule_kind == Some(signex_erc::RuleKind::BadHierSheetPin)
        })
        .expect("panel entry for the child label");
    assert_eq!(entry.sheet_path, child);
    assert!(entry.locatable);
}

#[test]
fn a_child_finding_whose_sheet_was_not_checked_is_reported_but_not_panned_to() {
    use signex_erc::{LocationSpace, RuleId, RuleKind, Severity, Violation};
    let top = PathBuf::from("/w/top.snxsch");
    let (mut app, _task) = Signex::new();
    let mut v = Violation::new(
        RuleId::builtin(RuleKind::BadHierSheetPin),
        RuleKind::BadHierSheetPin,
        Severity::Warning,
        "orphan",
        Point::new(CHILD_AT.0, CHILD_AT.1),
    );
    v.location_space = LocationSpace::ChildSheet;
    v.child_sheet = Some("gone.snxsch".to_string());
    app.ui_state
        .erc_violations_by_path
        .insert(top.clone(), vec![v]);

    app.refresh_active_erc_from_cache(Some(&top));
    let entries = app.build_erc_diagnostic_entries();

    assert!(app.interaction_state.active_canvas().erc_markers.is_empty());
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].locatable);
}
