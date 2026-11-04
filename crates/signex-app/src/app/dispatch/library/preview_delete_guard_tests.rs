//! #2045: a Component Preview edit is refused while its library is being
//! deleted, lands during a rename, and view-only events always go through.

use super::stale_row_tests::{app_with_row, editor};
use crate::app::Message;
use crate::app::Signex;
use crate::app::path_ops::PathOpKind;
use crate::app::state::scope::CaseFold;
use crate::library::LibraryMessage;
use crate::library::messages::{EditorMsg, FootprintEditorMsg};
use crate::library::state::{EditorAddress, PreviewTab};

fn preview_event(address: &EditorAddress, msg: EditorMsg) -> Message {
    Message::Library(LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg,
    })
}

fn app_with_clean_preview() -> (Signex, EditorAddress) {
    let (mut app, address) = app_with_row(1, 1);
    app.library.editors.get_mut(&address).unwrap().dirty = false;
    (app, address)
}

fn begin_op(app: &mut Signex, address: &EditorAddress, kind: PathOpKind) {
    app.document_state.pending_path_ops.begin(
        kind,
        vec![address.library_path.clone()],
        CaseFold::PRESERVE,
    );
}

#[test]
fn a_preview_edit_during_a_library_delete_is_refused_and_logged() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, address) = app_with_clean_preview();
    begin_op(&mut app, &address, PathOpKind::Delete);
    let before = editor(&app, &address).row.primary_mpn.mpn.clone();
    let since = crate::diagnostics::newest_entry_id();

    let _task = app.update(preview_event(
        &address,
        EditorMsg::SupplyPrimarySetMpn("CHANGED".into()),
    ));

    assert_eq!(editor(&app, &address).row.primary_mpn.mpn, before);
    assert!(!editor(&app, &address).dirty);
    assert!(
        crate::diagnostics::recent_entries().iter().any(|e| {
            Some(e.id) > since
                && e.message.contains("StaleRowLib")
                && e.message.contains("is being deleted")
                && e.message.contains("edit was not applied")
        }),
        "Messages names the library"
    );

    // View-only events still go through.
    let _task = app.update(preview_event(
        &address,
        EditorMsg::SelectTab(PreviewTab::Supply),
    ));
    assert_eq!(editor(&app, &address).active_tab, PreviewTab::Supply);
}

#[test]
fn a_preview_edit_during_a_library_rename_lands() {
    let (mut app, address) = app_with_clean_preview();
    begin_op(&mut app, &address, PathOpKind::RenameLibrary);

    let _task = app.update(preview_event(
        &address,
        EditorMsg::SupplyPrimarySetMpn("CHANGED".into()),
    ));

    assert_eq!(editor(&app, &address).row.primary_mpn.mpn, "CHANGED");
    assert!(editor(&app, &address).dirty);
}

fn warnings_since(since: Option<u64>) -> Vec<String> {
    crate::diagnostics::recent_entries()
        .iter()
        .filter(|e| Some(e.id) > since)
        .map(|e| e.message.clone())
        .collect()
}

#[test]
fn footprint_view_events_pass_a_delete_but_footprint_edits_are_refused() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, address) = app_with_clean_preview();
    begin_op(&mut app, &address, PathOpKind::Delete);
    let since = crate::diagnostics::newest_entry_id();

    let view = EditorMsg::Footprint(FootprintEditorMsg::CloseContextMenu);
    assert!(app.preview_edit_refusal(&address, &view).is_none());
    let _task = app.update(preview_event(&address, view));
    assert!(
        warnings_since(since)
            .iter()
            .all(|m| !m.contains("edit was not applied")),
        "a view event logs no refusal"
    );

    let edit = EditorMsg::Footprint(FootprintEditorMsg::DeleteSelected);
    assert!(app.preview_edit_refusal(&address, &edit).is_some());
}

#[test]
fn a_step_pick_answered_during_a_delete_names_the_file() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, address) = app_with_clean_preview();
    begin_op(&mut app, &address, PathOpKind::Delete);
    let since = crate::diagnostics::newest_entry_id();

    let _task = app.update(preview_event(
        &address,
        EditorMsg::StepAttachResult(Some((vec![1], "body.step".into()))),
    ));
    assert!(
        warnings_since(since)
            .iter()
            .any(|m| m.contains("body.step was not attached") && m.contains("is being deleted")),
        "Messages names the picked file"
    );

    let none = EditorMsg::StepAttachResult(None);
    let line = app.preview_edit_refusal(&address, &none).unwrap();
    assert!(line.starts_with("The picked file was not attached"));
}
