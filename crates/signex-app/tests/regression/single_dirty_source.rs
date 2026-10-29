//! #661 — a document's unsaved state has one owner (docs/DIRTY_STATE.md).
//!
//! The tab dot, the title-bar count, the quit gate, Save All and the History
//! panel all ask the same question, so an edit, a save, a stale completion
//! and a rejected edit cannot leave one of them disagreeing with another.
//! Each test drives `Signex::update` and then reads every reader.

use signex_app::app::{
    FileMsg, Message, ProjectCloseChoice, ProjectMsg, Signex, SymbolDocument, TabInfo, TabKind,
    WindowMsg,
};
use signex_app::library::messages::{
    FootprintEditorMsg, LibraryMessage, PrimitiveEdit, SymbolEditorMsg,
};
use signex_library::{Symbol, SymbolFile};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn open_symbol_editor(app: &mut Signex, path: &Path) {
    let file = SymbolFile::from_symbol(Symbol::empty("A"));
    app.document_state.symbol_editors.insert(
        path.to_path_buf(),
        SymbolDocument::new(path.to_path_buf(), file),
    );
    app.document_state.tabs.push(TabInfo {
        title: "sym".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.to_path_buf()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

/// Put the editor's container on disk, as an opened file is: Ctrl+S on a
/// draft with no file behind it asks where to save instead.
fn write_symbol_file(app: &Signex, path: &Path) {
    let text = app.document_state.symbol_editors[path]
        .file
        .to_toml_string()
        .expect("serialise");
    std::fs::write(path, text).expect("write");
}

fn write_footprint_file(app: &Signex, path: &Path) {
    let text = app.document_state.footprint_editors[path]
        .file
        .to_toml_string()
        .expect("serialise");
    std::fs::write(path, text).expect("write");
}

fn edit_symbol(app: &mut Signex, path: &Path) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Symbol(SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 }),
    }));
}

fn edit_footprint(app: &mut Signex, path: &Path, name: &str) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetName(name.into())),
    }));
}

/// Every reader of "is anything unsaved?" reports exactly `path`: the query,
/// the count, the path list and the quit gate.
fn assert_dirty_everywhere(app: &mut Signex, path: &Path, what: &str) {
    let d = &app.document_state;
    assert!(d.is_document_dirty(path), "{what}: is_document_dirty");
    assert!(d.has_dirty_documents(), "{what}: has_dirty_documents");
    assert_eq!(d.dirty_document_count(), 1, "{what}: the title-bar count");
    assert!(
        d.dirty_document_paths().contains(path),
        "{what}: dirty_document_paths"
    );
    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
    let modal = app
        .ui_state
        .app_quit_confirm
        .take()
        .unwrap_or_else(|| panic!("{what}: quitting must ask before discarding"));
    assert!(
        modal.dirty_paths.contains(&path.to_path_buf()),
        "{what}: the quit prompt lists it"
    );
}

fn assert_clean_everywhere(app: &mut Signex, what: &str) {
    let d = &app.document_state;
    assert!(!d.has_dirty_documents(), "{what}: has_dirty_documents");
    assert_eq!(d.dirty_document_count(), 0, "{what}: the title-bar count");
    assert!(d.dirty_document_paths().is_empty(), "{what}: path list");
    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
    assert!(
        app.ui_state.app_quit_confirm.is_none(),
        "{what}: nothing is unsaved, so quitting does not ask"
    );
}

#[test]
fn an_edit_in_each_document_kind_reaches_every_reader() {
    let tmp = TempDir::new().unwrap();

    let (mut app, _t) = Signex::new();
    let symbol = tmp.path().join("a.snxsym");
    open_symbol_editor(&mut app, &symbol);
    assert_clean_everywhere(&mut app, "a freshly opened symbol");
    edit_symbol(&mut app, &symbol);
    assert_dirty_everywhere(&mut app, &symbol, "a symbol edit");

    let (mut app, _t) = Signex::new();
    let footprint = tmp.path().join("b.snxfpt");
    crate::primitive_dirty_quit::open_footprint_editor(&mut app, &footprint);
    assert_clean_everywhere(&mut app, "a freshly opened footprint");
    edit_footprint(&mut app, &footprint, "renamed");
    assert_dirty_everywhere(&mut app, &footprint, "a footprint edit");

    let moved = uuid::Uuid::new_v4();
    let (mut app, sheet) =
        crate::dirty_paths_gateway::app_with(vec![crate::dirty_paths_gateway::symbol(
            moved, "R1", 50.0,
        )]);
    assert_clean_everywhere(&mut app, "a freshly opened sheet");
    crate::dirty_paths_gateway::move_selection(&mut app, moved);
    assert_dirty_everywhere(&mut app, &sheet, "a schematic edit");
}

