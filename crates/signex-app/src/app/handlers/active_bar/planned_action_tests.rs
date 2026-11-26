//! #1248 / #1249 - a planned tool (Reuse Block) must not be remembered as
//! its group's last tool, and no dispatch path may place anything for it.
//! Net Parameters, the tool #1248 was about, is live since #1489; its own
//! tests are in `tests/regression/schematic_directives.rs`.

use super::super::super::*;
use crate::active_bar::{ActiveBarAction, ActiveBarMsg};
use crate::app::command::CommandArgs;
use crate::keymap::AppCommandId;
use signex_types::schematic::{LabelType, SchematicSheet};

fn app_with_empty_sheet() -> Signex {
    let path = std::path::PathBuf::from("param-set-unit.snxsch");
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "param-set-unit".to_string(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

/// Drive every dispatch path that can name Reuse Block, then click twice
/// on the canvas; nothing may land on the sheet or be remembered.
fn assert_clean(app: &mut Signex, label: &str) {
    for (x, y) in [(10.0, 10.0), (30.0, 30.0)] {
        let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
            world_x: x,
            world_y: y,
        }));
    }
    let doc = app
        .document_state
        .engines
        .values()
        .next()
        .unwrap()
        .document();
    assert!(
        doc.labels.iter().all(|l| l.label_type != LabelType::Net),
        "{label}: a Net label was created: {:?}",
        doc.labels
    );
    assert!(doc.labels.is_empty(), "{label}: label created");
    assert!(doc.child_sheets.is_empty(), "{label}: sheet symbol created");
    assert!(doc.directives.is_empty(), "{label}: directive created");
    assert!(
        !app.interaction_state
            .last_tool
            .values()
            .any(crate::active_bar::is_planned),
        "{label}: planned action recorded in last_tool"
    );
    assert!(
        !app.interaction_state.last_tool.contains_key("sheet"),
        "{label}: sheet group was updated"
    );
}

#[test]
fn active_bar_path_neither_places_nor_records() {
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::PlaceReuseBlock,
    )));
    assert_clean(&mut app, "active bar");
}

#[test]
fn context_menu_path_neither_places_nor_records() {
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Action(
        ContextAction::Command {
            id: AppCommandId::new("place_reuse_block").unwrap(),
            project: None,
        },
    )));
    assert_clean(&mut app, "context menu");
}

#[test]
fn palette_and_keymap_command_path_neither_places_nor_records() {
    // The command palette and keymap profiles both resolve the stable
    // command id through `dispatch_command`.
    let mut app = app_with_empty_sheet();
    let id = AppCommandId::new("place_reuse_block").unwrap();
    let _ = app.dispatch_command(&id, CommandArgs::none(), None);
    assert_clean(&mut app, "palette/keymap");
}

#[test]
fn no_default_keymap_binding_reaches_a_live_reuse_block() {
    // Whatever the profile, the id only ever resolves to the inert action.
    assert_eq!(
        crate::app::command::active_bar::action_for_id("place_reuse_block"),
        Some(ActiveBarAction::PlaceReuseBlock)
    );
    assert!(crate::active_bar::is_planned(
        &ActiveBarAction::PlaceReuseBlock
    ));
}

#[test]
fn net_parameters_is_live_and_recorded_with_the_directives() {
    let mut app = app_with_empty_sheet();
    assert!(!crate::active_bar::is_planned(
        &ActiveBarAction::PlaceParameterSet
    ));
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::PlaceParameterSet,
    )));
    assert_eq!(app.interaction_state.current_tool, Tool::NetParameters);
    assert_eq!(
        app.interaction_state.last_tool.get("directives"),
        Some(&ActiveBarAction::PlaceParameterSet)
    );
}

#[test]
fn other_directives_still_recorded() {
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::PlaceNoERC,
    )));
    assert_eq!(
        app.interaction_state.last_tool.get("directives"),
        Some(&ActiveBarAction::PlaceNoERC)
    );
}
