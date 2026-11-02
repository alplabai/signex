//! #1903 — a restore that replaces an open symbol editor must make a
//! pending delete warning stale: the new document starts a fresh history,
//! so the warning cannot tell it from the one it described.

use crate::app::restore_job::RestoreJobDocument;
use crate::app::{Signex, SymbolDocument, TabInfo, TabKind};
use crate::library::symbol_delete::PendingSymbolDelete;
use crate::panels::SymbolDeleteWarning;
use signex_library::{Symbol, SymbolFile};
use std::path::PathBuf;

fn app_with_warning() -> (Signex, PathBuf, SymbolFile) {
    let (mut app, _task) = Signex::new();
    let path = PathBuf::from("lib.snxsym");
    let mut file = SymbolFile::from_symbol(Symbol::empty("A"));
    file.symbols.push(Symbol::empty("B"));
    // No edit before the warning: it is raised at generation 0, the same
    // generation a freshly built restored document starts at.
    let editor = SymbolDocument::new(path.clone(), file.clone());
    let symbol = editor.file.symbols[0].uuid;
    app.library.pending_symbol_delete = Some(PendingSymbolDelete {
        path: path.clone(),
        symbol,
        history: editor.history_generation,
        rows: 1,
        sheets: 0,
        unchecked: 0,
        warning: SymbolDeleteWarning {
            idx: 0,
            name: "A".into(),
            only_unknown: false,
            lines: vec!["Component rows: R1".into()],
        },
    });
    app.document_state
        .symbol_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "lib".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    (app, path, file)
}

fn warning_is_current(app: &Signex, path: &PathBuf) -> bool {
    let pending = app.library.pending_symbol_delete.as_ref().unwrap();
    pending.is_current(&app.document_state.symbol_editors[path])
}

#[test]
fn a_history_restore_makes_the_delete_warning_stale() {
    let (mut app, path, file) = app_with_warning();
    assert!(warning_is_current(&app, &path));

    let _ = app.apply_reloaded_with(
        0,
        Ok(RestoreJobDocument::Symbol(Box::new(file))),
        None,
        false,
    );

    assert!(!warning_is_current(&app, &path));
}

#[test]
fn an_autosave_copy_restore_makes_the_delete_warning_stale() {
    let (mut app, path, file) = app_with_warning();
    assert!(warning_is_current(&app, &path));

    assert!(app.restore_symbol_copy(&path, file));

    assert!(!warning_is_current(&app, &path));
}
