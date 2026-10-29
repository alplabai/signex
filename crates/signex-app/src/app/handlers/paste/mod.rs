//! Paste through the system clipboard (#1263), rubber-stamp copy and Paste
//! Array (#1264).
//!
//! Copy and Cut write the copied fragment to the OS clipboard as text
//! (`signex_engine::CopiedSelection::to_clipboard_text`). A paste first asks
//! the OS clipboard for its text — a `Task`, so `update` never waits on it —
//! and when the answer arrives a Signex fragment in it becomes the copy;
//! anything else leaves the copy made inside this window in charge.

mod array;
mod designators;
mod stamp;

use iced::Task;
use signex_engine::CopiedSelection;

use crate::app::{EditMsg, Message, PasteIntent, Signex};

impl Signex {
    /// Ask the system clipboard what it holds; `handle_paste_text_read`
    /// carries on with `intent` when it answers.
    pub(crate) fn request_paste(&mut self, intent: PasteIntent) -> Task<Message> {
        iced::clipboard::read()
            .map(move |text| Message::Edit(EditMsg::PasteTextRead { intent, text }))
    }

    pub(crate) fn handle_paste_text_read(
        &mut self,
        intent: PasteIntent,
        text: Option<String>,
    ) -> Task<Message> {
        self.adopt_clipboard_text(text.as_deref());
        match intent {
            PasteIntent::Paste => self.handle_clipboard_paste_requested(),
            PasteIntent::SmartPaste => self.handle_clipboard_smart_paste_requested(),
            PasteIntent::Array => self.open_paste_array(),
        }
        self.finish_update()
    }

    /// A Signex fragment in `text` replaces the in-app copy. Non-Signex text
    /// is none of our business; a fragment that cannot be read is reported
    /// and the in-app copy is used instead.
    fn adopt_clipboard_text(&mut self, text: Option<&str>) {
        let Some(text) = text else { return };
        let grid = crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm);
        match CopiedSelection::from_clipboard_text(text, grid) {
            Ok(Some(copied)) => self.interaction_state.clipboard = Some(copied),
            Ok(None) => {}
            Err(error) => crate::diagnostics::log_warning(format!(
                "Paste: the system clipboard holds a Signex fragment that could not be read ({error}); the copy made in this window is used instead."
            )),
        }
    }

    /// The task that puts the in-app copy on the system clipboard as text.
    /// A copy that cannot be written stays usable inside this window.
    pub(crate) fn clipboard_write_task(&self) -> Task<Message> {
        let Some(copied) = self.interaction_state.clipboard.as_ref() else {
            return Task::none();
        };
        match copied.to_clipboard_text() {
            Ok(text) => iced::clipboard::write(text),
            Err(error) => {
                crate::diagnostics::log_warning(format!(
                    "Copy: the selection could not be written to the system clipboard ({error}); it can still be pasted in this window."
                ));
                Task::none()
            }
        }
    }
}
