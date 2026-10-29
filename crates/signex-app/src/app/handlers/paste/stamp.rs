//! Rubber-stamp copy (#1264): copy the selection, then keep pasting it.

use iced::Task;

use crate::app::handlers::clipboard_workflows::CopyOutcome;
use crate::app::{Message, PasteIntent, Signex};

impl Signex {
    /// Copy the selection and attach the copy to the cursor as a stamp:
    /// every click places it as its own undo step and it stays armed until
    /// Esc or a right-click. With Next free numbering the stamp is armed once
    /// the project's numbers have been read.
    pub(crate) fn handle_rubber_stamp_requested(&mut self) -> Task<Message> {
        if self.interaction_state.active_canvas().selected.is_empty()
            || !self.has_active_schematic()
        {
            return self.finish_update();
        }
        let outcome = self.handle_selection_copy_requested();
        let write = self.mirror_copy_on_clipboard_task(outcome);
        if outcome != CopyOutcome::Carried {
            return Task::batch([write, self.finish_update()]);
        }
        Task::batch([write, self.begin_paste(PasteIntent::Stamp)])
    }
}
