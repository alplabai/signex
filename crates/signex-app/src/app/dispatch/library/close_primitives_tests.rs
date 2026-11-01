//! #1843 — the Close Library prompt lists the library's dirty symbol and
//! footprint editors, and Save All / Discard All act on them.

use super::*;
use crate::app::{SymbolDocument, TabInfo, TabKind};
use crate::library::messages::CloseLibraryChoice;

fn send(f: &mut Fixture, msg: LibraryMessage) {
    let task = f.app.update(Message::Library(msg));
    run_task(&mut f.app, task);
}

/// A dirty symbol editor (with its tab) at `path`.
fn open_dirty_symbol(f: &mut Fixture, path: &std::path::Path) {
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("Sym1"));
    f.app.document_state.symbol_editors.insert(
        path.to_path_buf(),
        SymbolDocument::new(path.to_path_buf(), file),
    );
    f.app.document_state.tabs.push(TabInfo {
        title: "Sym1".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.to_path_buf()),
    });
    f.app.document_state.mark_document_dirty(path);
}

fn symbol_in_library(f: &Fixture, name: &str) -> PathBuf {
    f.file.parent().unwrap().join("symbols").join(name)
}

fn request_close(f: &mut Fixture) {
    send(f, LibraryMessage::CloseLibrary(f.file.clone()));
}

#[test]
fn a_dirty_symbol_alone_raises_the_close_prompt() {
    let mut f = fixture();
    let symbol = symbol_in_library(&f, "s.snxsym");
    open_dirty_symbol(&mut f, &symbol);

    request_close(&mut f);

    let confirm = f.app.library.close_library_confirm.as_ref().expect("asked");
    assert_eq!(confirm.dirty_paths, vec![symbol]);
    assert!(f.app.library.library_at(&f.file).is_some(), "still mounted");
}

#[test]
fn the_prompt_lists_only_this_librarys_symbols() {
    let mut f = fixture();
    let mine = symbol_in_library(&f, "mine.snxsym");
    let other = f._dir.path().join("elsewhere").join("other.snxsym");
    open_dirty_symbol(&mut f, &mine);
    open_dirty_symbol(&mut f, &other);

    request_close(&mut f);

    let confirm = f.app.library.close_library_confirm.as_ref().expect("asked");
    assert_eq!(confirm.dirty_paths, vec![mine]);
}

#[test]
fn save_all_writes_the_dirty_symbol_and_closes_the_library() {
    let mut f = fixture();
    let symbol = symbol_in_library(&f, "s.snxsym");
    std::fs::create_dir_all(symbol.parent().unwrap()).unwrap();
    open_dirty_symbol(&mut f, &symbol);
    request_close(&mut f);

    send(
        &mut f,
        LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::SaveAll),
    );

    assert!(symbol.exists(), "Save All must write the .snxsym");
    assert!(!f.app.document_state.is_document_dirty(&symbol));
    assert!(f.app.library.library_at(&f.file).is_none(), "closed");
}

#[test]
fn discard_all_drops_the_dirty_symbol_and_its_tab() {
    let mut f = fixture();
    let symbol = symbol_in_library(&f, "s.snxsym");
    open_dirty_symbol(&mut f, &symbol);
    request_close(&mut f);

    send(
        &mut f,
        LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::DiscardAll),
    );

    assert!(!f.app.document_state.is_document_dirty(&symbol));
    assert!(!f.app.document_state.symbol_editors.contains_key(&symbol));
    assert!(f.app.document_state.tabs.iter().all(|t| t.path != symbol));
    assert!(!symbol.exists(), "nothing was written");
    assert!(f.app.library.library_at(&f.file).is_none(), "closed");
}

#[test]
fn a_symbol_and_a_row_are_both_listed_and_both_discarded() {
    let mut f = fixture();
    let symbol = symbol_in_library(&f, "s.snxsym");
    open_dirty_symbol(&mut f, &symbol);
    let open_row = LibraryMessage::OpenComponentRow {
        library_path: f.file.clone(),
        table: TABLE.into(),
        row_id: f.rows[0],
    };
    send(&mut f, open_row);
    let address = f.address(0);
    f.app.library.editors.get_mut(&address).unwrap().dirty = true;

    request_close(&mut f);
    let confirm = f.app.library.close_library_confirm.as_ref().expect("asked");
    assert_eq!(confirm.dirty_editors, vec![address]);
    assert_eq!(confirm.dirty_paths, vec![symbol.clone()]);

    send(
        &mut f,
        LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::DiscardAll),
    );
    assert!(!f.app.document_state.is_document_dirty(&symbol));
    assert!(f.app.library.editors.is_empty());
}
