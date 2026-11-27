//! Show History on a Projects tree leaf (mockup row 20): open the file, then
//! show the History panel filtered to it once the panel follows that file.

use super::*;

impl Signex {
    /// Show History on the leaf at `path`. A missing child sheet gets only
    /// its Create Sheet offer. A file read on a worker gets the panel when
    /// the read lands ([`Self::resume_tree_history`]). An open that fails
    /// leaves the History panel as it was.
    pub(super) fn show_tree_row_history(&mut self, path: &[usize]) -> Task<Message> {
        use signex_widgets::tree_view::get_node;
        let tree = self.document_view.panel_ctx.project_tree.as_slice();
        let Some(label) = get_node(tree, path)
            .filter(|node| tree_row_opens(node))
            .map(|node| node.label.clone())
        else {
            return Task::none();
        };
        if let Some(missing) = self.missing_child_sheet_at(path) {
            return self.offer_create_child_sheet(&missing);
        }
        // The spelling the open uses, so it compares with the tab and the read.
        let file = self
            .tree_path_to_file_path(path)
            .map(|file| self.resolve_held_spelling(file));
        let opened = self.open_tree_row(path, label);
        let Some(file) = file else {
            return opened;
        };
        if self.history_target_path().as_deref() == Some(file.as_path()) {
            self.ui_state.pending_tree_history = None;
            return Task::batch([opened, self.show_history_filtered()]);
        }
        // The latest Show History wins; one with no read to wait on drops it.
        self.ui_state.pending_tree_history = self
            .document_state
            .pending_opens
            .contains(&file)
            .then_some(file);
        opened
    }

    /// A read of `opened` finished. A Show History waiting on it shows the
    /// panel when the panel now follows that file; a failed or overtaken read
    /// drops the request. A newer read of the same file keeps it waiting.
    pub(crate) fn resume_tree_history(&mut self, opened: &std::path::Path) -> Task<Message> {
        if self.ui_state.pending_tree_history.as_deref() != Some(opened)
            || self.document_state.pending_opens.contains(opened)
        {
            return Task::none();
        }
        self.ui_state.pending_tree_history = None;
        if self.history_target_path().as_deref() != Some(opened) {
            return Task::none();
        }
        self.show_history_filtered()
    }

    /// This file only set (not toggled) and the History panel shown.
    fn show_history_filtered(&mut self) -> Task<Message> {
        use crate::panels::history::{HistoryFilterMsg, HistoryPanelMsg};
        let filtered = if self.document_view.history.ui.filter.this_file_only {
            Task::none()
        } else {
            self.handle_history_panel(HistoryPanelMsg::Filter(HistoryFilterMsg::ThisFileToggled))
        };
        let shown = self.show_panel(crate::panels::PanelKind::History);
        Task::batch([filtered, shown, self.finish_update()])
    }
}
