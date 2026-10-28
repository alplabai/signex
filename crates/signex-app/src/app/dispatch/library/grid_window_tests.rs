//! #1355 — the browser grid's scroll window is driven by its scrollable's
//! messages, and selecting a row never disturbs it.

use std::path::PathBuf;

use signex_library::RowId;

use crate::app::Signex;
use crate::library::messages::LibraryMessage;
use crate::library::state::LibraryBrowserState;

fn app_with_browser() -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let root = PathBuf::from("/tmp/grid-window-dispatch/Lib.snxlib");
    app.library
        .library_browsers
        .insert(root.clone(), LibraryBrowserState::new(root.clone()));
    (app, root)
}

#[test]
fn a_scroll_report_moves_the_window() {
    let (mut app, root) = app_with_browser();

    let _ = app.route_library_message(LibraryMessage::BrowserGridScrolled {
        library_path: root.clone(),
        offset_y: 4_800.0,
        viewport_h: 640.0,
    });

    let scroll = app.library.library_browsers[&root].grid_scroll;
    assert_eq!(scroll.offset_y, 4_800.0);
    assert_eq!(scroll.viewport_h, 640.0);
}

#[test]
fn selecting_an_off_screen_row_keeps_the_scroll_window() {
    let (mut app, root) = app_with_browser();
    let _ = app.route_library_message(LibraryMessage::BrowserGridScrolled {
        library_path: root.clone(),
        offset_y: 0.0,
        viewport_h: 640.0,
    });
    let row_id = RowId::new();

    let _ = app.route_library_message(LibraryMessage::BrowserSelectRow {
        library_path: root.clone(),
        table: "Resistors".into(),
        row_id,
    });

    let browser = &app.library.library_browsers[&root];
    assert_eq!(browser.selected_row, Some(row_id));
    assert_eq!(browser.grid_scroll.viewport_h, 640.0);
}
