//! #3129: a `.snxsym` an older build wrote with a CR in a pin name opens
//! repaired and read-only, the file is left as it is until the user saves,
//! and that save writes the repair.

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::library::messages::SymbolEditorMsg;
use signex_library::SymbolFile;

const PIN_NAME_CR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../signex-library/tests/fixtures/snxsym/pin_name_cr.snxsym"
));

#[test]
fn a_repaired_symbol_stays_read_only_and_unwritten_until_the_user_saves() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("OPAMP.snxsym");
    std::fs::write(&path, PIN_NAME_CR).unwrap();

    let (file, repairs) = SymbolFile::from_bytes_repairing(PIN_NAME_CR).expect("opens");
    app.open_symbol_editor_tab(path.clone(), file);
    app.open_repaired_symbol(&path, &repairs);

    let editor = app.document_state.symbol_editors.get(&path).unwrap();
    assert!(editor.is_dirty(), "the repair is not on disk yet");
    assert!(editor.draft.is_read_only());
    assert_eq!(editor.primitive().pins[0].name, "OUTA");

    let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
    crate::library::editor::symbol::updates::apply_symbol_primitive_edit(
        editor,
        SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 },
    );
    assert_eq!(editor.primitive().pins.len(), 2, "no edit while read-only");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        PIN_NAME_CR,
        "nothing written"
    );

    app.save_primitive_tab_blocking(&path).expect("save");

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
    assert_eq!(editor.primitive().pins.len(), 3);
}
