//! #1822 — undoing back to the last saved state makes the document clean
//! again (tab dot, title count); redo and edits below the save point do not.

use super::super::super::*;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind, Symbol};

const NAME: &str = "save-point.snxsch";

fn app_with_symbol() -> (Signex, uuid::Uuid) {
    let (mut app, _task) = Signex::new();
    let id = uuid::Uuid::now_v7();
    let path = std::path::PathBuf::from(NAME);
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: id,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(crate::app::TabInfo {
        title: NAME.to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    (app, id)
}

fn move_symbol(app: &mut Signex, id: uuid::Uuid) {
    let moved = app.apply_engine_command(
        signex_engine::Command::MoveSelection {
            items: vec![SelectedItem::new(id, SelectedKind::Symbol)],
            dx: 10.0,
            dy: 0.0,
            keep_wires: false,
        },
        true,
        true,
    );
    assert!(moved, "fixture: the move must change the document");
}

fn set_field(app: &mut Signex, id: uuid::Uuid, value: &str) {
    let _task = app.handle_parameter_manager_edit(id, "MPN".to_string(), value.to_string());
}

fn path() -> std::path::PathBuf {
    std::path::PathBuf::from(NAME)
}

/// What the tab dot and the title-bar "N unsaved" read.
fn shows_dirty(app: &Signex) -> bool {
    let tab_dot = app.document_state.dirty_document_paths().contains(&path());
    let title_count = app.document_state.dirty_document_count();
    assert_eq!(tab_dot, title_count == 1, "tab dot and title count agree");
    tab_dot
}

/// A save that landed for the current revision.
fn save(app: &mut Signex) {
    app.document_state.mark_document_clean(&path());
}

#[test]
fn undoing_the_only_edit_since_the_save_is_clean_and_redo_is_dirty() {
    let (mut app, id) = app_with_symbol();
    assert!(!shows_dirty(&app));
    move_symbol(&mut app, id);
    assert!(shows_dirty(&app));
    app.handle_undo_requested();
    assert!(!shows_dirty(&app), "back at the loaded state");
    app.handle_redo_requested();
    assert!(shows_dirty(&app));
}

#[test]
fn save_then_edit_then_undo_is_clean() {
    let (mut app, id) = app_with_symbol();
    move_symbol(&mut app, id);
    save(&mut app);
    assert!(!shows_dirty(&app));
    move_symbol(&mut app, id);
    assert!(shows_dirty(&app));
    app.handle_undo_requested();
    assert!(!shows_dirty(&app), "back at the saved state");
    app.handle_undo_requested();
    assert!(shows_dirty(&app), "below the saved state");
}

#[test]
fn an_edit_after_undoing_past_the_save_point_can_never_be_undone_clean() {
    let (mut app, id) = app_with_symbol();
    move_symbol(&mut app, id);
    save(&mut app);
    app.handle_undo_requested();
    move_symbol(&mut app, id);
    assert!(shows_dirty(&app));
    app.handle_undo_requested();
    assert!(
        shows_dirty(&app),
        "the saved state is gone from the history"
    );
    app.handle_redo_requested();
    assert!(shows_dirty(&app));
    save(&mut app);
    assert!(!shows_dirty(&app));
}

#[test]
fn a_cell_run_that_amends_the_saved_entry_stays_dirty() {
    let (mut app, id) = app_with_symbol();
    set_field(&mut app, id, "A");
    save(&mut app);
    set_field(&mut app, id, "AB");
    assert!(shows_dirty(&app));
    app.handle_undo_requested();
    assert!(
        shows_dirty(&app),
        "the file holds \"A\", which undo cannot restore"
    );
}

#[test]
fn undo_does_not_clear_a_mark_the_history_did_not_set() {
    let (mut app, id) = app_with_symbol();
    move_symbol(&mut app, id);
    app.handle_undo_requested();
    assert!(!shows_dirty(&app));
    // A net colour (or any other change outside the engine history).
    app.document_state.mark_document_dirty_display_only(&path());
    move_symbol(&mut app, id);
    app.handle_undo_requested();
    assert!(shows_dirty(&app), "the outside mark survives the undo");
    save(&mut app);
    assert!(!shows_dirty(&app));
}
