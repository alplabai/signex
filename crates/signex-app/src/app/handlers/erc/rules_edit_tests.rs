//! Storing and editing a project's ERC rule set (#1085): explicit seeding
//! from the machine preferences, undoable edits, dirty tracking, and the
//! next ERC run reading the project's set.

use std::collections::HashMap;

use signex_erc::{RuleKind, Severity};

use crate::app::handlers::menu::export::tests::app_workspace;

fn workspace(tag: &str) -> crate::app::Signex {
    let dir = std::env::temp_dir().join(format!("signex-erc-{tag}-{}", uuid::Uuid::new_v4()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    // `Signex::new` loads the machine preferences, which tests in this module
    // write; Store seeds from them, and an edit that matches the seed is a
    // no-op, so start from none.
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    app
}

fn is_dirty(app: &crate::app::Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.dirty_paths.contains(path)
}

#[test]
fn storing_seeds_the_project_from_machine_preferences_and_marks_it_dirty() {
    let mut app = workspace("store");
    app.ui_state
        .erc_severity_override
        .insert(RuleKind::DanglingWire, Severity::Off);
    assert!(!is_dirty(&app));
    let _ = app.handle_store_erc_rules_in_project();
    let rules = app.document_state.projects[0].data.rules.clone().unwrap();
    assert_eq!(rules.erc.severity["dangling_wire"], "off");
    assert_eq!(rules.erc.severity.len(), RuleKind::ALL.len());
    assert!(is_dirty(&app));

    // The run now uses the project set even after the machine changes.
    app.ui_state.erc_severity_override = HashMap::new();
    let (_, inputs) = app.begin_erc_run();
    assert_eq!(inputs.overrides[&RuleKind::DanglingWire], Severity::Off);
}

#[test]
fn an_edit_lands_in_the_project_not_the_preferences() {
    let mut app = workspace("edit");
    let _ = app.handle_store_erc_rules_in_project();
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Info);
    let rules = app.document_state.projects[0].data.rules.clone().unwrap();
    assert_eq!(rules.erc.severity["unused_pin"], "info");
    assert!(app.ui_state.erc_severity_override.is_empty());
}

#[test]
fn rule_edits_undo_and_redo_in_order() {
    let mut app = workspace("undo");
    let _ = app.handle_store_erc_rules_in_project();
    let seeded = app.document_state.projects[0].data.rules.clone();
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Info);
    let _ = app.handle_pin_matrix_cell_cycled(0, 1);
    let edited = app.document_state.projects[0].data.rules.clone();

    assert!(app.undo_project_rules());
    assert!(app.undo_project_rules());
    assert_eq!(app.document_state.projects[0].data.rules, seeded);
    assert!(app.redo_project_rules());
    assert!(app.redo_project_rules());
    assert_eq!(app.document_state.projects[0].data.rules, edited);
    assert!(!app.redo_project_rules());
}

#[test]
fn storing_can_be_undone_back_to_a_project_without_rules() {
    let mut app = workspace("undo-store");
    let _ = app.handle_store_erc_rules_in_project();
    assert!(app.undo_project_rules());
    assert!(app.document_state.projects[0].data.rules.is_none());
}

#[test]
fn a_project_without_rules_keeps_editing_the_preferences() {
    let mut app = workspace("prefs");
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Info);
    assert!(app.document_state.projects[0].data.rules.is_none());
    assert_eq!(
        app.ui_state.erc_severity_override[&RuleKind::UnusedPin],
        Severity::Info
    );
    assert!(!is_dirty(&app));
}
