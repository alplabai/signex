//! #1854: a symbol draft reads clean again when Undo returns to the content
//! on disk, and never otherwise.

use std::path::PathBuf;

use signex_library::{Symbol, SymbolFile};

use super::SymbolDocument;

fn editor() -> SymbolDocument {
    SymbolDocument::new(
        PathBuf::from("Lib.snxsym"),
        SymbolFile::from_symbol(Symbol::empty("Sym1")),
    )
}

/// One recorded edit that renames the active symbol.
fn rename(editor: &mut SymbolDocument, name: &str) {
    editor.record_edit(None, |e| {
        e.primitive_mut().name = name.into();
        e.mark_dirty();
        true
    });
}

#[test]
fn undo_back_to_the_opened_state_is_clean_and_redo_is_dirty() {
    let mut editor = editor();
    rename(&mut editor, "A");
    assert!(editor.is_dirty());

    assert!(editor.undo());
    assert!(!editor.is_dirty());
    assert!(editor.redo());
    assert!(editor.is_dirty());
}

#[test]
fn undo_back_to_the_saved_state_is_clean() {
    let mut editor = editor();
    rename(&mut editor, "A");
    editor.mark_saved();
    assert!(!editor.is_dirty());
    rename(&mut editor, "B");
    assert!(editor.is_dirty());

    assert!(editor.undo());
    assert!(!editor.is_dirty(), "the saved state");
    assert!(editor.undo());
    assert!(editor.is_dirty(), "before the save is not the file");
}

#[test]
fn an_edit_after_undoing_below_the_save_point_forgets_it() {
    let mut editor = editor();
    rename(&mut editor, "A");
    editor.mark_saved();
    assert!(editor.undo());
    rename(&mut editor, "B");
    assert!(editor.undo());
    assert!(editor.is_dirty(), "no step reproduces the saved content");
    assert!(editor.redo());
    assert!(editor.is_dirty());
}

#[test]
fn an_edit_coalesced_into_the_saved_step_forgets_the_save_point() {
    let mut editor = editor();
    let key = Some(("name", 0));
    editor.record_edit(key, |e| {
        e.mark_dirty();
        true
    });
    editor.mark_saved();
    editor.record_edit(key, |e| {
        e.primitive_mut().name = "typed on".into();
        e.mark_dirty();
        true
    });
    assert_eq!(editor.history.undo_len(), 1, "one coalesced step");

    assert!(editor.undo());
    assert!(editor.redo());
    assert!(editor.is_dirty(), "the redone state is not what was saved");
}

#[test]
fn undoing_past_an_evicted_save_point_stays_dirty() {
    let mut editor = editor();
    for n in 0..=SymbolDocument::HISTORY_DEPTH {
        rename(&mut editor, &format!("edit {n}"));
    }
    while editor.undo() {}
    assert!(editor.is_dirty(), "the opened state fell off the history");
}

#[test]
fn a_recovery_copy_never_reads_clean_after_undo() {
    let mut editor = editor();
    editor.mark_dirty(); // the autosave recovery restore
    rename(&mut editor, "A");
    assert!(editor.undo());
    assert!(editor.is_dirty(), "the recovered content is not on disk");
}

#[test]
fn rows_still_owed_keep_an_undone_draft_dirty() {
    let mut editor = editor();
    rename(&mut editor, "A");
    editor.cascade_owed = vec![uuid::Uuid::now_v7()];
    assert!(editor.undo());
    assert!(editor.is_dirty());
}

#[test]
fn an_undo_during_a_save_in_flight_keeps_the_document_dirty() {
    let (mut app, _task) = crate::app::Signex::new();
    let state = &mut app.document_state;
    let path = PathBuf::from("Lib.snxsym");
    let mut editor = editor();
    rename(&mut editor, "A");
    // The save snapshot: the flag resets, the set holds the path.
    editor.mark_saved();
    state.symbol_editors.insert(path.clone(), editor);
    state.keep_document_dirty_until_saved(&path);
    rename(state.symbol_editors.get_mut(&path).unwrap(), "B");

    assert!(state.symbol_editors.get_mut(&path).unwrap().undo());
    assert!(
        !state.symbol_editors[&path].is_dirty(),
        "the draft is back at the snapshot"
    );
    assert!(
        state.is_document_dirty(&path),
        "the write has not landed: the hold keeps it dirty"
    );

    state.mark_document_clean(&path);
    assert!(!state.is_document_dirty(&path));
}

/// `mark_document_clean` says the draft equals disk now: like an engine, the
/// draft's save point must move to the current depth.
#[test]
fn a_draft_marked_clean_by_the_workspace_does_not_read_clean_at_an_older_depth() {
    let (mut app, _task) = crate::app::Signex::new();
    let state = &mut app.document_state;
    let path = PathBuf::from("Lib.snxsym");
    let mut editor = editor();
    rename(&mut editor, "A"); // depth 1; save point still the opened depth 0
    state.symbol_editors.insert(path.clone(), editor);

    state.mark_document_clean(&path); // disk now holds depth 1 ("A")
    assert!(!state.is_document_dirty(&path));

    assert!(state.symbol_editors.get_mut(&path).unwrap().undo());
    assert!(
        state.is_document_dirty(&path),
        "depth 0 (Sym1) is not what disk holds (A), yet the draft reads clean"
    );
}