#[test]
fn a_save_clears_every_reader_for_each_draft_kind() {
    let tmp = TempDir::new().unwrap();

    let (mut app, _t) = Signex::new();
    let footprint = tmp.path().join("a.snxfpt");
    crate::primitive_dirty_quit::open_footprint_editor(&mut app, &footprint);
    write_footprint_file(&app, &footprint);
    edit_footprint(&mut app, &footprint, "renamed");
    assert_dirty_everywhere(&mut app, &footprint, "before the footprint save");
    crate::settle_support::settle(&mut app, Message::File(FileMsg::Save));
    assert!(
        std::fs::read_to_string(&footprint)
            .unwrap()
            .contains("renamed"),
        "the save wrote the edit"
    );
    assert_clean_everywhere(&mut app, "after the footprint save");

    let (mut app, _t) = Signex::new();
    let symbol = tmp.path().join("b.snxsym");
    open_symbol_editor(&mut app, &symbol);
    write_symbol_file(&app, &symbol);
    edit_symbol(&mut app, &symbol);
    assert_dirty_everywhere(&mut app, &symbol, "before the symbol save");
    crate::settle_support::settle(&mut app, Message::File(FileMsg::Save));
    assert!(symbol.exists(), "the save wrote the file");
    assert_clean_everywhere(&mut app, "after the symbol save");
}

/// The save snapshot resets the editor's own edit flag, but the write has not
/// landed: a quit, a tab close or a failed write must still see the draft.
#[test]
fn a_draft_stays_unsaved_while_its_write_is_in_flight() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("a.snxfpt");
    let (mut app, _t) = Signex::new();
    crate::primitive_dirty_quit::open_footprint_editor(&mut app, &path);
    write_footprint_file(&app, &path);
    edit_footprint(&mut app, &path, "renamed");

    let write = app.update(Message::File(FileMsg::Save));
    assert!(
        !std::fs::read_to_string(&path).unwrap().contains("renamed"),
        "update itself wrote nothing"
    );
    assert_dirty_everywhere(&mut app, &path, "while the write is in flight");

    crate::settle_support::settle_task(&mut app, write);
    assert_clean_everywhere(&mut app, "once the write landed");
}

/// A completion for an older snapshot must not clean an edit made after it:
/// the newer edit is unsaved and the next save writes it.
#[test]
fn a_stale_save_completion_does_not_clean_a_newer_edit() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("a.snxfpt");
    let (mut app, _t) = Signex::new();
    crate::primitive_dirty_quit::open_footprint_editor(&mut app, &path);
    write_footprint_file(&app, &path);
    edit_footprint(&mut app, &path, "first");
    let write = app.update(Message::File(FileMsg::Save));

    edit_footprint(&mut app, &path, "second");
    crate::settle_support::settle_task(&mut app, write);
    assert_dirty_everywhere(&mut app, &path, "after the stale completion");

    crate::settle_support::settle(&mut app, Message::File(FileMsg::Save));
    let written = std::fs::read_to_string(&path).expect("written");
    assert!(written.contains("second"), "the newer edit reached disk");
    assert_clean_everywhere(&mut app, "after the second save");
}

/// A rejected edit gives the draft's flag back as clean (#1047). Nothing
/// mirrors it, so no reader can still say dirty.
#[test]
fn a_draft_given_back_as_clean_is_clean_in_every_reader() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("a.snxfpt");
    let (mut app, _t) = Signex::new();
    crate::primitive_dirty_quit::open_footprint_editor(&mut app, &path);
    edit_footprint(&mut app, &path, "renamed");
    assert_dirty_everywhere(&mut app, &path, "after the edit");

    app.document_state
        .footprint_editors
        .get_mut(&path)
        .expect("editor")
        .set_dirty(false);

    assert_clean_everywhere(&mut app, "after the flag was given back");
}

/// Quit's Discard All must not drop a draft edited since the prompt.
#[test]
fn an_edit_after_the_quit_prompt_is_not_discarded() {
    let tmp = TempDir::new().unwrap();
    let path: PathBuf = tmp.path().join("a.snxfpt");
    let (mut app, _t) = Signex::new();
    crate::primitive_dirty_quit::open_footprint_editor(&mut app, &path);
    edit_footprint(&mut app, &path, "first");
    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
    assert!(app.ui_state.app_quit_confirm.is_some());

    let other = tmp.path().join("b.snxsym");
    open_symbol_editor(&mut app, &other);
    edit_symbol(&mut app, &other);
    let _ = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::DiscardAll,
    )));

    let modal = app
        .ui_state
        .app_quit_confirm
        .as_ref()
        .expect("the confirm stays open with the refreshed list");
    assert!(
        modal.dirty_paths.contains(&other),
        "the new draft is listed"
    );
    assert!(app.document_state.is_document_dirty(&other));
}
