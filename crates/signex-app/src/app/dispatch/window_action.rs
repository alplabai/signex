//! Actions started in an undocked document window act on that window's
//! document, not the main window's active tab (#1910, #1966).
//!
//! The view tags a window's messages with [`Signex::scope_to_window`]; the
//! router runs them through [`Signex::run_action_in_window`], which swaps the
//! window's canvas and document in for the duration of the handler.

use iced::Task;

use crate::app::state::WindowKind;
use crate::app::{Message, Signex, WindowMsg};

impl Signex {
    /// Whether `window` is an undocked tab window with a canvas of its own,
    /// so an action from it can run against the document it shows.
    pub(crate) fn is_document_window(&self, window: iced::window::Id) -> bool {
        self.ui_state.main_window_id != Some(window)
            && matches!(
                self.ui_state.windows.get(&window),
                Some(WindowKind::UndockedTab { .. })
            )
            && self.interaction_state.canvases.contains_key(&window)
    }

    /// `message` as the window `window` sends it: unchanged for the main
    /// window and for any window without a document of its own, tagged with
    /// the window otherwise.
    pub(crate) fn scope_to_window(&self, window: iced::window::Id, message: Message) -> Message {
        if !self.is_document_window(window)
            || matches!(
                message,
                Message::CanvasEventInWindow { .. }
                    | Message::Window(WindowMsg::ActionTakenInWindow { .. })
            )
        {
            return message;
        }
        Message::Window(WindowMsg::ActionTakenInWindow {
            window,
            message: Box::new(message),
        })
    }

    /// Run `message` against the document `window` shows. A window that has
    /// closed since the click drops the action rather than let it fall
    /// through to the main window.
    pub(crate) fn run_action_in_window(
        &mut self,
        window: iced::window::Id,
        message: Message,
    ) -> Task<Message> {
        if self.ui_state.main_window_id == Some(window) {
            return self.update(message);
        }
        if !self.is_document_window(window) {
            return Task::none();
        }
        self.run_in_window_document(window, |app| app.update(message))
    }

    /// Whether the surface stamped with `origin` (an undocked window, or
    /// `None` for the main window) belongs to `window`.
    pub(crate) fn belongs_to_window(
        &self,
        origin: Option<iced::window::Id>,
        window: iced::window::Id,
    ) -> bool {
        match origin {
            Some(id) => id == window,
            None => self.ui_state.main_window_id == Some(window),
        }
    }

    /// Whether `window` shows the sheet at `path`; `None` stands for the main
    /// window's own.
    pub(crate) fn window_shows_sheet(
        &self,
        window: iced::window::Id,
        path: Option<&std::path::Path>,
    ) -> bool {
        let Some(path) = path else {
            return self.ui_state.main_window_id == Some(window);
        };
        if self.ui_state.main_window_id == Some(window) {
            return self.document_state.active_path.as_deref() == Some(path);
        }
        matches!(
            self.ui_state.windows.get(&window),
            Some(WindowKind::UndockedTab { path: shown, .. }) if shown == path
        )
    }
}

#[cfg(test)]
#[path = "window_action_tests.rs"]
mod tests;
