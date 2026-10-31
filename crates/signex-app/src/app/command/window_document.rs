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
    /// A detached modal or panel: hosts no document.
    NotADocumentWindow,
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

impl Signex {
    /// The document the window `window` shows, or `None` when it is the main
    /// window (or names none) and the ordinary handlers apply.
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
                    None => {
                        tracing::warn!(
                            target: "signex::tabs",
                            path = %path.display(),
                            "undocked tab window matches no open tab, command ignored",
                        );
                        WindowDocument::NoTab
                    }
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
            WindowKind::DetachedModal(_) | WindowKind::DetachedPanel(_) => {
                Some(WindowDocument::NotADocumentWindow)
            }
        }
    }

    /// Close Document typed in `window`, or `None` when `window` is the main
    /// window (or names none) and the ordinary handler applies.
    ///
    /// - Undocked tab or Component Preview: close that window's own tab
    ///   through `close_tab_parking_dirty` (dirty drafts are parked), which
    ///   also closes the window. A Component Preview whose tab is gone is
    ///   closed as the OS close button does (`WindowCloseRequested`).
    /// - Detached modal or panel: hosts no document, so nothing happens.
    pub(super) fn close_document_in_window(
        &mut self,
        window: Option<iced::window::Id>,
    ) -> Option<Task<Message>> {
        let id = window?;
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
            WindowDocument::NoTab | WindowDocument::NotADocumentWindow => Task::none(),
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
    /// - A Component Preview, a window with no tab, or a detached modal or
    ///   panel does nothing: none has a file for these commands to write.
    pub(super) fn window_document_command(
        &mut self,
        command: &AppCommandId,
        window: Option<iced::window::Id>,
    ) -> Option<Task<Message>> {
        if !is_window_document_command(command) {
            return None;
        }
        Some(match self.document_of_window(window)? {
            WindowDocument::Tab(index) if self.tab_takes_window_command(index, command) => self
                .with_tab_active(index, |app| {
                    app.dispatch_command(command, super::CommandArgs::none(), None)
                })
                .unwrap_or_else(Task::none),
            _ => Task::none(),
        })
    }

    /// Whether the tab at `index` is one `command` should run against: not a
    /// tab stepper (a window holds one document), and not a Component
    /// Preview, which has no file of its own for Save, Save As or Print to
    /// write (the main window's Save there would write the project instead).
    fn tab_takes_window_command(&self, index: usize, command: &AppCommandId) -> bool {
        !matches!(
            command.as_str(),
            "next_document_tab" | "previous_document_tab"
        ) && self
            .document_state
            .tabs
            .get(index)
            .is_some_and(|tab| !tab.kind.is_component_editor())
    }

    /// The index of the schematic tab at `path` when an undocked window
    /// shows it. Save As needs it twice: its dialog and its completion are
    /// asynchronous, and by then the main window's active tab is another
    /// document, which only a retarget to this one can write correctly.
    pub(crate) fn undocked_schematic_tab(&self, path: &std::path::Path) -> Option<usize> {
        let shown = self
            .ui_state
            .windows
            .values()
            .any(|kind| matches!(kind, WindowKind::UndockedTab { path: p, .. } if p == path));
        if !shown {
            return None;
        }
        self.document_state
            .tabs
            .iter()
            .position(|t| t.path == path && matches!(t.kind, crate::app::TabKind::Schematic))
    }

    /// Run `handler` with `document_state.tabs[index]` as the active tab and
    /// its path as the active path, then restore the main window's choice.
    /// The tab is found again by path, so a handler that closes or reorders
    /// tabs does not leave the main window pointing at another document; a
    /// handler that renames the tab (Save As) moves the main window's
    /// choice with it when it was that same tab.
    pub(crate) fn with_tab_active<R>(
        &mut self,
        index: usize,
        handler: impl FnOnce(&mut Self) -> R,
    ) -> Option<R> {
        let document = &mut self.document_state;
        let target = document.tabs.get(index).map(|t| t.path.clone())?;
        let saved_tab = document.active_tab;
        let saved_tab_path = document.tabs.get(saved_tab).map(|t| t.path.clone());
        let saved_path = document.active_path.replace(target.clone());
        document.active_tab = index;
        let result = handler(self);
        let document = &mut self.document_state;
        let same_document = saved_tab_path.as_ref() == Some(&target);
        document.active_tab = if same_document && index < document.tabs.len() {
            index
        } else {
            saved_tab_path
                .and_then(|path| document.tabs.iter().position(|t| t.path == path))
                .unwrap_or_else(|| saved_tab.min(document.tabs.len().saturating_sub(1)))
        };
        document.active_path = match saved_path {
            Some(path) if path == target => document.tabs.get(index).map(|t| t.path.clone()),
            other => other,
        };
        Some(result)
    }
}

#[cfg(test)]
#[path = "window_document_tests.rs"]
mod tests;
