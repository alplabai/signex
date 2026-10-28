//! #1351 - a preference toggled from `update` is written by the `Task` it
//! returns, never inline: the file is untouched until that task runs.

use signex_types::coord::Unit;

use super::*;

#[test]
fn cycling_the_unit_leaves_prefs_json_to_the_returned_task() {
    let (mut app, _task) = Signex::new();
    crate::fonts::write_unit_pref(Unit::Mm);
    app.ui_state.unit = Unit::Mm;

    let task = app.dispatch_update(Message::Ui(UiMsg::UnitCycled));

    assert_eq!(app.ui_state.unit, Unit::Mil, "the model changes at once");
    assert_eq!(
        crate::fonts::read_unit_pref(),
        Unit::Mm,
        "update must not write prefs.json inline"
    );
    assert!(task.units() >= 1, "the write is handed back as a Task");
}

#[test]
fn setting_the_grid_size_leaves_prefs_json_to_the_returned_task() {
    let (mut app, _task) = Signex::new();
    crate::fonts::write_grid_size_mm_pref(1.27);

    let task = app.dispatch_update(Message::Dock(crate::dock::DockMessage::Panel(
        crate::panels::PanelMsg::SetGridSize(5.08),
    )));

    assert_eq!(app.ui_state.grid_size_mm, 5.08);
    assert_eq!(
        crate::fonts::read_grid_size_mm_pref(),
        Some(1.27),
        "update must not write prefs.json inline"
    );
    assert!(task.units() >= 1);
}

#[test]
fn saving_preferences_leaves_prefs_json_to_the_returned_tasks() {
    let (mut app, _task) = Signex::new();
    crate::fonts::write_autosave_enabled_pref(false);
    app.ui_state.autosave_enabled = false;
    app.ui_state.preferences_draft_autosave_enabled = true;

    let task = app.dispatch_update(Message::Preferences(PreferencesMsg::Inner(
        crate::preferences::PrefMsg::Save,
    )));

    assert!(app.ui_state.autosave_enabled, "the model changes at once");
    assert!(
        !crate::fonts::read_autosave_enabled_pref(),
        "Save must not write prefs.json inline"
    );
    assert!(task.units() >= 1, "the writes are handed back as Tasks");
}

#[test]
fn changing_an_erc_severity_leaves_prefs_json_to_the_returned_task() {
    use signex_erc::{RuleKind, Severity};
    let (mut app, _task) = Signex::new();
    crate::fonts::write_erc_severity_overrides(&std::collections::HashMap::new());

    let task = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Info);

    assert!(
        app.ui_state
            .erc_severity_override
            .contains_key(&RuleKind::UnusedPin)
    );
    assert!(
        crate::fonts::read_erc_severity_overrides().is_empty(),
        "update must not write prefs.json inline"
    );
    assert!(task.units() >= 1);
}

#[test]
fn clearing_recent_projects_leaves_prefs_json_to_the_returned_task() {
    let (mut app, _task) = Signex::new();
    let seeded = vec![std::path::PathBuf::from("/signex-1351/a.snxprj")];
    crate::fonts::write_recent_projects(&seeded);
    app.ui_state.recent_projects =
        crate::recent_projects::RecentProjects::from_paths(seeded.clone());

    let task = app.handle_clear_recent_projects();

    assert!(app.ui_state.recent_projects.entries().is_empty());
    assert_eq!(
        crate::fonts::read_recent_projects(),
        seeded,
        "update must not write prefs.json inline"
    );
    assert!(task.units() >= 1);
}

#[test]
fn adding_a_custom_filter_preset_leaves_prefs_json_to_the_returned_task() {
    let (mut app, _task) = Signex::new();
    app.interaction_state.custom_filter_presets.clear();
    crate::fonts::write_custom_filter_presets(&[]);

    let task = app.dispatch_update(Message::Dock(crate::dock::DockMessage::Panel(
        crate::panels::PanelMsg::AddCustomFilterPreset,
    )));

    assert_eq!(app.interaction_state.custom_filter_presets.len(), 1);
    assert!(
        crate::fonts::read_custom_filter_presets().is_empty(),
        "update must not write prefs.json inline"
    );
    assert!(task.units() >= 1);
}
