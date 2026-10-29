//! #1112 — a save queued behind a write to another spelling of the same file
//! still runs: the follow-up queue keeps one entry per asking spelling.

use std::path::{Path, PathBuf};

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex, SymbolDocument, TabInfo, TabKind};
use crate::persist::run_task::outputs;

fn add_dirty_symbol(app: &mut Signex, path: &Path) {
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("Sym1"));
    let mut editor = SymbolDocument::new(path.to_path_buf(), file);
    editor.mark_dirty();
    app.document_state
        .symbol_editors
        .insert(path.to_path_buf(), editor);
    app.document_state.mark_document_dirty(path);
    app.document_state.tabs.push(TabInfo {
        title: "Lib".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.to_path_buf()),
    });
}

/// Two editors for two spellings of one container name.
fn app_with_twin_symbols(dir: &Path) -> (Signex, PathBuf, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let (upper, lower) = (dir.join("Lib.snxsym"), dir.join("lib.snxsym"));
    add_dirty_symbol(&mut app, &upper);
    add_dirty_symbol(&mut app, &lower);
    (app, upper, lower)
}

fn completion(task: iced::Task<Message>) -> crate::persist::PrimitiveSaveDone {
    match outputs(task).pop() {
        Some(Message::File(crate::app::FileMsg::PrimitiveSaved(done))) => *done,
        other => panic!("expected a container save completion, got {other:?}"),
    }
}

fn is_clean(app: &Signex, path: &Path) -> bool {
    !app.document_state.symbol_editors[path].is_dirty()
        && !app.document_state.is_document_dirty(path)
}

#[test]
fn a_save_queued_behind_a_sibling_spellings_write_still_runs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = app_with_twin_symbols(dir.path());
    let running = app.save_primitive_tab_at(&upper).expect("save starts");
    let queued = app.save_primitive_tab_at(&lower).expect("queued");
    assert_eq!(queued.units(), 0, "nothing starts while the write runs");

    let follow_up = app.handle_primitive_saved(completion(running));
    assert_eq!(follow_up.units(), 1, "the sibling's save starts");
    let _ = app.handle_primitive_saved(completion(follow_up));

    assert!(is_clean(&app, &upper));
    assert!(is_clean(&app, &lower), "the queued spelling was saved");
}

#[test]
fn follow_ups_from_both_spellings_both_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = app_with_twin_symbols(dir.path());
    let running = app.save_primitive_tab_at(&upper).expect("save starts");
    for path in [&upper, &lower] {
        // An edit made while the write runs: both drafts have more to save.
        app.document_state
            .symbol_editors
            .get_mut(path)
            .expect("editor")
            .set_dirty(true);
        let queued = app.save_primitive_tab_at(path).expect("queued");
        assert_eq!(queued.units(), 0);
    }
    let first = app.handle_primitive_saved(completion(running));
    assert_eq!(first.units(), 1);
    let second = app.handle_primitive_saved(completion(first));
    assert_eq!(second.units(), 1, "the other spelling's follow-up runs too");
    let _ = app.handle_primitive_saved(completion(second));

    assert!(is_clean(&app, &upper));
    assert!(is_clean(&app, &lower));
}
