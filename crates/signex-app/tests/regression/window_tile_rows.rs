//! #2247 - Window ▸ Tile Horizontally / Tile Vertically arrange the main
//! window and the undocked document windows on the main window's monitor.
//! The window moves are iced tasks a test cannot run, so the plan they are
//! built from is checked, and each entry point is checked to reach it (its
//! Messages line names the screen size, which each test picks on its own,
//! so a line from a test running alongside cannot answer for it).

use signex_app::app::window_geometry::{MonitorInfo, WindowReading};
use signex_app::app::{
    CommandPaletteMsg, GeometryReadout, Message, Signex, TabInfo, TabKind, Tile, TileLayout, UiMsg,
    WindowMsg,
};
use signex_app::keymap::ShortcutTrigger;
use std::path::PathBuf;

/// A main window read on a `monitor`-sized screen, with one schematic tab
/// undocked into its own window.
fn app_with_an_undocked_tab(monitor: (f32, f32)) -> Signex {
    let (mut app, _task) = Signex::new();
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    let path = PathBuf::from("tile-rows.snxsch");
    let engine = signex_engine::Engine::new_with_path(Default::default(), Some(path.clone()))
        .expect("engine");
    app.document_state.engines.insert(path.clone(), engine);
    for (title, tab_path) in [
        ("main", PathBuf::from("tile-rows-main.snxsch")),
        ("undocked", path),
    ] {
        app.document_state.tabs.push(TabInfo {
            title: title.to_string(),
            path: tab_path,
            cached_document: None,
            project_id: None,
            kind: TabKind::Schematic,
        });
    }
    let _ = app.update(Message::Window(WindowMsg::UndockTab(1)));
    assert_eq!(app.ui_state.windows.len(), 1, "the tab has its own window");
    let _ = app.update(Message::Window(WindowMsg::WindowGeometryRead(Box::new(
        GeometryReadout {
            id: main,
            reading: Some(WindowReading {
                monitor_index: Some(0),
                position: (10.0, 10.0),
                size: (1400.0, 900.0),
                maximized: true,
            }),
            monitors: vec![MonitorInfo {
                name: None,
                size: monitor,
                primary: true,
            }],
        },
    ))));
    app
}

fn tiles_of(app: &Signex, layout: TileLayout) -> Vec<Tile> {
    let plan = app.tile_plan(layout).expect("a plan");
    assert_eq!(plan.monitor_index, 0);
    assert_eq!(
        plan.windows.first().map(|(id, _)| *id),
        app.ui_state.main_window_id,
        "the main window takes the first tile"
    );
    plan.windows.into_iter().map(|(_, tile)| tile).collect()
}

fn logged(text: &str) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|entry| entry.message.contains(text))
}

#[test]
fn tile_horizontally_puts_the_windows_side_by_side() {
    let app = app_with_an_undocked_tab((1920.0, 1080.0));
    assert_eq!(
        tiles_of(&app, TileLayout::SideBySide),
        vec![
            Tile {
                position: (0.0, 0.0),
                size: (960.0, 1080.0)
            },
            Tile {
                position: (960.0, 0.0),
                size: (960.0, 1080.0)
            },
        ]
    );
}

#[test]
fn tile_vertically_stacks_the_windows() {
    let app = app_with_an_undocked_tab((1920.0, 1080.0));
    let tiles = tiles_of(&app, TileLayout::Stacked);
    assert_eq!(tiles[0].size, (1920.0, 540.0));
    assert_eq!(tiles[1].position, (0.0, 540.0));
}

#[test]
fn the_menu_rows_run_the_tile() {
    let _ = signex_app::diagnostics::init_logging();
    let mut app = app_with_an_undocked_tab((2201.0, 1201.0));
    let _ = app.update(crate::menu_support::menu_row("tile_windows_horizontally"));
    assert!(logged("2 windows side by side on the 2201 x 1201 screen"));
    let _ = app.update(crate::menu_support::menu_row("tile_windows_vertically"));
    assert!(logged(
        "2 windows one above the other on the 2201 x 1201 screen"
    ));
}

#[test]
fn the_palette_runs_the_tile() {
    let _ = signex_app::diagnostics::init_logging();
    let mut app = app_with_an_undocked_tab((2202.0, 1202.0));
    let _ = app.update(Message::CommandPalette(CommandPaletteMsg::Open));
    let _ = app.update(Message::CommandPalette(CommandPaletteMsg::QueryChanged(
        "Tile windows one above the other".into(),
    )));
    let _ = app.update(Message::CommandPalette(CommandPaletteMsg::ExecuteSelected));
    assert!(logged(
        "2 windows one above the other on the 2202 x 1202 screen"
    ));
}

#[test]
fn a_user_key_binding_runs_the_tile() {
    let _ = signex_app::diagnostics::init_logging();
    let mut app = app_with_an_undocked_tab((2203.0, 1203.0));
    crate::number_schematic_sheets::bind(
        &mut app,
        "tile_windows_horizontally",
        "Ctrl+Alt+Shift+F12",
    );
    let ShortcutTrigger::KeySequence(mut strokes) =
        ShortcutTrigger::parse("Ctrl+Alt+Shift+F12").unwrap()
    else {
        panic!("a key");
    };
    let _ = app.update(Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke: strokes.remove(0),
    }));
    assert!(logged("2 windows side by side on the 2203 x 1203 screen"));
}

#[test]
fn with_no_undocked_window_there_is_nothing_to_tile() {
    let (mut app, _task) = Signex::new();
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    assert!(app.tile_plan(TileLayout::SideBySide).is_none());
}
