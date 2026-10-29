//! Library edits that write through the mounted adapter, refused while a
//! library rename runs (#859).
//!
//! The adapter keeps pointing at the old `.snxlib` until the rename's
//! completion relocates it, so a write issued in between would recreate the
//! old directory or fail halfway. Only intents that write are refused; reads,
//! navigation, in-memory editing and completions of earlier async work still
//! go through (dropping a completion would strand its state).

use super::*;

impl Signex {
    /// Why `msg` cannot run now, or `None` when it can. The cause leads: the
    /// Messages panel clips a record near 160 characters.
    pub(super) fn library_write_refusal(&self, msg: &LibraryMessage) -> Option<String> {
        let ops = &self.document_state.pending_path_ops;
        if !ops.library_rename_running() || !writes_through_adapter(msg) {
            return None;
        }
        // A write that names its library is refused only for the library
        // being moved; one that names none cannot be told apart, so it waits.
        if let Some(library) = library_named_by(msg)
            && ops.blocking_write(library).is_none()
        {
            return None;
        }
        Some(
            "A library rename or delete is still running, so this library change was not applied. \
             Try again once it finishes."
                .to_string(),
        )
    }
}

/// The library a message addresses, when it carries its path.
fn library_named_by(msg: &LibraryMessage) -> Option<&std::path::Path> {
    use LibraryMessage as L;
    match msg {
        L::BrowserConfirmAddTable { library_path }
        | L::BrowserDeleteTable { library_path, .. }
        | L::BrowserConfirmRenameTable { library_path }
        | L::BrowserAddComponent { library_path, .. }
        | L::BrowserDeleteRowConfirm { library_path, .. }
        | L::BrowserCellCommit { library_path, .. }
        | L::BrowserEdit { library_path, .. }
        | L::ComponentsPanelPromoteToGlobal(library_path) => Some(library_path),
        _ => None,
    }
}

/// The messages whose handlers call a mutating `LibraryAdapter` method.
fn writes_through_adapter(msg: &LibraryMessage) -> bool {
    use LibraryMessage as L;
    match msg {
        L::NewComponent
        | L::BrowserConfirmAddTable { .. }
        | L::BrowserDeleteTable { .. }
        | L::BrowserConfirmRenameTable { .. }
        | L::BrowserAddComponent { .. }
        | L::BrowserDeleteRowConfirm { .. }
        | L::BrowserCellCommit { .. }
        | L::LibraryUpdatesApply
        | L::DocumentOptionsApply
        | L::PrimitivePicker(PrimitivePickerMsg::Pick(_))
        | L::CloseLibraryConfirm(_)
        | L::ComponentsPanelPromoteToGlobal(_) => true,
        L::BrowserEdit { msg, .. } => matches!(msg, BrowserEditMsg::Save),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::path_ops::PathOpKind;
    use crate::app::state::scope::CaseFold;

    fn app_with_library_rename() -> Signex {
        let (mut app, _task) = Signex::new();
        app.document_state.pending_path_ops.begin(
            PathOpKind::RenameLibrary,
            vec!["/w/a.snxlib".into(), "/w/b.snxlib".into()],
            CaseFold::PRESERVE,
        );
        app
    }

    #[test]
    fn writes_are_refused_while_a_library_rename_runs_and_reads_are_not() {
        let app = app_with_library_rename();
        let write = LibraryMessage::BrowserCellCommit {
            library_path: "/w/a.snxlib".into(),
            table: "t".into(),
            row_id: signex_library::RowId::default(),
            column: String::new(),
        };
        let refusal = app.library_write_refusal(&write).expect("refused");
        assert!(refusal.starts_with("A library rename or delete is still running"));
        assert!(
            app.library_write_refusal(&LibraryMessage::BrowserSearchChanged {
                library_path: "/w/a.snxlib".into(),
                value: "x".into(),
            })
            .is_none()
        );
    }

    /// The refusal is a card as well as a Messages line, like every other
    /// refusal while a path operation runs.
    #[test]
    fn a_refused_library_write_shows_a_card_and_reaches_messages() {
        let _ = crate::diagnostics::init_logging();
        let mut app = app_with_library_rename();
        let since = crate::diagnostics::newest_entry_id();
        let _task = app.update(crate::app::Message::Library(LibraryMessage::NewComponent));
        let notice = app.document_state.error_notice.clone().expect("a card");
        assert_eq!(notice.title, "Library Change Not Applied");
        assert!(
            notice
                .detail
                .starts_with("A library rename or delete is still running")
        );
        assert!(
            crate::diagnostics::recent_entries()
                .iter()
                .any(|e| Some(e.id) > since && e.message.contains("library change was not applied"))
        );
    }

    #[test]
    fn a_write_to_an_unrelated_library_is_not_refused() {
        let app = app_with_library_rename();
        let commit = |library: &str| LibraryMessage::BrowserCellCommit {
            library_path: library.into(),
            table: "t".into(),
            row_id: signex_library::RowId::default(),
            column: String::new(),
        };
        assert!(
            app.library_write_refusal(&commit("/w/other.snxlib"))
                .is_none()
        );
        assert!(app.library_write_refusal(&commit("/w/a.snxlib")).is_some());
        assert!(app.library_write_refusal(&commit("/w/b.snxlib")).is_some());
    }

    #[test]
    fn a_library_delete_blocks_adapter_writes_and_so_does_a_picker_pick() {
        let (mut app, _task) = Signex::new();
        app.document_state.pending_path_ops.begin(
            PathOpKind::Delete,
            vec!["/w/a.snxlib".into()],
            CaseFold::PRESERVE,
        );
        let pick = LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Pick(
            signex_library::PrimitiveRef::new(uuid::Uuid::nil(), uuid::Uuid::nil()),
        ));
        assert!(app.library_write_refusal(&pick).is_some());
    }

    #[test]
    fn deleting_a_plain_file_does_not_block_library_edits() {
        let (mut app, _task) = Signex::new();
        app.document_state.pending_path_ops.begin(
            PathOpKind::Delete,
            vec!["/w/a.snxsch".into()],
            CaseFold::PRESERVE,
        );
        assert!(
            app.library_write_refusal(&LibraryMessage::NewComponent)
                .is_none()
        );
    }

    #[test]
    fn nothing_is_refused_without_a_library_rename() {
        let (app, _task) = Signex::new();
        assert!(
            app.library_write_refusal(&LibraryMessage::NewComponent)
                .is_none()
        );
    }
}
