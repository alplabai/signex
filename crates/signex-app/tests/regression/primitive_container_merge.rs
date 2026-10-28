//! #902 — a primitive editor's save merges its changes into the container
//! on disk instead of writing its whole in-memory copy back.
//!
//! Two Signex windows editing different symbols of one `.snxsym` each held
//! the whole container, so the second save wrote back a stale copy of the
//! first window's symbol and dropped that edit.

use signex_app::app::{
    FileMsg, FootprintDocument, Message, Signex, SymbolDocument, TabInfo, TabKind,
};
use signex_library::{Footprint, FootprintFile, Symbol, SymbolFile};
use std::path::Path;
use tempfile::TempDir;

fn two_symbol_file(path: &Path) -> SymbolFile {
    let mut file = SymbolFile::from_symbol(Symbol::empty("A"));
    file.symbols.push(Symbol::empty("B"));
    std::fs::write(path, file.to_toml_string().unwrap()).unwrap();
    file
}

/// A window with the container at `path` open in a symbol editor, read
/// off disk the way opening the tab reads it.
fn window_editing(path: &Path) -> Signex {
    let (mut app, _t) = Signex::new();
    let file = SymbolFile::from_toml_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    app.document_state.symbol_editors.insert(
        path.to_path_buf(),
        SymbolDocument::new(path.to_path_buf(), file),
    );
    open_tab(&mut app, path, TabKind::SymbolEditor(path.to_path_buf()));
    app
}

fn open_tab(app: &mut Signex, path: &Path, kind: TabKind) {
    app.document_state.tabs.push(TabInfo {
        title: "pair".into(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

/// Ctrl+S on the window's active primitive tab.
fn save(app: &mut Signex) {
    crate::settle_support::settle(app, Message::File(FileMsg::Save));
}

/// A Messages-panel line containing every needle. The panel cuts long
/// lines, and the file path comes last (#904 review: on macOS the temp path
/// alone pushed the reason out), so the needles name what only one test in
/// this file logs instead of matching the path.
fn logged(needles: &[&str]) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| needles.iter().all(|n| e.message.contains(n)))
}

fn edit_symbol(app: &mut Signex, path: &Path, index: usize, description: &str) {
    let editor = app.document_state.symbol_editors.get_mut(path).unwrap();
    editor.file.symbols[index].description = description.into();
    editor.dirty = true;
}

fn on_disk(path: &Path) -> SymbolFile {
    SymbolFile::from_toml_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn two_windows_saving_different_symbols_of_one_container_keep_both_edits() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("pair.snxsym");
    two_symbol_file(&path);
    let mut first = window_editing(&path);
    let mut second = window_editing(&path);

    edit_symbol(&mut first, &path, 0, "edited in the first window");
    edit_symbol(&mut second, &path, 1, "edited in the second window");
    save(&mut first);
    save(&mut second);

    let file = on_disk(&path);
    assert_eq!(file.symbols[0].description, "edited in the first window");
    assert_eq!(file.symbols[1].description, "edited in the second window");
}

/// The window keeps saving after the other one's save: its next save still
/// leaves the other window's symbol alone.
#[test]
fn a_second_save_from_the_same_window_still_keeps_the_other_windows_edit() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("pair.snxsym");
    two_symbol_file(&path);
    let mut first = window_editing(&path);
    let mut second = window_editing(&path);

    edit_symbol(&mut first, &path, 0, "first, once");
    save(&mut first);
    edit_symbol(&mut second, &path, 1, "second");
    save(&mut second);
    edit_symbol(&mut first, &path, 0, "first, twice");
    save(&mut first);

    let file = on_disk(&path);
    assert_eq!(file.symbols[0].description, "first, twice");
    assert_eq!(file.symbols[1].description, "second");
}

/// Both windows changed the same symbol: the later save wins, and the user
/// is told whose change it replaced.
#[test]
fn saving_a_symbol_another_window_also_changed_wins_and_says_so() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("pair.snxsym");
    two_symbol_file(&path);
    let mut first = window_editing(&path);
    let mut second = window_editing(&path);

    edit_symbol(&mut first, &path, 0, "first");
    save(&mut first);
    edit_symbol(&mut second, &path, 0, "second");
    save(&mut second);

    assert_eq!(on_disk(&path).symbols[0].description, "second");
    assert!(
        logged(&["this save replaced another window's change to A:"]),
        "the Messages panel must name the symbol whose other change was replaced"
    );
}

