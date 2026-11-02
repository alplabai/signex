//! Which tab a window is showing: the one decision the view, the overlay
//! stack, the status bar and the command layer share (#1818).

use super::{DocumentState, UiState, WindowKind};
use crate::app::TabInfo;

impl DocumentState {
    /// The tab a non-main window of `kind` is bound to: `None` for a window
    /// that is bound to no tab (a detached panel or modal), `Some(None)` for
    /// a document window whose tab is gone, else the tab's index. An
    /// undocked tab window is bound to the tab at its path, a Component
    /// Preview window to the tab of its row.
    pub(crate) fn tab_index_of_window_kind(&self, kind: &WindowKind) -> Option<Option<usize>> {
        let tabs = &self.tabs;
        match kind {
            WindowKind::UndockedTab { path, .. } => Some(tabs.iter().position(|t| &t.path == path)),
            WindowKind::ComponentEditor {
                library_path,
                table,
                row_id,
            } => Some(tabs.iter().position(|t| {
                t.kind.as_component_editor().is_some_and(|ce| {
                    &ce.library_path == library_path && &ce.table == table && &ce.row_id == row_id
                })
            })),
            WindowKind::DetachedModal(_) | WindowKind::DetachedPanel(_) => None,
        }
    }

    /// The tab `window_id` shows: the main window shows its active tab, any
    /// other document window the tab it is bound to. A window with no tab
    /// (a detached panel or modal, or a document window whose tab is gone)
    /// shows none.
    pub fn tab_shown_in_window(
        &self,
        window_id: iced::window::Id,
        ui: &UiState,
    ) -> Option<&TabInfo> {
        if ui.main_window_id == Some(window_id) {
            return self.tabs.get(self.active_tab);
        }
        let index = self.tab_index_of_window_kind(ui.windows.get(&window_id)?)??;
        self.tabs.get(index)
    }
}
