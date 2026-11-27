//! #2629: whether the canvas menu of an undocked window opened over an
//! object is worked out when the menu opens, inside the window's swap, with
//! the widths of the sheet that window shows; the view reads the answer.

use std::path::Path;

use super::*;
use crate::app::ContextMenuMsg;
use crate::app::handlers::menu::export::tests::app_workspace;

/// The wire of `b.snxsch` runs along y = 10.16; this point is 2 mm off it,
/// inside a 4 mm project wire width and outside the built-in one.
const OFF_THE_WIRE: (f64, f64) = (15.0, 12.16);

/// The window's sheet sits in a project whose wires are 4 mm wide; the main
/// window's sheet is in no project, so it draws the built-in width.
fn wide_wires_in_the_window() -> (Fixture, tempfile::TempDir, tempfile::TempDir) {
    let (main_dir, window_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut app = app_workspace(&window_dir.path().to_string_lossy(), &["b.snxsch"]);
    app.document_state.projects[0]
        .data
        .schematic_defaults
        .wire_width_mm = Some(4.0);
    let f = fixture_in_dirs(app, main_dir.path(), window_dir.path());
    (f, main_dir, window_dir)
}

/// Right-click `window`'s canvas with the pointer at `at` on `sheet`.
fn right_click(app: &mut Signex, window: iced::window::Id, sheet: &Path, at: (f64, f64)) {
    app.ui_state.cursor_document = Some(sheet.to_path_buf());
    (app.ui_state.cursor_x, app.ui_state.cursor_y) = at;
    let show = Box::new(ContextMenuMsg::Show(300.0, 300.0));
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ScopedToWindow(
        window, show,
    )));
}

#[test]
fn a_window_menu_over_a_wide_wire_counts_as_over_an_object() {
    let (mut f, _main_dir, _window_dir) = wide_wires_in_the_window();
    let (window, sheet) = (f.window, f.window_path.clone());
    right_click(&mut f.app, window, &sheet, OFF_THE_WIRE);
    let menu = f.app.interaction_state.context_menu.as_ref().expect("open");
    assert_eq!(menu.world, Some(OFF_THE_WIRE));
    assert!(menu.hit_object, "the window's 4 mm wire is under the point");
    assert!(
        has(&f.app, window, "Find Similar Objects..."),
        "the window's menu offers Find Similar"
    );
}

#[test]
fn a_window_menu_off_every_object_offers_no_object_rows() {
    let (mut f, _main_dir, _window_dir) = wide_wires_in_the_window();
    let (window, sheet) = (f.window, f.window_path.clone());
    right_click(&mut f.app, window, &sheet, (15.0, 40.0));
    let menu = f.app.interaction_state.context_menu.as_ref().expect("open");
    assert!(!menu.hit_object);
    assert!(!has(&f.app, window, "Find Similar Objects..."));
}
