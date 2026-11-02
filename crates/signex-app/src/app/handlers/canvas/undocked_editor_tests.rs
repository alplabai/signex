//! #1818: an undocked primitive editor window shows its editor, and no
//! status readout shows another surface's camera.

use super::super::super::*;
use crate::app::{TabInfo, TabKind};
use crate::layout_probe::lay_out_texts;
use std::path::PathBuf;

const SCHEMATIC_ZOOM: f64 = 100.0;

fn tab(path: &std::path::Path, kind: TabKind) -> TabInfo {
    TabInfo {
        title: "t".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind,
    }
}

/// A main window with one tab of `kind`, undocked through `Signex::update`;
/// returns the new window.
fn undocked(kind: TabKind, path: &std::path::Path) -> (Signex, iced::window::Id) {
    let (mut app, _task) = Signex::new();
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.document_state.tabs.push(tab(path, kind));
    app.document_state.active_tab = 0;
    let _ = app.update(Message::Window(WindowMsg::UndockTab(0)));
    let window = app
        .ui_state
        .windows
        .iter()
        .find_map(|(id, kind)| {
            matches!(kind, crate::app::state::WindowKind::UndockedTab { .. }).then_some(*id)
        })
        .expect("the tab is undocked");
    (app, window)
}

fn shows(app: &Signex, window: iced::window::Id, needle: &str) -> bool {
    lay_out_texts(app.view(window), iced::Size::new(1400.0, 900.0))
        .iter()
        .any(|t| t.text.contains(needle))
}

#[test]
fn an_undocked_symbol_window_shows_the_symbol_editor() {
    let path = PathBuf::from("undocked.snxsym");
    let (mut app, window) = undocked(TabKind::SymbolEditor(path.clone()), &path);
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
    let mut editor = SymbolDocument::new(path.clone(), file);
    editor.camera.scale *= 2.5;
    editor.cursor_mm = Some((1.5, -2.5));
    app.document_state.symbol_editors.insert(path, editor);

    assert!(shows(&app, window, "Part 1 / 1"), "the symbol toolbar");
    assert_eq!(app.status_zoom(window), SCHEMATIC_ZOOM * 2.5);
    assert_eq!(app.status_cursor(window), (1.5, -2.5));
}

#[test]
fn an_undocked_footprint_window_shows_the_footprint_editor() {
    let path = PathBuf::from("undocked.snxfpt");
    let (mut app, window) = undocked(TabKind::FootprintEditor(path.clone()), &path);
    let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
    let mut editor = FootprintDocument::new(path.clone(), file);
    editor.state.view_px_per_mm *= 3.0;
    editor.state.cursor_mm = Some((4.0, 5.0));
    app.document_state.footprint_editors.insert(path, editor);

    assert!(shows(&app, window, "Layers"), "the footprint toolbar");
    assert_eq!(app.status_zoom(window), SCHEMATIC_ZOOM * 3.0);
    assert_eq!(app.status_cursor(window), (4.0, 5.0));
}

/// A main window whose active tab is `kind`, with the schematic camera
/// zoomed and the schematic cursor at (7, 8).
fn main_showing(kind: TabKind, path: &std::path::Path) -> (Signex, iced::window::Id) {
    let (mut app, _task) = Signex::new();
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    app.ui_state.cursor_x = 7.0;
    app.ui_state.cursor_y = 8.0;
    app.interaction_state
        .canvas
        .set_viewport(iced::Rectangle::new(
            iced::Point::ORIGIN,
            iced::Size::new(800.0, 600.0),
        ));
    assert!(app.interaction_state.canvas.zoom_about_centre(true, 1.5));
    assert_ne!(app.interaction_state.canvas.zoom_percent(), SCHEMATIC_ZOOM);
    app.document_state.tabs.push(tab(path, kind));
    app.document_state.active_tab = 0;
    (app, window)
}

#[test]
fn a_library_browser_tab_does_not_read_the_schematic_camera() {
    let path = PathBuf::from("parts.snxlib");
    let (app, window) = main_showing(TabKind::LibraryBrowser(path.clone()), &path);
    assert_eq!(app.status_zoom(window), SCHEMATIC_ZOOM);
    assert_eq!(app.status_cursor(window), (0.0, 0.0));
}

#[test]
fn a_project_tab_does_not_read_the_schematic_camera() {
    let path = PathBuf::from("p.snxprj");
    let project = crate::app::state::ProjectId::test_id(1);
    let (app, window) = main_showing(TabKind::Project(project), &path);
    assert_eq!(app.status_zoom(window), SCHEMATIC_ZOOM);
    assert_eq!(app.status_cursor(window), (0.0, 0.0));
}
