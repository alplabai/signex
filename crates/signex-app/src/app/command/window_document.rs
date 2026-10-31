//! Commands that act on a document, resolved against the window the stroke
//! (or menu click) came from.
//!
//! Strokes route globally and the handlers act on the main window's active
//! tab, so Ctrl+F4 typed in another window used to close a document the user
//! was not looking at (#1787), and Save, Save As, Print and the tab stepper
//! did the same (#1794). Those commands are per-window, like Undo / Redo.

use iced::Task;

use crate::app::Message;
use crate::app::Signex;
use crate::app::state::WindowKind;
use crate::keymap::AppCommandId;

/// What a non-main window shows, as far as a document command is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowDocument {
    /// The window shows the document of `document_state.tabs[index]`.
    Tab(usize),
    /// A document window whose tab is gone; there is nothing to act on.
    NoTab,
}

/// Commands that act on the document the window shows (see
/// [`Signex::dispatch_command`]).
pub(super) fn is_window_document_command(command: &AppCommandId) -> bool {
    matches!(
        command.as_str(),
        "save_document"
            | "save_document_as"
            | "print"
            | "next_document_tab"
            | "previous_document_tab"
    )
}

fn is_tab_stepper(command: &AppCommandId) -> bool {
    matches!(
        command.as_str(),
        "next_document_tab" | "previous_document_tab"
    )
}

impl Signex {
    /// The document the window `window` shows, or `None` when it is the main
    /// window, a detached panel or modal, or names none; the ordinary
    /// handlers then apply to the main window's document.
    pub(super) fn document_of_window(
        &self,
        window: Option<iced::window::Id>,
    ) -> Option<WindowDocument> {
        let id = window?;
        if self.ui_state.main_window_id == Some(id) {
            return None;
        }
        let tabs = &self.document_state.tabs;
        match self.ui_state.windows.get(&id)? {
            WindowKind::UndockedTab { path, .. } => {
                Some(match tabs.iter().position(|t| &t.path == path) {
                    Some(index) => WindowDocument::Tab(index),
                    None => WindowDocument::NoTab,
                })
            }
            WindowKind::ComponentEditor {
                library_path,
                table,
                row_id,
            } => Some(
                match tabs.iter().position(|t| {
                    t.kind.as_component_editor().is_some_and(|ce| {
                        &ce.library_path == library_path
                            && &ce.table == table
                            && &ce.row_id == row_id
                    })
                }) {
                    Some(index) => WindowDocument::Tab(index),
                    None => WindowDocument::NoTab,
                },
            ),
            // A detached panel or modal edits the main window's document, so
            // its strokes keep the main-window behaviour.
            WindowKind::DetachedModal(_) | WindowKind::DetachedPanel(_) => None,
        }
    }

    /// Close Document typed in `window`, or `None` when `window` is the main
    /// window (or names none) and the ordinary handler applies.
    ///
    /// - Undocked tab or Component Preview: close that window's own tab
    ///   through `close_tab_parking_dirty` (dirty drafts are parked), which
    ///   also closes the window. A Component Preview whose tab is gone is
    ///   closed as the OS close button does (`WindowCloseRequested`).
    /// - Detached modal or panel: nothing happens.
    pub(super) fn close_document_in_window(
        &mut self,
        window: Option<iced::window::Id>,
    ) -> Option<Task<Message>> {
        let id = window?;
        // Close Document in a detached panel or modal does nothing (#1792):
        // closing the main window's tab from a tool window is surprising.
        if matches!(
            self.ui_state.windows.get(&id),
            Some(WindowKind::DetachedModal(_) | WindowKind::DetachedPanel(_))
        ) && self.ui_state.main_window_id != Some(id)
        {
            return Some(Task::none());
        }
        Some(match self.document_of_window(window)? {
            WindowDocument::Tab(index) => self.close_tab_parking_dirty(index),
            WindowDocument::NoTab
                if matches!(
                    self.ui_state.windows.get(&id),
                    Some(WindowKind::ComponentEditor { .. })
                ) =>
            {
                self.dispatch_update(Message::Window(
                    crate::app::WindowMsg::WindowCloseRequested(id),
                ))
            }
            WindowDocument::NoTab => Task::none(),
        })
    }

