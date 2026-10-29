//! Rubber-stamp copy (#1264): copy the selection, then keep pasting it.

use iced::Task;

use crate::app::state::PlacementKind;
use crate::app::{Message, Signex};

impl Signex {
    /// Copy the selection and attach the copy to the cursor as a stamp:
    /// every click places it as its own undo step and it stays armed until
    /// Esc or a right-click.
    pub(crate) fn handle_rubber_stamp_requested(&mut self) -> Task<Message> {
        if self.interaction_state.active_canvas().selected.is_empty()
            || !self.has_active_schematic()
            || !self.handle_selection_copy_requested()
        {
            return self.finish_update();
        }
        let write = self.clipboard_write_task();
        self.arm_paste_session(PlacementKind::Stamp);
        Task::batch([write, self.finish_update()])
    }
}
