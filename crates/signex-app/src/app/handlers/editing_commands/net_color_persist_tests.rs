//! #1041 — net colours are saved with their sheet: loaded when it opens,
//! dirtying it when they change, written (pruned) on save.

use super::super::super::*;
use signex_types::schematic::{Junction, SchematicSheet, Wire};
use signex_types::theme::Color;

const NAME: &str = "persist.snxsch";

fn color(r: u8) -> Color {
    Color::new(r, 0, 0, 255)
}

fn path() -> std::path::PathBuf {
    std::path::PathBuf::from(NAME)
}

fn sheet_with(
    wire: uuid::Uuid,
    junction: uuid::Uuid,
    saved: &[(uuid::Uuid, Color)],
) -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        wires: vec![Wire {
            uuid: wire,
            ..Default::default()
        }],
        junctions: vec![Junction {
            uuid: junction,
            ..Default::default()
        }],
        net_colors: saved.iter().copied().collect(),
        ..Default::default()
    }
}

/// An app with one active, clean schematic tab holding `sheet`.
fn app_with(sheet: SchematicSheet) -> Signex {
    let (mut app, _task) = Signex::new();
    app.document_state.tabs.push(crate::app::TabInfo {
        title: NAME.to_string(),
        path: path(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.apply_loaded_schematic(Some(sheet), false, false, false);
    app
}

fn is_dirty(app: &Signex) -> bool {
    app.document_state.tabs.iter().any(|t| t.dirty)
        && app.document_state.dirty_paths.contains(&path())
}

fn make_clean(app: &mut Signex) {
    app.document_state.tabs[0].dirty = false;
    app.document_state.dirty_paths.clear();
}

#[test]
fn opening_a_sheet_loads_its_saved_colours() {
    let (w, j) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let app = app_with(sheet_with(w, j, &[(w, color(1)), (j, color(2))]));
    assert_eq!(app.ui_state.wire_color_overrides.get(&w), Some(&color(1)));
    assert_eq!(app.ui_state.wire_color_overrides.get(&j), Some(&color(2)));
    assert!(!is_dirty(&app), "loading colours must not dirty the sheet");
}

#[test]
fn closing_forgets_and_reopening_restores() {
    let (w, j) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let sheet = sheet_with(w, j, &[(w, color(1))]);
    let mut app = app_with(sheet.clone());
    app.forget_net_colors_of(&path());
    assert!(app.ui_state.wire_color_overrides.is_empty());
    app.apply_loaded_schematic(Some(sheet), false, false, false);
    assert_eq!(app.ui_state.wire_color_overrides.get(&w), Some(&color(1)));
}

#[test]
fn reloading_replaces_the_old_engines_colours() {
    let (w, j) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let mut app = app_with(sheet_with(w, j, &[(w, color(1))]));
    app.apply_loaded_schematic(
        Some(sheet_with(w, j, &[(j, color(2))])),
        false,
        false,
        false,
    );
    assert!(!app.ui_state.wire_color_overrides.contains_key(&w));
    assert_eq!(app.ui_state.wire_color_overrides.get(&j), Some(&color(2)));
}

#[test]
fn a_pen_change_marks_the_sheet_dirty() {
    let (w, j) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let mut app = app_with(sheet_with(w, j, &[]));
    assert!(!is_dirty(&app));
    app.record_net_color_undo();
    app.ui_state.wire_color_overrides.insert(w, color(9));
    assert!(is_dirty(&app));
}

#[test]
fn clear_all_undo_and_redo_mark_the_sheet_dirty() {
    let (w, j) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let mut app = app_with(sheet_with(w, j, &[(w, color(1))]));
    app.clear_all_net_colors_undoably();
    assert!(is_dirty(&app), "Clear All");
    make_clean(&mut app);
    app.handle_undo_requested();
    assert!(app.ui_state.wire_color_overrides.contains_key(&w));
    assert!(is_dirty(&app), "undo of a colour step");
    make_clean(&mut app);
    app.handle_redo_requested();
    assert!(app.ui_state.wire_color_overrides.is_empty());
    assert!(is_dirty(&app), "redo of a colour step");
}

#[test]
fn a_colour_change_never_writes_to_disk() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join(NAME);
    let (w, j) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let mut app = app_with(sheet_with(w, j, &[]));
    app.document_state
        .engines
        .get_mut(&path())
        .expect("engine")
        .set_path(Some(target.clone()));
    app.record_net_color_undo();
    app.ui_state.wire_color_overrides.insert(w, color(9));
    assert!(!target.exists(), "only an explicit save writes");
}

#[test]
fn saving_writes_live_colours_and_prunes_the_stale() {
    let (w, j) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let stale = uuid::Uuid::new_v4();
    let mut app = app_with(sheet_with(w, j, &[]));
    app.ui_state.wire_color_overrides.insert(w, color(3));
    app.ui_state.wire_color_overrides.insert(stale, color(4));
    let engine = app.document_state.engines.get(&path()).expect("engine");
    let text = engine
        .serialize_with_net_colors(&app.ui_state.wire_color_overrides)
        .expect("serialize");
    let saved = signex_types::format::SnxSchematic::parse(&text)
        .expect("parse")
        .sheet;
    assert_eq!(saved.net_colors.len(), 1);
    assert_eq!(saved.net_colors.get(&w), Some(&color(3)));
}
