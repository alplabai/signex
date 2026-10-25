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
        if !self
            .document_state
            .pending_path_ops
            .library_rename_running()
        {
            return None;
        }
        writes_through_adapter(msg).then(|| {
            "A library rename is still running, so this library change was not applied. \
             Try again once it finishes."
                .to_string()
        })
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
        | L::BrowserConfirmAddClass { .. }
        | L::BrowserDeleteClass { .. }
        | L::BrowserConfirmRenameClass { .. }
        | L::BrowserAddComponent { .. }
        | L::BrowserDeleteRowConfirm { .. }
        | L::BrowserCellCommit { .. }
        | L::LibraryUpdatesApply
        | L::DocumentOptionsApply
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
        assert!(refusal.starts_with("A library rename is still running"));
        assert!(
            app.library_write_refusal(&LibraryMessage::BrowserSearchChanged {
                library_path: "/w/a.snxlib".into(),
                value: "x".into(),
            })
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
