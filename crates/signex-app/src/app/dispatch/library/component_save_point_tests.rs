//! #1854 — a Component editor's row keeps a saved position in its undo
//! history: undoing back to what was saved reads clean, a write in flight
//! keeps the row dirty, and a reload starts a fresh history.

use signex_library::{LifecycleState, RowId};
use uuid::Uuid;

use crate::app::Signex;
use crate::library::messages::EditorMsg;
use crate::library::state::{ComponentPreviewState, EditorAddress, OpenLibrary};
use crate::library::test_adapters::RevisionedRowAdapter;
use crate::library::writes::test_support::run_write;

const TABLE: &str = "R";

/// An app whose library stores the row, with a clean Component editor open.
fn app_with_clean_editor() -> (Signex, EditorAddress) {
    let (mut app, _task) = Signex::new();
    let library_id = Uuid::now_v7();
    let root = std::env::temp_dir()
        .join(format!("snx-save-point-{}", Uuid::now_v7().simple()))
        .join("lib.snxlib");
    app.library.open_libraries.clear();
    let mut row = crate::library::component_preview::test_row();
    row.server_revision = Some(1);
    let adapter = RevisionedRowAdapter::new(library_id, 1);
    adapter.store(row.clone());
    app.library
        .set
        .mount(Box::new(adapter))
        .expect("mount stub adapter");
    let address = EditorAddress::new(root.clone(), TABLE.into(), RowId::from_uuid(row.row_id));
    app.library.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "SavePointLib".into(),
        library_id,
        tables: std::collections::HashMap::from([(TABLE.to_string(), vec![row.clone()])]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        own_edit_stamp: None,
        display: Default::default(),
        pricing: Default::default(),
    });
    let preview = ComponentPreviewState::from_row(root, TABLE.into(), row);
    app.library.editors.insert(address.clone(), preview);
    (app, address)
}

fn set_lifecycle(app: &mut Signex, address: &EditorAddress, state: LifecycleState) {
    let _ = app.handle_editor_event(address.clone(), EditorMsg::SetLifecycle(state));
}

fn editor<'a>(app: &'a mut Signex, address: &EditorAddress) -> &'a mut ComponentPreviewState {
    app.library.editors.get_mut(address).expect("editor open")
}

#[test]
fn undo_back_to_the_row_that_was_saved_reads_clean() {
    let (mut app, address) = app_with_clean_editor();
    set_lifecycle(&mut app, &address, LifecycleState::Released);
    let task = app.handle_save_row(&address);
    run_write(&mut app, task);
    assert!(!editor(&mut app, &address).dirty, "the save landed");

    set_lifecycle(&mut app, &address, LifecycleState::Deprecated);
    let preview = editor(&mut app, &address);
    assert!(preview.dirty);
    assert!(preview.step_history(true));
    assert!(!preview.dirty, "undo returned to the saved row");
    assert!(preview.step_history(true));
    assert!(preview.dirty, "the opened row is not what was saved");
}

#[test]
fn an_edit_made_while_the_write_runs_keeps_the_row_dirty() {
    let (mut app, address) = app_with_clean_editor();
    set_lifecycle(&mut app, &address, LifecycleState::Released);
    let task = app.handle_save_row(&address);
    set_lifecycle(&mut app, &address, LifecycleState::Deprecated);
    run_write(&mut app, task);

    let preview = editor(&mut app, &address);
    assert!(preview.dirty, "the saved copy is not the row on screen");
    assert!(preview.step_history(true));
    assert!(!preview.dirty, "undo returns to the saved row");
}

#[test]
fn an_outside_write_to_a_dirty_row_makes_undo_stay_dirty() {
    let (mut app, address) = app_with_clean_editor();
    set_lifecycle(&mut app, &address, LifecycleState::Released);
    let root = address.library_path.clone();
    let _ = app.refresh_clean_editors(&root, &[address.row_id]);

    let preview = editor(&mut app, &address);
    assert!(preview.step_history(true));
    assert!(
        preview.dirty,
        "the saved position names replaced disk content"
    );
}

#[test]
fn a_reload_starts_clean_with_no_history() {
    let (mut app, address) = app_with_clean_editor();
    set_lifecycle(&mut app, &address, LifecycleState::Released);
    app.library.reload_editor_blocking(&address, false);
    let preview = editor(&mut app, &address);
    assert!(!preview.dirty);
    assert!(!preview.step_history(true), "the reload dropped the steps");
}
