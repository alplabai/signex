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

/// A second library mounted in a subfolder of the fixture's library folder
/// (display entry only; nothing is written through it).
fn mount_nested_library(f: &mut Fixture) -> PathBuf {
    let root = f.file.parent().unwrap().join("vendor").join("V.snxlib");
    f.app.library.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "V".into(),
        library_id: Uuid::now_v7(),
        tables: std::collections::HashMap::new(),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
        pricing: Default::default(),
    });
    root
}

fn nested_symbol(f: &Fixture) -> PathBuf {
    f.file
        .parent()
        .unwrap()
        .join("vendor")
        .join("symbols")
        .join("x.snxsym")
}

#[test]
fn closing_the_nested_library_prompts_for_its_own_draft() {
    let mut f = fixture();
    let nested = mount_nested_library(&mut f);
    let symbol = nested_symbol(&f);
    open_dirty_symbol(&mut f, &symbol);

    send(&mut f, LibraryMessage::CloseLibrary(nested));

    let confirm = f
        .app
        .library
        .close_library_confirm
        .as_ref()
        .expect("the nested library owns the draft, so closing it asks");
    assert_eq!(confirm.dirty_paths, vec![symbol]);
}

#[test]
fn closing_the_outer_library_does_not_offer_the_nested_librarys_draft() {
    let mut f = fixture();
    let _nested = mount_nested_library(&mut f);
    let symbol = nested_symbol(&f);
    open_dirty_symbol(&mut f, &symbol);

    request_close(&mut f);

    assert!(
        f.app.library.close_library_confirm.is_none(),
        "the draft is the nested library's, not the outer one's"
    );
    assert!(
        f.app.document_state.is_document_dirty(&symbol),
        "the draft is untouched"
    );
}

#[test]
fn discard_all_asks_again_when_a_draft_became_dirty_behind_the_prompt() {
    let mut f = fixture();
    let first = symbol_in_library(&f, "first.snxsym");
    open_dirty_symbol(&mut f, &first);
    request_close(&mut f);
    let second = symbol_in_library(&f, "second.snxsym");
    open_dirty_symbol(&mut f, &second);

    send(
        &mut f,
        LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::DiscardAll),
    );

    assert!(
        f.app.document_state.is_document_dirty(&first),
        "nothing discarded"
    );
    assert!(f.app.document_state.is_document_dirty(&second));
    let confirm = f
        .app
        .library
        .close_library_confirm
        .as_ref()
        .expect("asked again");
    assert_eq!(confirm.dirty_paths, vec![first, second]);
    assert!(f.app.library.library_at(&f.file).is_some(), "still mounted");
}

#[test]
fn drafts_are_labelled_by_their_path_in_the_library() {
    use crate::library::close_prompt::dirty_path_label;
    let dir = std::path::Path::new("/w/Lib");
    let nested = dir.join("vendor").join("symbols").join("s.snxsym");
    let plain = dir.join("symbols").join("s.snxsym");
    assert_eq!(
        dirty_path_label(&nested, Some(dir)),
        "vendor/symbols/s.snxsym"
    );
    assert_eq!(dirty_path_label(&plain, Some(dir)), "symbols/s.snxsym");
    assert_eq!(dirty_path_label(&plain, None), "s.snxsym");
}
