//! Tools ▸ Annotation ▸ Number Schematic Sheets and Window ▸ Tile
//! Horizontally / Tile Vertically, the rows #2247 wired from stubs: when
//! they are live and what a click sends.

use super::*;

const NUMBER: &str = "Tools/Annotation/Number Schematic Sheets...";
const TILE_H: &str = "Window/Tile Horizontally";
const TILE_V: &str = "Window/Tile Vertically";

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
