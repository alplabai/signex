//! Library Browser tab dispatch — filter + row selection.
//!
//! Maps to LIBRARY_PLAN.md §10 (Component Editor / Symbol tab) for
//! preview rendering and §11 item 4 (parametric / faceted picker) for
//! the live filter. v0.11 ships substring match + read-only preview;
//! the tantivy-backed parametric search and the per-component editor
//! flows layer on top of these same hooks without reshaping state.

use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_library_browser_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::LibraryBrowserFilterChanged(text) => {
                self.document_state.library_browser_filter = text;
                // Filter change can shrink the visible row set; the
                // currently-highlighted row may have just dropped out
                // of view. Clear the selection so the preview pane
                // doesn't show a row the user can no longer click.
                self.document_state.library_browser_selection = None;
                self.finish_update()
            }
            Message::LibraryBrowserSelectRow(index) => {
                self.document_state.library_browser_selection = Some(index);
                self.finish_update()
            }
            _ => unreachable!("dispatch_library_browser_message received non-library message"),
        }
    }
}