#[test]
fn two_windows_saving_different_footprints_of_one_container_keep_both_edits() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("pair.snxfpt");
    let mut file = FootprintFile::from_footprint(Footprint::empty("A"));
    file.footprints.push(Footprint::empty("B"));
    std::fs::write(&path, file.to_toml_string().unwrap()).unwrap();
    let open = || {
        let (mut app, _t) = Signex::new();
        let file = FootprintFile::from_toml_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        app.document_state
            .footprint_editors
            .insert(path.clone(), FootprintDocument::new(path.clone(), file));
        open_tab(&mut app, &path, TabKind::FootprintEditor(path.clone()));
        app
    };
    let (mut first, mut second) = (open(), open());

    for (app, index, text) in [(&mut first, 0, "first"), (&mut second, 1, "second")] {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.file.footprints[index].description = text.into();
        editor.dirty = true;
    }
    save(&mut first);
    save(&mut second);

    let file = FootprintFile::from_toml_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(file.footprints[0].description, "first");
    assert_eq!(file.footprints[1].description, "second");
}

/// A container another program left unparseable is not overwritten.
#[test]
fn a_container_that_no_longer_parses_on_disk_is_not_overwritten() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("pair.snxsym");
    two_symbol_file(&path);
    let mut window = window_editing(&path);
    std::fs::write(&path, b"not a symbol file").unwrap();

    let _ = signex_app::diagnostics::init_logging();
    edit_symbol(&mut window, &path, 0, "edited");
    save(&mut window);

    assert!(
        logged(&["the save was refused", "no longer parses"]),
        "the Messages panel must say the save was refused"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"not a symbol file");
}

fn delete_symbol(app: &mut Signex, path: &Path, index: usize) {
    let editor = app.document_state.symbol_editors.get_mut(path).unwrap();
    editor.file.symbols.remove(index);
    editor.active_idx = 0;
    editor.dirty = true;
}

/// #902 review: each window deletes a different symbol of a two-symbol
/// file. Together they would leave none, and a container with no symbol
/// cannot be opened again, so the second save is refused.
#[test]
fn deletes_from_two_windows_never_leave_the_container_empty() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("pair.snxsym");
    two_symbol_file(&path);
    let mut first = window_editing(&path);
    let mut second = window_editing(&path);

    delete_symbol(&mut first, &path, 0);
    save(&mut first);
    delete_symbol(&mut second, &path, 1);
    save(&mut second);

    let file = on_disk(&path);
    assert_eq!(file.symbols.len(), 1);
    assert_eq!(file.symbols[0].name, "B");
    assert!(logged(&[
        "the save was refused",
        "it would leave no symbol"
    ]));
}

/// #902 review: deleting a symbol another window has changed since says so.
#[test]
fn deleting_a_symbol_another_window_changed_says_so() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("pair.snxsym");
    two_symbol_file(&path);
    let mut first = window_editing(&path);
    let mut second = window_editing(&path);

    edit_symbol(&mut first, &path, 0, "edited in the first window");
    save(&mut first);
    delete_symbol(&mut second, &path, 0);
    save(&mut second);

    assert_eq!(on_disk(&path).symbols.len(), 1);
    assert!(
        logged(&["this save deleted another window's change to A:"]),
        "the Messages panel must name the deleted symbol another window had changed"
    );
}

/// #902 review: a symbol file opened on its own, inside a library this
/// window has not mounted, still waits for that library's write lock.
#[test]
fn a_lone_file_inside_a_library_waits_for_that_librarys_write_lock() {
    let tmp = TempDir::new().unwrap();
    let library = tmp.path().join("Lib").join("Lib.snxlib");
    std::fs::create_dir_all(library.parent().unwrap().join("symbols")).unwrap();
    std::fs::write(&library, b"").unwrap();
    let path = library
        .parent()
        .unwrap()
        .join("symbols")
        .join("pair.snxsym");
    two_symbol_file(&path);
    let mut window = window_editing(&path);

    let lock_path = signex_library::write_lock_path(&library);
    std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let held = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .unwrap();
    held.lock().unwrap();
    // Another process, holding the lock, read the file before this save
    // and writes its own change to the other symbol after.
    let mut theirs = on_disk(&path);
    theirs.symbols[1].description = "written under the lock".into();
    let other = path.clone();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        std::fs::write(&other, theirs.to_toml_string().unwrap()).unwrap();
        drop(held);
    });
    edit_symbol(&mut window, &path, 0, "edited here");
    save(&mut window);
    release.join().unwrap();

    let file = on_disk(&path);
    assert_eq!(file.symbols[0].description, "edited here");
    assert_eq!(file.symbols[1].description, "written under the lock");
}
