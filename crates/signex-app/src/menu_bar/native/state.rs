//! The native menu's state in the Model: whether the system menu bar could
//! be built. The menu itself is rebuilt from the Model after every update
//! (`super::build`); nothing about it is cached here.

use crate::diagnostics::log_warning;

#[derive(Debug, Default)]
pub(crate) struct NativeMenuState {
    /// The system menu bar could not be built; the in-window row stays.
    /// Sticky: a later successful menu does not bring the system bar back
    /// mid-session.
    failed: bool,
}

impl NativeMenuState {
    /// The iced runtime reported that the system menu bar cannot be built:
    /// let the in-window row come back, and say why in the Messages panel.
    pub(crate) fn fall_back(&mut self, reason: &str) {
        self.failed = true;
        log_warning(format!(
            "Main menu: could not build the system menu bar ({reason}); using the in-window menu"
        ));
    }

    /// Whether the in-window menu row is left out: on macOS the system menu
    /// bar carries the menus from the first frame on (the row would flash
    /// until the first install), unless building it failed.
    pub(crate) fn hides_in_window_menu(&self) -> bool {
        cfg!(target_os = "macos") && !self.failed
    }

    /// Whether building the system menu bar was reported as failed (tests).
    #[cfg(test)]
    pub(crate) fn has_failed(&self) -> bool {
        self.failed
    }
}
