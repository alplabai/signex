//! The single "open panel X" entry point.

use iced::Task;

use crate::panels::PanelKind;

use super::super::super::*;

impl Signex {
    /// Open `kind`, or reveal it if it is already on screen.
    ///
    /// A panel kind exists at most once across the whole app (#641).
    /// It can live in one of three places, and this checks all of
    /// them before placing anything:
    ///
    /// - a detached OS window — raise that window,
    /// - somewhere in the dock — reveal it there (`DockArea::show_panel`
    ///   handles both docked tabs and floating panels),
    /// - nowhere — dock it at `PanelPosition::default_for(kind)`.
    ///
    /// Every open gesture routes through here: the View menu, the
    /// status-bar panel list, an ERC run surfacing its results, TAB
    /// during placement surfacing Properties.
    pub(crate) fn show_panel(&mut self, kind: PanelKind) -> Task<Message> {
        if let Some(window_id) = self.detached_panel_window(kind) {
            // The panel is a window of its own. Docking a second copy
            // would render the same panel twice on two screens.
            return iced::window::gain_focus(window_id);
        }
        self.document_state.dock.show_panel(kind);
        // Opening from the View menu used to skip this while opening
        // from the panel list persisted — so which entry point you
        // used decided whether the layout survived a restart.
        crate::fonts::write_dock_layout(&self.document_state.dock);
        Task::none()
    }

    /// The OS window hosting `kind` as a detached panel, if any.
    fn detached_panel_window(&self, kind: PanelKind) -> Option<iced::window::Id> {
        self.ui_state
            .windows
            .iter()
            .find(
                |(_, w)| matches!(w, crate::app::state::WindowKind::DetachedPanel(k) if *k == kind),
            )
            .map(|(id, _)| *id)
    }
}
