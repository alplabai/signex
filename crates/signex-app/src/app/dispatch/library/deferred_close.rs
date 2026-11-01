//! A Close Library that waits for the library's writes (#893, #1728): the
//! Messages lines that make it visible, and the check that finishes it.
//! Design: `docs/architecture/library-writes.md`, "Closing a library".

use std::path::Path;

use super::*;

impl Signex {
    /// Close the library at `root` now, or record that the close waits for
    /// its writes and say so in Messages.
    pub(super) fn close_library_or_defer(&mut self, root: &Path) {
        self.library.close_library(root);
        if self.library.is_closing(root) {
            crate::diagnostics::log_info(format!(
                "Close Library waits for the writes of {}; it closes once they are done.",
                self.library.library_label(root)
            ));
        }
    }

    /// Finish every deferred close whose library has no write left. Called
    /// wherever a write turn is released (a write, a refresh, a container
    /// save, a git re-init) and on the external-edit poll tick, which also
    /// catches a turn released by a task the runtime dropped.
    pub(crate) fn close_libraries_whose_writes_are_done(&mut self) {
        let ready = self.library.closes_ready();
        if ready.is_empty() {
            return;
        }
        for root in ready {
            self.finish_deferred_close(&root);
        }
        self.refresh_panel_ctx();
    }

    /// Close the Library Browser and Component Preview tabs of every
    /// library unmounted since the last update, whichever path closed it
    /// (at once or once its writes were done). Their drafts were already
    /// saved or discarded through the close prompt. A library mounted again
    /// meanwhile keeps its tabs.
    pub(crate) fn close_tabs_of_unmounted_libraries(&mut self) -> Task<Message> {
        let unmounted = std::mem::take(&mut self.library.unmounted);
        let mut tasks = Vec::new();
        for root in unmounted {
            if self.library.library_at(&root).is_some() {
                continue;
            }
            while let Some(idx) = self
                .document_state
                .tabs
                .iter()
                .position(|tab| tab_belongs_to_library(&tab.kind, &root))
            {
                tasks.push(self.close_tab_now(idx));
            }
        }
        Task::batch(tasks)
    }

    /// The unsaved-editor check runs again when the close comes due: an
    /// editor opened or edited while the close waited is never dropped
    /// silently. A dirty one cancels the close and raises the same prompt
    /// Close Library raises.
    fn finish_deferred_close(&mut self, root: &Path) {
        let name = self.library.library_label(root);
        let dirty = self.library.dirty_editors_for_library(root);
        if dirty.is_empty() {
            self.library.close_library(root);
            crate::diagnostics::log_info(format!("Library {name} closed: its writes are done."));
            return;
        }
        self.library.cancel_close(root);
        crate::diagnostics::log_warning(format!(
            "Close Library of {name} stopped: {} editor(s) opened while it waited have unsaved edits.",
            dirty.len()
        ));
        // Another library's prompt stays up; this close is simply cancelled
        // and the user can ask again.
        if self.library.close_library_confirm.is_none() {
            self.raise_close_library_prompt(root.to_path_buf(), dirty);
        }
    }
}

/// Whether a tab of `kind` shows the library at `root`: its browser, or a
/// Component Preview of one of its rows.
fn tab_belongs_to_library(kind: &crate::app::TabKind, root: &Path) -> bool {
    match kind {
        crate::app::TabKind::LibraryBrowser(path) => path == root,
        crate::app::TabKind::ComponentEditor(preview) => preview.library_path == root,
        _ => false,
    }
}
