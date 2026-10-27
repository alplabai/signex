//! #1160 — the Place Component picker and the Components panel rows reach
//! the library placement code instead of only logging / opening a preview.
//!
//! Driven through the real `Message::Library` dispatcher against a real
//! temp `.snxlib`; every task the dispatcher returns is run to completion
//! and its messages fed back, as the iced runtime would.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::library::LibraryMessage;
use signex_app::library::messages::PickerMsg;
use signex_app::library::state::PickerState;

use crate::library_place_symbol::{Fixture, PLACE_AT, app_with_library, click, doc, fixture, run};

/// Dispatch `message`, then keep running the tasks it spawns until quiet.
fn dispatch_to_completion(app: &mut Signex, message: Message) {
    let mut pending = vec![message];
    while let Some(next) = pending.pop() {
        let task = app.update(next);
        pending.extend(run(task));
    }
}

fn select_in_picker(app: &mut Signex, f: &Fixture) {
    let summary = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, c)| c.row_id == f.row_id)
        .map(|(_, c)| c)
        .expect("fixture row is listed");
    app.library.picker = Some(PickerState {
        filter: String::new(),
        selected: None,
    });
    dispatch_to_completion(
        app,
        Message::Library(LibraryMessage::Picker(PickerMsg::SelectComponent(summary))),
    );
}

fn place_selected(app: &mut Signex) {
    dispatch_to_completion(
        app,
        Message::Library(LibraryMessage::Picker(PickerMsg::PlaceSelected)),
    );
}

#[test]
fn the_picker_place_action_places_on_the_active_schematic() {
    let f = fixture();
    let mut app = app_with_library(&f);
    select_in_picker(&mut app, &f);

    place_selected(&mut app);
    click(&mut app, PLACE_AT);

    assert_eq!(doc(&app).symbols.len(), 1);
    assert_eq!(doc(&app).symbols[0].row_id, Some(f.row_id));
    assert!(app.library.picker.is_none(), "the picker closes");
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn the_picker_place_is_one_undo_step() {
    let f = fixture();
    let mut app = app_with_library(&f);
    select_in_picker(&mut app, &f);
    place_selected(&mut app);
    click(&mut app, PLACE_AT);
    assert_eq!(doc(&app).symbols.len(), 1);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(doc(&app).symbols.is_empty());
    assert!(doc(&app).lib_symbols.is_empty());
}

#[test]
fn a_components_panel_row_place_action_places_on_the_active_schematic() {
    let f = fixture();
    let mut app = app_with_library(&f);

    dispatch_to_completion(
        &mut app,
        Message::Library(LibraryMessage::ComponentsPanelPlace {
            library_path: f.snxlib.clone(),
            table: "ICs".into(),
            row_id: signex_library::RowId(f.row_id),
        }),
    );
    click(&mut app, PLACE_AT);

    assert_eq!(doc(&app).symbols.len(), 1);
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn placing_without_an_active_schematic_shows_the_notice() {
    let f = fixture();
    let (mut app, _t) = Signex::new();
    app.library
        .open_library(f.snxlib.clone())
        .expect("mount library");
    select_in_picker(&mut app, &f);

    place_selected(&mut app);

    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("placing with nothing open must say so");
    assert_eq!(notice.title, "Place Failed");
    assert!(notice.detail.contains("schematic"), "{:?}", notice.detail);
}
