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

/// Open a Component editor tab for `address` as the main window's active tab,
/// the way opening it does (`active_path` is `None`).
fn open_component_tab(app: &mut Signex, address: &EditorAddress) {
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "row".into(),
        path: address.synthetic_tab_path(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::ComponentEditor(crate::app::ComponentEditorTab {
            library_path: address.library_path.clone(),
            table: address.table.clone(),
            row_id: address.row_id,
        }),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = None;
}

fn command(name: &str) -> crate::keymap::AppCommandId {
    crate::keymap::AppCommandId::new(name).unwrap()
}

#[test]
fn ctrl_z_in_a_component_tab_reaches_the_row_history() {
    let (mut app, address) = app_with_clean_editor();
    open_component_tab(&mut app, &address);
    set_lifecycle(&mut app, &address, LifecycleState::Released);
    assert!(editor(&mut app, &address).dirty);
    let ctx = app.command_context(None);
    assert!(
        ctx.can_undo && !ctx.can_redo,
        "the menu rows follow the row history"
    );
    let _task = app.dispatch_command(
        &command("undo"),
        crate::app::command::CommandArgs::none(),
        None,
    );
    assert!(
        !editor(&mut app, &address).dirty,
        "Ctrl+Z was dropped by the enablement gate"
    );
    assert!(app.command_context(None).can_redo);
    let _task = app.dispatch_command(
        &command("redo"),
        crate::app::command::CommandArgs::none(),
        None,
    );
    assert!(editor(&mut app, &address).dirty);
}

#[test]
fn ctrl_z_in_a_component_window_steps_that_window_not_the_main_tab() {
    let (mut app, address) = app_with_clean_editor();
    open_component_tab(&mut app, &address);
    set_lifecycle(&mut app, &address, LifecycleState::Released);
    // The main window shows another tab.
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "other".into(),
        path: std::path::PathBuf::from("other.snxsch"),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        crate::app::state::WindowKind::ComponentEditor {
            library_path: address.library_path.clone(),
            table: address.table.clone(),
            row_id: address.row_id,
        },
    );
    assert!(app.command_context(Some(window)).can_undo);
    let _task = app.dispatch_command(
        &command("undo"),
        crate::app::command::CommandArgs::none(),
        Some(window),
    );
    assert!(
        !editor(&mut app, &address).dirty,
        "the window's own row stepped"
    );
}

#[test]
fn an_edit_typed_before_an_outside_write_reload_lands_cannot_undo_clean() {
    let (mut app, address) = app_with_clean_editor();
    let root = address.library_path.clone();
    let _reload = app.refresh_clean_editors(&root, &[address.row_id]);
    set_lifecycle(&mut app, &address, LifecycleState::Released);
    let preview = editor(&mut app, &address);
    assert!(preview.step_history(true));
    assert!(preview.dirty, "the library was written behind this editor");
}