    /// Save, Save As, Print and the tab stepper typed in `window`, or `None`
    /// when `window` is the main window (or names none).
    ///
    /// - A window showing a tab runs the command with that tab as the active
    ///   one, so the document written, exported or enabled-checked is the
    ///   window's own, never the main window's.
    /// - Next / Previous Document Tab does nothing there: the window holds a
    ///   single document, so there is no tab to step to, and the main
    ///   window's tab strip is not the user's focus.
    /// - A Component Preview does nothing but say why in the Messages panel;
    ///   a window with no tab does nothing.
    /// - A detached panel or modal keeps the main-window behaviour: it edits
    ///   the main window's document.
    pub(super) fn window_document_command(
        &mut self,
        command: &AppCommandId,
        window: Option<iced::window::Id>,
    ) -> Option<Task<Message>> {
        if !is_window_document_command(command) {
            return None;
        }
        Some(match self.document_of_window(window)? {
            WindowDocument::Tab(index) if self.tab_takes_window_command(index, command) => {
                let window = window?;
                self.run_in_window_document(window, |app| {
                    app.dispatch_command(command, super::CommandArgs::none(), None)
                })
            }
            WindowDocument::Tab(index) => {
                self.explain_window_refusal(index, command);
                Task::none()
            }
            WindowDocument::NoTab => {
                crate::diagnostics::log_info(
                    "This window's document is no longer open, so the command did nothing.",
                );
                Task::none()
            }
        })
    }

    /// Say why a window-document command did nothing instead of leaving the
    /// keystroke silent: a Component Preview has no file for Save, Save As or
    /// Print, and a single-document window has no tab to step to.
    fn explain_window_refusal(&self, index: usize, command: &AppCommandId) {
        let is_preview = self
            .document_state
            .tabs
            .get(index)
            .is_some_and(|tab| tab.kind.is_component_editor());
        if is_tab_stepper(command) {
            crate::diagnostics::log_info(
                "This window shows one document, so there is no tab to step to; \
                 use the main window's tab strip.",
            );
        } else if is_preview {
            crate::diagnostics::log_info(
                "A Component Preview has no file of its own: Save, Save As and Print \
                 do nothing here. Save the component from the library table.",
            );
        }
    }

    /// Whether the tab at `index` is one `command` should run against: not a
    /// tab stepper (a window holds one document), and not a Component
    /// Preview, which has no file of its own for Save, Save As or Print to
    /// write (the main window's Save there would write the project instead).
    fn tab_takes_window_command(&self, index: usize, command: &AppCommandId) -> bool {
        !is_tab_stepper(command)
            && self
                .document_state
                .tabs
                .get(index)
                .is_some_and(|tab| !tab.kind.is_component_editor())
    }

    /// The undocked window showing the schematic at `path`. Save As and a
    /// deferred Save finish asynchronously, when the main window's active
    /// tab is another document; they find their window again by path.
    pub(crate) fn undocked_schematic_window(
        &self,
        path: &std::path::Path,
    ) -> Option<iced::window::Id> {
        let window = self.ui_state.windows.iter().find_map(|(id, kind)| {
            matches!(kind, WindowKind::UndockedTab { path: p, .. } if p == path).then_some(*id)
        })?;
        self.document_state
            .tabs
            .iter()
            .any(|t| t.path == path && matches!(t.kind, crate::app::TabKind::Schematic))
            .then_some(window)
    }

    /// [`Self::run_in_window`] for a command that reads the active tab and
    /// path (Save, Save As, Print): the one retarget-and-restore rule, with
    /// its unwind guard, then the panels and History show the main window's
    /// document again at once.
    pub(crate) fn run_in_window_document(
        &mut self,
        window: iced::window::Id,
        handler: impl FnOnce(&mut Self) -> Task<Message>,
    ) -> Task<Message> {
        let task = self.run_in_window(window, true, handler);
        self.refresh_panel_ctx();
        task
    }
}

#[cfg(test)]
#[path = "window_document_tests.rs"]
mod tests;
