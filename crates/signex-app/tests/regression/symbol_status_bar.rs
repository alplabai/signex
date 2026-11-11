//! #2262 — on a symbol tab the global status bar shows the symbol editor's
//! own grid, unit and tool, and its Grid and unit buttons drive the
//! symbol's library, not the schematic.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_app::app::{Message, Signex, StatusBarRequest, SymbolDocument, TabInfo, TabKind};
use signex_app::library::messages::{
    LibraryMessage, PrimitiveEdit, SymbolEditorMsg, SymbolToolMsg,
};
use signex_app::library::state::{LibraryDisplaySettings, OpenLibrary};
use signex_types::coord::Unit;
use uuid::Uuid;

/// A main window showing a symbol tab from a library whose Document
/// Options set a 0.254 mm grid in mil; the schematic keeps mm and its own
/// grid.
fn app_on_symbol_tab() -> (Signex, iced::window::Id, PathBuf, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (mut app, _boot) = Signex::new();
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    app.ui_state.unit = Unit::Mm;
    app.ui_state.grid_visible = true;
    app.library.open_libraries.clear();
    app.library.open_libraries.push(OpenLibrary {
        root: tmp.path().join("Parts.snxlib"),
        display_name: "Parts".into(),
        library_id: Uuid::now_v7(),
        tables: HashMap::new(),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        own_edit_stamp: None,
        display: LibraryDisplaySettings {
            unit: Unit::Mil,
            grid_size_mm: 0.254,
            grid_visible: true,
            ..LibraryDisplaySettings::default()
        },
        dangling: Default::default(),
        pricing: Default::default(),
    });
    let path = tmp.path().join("symbols").join("op.snxsym");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("op"));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "op".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    (app, window, path, tmp)
}

/// What the status bar of `window` sends for a click on `request`.
fn click(app: &mut Signex, window: iced::window::Id, request: StatusBarRequest) {
    let symbol = app.status_symbol_path(window);
    let message = Signex::status_bar_message(window, symbol.as_deref(), request);
    let _ = app.update(message);
}

#[test]
fn the_status_bar_shows_the_symbol_librarys_grid_and_unit() {
    let (app, window, _path, _tmp) = app_on_symbol_tab();
    let grid = app.status_grid(window);
    assert_eq!(grid.unit, Unit::Mil);
    assert_eq!(grid.size_mm, 0.254);
    assert!(grid.visible);
    assert_ne!(
        app.ui_state.grid_size_mm, 0.254,
        "the schematic grid differs"
    );
}

#[test]
fn the_grid_button_toggles_the_symbol_grid_not_the_schematics() {
    let (mut app, window, _path, _tmp) = app_on_symbol_tab();
    click(&mut app, window, StatusBarRequest::ToggleGrid);

    assert!(!app.library.open_libraries[0].display.grid_visible);
    assert!(!app.status_grid(window).visible);
    assert!(app.ui_state.grid_visible, "the schematic grid stays on");
}

#[test]
fn the_unit_button_cycles_the_symbol_unit_not_the_schematics() {
    let (mut app, window, _path, _tmp) = app_on_symbol_tab();
    click(&mut app, window, StatusBarRequest::CycleUnit);

    assert_eq!(app.library.open_libraries[0].display.unit, Unit::Inch);
    assert_eq!(app.status_grid(window).unit, Unit::Inch);
    assert_eq!(app.ui_state.unit, Unit::Mm, "the schematic unit stays mm");
}

#[test]
fn the_tool_readout_names_the_symbol_editors_tool() {
    let (mut app, window, path, _tmp) = app_on_symbol_tab();
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path,
        msg: PrimitiveEdit::Symbol(SymbolEditorMsg::SetTool(SymbolToolMsg::AddPin)),
    }));

    assert_eq!(app.status_tool(window), "Add Pin");
}

/// Off a symbol tab the buttons keep driving the app-wide grid.
#[test]
fn a_schematic_window_keeps_the_app_grid() {
    let (mut app, window, _path, _tmp) = app_on_symbol_tab();
    app.document_state.tabs.clear();
    assert!(app.status_symbol_path(window).is_none());
    click(&mut app, window, StatusBarRequest::ToggleGrid);

    assert!(!app.ui_state.grid_visible);
    assert!(app.library.open_libraries[0].display.grid_visible);
}
