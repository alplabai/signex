//! #1245 - the Parameter Manager was built but nothing could open it.
//!
//! It is now a catalog command (`open_parameter_manager`) that the Design
//! menu row, the command palette and a keymap binding all resolve, and an
//! edit made in its table reaches dispatch and is undoable.

use signex_app::app::command_palette::{CommandAction, build_catalog};
use signex_app::app::{CommandPaletteMsg, EditMsg, Message, ParameterManagerMsg, Signex};
use signex_app::keymap::{AppCommandId, all_command_ids, metadata_for};

use crate::dirty_paths_gateway::{app_with, symbol};

const COMMAND: &str = "open_parameter_manager";

fn palette_ids(app: &Signex) -> Vec<String> {
    build_catalog(app)
        .iter()
        .filter_map(|entry| match &entry.action {
            CommandAction::Command(id) => Some(id.as_str().to_string()),
            _ => None,
        })
        .collect()
}

fn field(app: &Signex, key: &str) -> Option<String> {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols[0]
        .fields
        .get(key)
        .cloned()
}

#[test]
fn the_command_is_in_the_catalog() {
    assert!(all_command_ids().any(|id| id == COMMAND));
    let meta = metadata_for(&AppCommandId::new(COMMAND).unwrap()).expect("catalog entry");
    assert_eq!(meta.menu_label(), "Parameter Manager...");
}

#[test]
fn the_palette_lists_it_when_a_schematic_is_open() {
    let (start, _t) = Signex::new();
    assert!(
        !palette_ids(&start).iter().any(|id| id == COMMAND),
        "nothing to tabulate on the start screen"
    );

    let (app, _path) = app_with(vec![symbol(uuid::Uuid::now_v7(), "R1", 0.0)]);
    assert!(
        palette_ids(&app).iter().any(|id| id == COMMAND),
        "the palette must offer Open Parameter Manager"
    );
}

#[test]
fn the_command_opens_the_manager_and_a_cell_edit_is_undoable() {
    let target = uuid::Uuid::now_v7();
    let (mut app, _path) = app_with(vec![symbol(target, "R1", 0.0)]);

    // Seed a parameter so the table has a column to edit.
    let _ = app.update(Message::ParameterManager(ParameterManagerMsg::Edit {
        symbol_uuid: target,
        key: "Tolerance".into(),
        value: "5%".into(),
    }));
    let _ = app.update(Message::ParameterManager(
        ParameterManagerMsg::CellSubmitted,
    ));

    // Typed into the palette and run with Enter.
    assert!(!app.ui_state.parameter_manager_open);
    let _ = app.update(Message::CommandPalette(CommandPaletteMsg::Open));
    let _ = app.update(Message::CommandPalette(CommandPaletteMsg::QueryChanged(
        "Parameter Manager".into(),
    )));
    let _ = app.update(Message::CommandPalette(CommandPaletteMsg::ExecuteSelected));
    assert!(app.ui_state.parameter_manager_open);

    let _ = app.update(Message::ParameterManager(ParameterManagerMsg::Edit {
        symbol_uuid: target,
        key: "Tolerance".into(),
        value: "1%".into(),
    }));
    assert_eq!(field(&app, "Tolerance").as_deref(), Some("1%"));

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(field(&app, "Tolerance").as_deref(), Some("5%"));
}
