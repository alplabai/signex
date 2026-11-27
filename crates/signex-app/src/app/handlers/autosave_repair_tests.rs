//! #3129: a symbol file that opened repaired gets no recovery copy, so no
//! background write carries the repair before the person saves.

use crate::app::handlers::menu::export::tests::app_workspace;
use signex_library::SymbolFile;

const PIN_NAME_CR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../signex-library/tests/fixtures/snxsym/pin_name_cr.snxsym"
));

#[test]
fn a_repaired_symbol_gets_no_recovery_copy_until_saved() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("OPAMP.snxsym");
    std::fs::write(&path, PIN_NAME_CR).unwrap();
    let (file, repairs) = SymbolFile::from_bytes_repairing(PIN_NAME_CR).expect("opens");
    app.open_symbol_editor_tab(path.clone(), file);
    app.open_repaired_symbol(&path, &repairs);
    let sidecars = dir.path().join("autosave");
    assert!(
        app.autosave_primitive_snapshot_task(&sidecars, &path)
            .is_none()
    );
    let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
    editor.draft.repair_written();
    assert!(
        app.autosave_primitive_snapshot_task(&sidecars, &path)
            .is_some(),
        "a dirty draft gets its copy again once the repair is on disk"
    );
}
