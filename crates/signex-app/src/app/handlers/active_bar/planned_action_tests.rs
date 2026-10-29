//! #1248 - the planned Parameter Set action must not be remembered as the
//! Directives group's last tool, and no dispatch path may create a Net
//! label named `PARAM=VALUE`.

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
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

/// Drive every dispatch path that can name Parameter Set, then click twice
/// on the canvas; return the doc's labels.
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
    assert!(
        !app.interaction_state
            .last_tool
            .values()
            .any(crate::active_bar::is_planned),
        "{label}: planned action recorded in last_tool"
    );
    assert!(
        !app.interaction_state.last_tool.contains_key("directives"),
        "{label}: directives group was updated"
    );
}

#[test]
fn active_bar_path_neither_labels_nor_records() {
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::PlaceParameterSet,
    )));
    assert_clean(&mut app, "active bar");
}

#[test]
fn context_menu_path_neither_labels_nor_records() {
    let mut app = app_with_empty_sheet();
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Action(
        ContextAction::ActiveBar(ActiveBarAction::PlaceParameterSet),
    )));
    assert_clean(&mut app, "context menu");
}

#[test]
fn palette_and_keymap_command_path_neither_labels_nor_records() {
    // The command palette and keymap profiles both resolve the stable
    // command id through `dispatch_command`.
    let mut app = app_with_empty_sheet();
    let id = AppCommandId::new("place_parameter_set").unwrap();
    let _ = app.dispatch_command(&id, CommandArgs::none(), None);
    assert_clean(&mut app, "palette/keymap");
}

#[test]
fn no_default_keymap_binding_reaches_parameter_set_label() {
    // Whatever the profile, the id only ever resolves to the inert action.
    assert_eq!(
        crate::app::command::active_bar::action_for_id("place_parameter_set"),
        Some(ActiveBarAction::PlaceParameterSet)
    );
    assert!(crate::active_bar::is_planned(
        &ActiveBarAction::PlaceParameterSet
    ));
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
