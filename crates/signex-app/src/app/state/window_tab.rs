//! Which tab a window is showing: the one decision the view, the status
//! bar and the shortcut contexts share (#1818).

use super::{DocumentState, UiState, WindowKind};
use crate::app::TabInfo;

impl DocumentState {
    /// The tab `window_id` shows: the main window shows its active tab, an
    /// undocked tab window shows the tab it was opened on. A window with no
    /// tab (a detached panel, a modal, a Component Preview window, or an
    /// undocked window whose tab is gone) shows none.
    pub fn tab_shown_in_window(
        &self,
        window_id: iced::window::Id,
        ui: &UiState,
    ) -> Option<&TabInfo> {
        if ui.main_window_id == Some(window_id) {
            return self.tabs.get(self.active_tab);
        }
        match ui.windows.get(&window_id)? {
            WindowKind::UndockedTab { path, .. } => self.tabs.iter().find(|t| &t.path == path),
            _ => None,
        }
    }
}
