//! Paste through the system clipboard (#1263), rubber-stamp copy and Paste
//! Array (#1264).
//!
//! Copy and Cut write the copied fragment to the OS clipboard as text
//! (`signex_engine::CopiedSelection::to_clipboard_text`). A paste is a short
//! chain of steps, each a `Task` so `update` never waits on I/O:
//!
//! 1. when Next free numbering needs the project's designators, read them off
//!    the UI thread (`seed`), once;
//! 2. ask the OS clipboard for its text; a Signex fragment in it becomes the
//!    copy, anything else leaves the copy made in this window in charge;
//! 3. arm, place or open the dialog.
//!
//! Every commit therefore happens after the numbers landed: a request whose
//! numbers are still being read has not armed or placed anything yet. A
//! request answered after Esc, a tool change or a sheet switch is dropped
//! (`PasteRequest`).

mod array;
mod designators;
mod seed;
mod stamp;

use iced::Task;
use signex_engine::CopiedSelection;

pub use seed::PasteSeed;

use crate::app::handlers::clipboard_workflows::CopyOutcome;
use crate::app::state::PlacementKind;
use crate::app::{EditMsg, Message, PasteIntent, PasteRequest, Signex};

impl Signex {
    /// Start a paste-family request: read the project's numbers first when
    /// the numbering needs them, then carry on.
    pub(crate) fn begin_paste(&mut self, intent: PasteIntent) -> Task<Message> {
        let request = PasteRequest {
            intent,
            epoch: self.interaction_state.placement_epoch,
            path: self.document_state.active_path.clone(),
            dialog: 0,
        };
        match self.paste_seed_task(&request) {
            Some(seeding) => seeding,
            None => self.continue_paste(request),
        }
    }

    /// Paste, Smart Paste and Paste Array requests start here.
    pub(crate) fn request_paste(&mut self, intent: PasteIntent) -> Task<Message> {
        self.begin_paste(intent)
    }

    /// The step after the numbers are ready.
    fn continue_paste(&mut self, request: PasteRequest) -> Task<Message> {
        match request.intent {
            PasteIntent::Paste | PasteIntent::SmartPaste | PasteIntent::Array => {
                iced::clipboard::read().map(move |text| {
                    Message::Edit(EditMsg::PasteTextRead {
                        request: request.clone(),
                        text,
                    })
                })
            }
            PasteIntent::Duplicate => {
                use crate::app::handlers::clipboard_workflows::DEFAULT_PASTE_OFFSET_MM as OFFSET;
                self.paste_with_offset(OFFSET, OFFSET);
                self.finish_update()
            }
            PasteIntent::Stamp => {
                self.arm_paste_session(PlacementKind::Stamp);
                self.finish_update()
            }
            PasteIntent::ArraySeed => self.finish_update(),
        }
    }

    pub(crate) fn handle_paste_counters_seeded(
        &mut self,
        request: PasteRequest,
        seed: &PasteSeed,
    ) -> Task<Message> {
        if request.intent == PasteIntent::ArraySeed {
            // Only the dialog that asked is released; an answer for one that
            // closed or was replaced is dropped without a word.
            let asked = self
                .ui_state
                .paste_array
                .as_mut()
                .filter(|dialog| dialog.id == request.dialog);
            let Some(dialog) = asked else {
                return self.finish_update();
            };
            dialog.seed_pending = false;
        }
        if self.paste_request_is_stale(&request) {
            return self.finish_update();
        }
        self.store_paste_seed(&request, seed);
        self.continue_paste(request)
    }

    pub(crate) fn handle_paste_text_read(
        &mut self,
        request: PasteRequest,
        text: Option<String>,
    ) -> Task<Message> {
        if self.paste_request_is_stale(&request) {
            return self.finish_update();
        }
        self.adopt_clipboard_text(text.as_deref());
        match request.intent {
            PasteIntent::SmartPaste => self.handle_clipboard_smart_paste_requested(),
            PasteIntent::Array => self.open_paste_array(),
            _ => self.handle_clipboard_paste_requested(),
        }
        self.finish_update()
    }

    /// Whether the sheet or the tool state moved on since `request` was made
    /// (Esc, another tool, a tab switch); said once in the Messages panel.
    fn paste_request_is_stale(&self, request: &PasteRequest) -> bool {
        // The numbers for an open dialog stay good across tool changes: only
        // the sheet matters (the dialog closes when it changes).
        let tool_moved = request.intent != PasteIntent::ArraySeed
            && request.epoch != self.interaction_state.placement_epoch;
        let stale = tool_moved || request.path != self.document_state.active_path;
        if stale && request.intent != PasteIntent::ArraySeed {
            crate::diagnostics::log_info(
                "Paste cancelled: the tool or the sheet changed before the paste was ready.",
            );
        }
        stale
    }

    /// A Signex fragment in `text` replaces the in-app copy. Non-Signex text
    /// is none of our business; a fragment that cannot be read is reported
    /// and the in-app copy is used instead.
    fn adopt_clipboard_text(&mut self, text: Option<&str>) {
        let Some(text) = text else { return };
        if self.interaction_state.clipboard_text.as_deref() == Some(text) {
            return;
        }
        let grid = crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm);
        match CopiedSelection::from_clipboard_text(text, grid) {
            Ok(Some(copied)) => {
                self.interaction_state.clipboard = Some(copied);
                self.interaction_state.clipboard_text = Some(text.to_owned());
            }
            Ok(None) => {}
            Err(error) => crate::diagnostics::log_warning(format!(
                "Paste: the system clipboard holds a Signex fragment that could not be read ({error}); the copy made in this window is used instead."
            )),
        }
    }

    /// Copy the selection and the task that mirrors the result on the system
    /// clipboard.
    pub(crate) fn copy_and_write_clipboard_task(&mut self) -> Task<Message> {
        let outcome = self.handle_selection_copy_requested();
        self.mirror_copy_on_clipboard_task(outcome)
    }

    /// The system-clipboard side of a copy. One that carried nothing empties
    /// it, so an older fragment left there cannot be pasted in its place; a
    /// copy that was not made leaves it alone.
    pub(crate) fn mirror_copy_on_clipboard_task(&mut self, outcome: CopyOutcome) -> Task<Message> {
        match outcome {
            CopyOutcome::Carried => self.clipboard_write_task(),
            CopyOutcome::NothingToCarry => iced::clipboard::write(String::new()),
            CopyOutcome::Failed => Task::none(),
        }
    }

    /// The task that puts the in-app copy on the system clipboard as text.
    /// When it cannot be serialised the clipboard is emptied instead, so an
    /// older fragment left there cannot be pasted in its place; the copy
    /// stays usable inside this window.
    pub(crate) fn clipboard_write_task(&mut self) -> Task<Message> {
        let Some(copied) = self.interaction_state.clipboard.as_ref() else {
            return Task::none();
        };
        match copied.to_clipboard_text() {
            Ok(text) => {
                self.interaction_state.clipboard_text = Some(text.clone());
                iced::clipboard::write(text)
            }
            Err(error) => {
                self.interaction_state.clipboard_text = None;
                crate::diagnostics::log_warning(format!(
                    "Copy: the selection could not be written to the system clipboard ({error}); the clipboard was emptied so an older copy cannot be pasted instead, and this selection can still be pasted in this window."
                ));
                iced::clipboard::write(String::new())
            }
        }
    }
}
