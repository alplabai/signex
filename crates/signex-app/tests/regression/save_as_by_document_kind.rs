//! #671 — File > Save As routes by the active tab's document kind.
//!
//! A symbol / footprint tab used to fall through the schematic-only
//! Save As: the dialog was titled "Save Schematic As" and the handler
//! returned `Ok(())` without writing or logging anything. Review
//! follow-ups: the source tab travels with the dialog result, a failed
//! write rolls the re-key back, and every refusal reaches the Messages
//! panel.

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
    // Install the logger before the act step so `logged` can see it.
    let _ = signex_app::diagnostics::init_logging();
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

/// Messages-panel line containing both `needle` and the temp-dir name
/// (unique per run, so parallel tests cannot satisfy each other).
fn logged(tmp: &TempDir, needle: &str) -> bool {
    let _ = signex_app::diagnostics::init_logging();
    let marker = tmp
        .path()
        .file_name()
        .and_then(|n| n.to_str())
        .expect("temp dir name")
        .to_string();
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(needle) && e.message.contains(&marker))
}

fn primitive_save_as(from: &Path, to: &Path) -> Message {
    Message::File(FileMsg::SavePrimitiveAs {
        from_path: from.to_path_buf(),
        to_path: to.to_path_buf(),
    })
}

fn schematic_save_as(from: &Path, to: &Path) -> Message {
    Message::File(FileMsg::SaveAs {
        from_path: from.to_path_buf(),
        to_path: to.to_path_buf(),
    })
}

#[test]
fn symbol_save_as_writes_symbol_file_and_rekeys_tab() {
    let tmp = TempDir::new().unwrap();
    let old = tmp.path().join("old.snxsym");
    let new = tmp.path().join("new.snxsym");
    std::fs::write(&old, b"original").unwrap();
    let (mut app, _t) = Signex::new();
    open_symbol_tab(&mut app, &old);

    let _ = app.update(primitive_save_as(&old, &new));

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

    let _ = app.update(primitive_save_as(&old, &new));

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
fn switching_tabs_while_the_dialog_is_open_still_saves_the_source_document() {
    // The dialog is not modal: the user opens Save As on the symbol tab,
    // switches to another tab, then the dialog returns. The symbol (not
    // the tab that is active now) must be written to the picked path.
    let tmp = TempDir::new().unwrap();
    let sym = tmp.path().join("old.snxsym");
    let other = tmp.path().join("other.snxpcb");
    let picked = tmp.path().join("foo.snxsym");
    std::fs::write(&sym, b"original").unwrap();
    std::fs::write(&other, b"other doc").unwrap();
    let (mut app, _t) = Signex::new();
    open_symbol_tab(&mut app, &sym);
    push_tab(&mut app, &other, TabKind::Pcb);
    assert_eq!(app.document_state.active_tab, 1, "other tab is active");

    let _ = app.update(primitive_save_as(&sym, &picked));

    let text = std::fs::read_to_string(&picked).expect("the symbol must be written");
    SymbolFile::from_toml_str(&text).expect("picked file is the symbol, a valid .snxsym");
    assert_eq!(std::fs::read(&other).unwrap(), b"other doc");
    let ds = &app.document_state;
    assert!(ds.symbol_editors.contains_key(&picked));
    assert_eq!(ds.tabs[0].path, picked, "symbol tab re-keyed");
    assert_eq!(ds.tabs[1].path, other, "other tab untouched");
    assert!(matches!(ds.tabs[1].kind, TabKind::Pcb));
}

#[test]
fn schematic_save_as_is_refused_when_the_active_tab_changed() {
    let tmp = TempDir::new().unwrap();
    let source = tmp.path().join("source.snxsch");
    let active = tmp.path().join("active.snxsch");
    let target = tmp.path().join("foo.snxsch");
    let (mut app, _t) = Signex::new();
    push_tab(&mut app, &source, TabKind::Schematic);
    push_tab(&mut app, &active, TabKind::Schematic);

    let _ = app.update(schematic_save_as(&source, &target));

    assert!(!target.exists(), "the active sheet must not be written");
    assert!(
        logged(&tmp, "no longer"),
        "the refusal must reach the Messages panel"
    );
}

#[test]
fn failed_save_as_write_rolls_the_rekey_back() {
    let tmp = TempDir::new().unwrap();
    let old = tmp.path().join("old.snxsym");
    // A regular file where a directory is needed: the write cannot succeed.
    let blocker = tmp.path().join("blocker");
    std::fs::write(&blocker, b"file").unwrap();
    let new = blocker.join("new.snxsym");
    std::fs::write(&old, b"original").unwrap();
    let (mut app, _t) = Signex::new();
    open_symbol_tab(&mut app, &old);
    app.document_state.tabs[0].title = "keep me".into();

    let _ = app.update(primitive_save_as(&old, &new));

    let ds = &app.document_state;
    assert!(ds.symbol_editors.contains_key(&old), "editor back at old");
    assert!(!ds.symbol_editors.contains_key(&new));
    assert_eq!(ds.tabs[0].path, old);
    assert_eq!(ds.tabs[0].title, "keep me");
    assert!(matches!(&ds.tabs[0].kind, TabKind::SymbolEditor(p) if p == &old));
    assert!(ds.dirty_paths.contains(&old), "still dirty: nothing saved");
    assert!(!ds.dirty_paths.contains(&new));
    assert_eq!(std::fs::read(&old).unwrap(), b"original");
    assert!(
        logged(&tmp, "Save As failed"),
        "the failure must reach the Messages panel"
    );
}

#[test]
fn symbol_save_as_onto_another_open_tab_is_refused() {
    let tmp = TempDir::new().unwrap();
    let a = tmp.path().join("a.snxsym");
    let b = tmp.path().join("b.snxsym");
    let (mut app, _t) = Signex::new();
    open_symbol_tab(&mut app, &b);
    open_symbol_tab(&mut app, &a);

    let _ = app.update(primitive_save_as(&a, &b));

    assert!(!b.exists(), "nothing may be written over the other tab");
    assert!(app.document_state.symbol_editors.contains_key(&a));
    assert!(app.document_state.symbol_editors.contains_key(&b));
    assert!(
        logged(&tmp, "already open in another tab"),
        "the refusal must reach the Messages panel"
    );
}

#[test]
fn save_as_after_the_source_tab_changed_kind_writes_nothing() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("x.snxsch");
    let pcb = tmp.path().join("b.snxpcb");
    let (mut app, _t) = Signex::new();
    push_tab(&mut app, &pcb, TabKind::Pcb);

    let _ = app.update(schematic_save_as(&pcb, &target));

    assert!(!target.exists());
    assert!(
        logged(&tmp, "no longer the schematic"),
        "the refusal must reach the Messages panel"
    );
}

