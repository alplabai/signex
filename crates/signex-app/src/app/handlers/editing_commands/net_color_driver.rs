//! The one place the net-colour timeline tests touch how colours are stored,
//! so a change of owner edits this file and not every test.

use super::super::super::*;
use signex_types::schematic::{Junction, SchematicSheet, Symbol, Wire};
use signex_types::theme::Color;
use std::path::{Path, PathBuf};

pub(crate) fn color(r: u8) -> Color {
    Color::new(r, 0, 0, 255)
}

/// A clean, active one-sheet app: one symbol, one wire, one junction.
pub(crate) fn app_with_sheet(name: &str) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let path = PathBuf::from(name);
    add_sheet(&mut app, &path);
    (app, path)
}

/// Open another clean sheet at `path` and make it the active one.
pub(crate) fn add_sheet(app: &mut Signex, path: &Path) {
    add_sheet_with(app, path, fixture_sheet());
}

/// [`add_sheet`] holding `sheet`.
pub(crate) fn add_sheet_with(app: &mut Signex, path: &Path, sheet: SchematicSheet) {
    app.document_state.tabs.push(crate::app::TabInfo {
        title: path.display().to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.apply_loaded_schematic(Some(sheet), false, false, false);
}

pub(crate) fn fixture_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: uuid::Uuid::now_v7(),
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            ..Default::default()
        }],
        wires: vec![Wire {
            uuid: uuid::Uuid::now_v7(),
            ..Default::default()
        }],
        junctions: vec![Junction {
            uuid: uuid::Uuid::now_v7(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

pub(crate) fn wire_of(app: &Signex, path: &Path) -> uuid::Uuid {
    app.document_state.engines[path].document().wires[0].uuid
}

pub(crate) fn symbol_of(app: &Signex, path: &Path) -> uuid::Uuid {
    app.document_state.engines[path].document().symbols[0].uuid
}

/// One colour action on the active sheet, as a canvas click makes it.
pub(crate) fn paint(app: &mut Signex, ids: &[uuid::Uuid], color: Color) {
    let (outcome, why) = app.commit_net_colors(ids, color);
    assert_eq!(
        outcome,
        crate::app::mutation_gateway::CommandOutcome::Changed,
        "the sheet must take a colour: {why:?}"
    );
}

/// [`paint`] on the sheet at `path`, whichever is active.
pub(crate) fn paint_on(app: &mut Signex, path: &Path, ids: &[uuid::Uuid], color: Color) {
    let previous = app.document_state.active_path.replace(path.to_path_buf());
    paint(app, ids, color);
    app.document_state.active_path = previous;
}

/// The colour any open sheet holds for `id`.
pub(crate) fn colour_of(app: &Signex, id: uuid::Uuid) -> Option<Color> {
    app.document_state
        .engines
        .values()
        .find_map(|engine| engine.document().net_colors.get(&id).copied())
}

/// The colours a save of the sheet at `path` would write.
pub(crate) fn saved_colours(
    app: &Signex,
    path: &Path,
) -> std::collections::HashMap<uuid::Uuid, Color> {
    let text = app.document_state.engines[path]
        .serialize()
        .expect("serialize");
    signex_types::format::SnxSchematic::parse(&text)
        .expect("parse")
        .sheet
        .net_colors
}

pub(crate) fn is_dirty(app: &Signex, path: &Path) -> bool {
    app.document_state.is_document_dirty(path)
}

pub(crate) fn activate(app: &mut Signex, path: &Path) {
    app.document_state.active_tab = app
        .document_state
        .tabs
        .iter()
        .position(|tab| tab.path == path)
        .expect("tab");
    app.document_state.active_path = Some(path.to_path_buf());
}
