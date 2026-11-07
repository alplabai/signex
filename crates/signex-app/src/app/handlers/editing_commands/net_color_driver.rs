//! The one place the net-colour timeline tests touch how colours are stored,
//! so a change of owner edits this file and not every test.

use super::super::super::*;
use signex_types::schematic::{Junction, SchematicSheet, Symbol, Wire};
use signex_types::theme::Color;
use std::path::{Path, PathBuf};

pub(super) fn color(r: u8) -> Color {
    Color::new(r, 0, 0, 255)
}

/// A clean, active one-sheet app: one symbol, one wire, one junction.
pub(super) fn app_with_sheet(name: &str) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let path = PathBuf::from(name);
    add_sheet(&mut app, &path);
    (app, path)
}

/// Open another clean sheet at `path` and make it the active one.
pub(super) fn add_sheet(app: &mut Signex, path: &Path) {
    app.document_state.tabs.push(crate::app::TabInfo {
        title: path.display().to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.apply_loaded_schematic(Some(fixture_sheet()), false, false, false);
}

pub(super) fn fixture_sheet() -> SchematicSheet {
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

pub(super) fn wire_of(app: &Signex, path: &Path) -> uuid::Uuid {
    app.document_state.engines[path].document().wires[0].uuid
}

pub(super) fn symbol_of(app: &Signex, path: &Path) -> uuid::Uuid {
    app.document_state.engines[path].document().symbols[0].uuid
}

/// One colour action on the active sheet, as a canvas click makes it.
pub(super) fn paint(app: &mut Signex, ids: &[uuid::Uuid], color: Color) {
    assert!(app.record_net_color_undo(), "the sheet must take a colour");
    app.ui_state
        .wire_color_overrides
        .extend(ids.iter().map(|id| (*id, color)));
}

/// [`paint`] on the sheet at `path`, whichever is active.
pub(super) fn paint_on(app: &mut Signex, path: &Path, ids: &[uuid::Uuid], color: Color) {
    let previous = app.document_state.active_path.replace(path.to_path_buf());
    paint(app, ids, color);
    app.document_state.active_path = previous;
}

pub(super) fn colour_of(app: &Signex, id: uuid::Uuid) -> Option<Color> {
    app.ui_state.wire_color_overrides.get(&id).copied()
}

/// The colours a save of the sheet at `path` would write.
pub(super) fn saved_colours(
    app: &Signex,
    path: &Path,
) -> std::collections::HashMap<uuid::Uuid, Color> {
    let text = app.document_state.engines[path]
        .serialize_with_net_colors(&app.ui_state.wire_color_overrides)
        .expect("serialize");
    signex_types::format::SnxSchematic::parse(&text)
        .expect("parse")
        .sheet
        .net_colors
}

pub(super) fn is_dirty(app: &Signex, path: &Path) -> bool {
    app.document_state.is_document_dirty(path)
}

pub(super) fn activate(app: &mut Signex, path: &Path) {
    app.document_state.active_tab = app
        .document_state
        .tabs
        .iter()
        .position(|tab| tab.path == path)
        .expect("tab");
    app.document_state.active_path = Some(path.to_path_buf());
}