#[test]
fn saving_outside_any_snxlib_logs_to_the_messages_panel() {
    // Outside any `.snxlib/`: the file is written but not attached, and
    // the user is told why.
    let tmp = TempDir::new().unwrap();
    let old = tmp.path().join("old.snxsym");
    let new = tmp.path().join("loose.snxsym");
    let (mut app, _t) = Signex::new();
    open_symbol_tab(&mut app, &old);

    let _ = app.update(primitive_save_as(&old, &new));

    assert!(new.exists());
    assert!(
        logged(&tmp, "outside any `.snxlib/`"),
        "the unattached save must reach the Messages panel"
    );
}

#[test]
fn extensionless_pick_never_overwrites_an_existing_file_with_the_appended_extension() {
    // The OS dialog confirmed overwriting "foo", not "foo.snxsym". An
    // existing foo.snxsym that no tab hosts must survive untouched.
    let tmp = TempDir::new().unwrap();
    let old = tmp.path().join("old.snxsym");
    let existing = tmp.path().join("foo.snxsym");
    let picked = tmp.path().join("foo");
    std::fs::write(&existing, b"precious").unwrap();
    let (mut app, _t) = Signex::new();
    open_symbol_tab(&mut app, &old);
    app.document_state.tabs[0].title = "keep me".into();

    let _ = app.update(primitive_save_as(&old, &picked));

    assert_eq!(std::fs::read(&existing).unwrap(), b"precious");
    assert!(!picked.exists(), "the bare name must not be written either");
    let ds = &app.document_state;
    assert!(ds.symbol_editors.contains_key(&old), "no re-key on refusal");
    assert!(!ds.symbol_editors.contains_key(&existing));
    assert_eq!(ds.tabs[0].path, old);
    assert_eq!(ds.tabs[0].title, "keep me");
    assert!(ds.dirty_paths.contains(&old), "still dirty: nothing saved");
    assert!(
        logged(&tmp, "already exists and was not overwritten"),
        "the refusal must reach the Messages panel"
    );
}
