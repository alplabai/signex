//! #1768: a finding about an object the edit removed must leave the canvas and
//! the ERC panel, without a new ERC run.

use std::path::PathBuf;

use signex_erc::{RuleId, RuleKind, Severity, Violation};
use signex_types::schematic::{Point, SelectedItem, SelectedKind};

use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};
use crate::app::{EditMsg, Message, Signex, TabInfo, TabKind};
use crate::dock::DockMessage;

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
    assert_eq!(
        app.ui_state.erc_violations_by_path[&path].len(),
        2,
        "cache intact"
    );
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

// ---- regression probes for the read-time filter ----

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

fn delete_part(app: &mut Signex, part: SelectedItem) {
    app.apply_engine_command(
        signex_engine::Command::DeleteSelection { items: vec![part] },
        true,
        true,
    );
}

fn report_rows(app: &Signex) -> usize {
    app.erc_report_options(signex_output::ErcReportFormat::Text)
        .violations
        .expect("ERC has run")
        .len()
}

#[test]
fn delete_then_undo_restores_markers_panel_and_report_and_marks_stale() {
    let (mut app, _path, part) = app_with_two_findings();
    delete_part(&mut app, part);
    assert_eq!(report_rows(&app), 1);

    app.ui_state.erc_epoch = app.document_state.sheet_edit_epoch;
    undo(&mut app);

    assert_eq!(app.interaction_state.active_canvas().erc_markers.len(), 2);
    assert_eq!(app.build_erc_diagnostic_entries().len(), 2);
    assert_eq!(report_rows(&app), 2);
    assert!(app.erc_results_stale(), "an undo is an edit too");
}

#[test]
fn a_clear_resets_the_stale_flag() {
    let (mut app, _path, part) = app_with_two_findings();
    delete_part(&mut app, part);
    assert!(app.erc_results_stale());
    let _ = app.update(Message::Dock(DockMessage::Panel(
        crate::panels::PanelMsg::ClearErc,
    )));
    assert!(!app.erc_results_stale());
}

#[test]
fn an_edit_while_a_run_is_in_flight_keeps_the_notice() {
    let (mut app, _path, part) = app_with_two_findings();
    let (generation, inputs) = app.begin_erc_run();
    assert!(
        !app.erc_results_stale(),
        "nothing changed since the results"
    );

    delete_part(&mut app, part);
    let outcome = Ok(super::erc_job::run_erc_job(inputs));
    app.apply_erc_run_result(generation, &outcome);

    assert!(
        app.erc_results_stale(),
        "the run read the sheet before the edit"
    );
}

#[test]
fn a_failed_run_restores_the_flag_it_found() {
    let (mut app, _path, part) = app_with_two_findings();
    delete_part(&mut app, part);
    let (generation, _inputs) = app.begin_erc_run();
    assert!(
        app.erc_results_stale(),
        "while it runs the old results are still on show"
    );

    app.apply_erc_run_result(generation, &Err("worker failed".to_string()));

    assert!(
        app.erc_results_stale(),
        "the old results are still the old results"
    );
}

#[test]
fn navigation_and_quick_fix_use_the_right_row_when_an_earlier_one_is_hidden() {
    let (mut app, _path, part) = app_with_two_findings();
    delete_part(&mut app, part);
    let entries = app.build_erc_diagnostic_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].global_index, 1,
        "stays the finding's place in the cache"
    );

    let _ = app.handle_focus_erc_diagnostic_offset(1);
    assert_eq!(app.ui_state.erc_focus_global_index, Some(1));

    let no_connects = |app: &Signex| {
        app.document_state
            .active_engine()
            .unwrap()
            .document()
            .no_connects
            .len()
    };
    let before = no_connects(&app);
    let _ = app.handle_erc_quick_fix(0); // the hidden finding: nothing happens
    assert_eq!(no_connects(&app), before);
    let _ = app.handle_erc_quick_fix(1); // the shown UnusedPin: a no-connect lands
    assert_eq!(no_connects(&app), before + 1);
}

#[test]
fn a_wire_that_got_a_new_id_keeps_its_finding_and_the_panel_says_stale() {
    let (mut app, path, _part) = app_with_two_findings();
    let wire = app.document_state.active_engine().unwrap().document().wires[0].clone();
    let dangling = finding(
        wire.end,
        Some(SelectedItem::new(wire.uuid, SelectedKind::Wire)),
    );
    app.ui_state
        .erc_violations_by_path
        .get_mut(&path)
        .unwrap()
        .push(dangling);
    app.refresh_active_erc_from_cache(Some(&path));
    let before = app.build_erc_diagnostic_entries().len();

    let mut renewed = wire.clone();
    renewed.uuid = uuid::Uuid::now_v7();
    app.apply_engine_commands(
        vec![
            signex_engine::Command::DeleteSelection {
                items: vec![SelectedItem::new(wire.uuid, SelectedKind::Wire)],
            },
            signex_engine::Command::PlaceWireSegmentAsIs { wire: renewed },
        ],
        true,
        true,
    );

    assert_eq!(app.build_erc_diagnostic_entries().len(), before);
    assert!(app.erc_results_stale());
}

#[test]
fn an_edit_marks_the_results_stale_and_a_run_clears_it() {
    let (mut app, _path, part) = app_with_two_findings();
    assert!(!app.erc_results_stale());
    delete_part(&mut app, part);
    assert!(app.erc_results_stale());
    app.run_erc_blocking();
    assert!(!app.erc_results_stale());
}

#[test]
fn old_results_stay_stale_while_a_run_is_in_flight_and_after_it_lands_clean() {
    let (mut app, _path, part) = app_with_two_findings();
    delete_part(&mut app, part);
    let (generation, inputs) = app.begin_erc_run();
    assert!(app.erc_results_stale(), "in flight: the old cache is shown");
    let panel = app.erc_report_options(signex_output::ErcReportFormat::Text);
    assert!(panel.results_stale, "and exported as stale");

    let outcome = Ok(super::erc_job::run_erc_job(inputs));
    app.apply_erc_run_result(generation, &outcome);
    assert!(
        !app.erc_results_stale(),
        "the new run read the current sheet"
    );
}

#[test]
fn a_bulk_edit_path_that_marks_a_sheet_dirty_marks_the_results_stale() {
    let (mut app, path, _part) = app_with_two_findings();
    assert!(!app.erc_results_stale());
    app.document_state.mark_document_dirty(&path);
    assert!(app.erc_results_stale());
}
