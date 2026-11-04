//! #2070 — a part edited while an Apply is in flight keeps the edit. The
//! worker decides against the fields captured at the click; the completion
//! re-reads the part and never writes over a field that changed since.

use signex_app::app::{Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use uuid::Uuid;

use crate::library_place_symbol::{Fixture, app_with_library, doc, place};
use crate::library_symbol_drift::{check_drift, run_task, select_first};
use crate::library_update_apply::{click_apply, edit_row, scan, select_all_updates, team_fixture};

fn mpn(app: &Signex) -> Option<String> {
    doc(app).symbols[0].fields.get("MPN").cloned()
}

fn set_field(app: &mut Signex, key: &str, value: &str) {
    let symbol_id = doc(app).symbols[0].uuid;
    app.document_state
        .active_engine_mut()
        .unwrap()
        .execute(signex_engine::Command::SetSymbolField {
            symbol_id,
            key: key.into(),
            value: value.into(),
        })
        .unwrap();
}

/// The row moved both its MPN and its manufacturer; the part still holds the
/// old ones, so both follow.
fn move_row(f: &Fixture) {
    edit_row(f, |row| {
        row.primary_mpn.mpn = "ROW-NEW".into();
        row.primary_mpn.manufacturer = "NewCo".into();
        row.version = "9.9.9".into();
    });
}

fn logged(fragment: &str) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(fragment))
}

fn run_and_edit_then_deliver(app: &mut Signex, task: iced::Task<Message>) {
    // The worker has run against the values captured at the click.
    let messages = run_task(task);
    assert!(!messages.is_empty(), "the worker produced a result");
    set_field(app, "MPN", "EDITED-9");
    for message in messages {
        let _ = app.update(message);
    }
}

fn assert_edit_kept_other_field_followed(app: &Signex) {
    assert_eq!(mpn(app).as_deref(), Some("EDITED-9"), "the edit survives");
    assert_eq!(
        doc(app).symbols[0]
            .fields
            .get("Manufacturer")
            .map(String::as_str),
        Some("NewCo"),
        "a field not edited meanwhile still follows the row"
    );
    assert!(
        logged("kept your MPN \"EDITED-9\"; it changed while the update ran"),
        "Messages names the part and the field"
    );
}

#[test]
fn update_from_library_keeps_an_mpn_edited_while_it_runs() {
    let _ = signex_app::diagnostics::init_logging();
    let f = crate::library_place_symbol::fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let id: Uuid = select_first(&mut app);
    move_row(&f);
    check_drift(&mut app);
    let task = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::UpdateSymbolFromLibrary(id),
    )));
    run_and_edit_then_deliver(&mut app, task);
    assert_edit_kept_other_field_followed(&app);
}

#[test]
fn library_updates_apply_keeps_an_mpn_edited_while_it_runs() {
    let _ = signex_app::diagnostics::init_logging();
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    move_row(&f);
    scan(&mut app);
    assert!(app.library.library_updates.is_some());
    select_all_updates(&mut app);
    let task = click_apply(&mut app);
    run_and_edit_then_deliver(&mut app, task);
    assert_edit_kept_other_field_followed(&app);
}

#[test]
fn an_unedited_part_still_follows_its_row() {
    let f = team_fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    move_row(&f);
    scan(&mut app);
    select_all_updates(&mut app);
    let task = click_apply(&mut app);
    for message in run_task(task) {
        let _ = app.update(message);
    }
    assert_eq!(mpn(&app).as_deref(), Some("ROW-NEW"));
}
