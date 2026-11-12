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

/// Whether the window shows `needle`. Fluent wraps each argument in
/// invisible isolation marks (U+2068 / U+2069); they are not part of what
/// the user reads, so they are dropped before matching.
fn shows(app: &Signex, window: iced::window::Id, needle: &str) -> bool {
    lay_out_texts(app.view(window), iced::Size::new(1400.0, 900.0))
        .iter()
        .any(|t| {
            t.text
                .replace(['\u{2068}', '\u{2069}'], "")
                .contains(needle)
        })
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

    assert!(shows(&app, window, "3D"), "the footprint mode switcher");
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

fn footprint(path: &std::path::Path) -> FootprintDocument {
    let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
    FootprintDocument::new(path.to_path_buf(), file)
}

fn symbol(path: &std::path::Path) -> SymbolDocument {
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
    SymbolDocument::new(path.to_path_buf(), file)
}

#[test]
fn an_undocked_footprints_own_align_menu_shows_in_its_window() {
    let a = PathBuf::from("a.snxfpt");
    let (mut app, window) = undocked(TabKind::FootprintEditor(a.clone()), &a);
    let mut own = footprint(&a);
    own.state.active_bar_menu =
        Some(crate::library::editor::footprint::state::FpActiveBarMenu::Align);
    app.document_state.footprint_editors.insert(a, own);
    assert!(shows(&app, window, "Align Left"));
}

#[test]
fn an_undocked_footprint_window_does_not_draw_another_footprints_active_bar() {
    let a = PathBuf::from("a.snxfpt");
    let b = PathBuf::from("b.snxfpt");
    let (mut app, window) = undocked(TabKind::FootprintEditor(a.clone()), &a);
    app.document_state
        .footprint_editors
        .insert(a.clone(), footprint(&a));
    let mut other = footprint(&b);
    other.state.active_bar_menu =
        Some(crate::library::editor::footprint::state::FpActiveBarMenu::Align);
    app.document_state
        .footprint_editors
        .insert(b.clone(), other);
    // The user picks B in the main window; A stays in its own window.
    app.document_state
        .tabs
        .push(tab(&b, TabKind::FootprintEditor(b.clone())));
    app.document_state.active_tab = 1;
    assert!(
        shows(&app, window, "3D"),
        "A's window draws a footprint editor"
    );
    assert!(
        !shows(&app, window, "Align Left"),
        "A's window draws B's Align menu; its entries carry b.snxfpt, so a click there edits B"
    );
}

#[test]
fn an_undocked_symbols_own_align_menu_shows_in_its_window() {
    let a = PathBuf::from("a.snxsym");
    let (mut app, window) = undocked(TabKind::SymbolEditor(a.clone()), &a);
    let mut own = symbol(&a);
    own.active_bar_menu = Some(crate::library::editor::symbol::state::SymActiveBarMenu::Align);
    app.document_state.symbol_editors.insert(a, own);
    assert!(shows(&app, window, "Align Left"));
}

#[test]
fn an_undocked_symbol_window_does_not_draw_another_symbols_active_bar() {
    let a = PathBuf::from("a.snxsym");
    let b = PathBuf::from("b.snxsym");
    let (mut app, window) = undocked(TabKind::SymbolEditor(a.clone()), &a);
    app.document_state
        .symbol_editors
        .insert(a.clone(), symbol(&a));
    let mut other = symbol(&b);
    other.active_bar_menu = Some(crate::library::editor::symbol::state::SymActiveBarMenu::Align);
    app.document_state.symbol_editors.insert(b.clone(), other);
    app.document_state
        .tabs
        .push(tab(&b, TabKind::SymbolEditor(b.clone())));
    app.document_state.active_tab = 1;
    assert!(
        shows(&app, window, "Part 1 / 1"),
        "A's window draws a symbol editor"
    );
    assert!(
        !shows(&app, window, "Align Left"),
        "A's window draws B's Align menu; its entries carry b.snxsym"
    );
}

#[test]
fn an_undocked_symbols_own_context_menu_shows_in_its_window_not_the_main_ones() {
    let a = PathBuf::from("a.snxsym");
    let b = PathBuf::from("b.snxsym");
    let menu = || crate::library::editor::symbol::state::SymbolContextMenuState {
        x: 10.0,
        y: 10.0,
        target: crate::library::editor::symbol::state::SymbolContextTarget::Empty,
        open_submenu: None,
    };
    let (mut app, window) = undocked(TabKind::SymbolEditor(a.clone()), &a);
    app.document_state
        .symbol_editors
        .insert(a.clone(), symbol(&a));
    let mut other = symbol(&b);
    other.context_menu = Some(menu());
    app.document_state.symbol_editors.insert(b.clone(), other);
    app.document_state
        .tabs
        .push(tab(&b, TabKind::SymbolEditor(b.clone())));
    app.document_state.active_tab = 1;
    assert!(
        !shows(&app, window, "Select All"),
        "B's context menu is not A's"
    );

    let own = app.document_state.symbol_editors.get_mut(&a).unwrap();
    own.context_menu = Some(menu());
    assert!(
        shows(&app, window, "Select All"),
        "A's own context menu shows"
    );
}

/// B is active in main; A is undocked. `set` opens a menu or dialog on B
/// only, so only the window that owns it may draw it.
fn two_footprints(set: fn(&mut FootprintDocument)) -> (Signex, iced::window::Id) {
    let a = PathBuf::from("a.snxfpt");
    let b = PathBuf::from("b.snxfpt");
    let (mut app, window) = undocked(TabKind::FootprintEditor(a.clone()), &a);
    app.document_state
        .footprint_editors
        .insert(a.clone(), footprint(&a));
    let mut other = footprint(&b);
    set(&mut other);
    app.document_state
        .footprint_editors
        .insert(b.clone(), other);
    app.document_state
        .tabs
        .push(tab(&b, TabKind::FootprintEditor(b.clone())));
    app.document_state.active_tab = 1;
    (app, window)
}

fn open_context_menu(editor: &mut FootprintDocument) {
    editor.state.context_menu = Some(
        crate::library::editor::footprint::state::FootprintContextMenuState {
            x: 10.0,
            y: 10.0,
            target: crate::library::editor::footprint::state::FootprintContextTarget::Empty,
            submenu: None,
        },
    );
}

fn open_move_by(editor: &mut FootprintDocument) {
    editor.state.move_by_modal = Some(crate::library::editor::footprint::state::MoveByModal {
        dx_buf: String::new(),
        dy_buf: String::new(),
    });
}

#[test]
fn an_undocked_footprint_window_does_not_draw_another_footprints_context_menu() {
    let (mut app, window) = two_footprints(open_context_menu);
    assert!(
        !shows(&app, window, "Find Similar Objects"),
        "B's context menu is not A's"
    );
    let own = app
        .document_state
        .footprint_editors
        .get_mut(&PathBuf::from("a.snxfpt"))
        .unwrap();
    open_context_menu(own);
    assert!(
        shows(&app, window, "Find Similar Objects"),
        "A's own menu shows"
    );
}

#[test]
fn an_undocked_footprint_window_does_not_draw_another_footprints_move_by_dialog() {
    let (mut app, window) = two_footprints(open_move_by);
    assert!(
        !shows(&app, window, "Move Selection By"),
        "B's dialog is not A's"
    );
    let own = app
        .document_state
        .footprint_editors
        .get_mut(&PathBuf::from("a.snxfpt"))
        .unwrap();
    open_move_by(own);
    assert!(
        shows(&app, window, "Move Selection By"),
        "A's own dialog shows"
    );
}
