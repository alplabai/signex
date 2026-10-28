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
