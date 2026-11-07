//! #1041 — net colours are saved with their sheet: loaded when it opens,
//! dirtying it when they change, written (pruned) on save.

use super::super::super::*;
use super::net_color_driver::{color, colour_of, paint};
use crate::app::{FileMsg, Message};
use signex_types::schematic::{Junction, SchematicSheet, Wire};
use signex_types::theme::Color;

const NAME: &str = "persist.snxsch";

fn path() -> std::path::PathBuf {
    std::path::PathBuf::from(NAME)
}

fn sheet_with(
    wire: uuid::Uuid,
    junction: uuid::Uuid,
    saved: &[(uuid::Uuid, Color)],
) -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
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
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.apply_loaded_schematic(Some(sheet), false, false, false);
    app
}

fn is_dirty(app: &Signex) -> bool {
    app.document_state.has_dirty_documents() && app.document_state.is_document_dirty(&path())
}

fn make_clean(app: &mut Signex) {
    {
        let path = app.document_state.tabs[0].path.clone();
        app.document_state.mark_document_clean(&path);
    }
    app.document_state.mark_documents_clean_where(|_| true);
}

fn saved(app: &Signex) -> SchematicSheet {
    let text = app.document_state.engines[&path()]
        .serialize()
        .expect("serialize");
    signex_types::format::SnxSchematic::parse(&text)
        .expect("parse")
        .sheet
}

#[test]
fn opening_a_sheet_loads_its_saved_colours() {
    let (w, j) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    let app = app_with(sheet_with(w, j, &[(w, color(1)), (j, color(2))]));
    assert_eq!(colour_of(&app, w), Some(color(1)));
    assert_eq!(colour_of(&app, j), Some(color(2)));
    assert!(!is_dirty(&app), "loading colours must not dirty the sheet");
}

#[test]
fn closing_forgets_and_reopening_restores() {
    let (w, j) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    let sheet = sheet_with(w, j, &[(w, color(1))]);
    let mut app = app_with(sheet.clone());
    app.document_state.remove_engine(&path());
    assert_eq!(colour_of(&app, w), None);
    app.apply_loaded_schematic(Some(sheet), false, false, false);
    assert_eq!(colour_of(&app, w), Some(color(1)));
}

#[test]
fn reloading_replaces_the_old_engines_colours() {
    let (w, j) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    let mut app = app_with(sheet_with(w, j, &[(w, color(1)), (j, color(2))]));
    app.apply_loaded_schematic(
        Some(sheet_with(w, j, &[(j, color(2))])),
        false,
        false,
        false,
    );
    assert_eq!(colour_of(&app, w), None);
    assert_eq!(colour_of(&app, j), Some(color(2)));
}

#[test]
fn a_pen_change_marks_the_sheet_dirty() {
    let (w, j) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    let mut app = app_with(sheet_with(w, j, &[]));
    assert!(!is_dirty(&app));
    paint(&mut app, &[w], color(9));
    assert!(is_dirty(&app));
}

#[test]
fn clear_all_undo_and_redo_mark_the_sheet_dirty() {
    let (w, j) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    let mut app = app_with(sheet_with(w, j, &[(w, color(1))]));
    app.clear_net_colors(None);
    assert!(is_dirty(&app), "Clear All");
    make_clean(&mut app);
    app.handle_undo_requested();
    assert_eq!(colour_of(&app, w), Some(color(1)));
    assert!(is_dirty(&app), "undo of a colour step");
    make_clean(&mut app);
    app.handle_redo_requested();
    assert_eq!(colour_of(&app, w), None);
    assert!(is_dirty(&app), "redo of a colour step");
}

#[test]
fn a_colour_change_never_writes_to_disk() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join(NAME);
    let (w, j) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    let mut app = app_with(sheet_with(w, j, &[]));
    app.document_state
        .engines
        .get_mut(&path())
        .expect("engine")
        .set_path(Some(target.clone()));
    paint(&mut app, &[w], color(9));
    assert!(!target.exists(), "only an explicit save writes");
}

#[test]
fn saving_writes_live_colours_and_prunes_the_stale() {
    let (w, j) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    let stale = uuid::Uuid::now_v7();
    let mut app = app_with(sheet_with(w, j, &[]));
    paint(&mut app, &[w], color(3));
    paint(&mut app, &[stale], color(4));
    let written = saved(&app);
    assert_eq!(written.net_colors.len(), 1);
    assert_eq!(written.net_colors.get(&w), Some(&color(3)));
}

fn land(app: &mut Signex, revision: u64) {
    let _ = app.update(Message::File(FileMsg::DocumentSaved(
        crate::persist::DocumentSaveDone {
            path: path(),
            revision,
            save_as_from: None,
            bytes: std::sync::Arc::new(b"saved".to_vec()),
            sequence: 0,
            result: Ok(crate::persist::SaveOutcome::Written),
            recovery_hold: None,
        },
    )));
}

/// A save snapshot: the engine revision it saw.
fn snapshot(app: &Signex) -> u64 {
    app.document_state.engines[&path()].revision()
}

#[test]
fn a_colour_change_during_a_save_keeps_the_sheet_dirty_until_the_next_save() {
    let (w, j) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    let mut app = app_with(sheet_with(w, j, &[]));
    paint(&mut app, &[w], color(1));
    let revision = snapshot(&app);

    // The write is in flight; the user recolours the net.
    paint(&mut app, &[w], color(2));
    land(&mut app, revision);
    assert!(is_dirty(&app), "the file lacks the second colour");

    // The next save snapshots the new colour and may clear the flag.
    assert_eq!(saved(&app).net_colors.get(&w), Some(&color(2)));
    let revision = snapshot(&app);
    land(&mut app, revision);
    assert!(!is_dirty(&app));
}

#[test]
fn a_save_with_no_colour_change_marks_the_sheet_clean() {
    let (w, j) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    let mut app = app_with(sheet_with(w, j, &[]));
    paint(&mut app, &[w], color(1));
    let revision = snapshot(&app);
    land(&mut app, revision);
    assert!(!is_dirty(&app));
}
