//! #1818: the schematic Active Bar and the in-canvas text editor follow
//! the engine of the window that draws them, not the main window's.

use std::path::{Path, PathBuf};

use super::*;
use crate::app::state::WindowKind;
use crate::app::{TabInfo, TabKind};

fn tab(path: &Path, kind: TabKind) -> TabInfo {
    TabInfo {
        title: "t".to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind,
    }
}

fn engine() -> signex_engine::Engine {
    signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap()
}

fn editing() -> crate::app::TextEditState {
    crate::app::TextEditState {
        uuid: uuid::Uuid::nil(),
        kind: signex_types::schematic::SelectedKind::Symbol,
        text: "R1".to_string(),
        original_text: "R1".to_string(),
        world_x: 0.0,
        world_y: 0.0,
        sheet: None,
        placement: None,
    }
}

/// An undocked window on `window_path`, with the main window's active tab
/// being `main_tab`.
fn app_with_window(main_tab: TabInfo, window_path: &Path) -> (Signex, iced::window::Id) {
    let (mut app, _task) = Signex::new();
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.document_state.tabs.push(main_tab);
    app.document_state.active_tab = 0;
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: window_path.to_path_buf(),
            title: "w".to_string(),
        },
    );
    app.interaction_state.editing_text = Some(editing());
    (app, window)
}

/// Main is on a footprint (no schematic engine); the undocked window holds
/// a sheet with an engine.
#[test]
fn an_undocked_schematic_window_gets_its_bar_while_main_shows_a_footprint() {
    let fp = PathBuf::from("m.snxfpt");
    let sheet = PathBuf::from("w.snxsch");
    let (mut app, window) = app_with_window(tab(&fp, TabKind::FootprintEditor(fp.clone())), &sheet);
    app.document_state.engines.insert(sheet, engine());
    assert!(!app.has_active_schematic(), "main has no schematic");

    assert!(app.schematic_active_bar_overlay(window).is_some());
    assert!(app.text_edit_overlay(window).is_some());
}

/// Main holds a sheet with an engine; the undocked window's sheet has none.
#[test]
fn an_undocked_window_without_an_engine_gets_no_schematic_bar_from_main() {
    let main_sheet = PathBuf::from("m.snxsch");
    let sheet = PathBuf::from("w.snxsch");
    let (mut app, window) = app_with_window(tab(&main_sheet, TabKind::Schematic), &sheet);
    app.document_state
        .engines
        .insert(main_sheet.clone(), engine());
    app.document_state.active_path = Some(main_sheet);
    assert!(app.has_active_schematic(), "main has a schematic");

    assert!(app.schematic_active_bar_overlay(window).is_none());
    assert!(app.text_edit_overlay(window).is_none());
}
