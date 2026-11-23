//! Tools ▸ Annotation ▸ Number Schematic Sheets and View ▸ Window ▸ Tile
//! Horizontally / Tile Vertically, the rows #2247 wired from stubs: when
//! they are live and what a click sends; View ▸ Window ▸ Untile (#2249).

use super::*;

const NUMBER: &str = "Tools/Annotation/Number Schematic Sheets...";
const TILE_H: &str = "View/Window/Tile Horizontally";
const TILE_V: &str = "View/Window/Tile Vertically";
const UNTILE: &str = "View/Window/Untile";

#[test]
fn number_schematic_sheets_needs_a_sheet_of_a_project() {
    let mut app = one_symbol_selected();
    assert!(!row_is_live(&app, NUMBER), "a loose sheet has no numbering");
    let project = open_project(&mut app, "numbering.snxprj");
    assert!(
        !row_is_live(&app, NUMBER),
        "the active sheet is not one of the project's"
    );
    app.document_state.projects[project]
        .data
        .sheets
        .push(signex_types::project::SheetEntry {
            name: "menu-rows".into(),
            filename: "menu-rows.snxsch".into(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        });
    assert!(row_is_live(&app, NUMBER));
    let task = click(&mut app, NUMBER);
    assert!(has_work(&task), "the pages are read off the UI thread");
    let state = app.ui_state.sheet_numbering.as_ref().expect("dialog open");
    assert_eq!(state.page_count(), None, "still reading");
}

/// The main window read on a 1920 x 1080 screen.
fn main_window_read(app: &mut Signex) {
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    let _ = app.window_geometry_read(crate::app::GeometryReadout {
        id: main,
        reading: Some(crate::app::window_geometry::WindowReading {
            monitor_index: Some(0),
            position: (0.0, 0.0),
            size: (1400.0, 900.0),
            maximized: false,
        }),
        monitors: vec![crate::app::window_geometry::MonitorInfo {
            name: None,
            size: (1920.0, 1080.0),
            primary: true,
        }],
    });
}

fn undock(app: &mut Signex) {
    app.ui_state.windows.insert(
        iced::window::Id::unique(),
        crate::app::WindowKind::UndockedTab {
            path: std::path::PathBuf::from("/w/undocked.snxsch"),
            title: "undocked".into(),
        },
    );
}

#[test]
fn tile_rows_are_greyed_with_no_undocked_window() {
    let mut app = one_symbol_selected();
    main_window_read(&mut app);
    assert!(!row_is_live(&app, TILE_H));
    assert!(!row_is_live(&app, TILE_V));
}

#[test]
fn tile_rows_are_greyed_until_the_main_window_screen_is_known() {
    let mut app = one_symbol_selected();
    undock(&mut app);
    assert!(!row_is_live(&app, TILE_H));
    main_window_read(&mut app);
    assert!(row_is_live(&app, TILE_H));
    assert!(row_is_live(&app, TILE_V));
    assert!(has_work(&click(&mut app, TILE_H)), "the windows move");
}

/// #2249: two documents in the main window tile into panes from the row,
/// and Untile is live only while they are tiled.
#[test]
fn two_docked_documents_tile_into_panes_and_untile() {
    let mut app = one_symbol_selected();
    app.document_state.tabs.push(TabInfo {
        title: "second".to_string(),
        path: std::path::PathBuf::from("/w/second.snxsch"),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    assert!(row_is_live(&app, TILE_V));
    assert!(!row_is_live(&app, UNTILE));
    let _ = click(&mut app, TILE_V);
    assert_eq!(
        app.ui_state.document_tiling,
        Some(crate::app::TileLayout::SideBySide)
    );
    assert!(row_is_live(&app, UNTILE));
    let _ = click(&mut app, UNTILE);
    assert_eq!(app.ui_state.document_tiling, None);
    assert!(!row_is_live(&app, UNTILE));
}
