//! The status bar's Grid & Snap popover (mockup row 6) never outlives what it
//! was opened over. `Signex::update` reads a [`GridSnapPopoverWatch`] before
//! each message and closes the popover after it when its window shows another
//! document now (another tab, or the window closed) or a dialog opened. One
//! place for every route, so no handler clears it by hand.

use std::path::PathBuf;

use super::super::*;

/// What the open popover stood over before a message.
pub(crate) struct GridSnapPopoverWatch {
    window: iced::window::Id,
    tab: Option<PathBuf>,
    dialogs: usize,
}

impl Signex {
    /// `None` while the popover is closed: there is nothing to watch.
    pub(crate) fn watch_grid_snap_popover(&self) -> Option<GridSnapPopoverWatch> {
        let window = self.ui_state.grid_snap_popover.as_deref()?.window;
        Some(GridSnapPopoverWatch {
            window,
            tab: self.grid_snap_popover_tab(window),
            dialogs: self.dialogs_open_above_grid_snap_popover(),
        })
    }

    /// Close the popover when the message moved its window to another
    /// document or opened a dialog. A popover the message (re)opened in
    /// another window is new, not watched.
    pub(crate) fn sync_grid_snap_popover(&mut self, before: Option<GridSnapPopoverWatch>) {
        let Some(before) = before else {
            return;
        };
        let Some(window) = self.ui_state.grid_snap_popover.as_deref().map(|p| p.window) else {
            return;
        };
        if window != before.window {
            return;
        }
        let moved = self.grid_snap_popover_tab(window) != before.tab;
        if moved || self.dialogs_open_above_grid_snap_popover() > before.dialogs {
            self.ui_state.grid_snap_popover = None;
        }
    }

    fn grid_snap_popover_tab(&self, window: iced::window::Id) -> Option<PathBuf> {
        self.document_state
            .tab_shown_in_window(window, &self.ui_state)
            .map(|tab| tab.path.clone())
    }
}
