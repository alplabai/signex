//! #671 — File > Save As routes by the active tab's document kind.
//!
//! A symbol / footprint tab used to fall through the schematic-only
//! Save As: the dialog was titled "Save Schematic As" and the handler
//! returned `Ok(())` without writing or logging anything.

use signex_app::app::{
    FileMsg, FootprintEditorState, Message, Signex, SymbolEditorState, TabInfo, TabKind,
};
use signex_library::{Footprint, FootprintFile, Symbol, SymbolFile};
use std::path::Path;
use tempfile::TempDir;

fn open_symbol_tab(app: &mut Signex, path: &Path) {
    let file = SymbolFile::from_symbol(Symbol::empty("sym"));
    app.document_state.symbol_editors.insert(
        path.to_path_buf(),
        SymbolEditorState::new(path.to_path_buf(), file),
    );
    push_tab(app, path, TabKind::SymbolEditor(path.to_path_buf()));
}

fn open_footprint_tab(app: &mut Signex, path: &Path) {
    let file = FootprintFile::from_footprint(Footprint::empty("fp"));
    app.document_state.footprint_editors.insert(
        path.to_path_buf(),
        FootprintEditorState::new(path.to_path_buf(), file),
    );
    push_tab(app, path, TabKind::FootprintEditor(path.to_path_buf()));
}

fn push_tab(app: &mut Signex, path: &Path, kind: TabKind) {
    app.document_state.tabs.push(TabInfo {
        title: "doc".into(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind,
    });
    app.document_state.dirty_paths.insert(path.to_path_buf());
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

#[test]
fn symbol_save_as_writes_symbol_file_and_rekeys_tab() {
    let tmp = TempDir::new().unwrap();
    let old = tmp.path().join("old.snxsym");
    let new = tmp.path().join("new.snxsym");
    std::fs::write(&old, b"original").unwrap();
    let (mut app, _t) = Signex::new();
    open_symbol_tab(&mut app, &old);

    let _ = app.update(Message::File(FileMsg::SaveAs(new.clone())));

    let text = std::fs::read_to_string(&new).expect("Save As must write the new file");
    SymbolFile::from_toml_str(&text).expect("new file must be a valid .snxsym");
    assert_eq!(
        std::fs::read(&old).unwrap(),
        b"original",
        "original untouched"
    );
    let ds = &app.document_state;
    assert!(ds.symbol_editors.contains_key(&new));
    assert!(!ds.symbol_editors.contains_key(&old));
    assert_eq!(ds.tabs[0].path, new);
    assert!(matches!(&ds.tabs[0].kind, TabKind::SymbolEditor(p) if p == &new));
    assert!(!ds.dirty_paths.contains(&old));
    assert!(!ds.dirty_paths.contains(&new), "written copy is clean");
}

#[test]
fn footprint_save_as_writes_footprint_file_and_rekeys_tab() {
    let tmp = TempDir::new().unwrap();
    let old = tmp.path().join("old.snxfpt");
    let new = tmp.path().join("new.snxfpt");
    std::fs::write(&old, b"original").unwrap();
    let (mut app, _t) = Signex::new();
    open_footprint_tab(&mut app, &old);

    let _ = app.update(Message::File(FileMsg::SaveAs(new.clone())));

    let text = std::fs::read_to_string(&new).expect("Save As must write the new file");
    FootprintFile::from_toml_str(&text).expect("new file must be a valid .snxfpt");
    assert_eq!(
        std::fs::read(&old).unwrap(),
        b"original",
        "original untouched"
    );
    let ds = &app.document_state;
    assert!(ds.footprint_editors.contains_key(&new));
    assert!(!ds.footprint_editors.contains_key(&old));
    assert_eq!(ds.tabs[0].path, new);
    assert!(matches!(&ds.tabs[0].kind, TabKind::FootprintEditor(p) if p == &new));
    assert!(!ds.dirty_paths.contains(&old));
}

#[test]
fn symbol_save_as_onto_another_open_tab_is_refused() {
    let tmp = TempDir::new().unwrap();
    let a = tmp.path().join("a.snxsym");
    let b = tmp.path().join("b.snxsym");
    let (mut app, _t) = Signex::new();
    open_symbol_tab(&mut app, &b);
    open_symbol_tab(&mut app, &a);

    let _ = app.update(Message::File(FileMsg::SaveAs(b.clone())));

    assert!(!b.exists(), "nothing may be written over the other tab");
    assert!(app.document_state.symbol_editors.contains_key(&a));
    assert!(app.document_state.symbol_editors.contains_key(&b));
}

#[test]
fn save_as_on_an_unsupported_tab_writes_nothing() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("x.snxsch");
    let (mut app, _t) = Signex::new();
    push_tab(&mut app, &tmp.path().join("b.snxpcb"), TabKind::Pcb);

    let _ = app.update(Message::File(FileMsg::SaveAs(target.clone())));

    assert!(!target.exists());
}
