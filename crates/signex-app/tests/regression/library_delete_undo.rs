//! #760 — deleting a footprint or symbol from a library document is an
//! undo step of its own. Without a snapshot taken before the delete,
//! Undo restored the snapshot of the *previous* edit: the deleted
//! primitive came back and the earlier, unrelated edit was reverted.

use signex_app::app::{
    EditMsg, FootprintEditorState, Message, Signex, SymbolEditorState, TabInfo, TabKind,
};
use signex_app::dock::DockMessage;
use signex_app::library::messages::{
    FootprintEditorMsg, LibraryMessage, PrimitiveEdit, SymbolEditorMsg,
};
use signex_app::panels::PanelMsg;
use signex_library::{Footprint, FootprintFile, Symbol, SymbolFile};
use std::path::{Path, PathBuf};

fn open_tab(app: &mut Signex, path: &Path, kind: TabKind) {
    app.document_state.tabs.push(TabInfo {
        title: "lib".into(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind,
    });
    app.document_state.active_tab = 0;
}

fn footprint_app() -> (Signex, PathBuf) {
    let (mut app, _t) = Signex::new();
    let path = PathBuf::from("lib.snxfpt");
    let mut file = FootprintFile::from_footprint(Footprint::empty("A"));
    file.footprints.push(Footprint::empty("B"));
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintEditorState::new(path.clone(), file));
    open_tab(&mut app, &path, TabKind::FootprintEditor(path.clone()));
    (app, path)
}

fn symbol_app() -> (Signex, PathBuf) {
    let (mut app, _t) = Signex::new();
    let path = PathBuf::from("lib.snxsym");
    let mut file = SymbolFile::from_symbol(Symbol::empty("A"));
    file.symbols.push(Symbol::empty("B"));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolEditorState::new(path.clone(), file));
    open_tab(&mut app, &path, TabKind::SymbolEditor(path.clone()));
    (app, path)
}

fn fp_names(app: &Signex, path: &Path) -> Vec<String> {
    let editor = &app.document_state.footprint_editors[path];
    editor
        .file
        .footprints
        .iter()
        .map(|f| f.name.clone())
        .collect()
}

fn sym_summary(app: &Signex, path: &Path) -> Vec<(String, usize)> {
    let editor = &app.document_state.symbol_editors[path];
    editor
        .file
        .symbols
        .iter()
        .map(|s| (s.name.clone(), s.pins.len()))
        .collect()
}

#[test]
fn footprint_delete_is_its_own_undo_step() {
    let (mut app, path) = footprint_app();
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetName("A-edited".into())),
    }));
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpLibraryDeleteInternal(1),
    )));
    assert_eq!(fp_names(&app, &path), ["A-edited"]);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        fp_names(&app, &path),
        ["A-edited", "B"],
        "undo brings B back and keeps A's edit"
    );
    assert_eq!(app.document_state.footprint_editors[&path].active_idx, 0);

    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(fp_names(&app, &path), ["A-edited"], "redo re-deletes B");
}

#[test]
fn symbol_delete_is_its_own_undo_step() {
    let (mut app, path) = symbol_app();
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Symbol(SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 }),
    }));
    assert_eq!(sym_summary(&app, &path), [("A".into(), 1), ("B".into(), 0)]);
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::SchLibraryDeleteSymbol(1),
    )));
    assert_eq!(sym_summary(&app, &path), [("A".into(), 1)]);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        sym_summary(&app, &path),
        [("A".into(), 1), ("B".into(), 0)],
        "undo brings B back and keeps A's pin"
    );

    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(
        sym_summary(&app, &path),
        [("A".into(), 1)],
        "redo re-deletes B"
    );
}

#[test]
fn symbol_undo_never_writes_one_symbols_edit_into_another() {
    let (mut app, path) = symbol_app();
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Symbol(SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 }),
    }));
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::SchLibrarySelectSymbol(1),
    )));

    let _ = app.update(Message::Edit(EditMsg::Undo));

    assert_eq!(
        sym_summary(&app, &path),
        [("A".into(), 0), ("B".into(), 0)],
        "undo reverts A's pin; B keeps its own name and contents"
    );
}
