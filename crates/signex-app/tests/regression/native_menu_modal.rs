//! The system menu bar must not act on the document while a confirmation
//! dialog is open: the quit confirm lists a snapshot of the dirty files, and
//! an edit made through the bar behind it would be lost by Discard All.
//!
//! The row map is private to the crate and only filled on macOS, so this
//! drives the public message route with the ids a live bar would send; the
//! seeded-map cases (live id dispatches, refusal under a stale map) are unit
//! tests in `app/handlers/native_menu.rs`.

use signex_app::app::{AppQuitConfirmState, Message, Signex, WindowMsg};
use std::path::PathBuf;

#[test]
fn native_edit_rows_do_not_change_the_document_behind_the_quit_confirm() {
    let (mut app, _boot) = Signex::new();
    let a = PathBuf::from("/project/a.snxsch");
    app.document_state.dirty_paths.insert(a.clone());
    app.ui_state.app_quit_confirm = Some(AppQuitConfirmState {
        dirty_paths: vec![a.clone()],
        dirty_rows: Vec::new(),
    });

    for id in ["Edit/Delete", "Edit/Paste", "File/Save", "File/Exit"] {
        let _ = app.update(Message::Window(WindowMsg::NativeMenuItemActivated(
            id.to_owned(),
        )));
    }

    assert_eq!(
        app.document_state.dirty_paths.iter().collect::<Vec<_>>(),
        vec![&a]
    );
    let confirm = app.ui_state.app_quit_confirm.as_ref().expect("still open");
    assert_eq!(confirm.dirty_paths, vec![a]);
}
