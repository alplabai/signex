//! #1966 — a Components row placed from an undocked window arms on, and
//! lands in, that window's sheet.
//!
//! Driven through the real messages against a real temp `.snxlib`.

use std::path::PathBuf;

use signex_app::app::{Message, Signex, WindowMsg};
use signex_app::dock::DockMessage;
use signex_app::library::LibraryMessage;
use signex_app::library::state::{ComponentsPanelRowKey, ComponentsRowAction};

use crate::library_place_symbol::{
    Fixture, PLACE_AT, app_with_library, complete, fixture, resolve, run,
};
use crate::placement_session::second_sheet;

/// The app with a second sheet shown by an undocked window; the main window
/// keeps the first sheet.
fn app_with_a_window(f: &Fixture) -> (Signex, iced::window::Id, PathBuf, PathBuf) {
    let mut app = app_with_library(f);
    let first = PathBuf::from("place-364.snxsch");
    let other = second_sheet(&mut app);
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let index = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == other)
        .expect("tab of the second sheet");
    let _ = app.update(Message::Window(WindowMsg::UndockTab(index)));
    let window = *app.ui_state.windows.keys().next().expect("undocked window");
    let _ = app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: other.clone(),
        id: window,
    }));
    // The main window shows the first sheet.
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(first.clone());
    (app, window, first, other)
}

fn symbols_in(app: &Signex, path: &PathBuf) -> usize {
    app.document_state.engines[path].document().symbols.len()
}

fn scoped(window: iced::window::Id, message: LibraryMessage) -> Message {
    Message::Dock(DockMessage::ScopedToWindow(
        window,
        Box::new(DockMessage::Library(message)),
    ))
}

fn row_key(f: &Fixture) -> ComponentsPanelRowKey {
    ComponentsPanelRowKey {
        library_path: f.snxlib.clone(),
        table: "ICs".into(),
        row_id: signex_library::RowId(f.row_id),
    }
}

#[test]
fn place_from_a_windows_row_menu_arms_on_the_windows_sheet_and_lands_there() {
    let f = fixture();
    let (mut app, window, first, other) = app_with_a_window(&f);
    let _ = app.update(scoped(
        window,
        LibraryMessage::ComponentsPanelRowRightClicked(row_key(&f)),
    ));
    let task = app.update(scoped(
        window,
        LibraryMessage::ComponentsPanelRowMenuPicked(ComponentsRowAction::Place),
    ));
    for message in run(task) {
        let _ = app.update(message);
    }
    assert!(
        app.document_state.error_notice.is_none(),
        "the place was refused: {:?}",
        app.document_state.error_notice
    );
    let session = app.interaction_state.placement.as_ref().expect("armed");
    assert_eq!(session.target_sheet, other);

    let _ = app.update(Message::CanvasEventInWindow {
        window_id: window,
        event: signex_app::canvas::CanvasEvent::Clicked {
            world_x: PLACE_AT.0,
            world_y: PLACE_AT.1,
        },
    });
    assert_eq!(
        symbols_in(&app, &other),
        1,
        "the symbol is in the window's sheet"
    );
    assert_eq!(symbols_in(&app, &first), 0, "the main sheet got nothing");
}

#[test]
fn a_row_dropped_in_a_window_commits_in_that_windows_sheet_when_the_read_lands() {
    let f = fixture();
    let (mut app, _window, first, other) = app_with_a_window(&f);
    // The read started in the window; its drop is waiting for the part.
    app.interaction_state.placement_epoch += 1;
    let epoch = app.interaction_state.placement_epoch;
    app.library.components_panel.pending_drop = Some((epoch, PLACE_AT));
    complete(&mut app, &f, "other-1252.snxsch", resolve(&f, f.row_id));
    assert!(
        app.document_state.error_notice.is_none(),
        "the arm was refused: {:?}",
        app.document_state.error_notice
    );
    assert_eq!(
        symbols_in(&app, &other),
        1,
        "the symbol is in the window's sheet"
    );
    assert_eq!(symbols_in(&app, &first), 0, "the main sheet got nothing");
}
