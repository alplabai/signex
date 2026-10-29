//! #1728 — a Close Library deferred behind a running write. Every test
//! finishes the close through the real completion path (a write, a
//! container save), never by calling the completion check by hand.

use super::*;
use crate::app::{SymbolDocument, TabInfo, TabKind};

fn closed(f: &Fixture) -> bool {
    f.app.library.library_at(&f.file).is_none()
}

fn send(f: &mut Fixture, msg: LibraryMessage) {
    let task = f.app.update(Message::Library(msg));
    run_task(&mut f.app, task);
}

fn request_close(f: &mut Fixture) {
    send(f, LibraryMessage::CloseLibrary(f.file.clone()));
}

/// The fixture with a library name no other test logs, so the shared
/// Messages ring can be searched for it.
fn named_fixture() -> Fixture {
    let mut f = fixture();
    f.app.library.open_libraries[0].display_name = format!("Lib-{}", Uuid::now_v7());
    f
}

/// A Messages-panel line that names the library `name` and says `what`.
fn logged(name: &str, what: &str) -> bool {
    crate::diagnostics::recent_entries()
        .iter()
        .any(|entry| entry.message.contains(what) && entry.message.contains(name))
}

#[test]
fn a_dirty_editor_opened_while_the_close_waits_is_kept_and_the_user_is_asked() {
    let _ = crate::diagnostics::init_logging();
    let mut f = named_fixture();
    let name = f.app.library.library_label(&f.file);
    let write = start_cell_edit(&mut f);
    request_close(&mut f);
    assert!(!closed(&f), "the close waits for the write");

    let address = f.open_preview(1);
    let editor = f.app.library.editors.get_mut(&address).unwrap();
    editor.row.description = "typed while the close waited".into();
    editor.dirty = true;

    finish(&mut f.app, write);

    assert!(!closed(&f), "a dirty editor holds the close");
    assert_eq!(
        f.app.library.editors[&address].row.description, "typed while the close waited",
        "the unsaved edit survives"
    );
    let confirm = f
        .app
        .library
        .close_library_confirm
        .as_ref()
        .expect("the user is asked, as for a Close with unsaved editors");
    assert_eq!(confirm.dirty_editors, vec![address]);
    assert!(
        f.app.library.closing.is_empty(),
        "the deferred close is over"
    );
    assert!(logged(&name, "unsaved"), "and Messages says why");
}

#[test]
fn a_deferred_close_and_its_completion_reach_messages() {
    let _ = crate::diagnostics::init_logging();
    let mut f = named_fixture();
    let name = f.app.library.library_label(&f.file);
    let write = start_cell_edit(&mut f);
    request_close(&mut f);
    assert!(logged(&name, "closes once"), "the deferral is visible");

    finish(&mut f.app, write);

    assert!(closed(&f));
    assert!(logged(&name, "closed"), "and so is the completion");
}

#[test]
fn reopening_a_library_cancels_its_pending_close() {
    let _ = crate::diagnostics::init_logging();
    let mut f = named_fixture();
    let name = f.app.library.library_label(&f.file);
    let write = start_cell_edit(&mut f);
    request_close(&mut f);

    let reopen = LibraryMessage::OpenLibraryAt(Some(f.file.clone()));
    send(&mut f, reopen);
    assert!(f.app.library.closing.is_empty(), "the reopen cancels it");

    finish(&mut f.app, write);
    assert!(!closed(&f), "the library the user reopened stays open");
    assert!(logged(&name, "cancelled"));
}

#[test]
fn a_discard_all_close_does_not_ask_again_when_it_completes() {
    let mut f = fixture();
    let address = f.open_preview(0);
    f.app.library.editors.get_mut(&address).unwrap().dirty = true;
    let write = start_cell_edit(&mut f);
    request_close(&mut f);
    send(
        &mut f,
        LibraryMessage::CloseLibraryConfirm(
            crate::library::messages::CloseLibraryChoice::DiscardAll,
        ),
    );

    finish(&mut f.app, write);

    assert!(closed(&f), "the discarded editor does not hold the close");
    assert!(f.app.library.close_library_confirm.is_none());
}

/// A dirty symbol editor (and its tab) for a container inside the library.
fn dirty_symbol_in_library(f: &mut Fixture) -> PathBuf {
    let dir = f.file.parent().unwrap().join("symbols");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Held.snxsym");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("Held"));
    let mut editor = SymbolDocument::new(path.clone(), file);
    editor.mark_dirty();
    let documents = &mut f.app.document_state;
    documents.symbol_editors.insert(path.clone(), editor);
    documents.mark_document_dirty(&path);
    documents.tabs.push(TabInfo {
        title: "Held".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    path
}

#[test]
fn a_close_deferred_behind_a_container_save_completes_when_the_save_lands() {
    let mut f = fixture();
    let path = dirty_symbol_in_library(&mut f);
    let save = f.app.save_primitive_tab_at(&path).expect("save starts");
    assert_eq!(
        f.app.library.writes.pending(&f.file),
        1,
        "the save holds a turn"
    );
    request_close(&mut f);
    assert!(!closed(&f), "the close waits for the container save");

    run_task(&mut f.app, save);

    assert!(path.exists(), "the container was written");
    assert!(closed(&f), "and the close finished with it");
}
