//! #661 slice 1: closing a Component Preview from its header close button
//! keeps a dirty row draft, as the tab bar close does (#110); the
//! explicit discards (Close Project → Discard All) still drop it.

use std::path::PathBuf;

use signex_library::{LifecycleState, RowId};

use crate::app::{ComponentEditorTab, Message, Signex, TabInfo, TabKind};
use crate::library::component_preview::test_row;
use crate::library::component_preview::updates::apply_inline_edit;
use crate::library::messages::{EditorMsg, LibraryMessage};
use crate::library::state::{ComponentPreviewState, EditorAddress};

/// A Component Preview tab, active, showing a row whose copy is `dirty`.
fn open_preview(dirty: bool) -> (Signex, EditorAddress) {
    let (mut app, _task) = Signex::new();
    let row = test_row();
    let library_path = PathBuf::from("/tmp/close-preview/Lib.snxlib");
    let address = EditorAddress::new(
        library_path.clone(),
        "R".into(),
        RowId::from_uuid(row.row_id),
    );
    let mut preview = ComponentPreviewState::from_row(library_path.clone(), "R".into(), row);
    if dirty {
        apply_inline_edit(
            &mut preview,
            EditorMsg::SetLifecycle(LifecycleState::Obsolete),
        );
        assert!(preview.dirty, "the edit marks the copy dirty");
    }
    app.library.editors.insert(address.clone(), preview);
    app.document_state.tabs.push(TabInfo {
        title: "R1".into(),
        path: address.synthetic_tab_path(),
        cached_document: None,
        project_id: None,
        kind: TabKind::ComponentEditor(ComponentEditorTab {
            library_path,
            table: "R".into(),
            row_id: address.row_id,
        }),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    (app, address)
}

/// The header close button's message, routed through `update`.
fn press_header_close(app: &mut Signex, address: &EditorAddress) {
    let _ = app.update(Message::Library(LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg: EditorMsg::CloseEditor,
    }));
}

fn has_preview_tab(app: &Signex) -> bool {
    app.document_state
        .tabs
        .iter()
        .any(|t| t.kind.is_component_editor())
}

#[test]
fn the_header_close_parks_a_dirty_row_draft() {
    let (mut app, address) = open_preview(true);

    press_header_close(&mut app, &address);

    assert!(!has_preview_tab(&app), "the tab closed");
    let parked = app
        .library
        .editors
        .get(&address)
        .expect("the dirty draft is parked, not discarded");
    assert!(parked.dirty, "the parked draft keeps its edit");
    assert_eq!(parked.row.state, LifecycleState::Obsolete);
}

#[test]
fn the_header_close_drops_a_clean_preview() {
    let (mut app, address) = open_preview(false);

    press_header_close(&mut app, &address);

    assert!(!has_preview_tab(&app), "the tab closed");
    assert!(
        !app.library.editors.contains_key(&address),
        "nothing to keep"
    );
}

/// Close Project → Discard All closes the row through
/// `close_component_preview`: that one is the user's discard.
#[test]
fn an_explicit_discard_still_drops_a_dirty_row_draft() {
    let (mut app, address) = open_preview(true);

    let _ = app.close_component_preview(&address);

    assert!(!has_preview_tab(&app), "the tab closed");
    assert!(
        !app.library.editors.contains_key(&address),
        "the discard dropped the draft"
    );
}
