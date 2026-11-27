//! #3129: a `.snxsym` an older build wrote with a CR in a pin name opens
//! repaired and read-only. No write but the person's own Save touches the
//! file meanwhile: not an edit from the library panel, not Save All, not a
//! close's save, not a command or agent save, not autosave. That Save
//! writes the repair and the draft takes edits again.

use std::path::{Path, PathBuf};

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::library::messages::SymbolEditorMsg;
use crate::panels::PanelMsg;
use signex_library::SymbolFile;

const PIN_NAME_CR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../signex-library/tests/fixtures/snxsym/pin_name_cr.snxsym"
));

/// The fixture opened in an editor tab the way the open path opens it.
fn open_repaired(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("OPAMP.snxsym");
    std::fs::write(&path, PIN_NAME_CR).unwrap();
    let (file, repairs) = SymbolFile::from_bytes_repairing(PIN_NAME_CR).expect("opens");
    app.open_symbol_editor_tab(path.clone(), file);
    app.open_repaired_symbol(&path, &repairs);
    (app, path)
}

fn unwritten(path: &Path, after: &str) {
    assert_eq!(
        std::fs::read(path).unwrap(),
        PIN_NAME_CR,
        "{after} wrote the file"
    );
}

fn pins(app: &Signex, path: &Path) -> usize {
    let editor = app.document_state.symbol_editors.get(path).unwrap();
    editor.primitive().pins.len()
}

#[test]
fn a_repaired_symbol_is_written_by_no_path_but_the_persons_save() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = open_repaired(dir.path());
    let editor = app.document_state.symbol_editors.get(&path).unwrap();
    assert!(editor.is_dirty(), "the repair is not on disk yet");
    assert!(editor.draft.is_read_only());
    assert_eq!(editor.primitive().pins[0].name, "OUTA");

    // Canvas and library-panel edits are refused.
    let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
    crate::library::editor::symbol::updates::apply_symbol_primitive_edit(
        editor,
        SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 },
    );
    assert_eq!(pins(&app, &path), 2, "no pin added while read-only");
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::SchLibraryAddSymbol,
    )));
    let symbols = app.document_state.symbol_editors[&path]
        .draft
        .file
        .symbols
        .len();
    assert_eq!(symbols, 1, "the library panel added no symbol");
    unwritten(&path, "a panel add");

    // Save All, a close's blocking save and a command or agent save.
    let save_all = app.begin_save_all(
        "Save All",
        std::slice::from_ref(&path),
        &[],
        crate::app::contracts::SaveAllNext::Nothing,
    );
    let _ = crate::task_driver::TaskDriver::new().settle(&mut app, save_all);
    unwritten(&path, "Save All");
    assert!(app.save_primitive_tab_blocking(&path).is_err());
    unwritten(&path, "a close's save");
    let refusal = app
        .begin_document_path_save(&path)
        .expect_err("a command or agent save is refused");
    assert!(
        format!("{refusal:#}").contains("OPAMP.snxsym"),
        "{refusal:#}"
    );
    unwritten(&path, "an agent save");
    assert!(
        app.document_state.symbol_editors[&path]
            .draft
            .is_read_only()
    );

    // The person's Save writes the repair.
    app.save_primitive_tab_blocking_explicitly(&path)
        .expect("the person's save");
    let bytes = std::fs::read(&path).unwrap();
    assert!(!bytes.contains(&b'\r'), "the saved file holds no CR");
    let saved = SymbolFile::from_bytes(&bytes).expect("a strict read opens it");
    assert_eq!(saved.symbols[0].pins[0].name, "OUTA");
    assert_eq!(saved.symbols[0].pins[1].name, "IN");
    let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
    assert!(!editor.draft.is_read_only(), "editable once saved");
    crate::library::editor::symbol::updates::apply_symbol_primitive_edit(
        editor,
        SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 },
    );
    assert_eq!(pins(&app, &path), 3);
}

/// Ctrl+S runs the same explicit save off the UI thread.
#[test]
fn ctrl_s_writes_the_repair() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = open_repaired(dir.path());
    let save = app
        .save_primitive_tab_explicitly(&path)
        .expect("the save starts");
    let _ = crate::task_driver::TaskDriver::new().settle(&mut app, save);
    let saved = SymbolFile::from_bytes(&std::fs::read(&path).unwrap()).expect("repaired");
    assert_eq!(saved.symbols[0].pins[0].name, "OUTA");
    assert!(
        !app.document_state.symbol_editors[&path]
            .draft
            .is_read_only()
    );
}
