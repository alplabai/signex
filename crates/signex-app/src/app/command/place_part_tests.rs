//! #1767 — every entry that places a part without one in hand opens the
//! component picker instead of arming an empty-cursor tool.

use super::*;
use crate::active_bar::{ActiveBarAction, ActiveBarMsg};
use crate::app::{TabInfo, TabKind};
use crate::keymap::AppCommandId;
use signex_types::schematic::SchematicSheet;

fn app_with_sheet() -> Signex {
    let path = std::path::PathBuf::from("place_part.snxsch");
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "place_part".to_string(),
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
fn the_part_commands_open_the_picker_and_arm_no_tool() {
    for id in ["open_components_panel", "place_symbol", "place_component"] {
        let mut app = app_with_sheet();
        let command = AppCommandId::new(id).unwrap();
        let _ = app.dispatch_command(&command, CommandArgs::none(), None);
        assert!(app.library.picker.is_some(), "{id} opens the picker");
        assert_eq!(app.interaction_state.current_tool, Tool::Select, "{id}");
    }
}

#[test]
fn the_active_bar_part_button_opens_the_picker() {
    let mut app = app_with_sheet();
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::PlaceComponent,
    )));
    assert!(app.library.picker.is_some());
    assert_eq!(app.interaction_state.current_tool, Tool::Select);
}

#[test]
fn a_power_port_still_arms_the_component_tool() {
    let mut app = app_with_sheet();
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::PlacePowerGND,
    )));
    assert_eq!(app.interaction_state.current_tool, Tool::Component);
    assert!(app.interaction_state.pending_power.is_some());
    assert!(app.library.picker.is_none());
}
