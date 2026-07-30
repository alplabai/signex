use iced::Task;

use super::super::*;

impl Signex {
    /// Tools > Passive Network Calculator. Opens the in-app modal.
    ///
    /// Deliberately not an OS window: Signex runs borderless
    /// (`bootstrap/new.rs` - `decorations: false`), so a default
    /// `iced::window::open` would arrive with a native title bar that
    /// appears nowhere else in the app. Re-invoking while the modal is
    /// already open is a no-op, matching every other modal here.
    pub(super) fn handle_open_passive_calculator(&mut self) -> Task<Message> {
        self.ui_state.passive_calculator_open = true;
        Task::none()
    }
}
