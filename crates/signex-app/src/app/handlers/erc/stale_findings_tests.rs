//! #1768: a finding about an object the edit removed must leave the canvas and
//! the ERC panel, without a new ERC run.

use std::path::PathBuf;

use signex_erc::{RuleId, RuleKind, Severity, Violation};
use signex_types::schematic::{Point, SelectedItem, SelectedKind};

use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};
use crate::app::{Signex, TabInfo, TabKind};

fn finding(at: Point, primary: Option<SelectedItem>) -> Violation {
    let mut v = Violation::new(
        RuleId::builtin(RuleKind::UnusedPin),
        RuleKind::UnusedPin,
        Severity::Warning,
        "finding",
        at,
    );
    v.primary = primary;
    v
}

/// One sheet holding the part `R_X`, with one finding on the part and one on
/// nothing in particular (a sheet-level finding), both already filed.
fn app_with_two_findings() -> (Signex, PathBuf, SelectedItem) {
    let dir = PathBuf::from("/w/stale");
    let path = dir.join("top.snxsch");
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    let sheet = sheet_with_net("R_X", "NET_X", &[]);
    let part = SelectedItem::new(sheet.symbols[0].uuid, SelectedKind::Symbol);
    open_with(&mut app.document_state, &path, sheet);
    app.document_state.tabs.push(TabInfo {
        title: "top".to_owned(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.ui_state.erc_violations_by_path.insert(
        path.clone(),
        vec![
            finding(Point::new(0.0, 0.0), Some(part)),
            finding(Point::new(5.0, 5.0), None),
        ],
    );
    app.refresh_active_erc_from_cache(Some(&path));
    (app, path, part)
}

#[test]
fn deleting_the_flagged_part_drops_its_finding_everywhere_but_keeps_the_rest() {
    let (mut app, path, part) = app_with_two_findings();
    assert_eq!(app.interaction_state.active_canvas().erc_markers.len(), 2);

    app.apply_engine_command(
        signex_engine::Command::DeleteSelection { items: vec![part] },
        true,
        true,
    );

    let markers = &app.interaction_state.active_canvas().erc_markers;
    assert_eq!(markers.len(), 1, "only the sheet-level marker is left");
    assert_eq!((markers[0].x, markers[0].y), (5.0, 5.0));
    assert_eq!(app.ui_state.erc_violations.len(), 1);
    assert_eq!(app.ui_state.erc_violations_by_path[&path].len(), 1);
    assert_eq!(app.build_erc_diagnostic_entries().len(), 1, "panel agrees");
}

#[test]
fn an_edit_that_leaves_the_flagged_part_keeps_every_finding() {
    let (mut app, path, _part) = app_with_two_findings();
    let wire = app.document_state.active_engine().unwrap().document().wires[0].uuid;

    app.apply_engine_command(
        signex_engine::Command::DeleteSelection {
            items: vec![SelectedItem::new(wire, SelectedKind::Wire)],
        },
        true,
        true,
    );

    assert_eq!(app.interaction_state.active_canvas().erc_markers.len(), 2);
    assert_eq!(app.ui_state.erc_violations_by_path[&path].len(), 2);
}

#[test]
fn the_bottom_dock_opens_tall_enough_for_several_finding_rows() {
    let (app, _task) = Signex::new();
    assert!(app.ui_state.bottom_height >= 200.0);
}
