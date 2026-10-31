//! #1767 — Place > Component… opens the component picker instead of
//! only arming a tool with nothing on the cursor.

use signex_app::app::{Message, Signex, TabInfo, TabKind, Tool};
use signex_app::menu_bar::MenuMessage;
use signex_types::schematic::SchematicSheet;
use std::path::PathBuf;

fn app_with_empty_sheet() -> Signex {
    let path = PathBuf::from("place_component.snxsch");
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _initial_task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "place_component".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

#[test]
fn place_menu_component_opens_the_picker() {
    let mut app = app_with_empty_sheet();
    assert!(app.library.picker.is_none());
    let _ = app.update(Message::Menu(MenuMessage::PlaceComponent));
    assert!(app.library.picker.is_some());
    assert_eq!(app.interaction_state.current_tool, Tool::Select);
}

#[test]
fn library_menu_place_component_opens_the_same_picker() {
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::Menu(MenuMessage::LibraryPlaceComponent));
    assert!(app.library.picker.is_some());
    assert_eq!(app.interaction_state.current_tool, Tool::Select);
}
