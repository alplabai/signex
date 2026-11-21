//! A panel in an undocked window edits that window's document, whatever its
//! kind: with two documents of a kind open, the other stays clean. And what
//! undocking and docking back do to the main window's tab.

use std::path::{Path, PathBuf};

use super::*;
use crate::app::{FootprintDocument, SymbolDocument};
use crate::panels::PanelMsg;

/// A window over the tab at `path`, seated as `UndockedTabOpened` leaves it.
fn window_over(app: &mut Signex, path: &Path) -> iced::window::Id {
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: path.to_path_buf(),
            title: "doc".to_string(),
        },
    );
    app.interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());
    window
}

fn tab_of(path: &Path, kind: TabKind) -> TabInfo {
    TabInfo {
        title: "doc".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind,
    }
}

fn app_with_main() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app
}

fn send_from(app: &mut Signex, window: iced::window::Id, message: PanelMsg) {
    let scoped = app.scope_dock_message(window, dock(message));
    assert!(is_scoped(&scoped, window), "not scoped to the window");
    let _ = app.update(scoped);
}

#[test]
fn a_footprint_panel_edit_changes_only_the_windows_footprint() {
    let mut app = app_with_main();
    let (a, b) = (PathBuf::from("a.snxfpt"), PathBuf::from("b.snxfpt"));
    for path in [&a, &b] {
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
        app.document_state
            .footprint_editors
            .insert(path.clone(), FootprintDocument::new(path.clone(), file));
        app.document_state
            .tabs
            .push(tab_of(path, TabKind::FootprintEditor(path.clone())));
    }
    app.document_state.active_tab = app.document_state.tabs.len() - 2;
    let window = window_over(&mut app, &b);

    send_from(
        &mut app,
        window,
        PanelMsg::FpEditorSetFootprintDescription("edited".into()),
    );

    let editors = &app.document_state.footprint_editors;
    assert_eq!(editors[&b].primitive().description, "edited");
    assert_eq!(editors[&a].primitive().description, "");
    assert_eq!(editors[&a].history.undo_len(), 0, "A has no undo step");
    assert!(!app.document_state.is_document_dirty(&a), "A stays clean");
    assert!(app.document_state.is_document_dirty(&b));
}

#[test]
fn a_symbol_panel_edit_changes_only_the_windows_symbol() {
    let mut app = app_with_main();
    let (a, b) = (PathBuf::from("a.snxsym"), PathBuf::from("b.snxsym"));
    for path in [&a, &b] {
        let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("Sym"));
        app.document_state
            .symbol_editors
            .insert(path.clone(), SymbolDocument::new(path.clone(), file));
        app.document_state
            .tabs
            .push(tab_of(path, TabKind::SymbolEditor(path.clone())));
    }
    app.document_state.active_tab = app.document_state.tabs.len() - 2;
    let window = window_over(&mut app, &b);

    send_from(
        &mut app,
        window,
        PanelMsg::SymEditorSetSymbolDescription("edited".into()),
    );

    let editors = &app.document_state.symbol_editors;
    assert_eq!(editors[&b].file.symbols[0].description, "edited");
    assert_eq!(editors[&a].file.symbols[0].description, "");
    assert_eq!(editors[&a].history.undo_len(), 0, "A has no undo step");
    assert!(!app.document_state.is_document_dirty(&a), "A stays clean");
    assert!(app.document_state.is_document_dirty(&b));
}

/// Sheets: a window's edit leaves the main sheet at its revision.
#[test]
fn a_sheet_panel_edit_changes_only_the_windows_sheet() {
    let mut f = fixture();
    let before = f.app.document_state.engines[&f.main_path].revision();
    send_from(&mut f.app, f.window, PanelMsg::SetPaperSize("A2".into()));
    let engines = &f.app.document_state.engines;
    assert_eq!(engines[&f.window_path].document().paper_size, "A2");
    assert_ne!(engines[&f.main_path].document().paper_size, "A2");
    assert_eq!(engines[&f.main_path].revision(), before);
}

/// The app-wide messages stay on the main window.
#[test]
fn app_wide_panel_messages_are_not_scoped() {
    let f = fixture();
    for message in [
        PanelMsg::ToggleGrid,
        PanelMsg::SetGridSize(1.27),
        PanelMsg::Noop,
    ] {
        let sent = f.app.scope_dock_message(f.window, dock(message.clone()));
        assert!(!is_scoped(&sent, f.window), "{message:?}");
    }
}

/// Two sheets and a third, each a tab, with the first in view.
fn three_sheets() -> (Signex, [PathBuf; 3]) {
    let mut app = app_with_main();
    let paths = ["1.snxsch", "2.snxsch", "3.snxsch"].map(PathBuf::from);
    for path in &paths {
        let engine =
            signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.tabs.push(tab(path));
    }
    let first = app.document_state.tabs.len() - 3;
    app.document_state.active_tab = first;
    app.document_state.active_path = Some(paths[0].clone());
    (app, paths)
}

fn undock(app: &mut Signex, path: &Path) -> iced::window::Id {
    let index = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .expect("the tab");
    let before: Vec<iced::window::Id> = app.ui_state.windows.keys().copied().collect();
    let _ = app.update(Message::Window(WindowMsg::UndockTab(index)));
    let window = *app
        .ui_state
        .windows
        .keys()
        .find(|id| !before.contains(id))
        .expect("a window");
    let _ = app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: path.to_path_buf(),
        id: window,
    }));
    window
}

#[test]
fn undocking_another_tab_leaves_main_where_it_was() {
    let (mut app, paths) = three_sheets();
    let _ = undock(&mut app, &paths[2]);
    assert_eq!(app.active_tab_path().as_ref(), Some(&paths[0]));
}

#[test]
fn a_redocked_tab_becomes_the_active_tab() {
    let (mut app, paths) = three_sheets();
    let window = undock(&mut app, &paths[2]);
    let _ = app.update(Message::Window(WindowMsg::ReattachTab(window)));
    assert_eq!(app.active_tab_path().as_ref(), Some(&paths[2]));
}

/// A wire being drawn on the sheet in view does not carry over to the sheet
/// main moves to.
#[test]
fn undocking_the_sheet_in_view_ends_a_wire_in_progress() {
    let (mut app, paths) = three_sheets();
    app.interaction_state.wire_drawing = true;
    app.interaction_state
        .wire_points
        .push(signex_types::schematic::Point::new(1.0, 1.0));
    let _ = undock(&mut app, &paths[0]);
    assert_ne!(app.active_tab_path().as_ref(), Some(&paths[0]));
    assert!(!app.interaction_state.wire_drawing);
    assert!(app.interaction_state.wire_points.is_empty());
}
