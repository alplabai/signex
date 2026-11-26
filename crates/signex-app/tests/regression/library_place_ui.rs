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
use signex_i18n::RenderTranslatable;

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
        ..PickerState::default()
    });
    dispatch_to_completion(
        app,
        Message::Library(LibraryMessage::Picker(PickerMsg::SelectComponent(
            summary.row_id,
        ))),
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

/// A filter that hides the selected row drops the selection, and Enter
/// (the search field's Place) then places nothing. A selection the list
/// does not show is never placed either.
#[test]
fn enter_after_the_filter_hid_the_selected_row_places_nothing() {
    let f = fixture();
    let mut app = app_with_library(&f);
    select_in_picker(&mut app, &f);
    let selected = app.library.picker.as_ref().and_then(|p| p.selected.clone());
    assert!(selected.is_some(), "the fixture row is selected");

    dispatch_to_completion(
        &mut app,
        Message::Library(LibraryMessage::Picker(PickerMsg::FilterChanged(
            "zzzz-no-such-part".into(),
        ))),
    );
    let picker = app.library.picker.as_ref().expect("the picker stays open");
    assert!(picker.selected.is_none(), "the hidden row is not selected");

    place_selected(&mut app);
    click(&mut app, PLACE_AT);
    assert!(doc(&app).symbols.is_empty(), "nothing is placed");
    assert!(app.library.picker.is_some(), "the picker stays open");

    // The same row held as selected while the filter hides it.
    app.library.picker.as_mut().expect("picker").selected = selected;
    place_selected(&mut app);
    click(&mut app, PLACE_AT);
    assert!(doc(&app).symbols.is_empty(), "a hidden row is not placed");
    assert!(app.library.picker.is_some(), "the picker stays open");
}

/// A library opened read-only by Skip for now on Git Repository Missing
/// (mockup row 138) still places: placing reads the library and never
/// needs its Git history.
#[test]
fn a_library_skipped_to_read_only_still_places() {
    use signex_app::library::recovery::{GitMissingChoice, RecoveryDialog};

    let f = fixture();
    let dir = f.snxlib.parent().expect("library folder");
    // An empty `.git/` does not open: the library's history is missing.
    std::fs::remove_dir_all(dir.join(".git")).expect("drop the history");
    std::fs::create_dir(dir.join(".git")).expect("an empty .git/");

    let mut app = crate::library_place_symbol::app_with_empty_sheet();
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    dispatch_to_completion(
        &mut app,
        Message::Library(LibraryMessage::OpenLibraryAt(Some(f.snxlib.clone()))),
    );
    assert!(
        matches!(
            app.library.recovery,
            Some(RecoveryDialog::GitMissing { .. })
        ),
        "the open raises Git Repository Missing, got {:?}",
        app.library.recovery
    );
    dispatch_to_completion(
        &mut app,
        Message::Library(LibraryMessage::RecoveryGitMissing(GitMissingChoice::Skip)),
    );
    assert!(app.library.is_read_only(&f.snxlib), "mounted read-only");

    // Back to the sheet from the library's browser tab.
    dispatch_to_completion(
        &mut app,
        Message::Tab {
            window_id: main,
            msg: signex_app::tab_bar::TabMessage::Select(0),
        },
    );
    select_in_picker(&mut app, &f);
    place_selected(&mut app);
    click(&mut app, PLACE_AT);

    assert_eq!(
        doc(&app).symbols.len(),
        1,
        "the part lands, notice: {:?}",
        app.document_state.error_notice
    );
    assert_eq!(doc(&app).symbols[0].row_id, Some(f.row_id));
    assert!(app.document_state.error_notice.is_none());
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
    assert_eq!(notice.title.render_en_us(), "Place Failed");
    assert!(
        notice.detail.render_en_us().contains("schematic"),
        "{:?}",
        notice.detail.render_en_us()
    );
}
