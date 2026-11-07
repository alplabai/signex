//! Commands that act on a document, resolved against the window the stroke
//! (or menu click) came from.
//!
//! Strokes route globally and the handlers act on the main window's active
//! tab, so Ctrl+F4 typed in another window used to close a document the user
//! was not looking at (#1787), and Save, Save As, Print and the tab stepper
//! did the same (#1794). Those commands are per-window, like Undo / Redo.

use iced::Task;

use super::CommandOutcome;
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

const DETACHED_WINDOW_CLOSE_MESSAGE: &str =
    "Close Document does nothing in a detached panel or modal window.";

pub(super) const DOCUMENT_GONE_MESSAGE: &str =
    "This window's document is no longer open, so the command did nothing.";

/// Say why a command typed in a window whose tab is gone did nothing.
fn explain_document_gone() -> (CommandOutcome, Task<Message>) {
    crate::diagnostics::log_info(DOCUMENT_GONE_MESSAGE);
    (
        CommandOutcome::disabled(DOCUMENT_GONE_MESSAGE),
        Task::none(),
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
        // A detached panel or modal edits the main window's document, so its
        // strokes keep the main-window behaviour.
        let bound = self
            .document_state
            .tab_index_of_window_kind(self.ui_state.windows.get(&id)?)?;
        Some(match bound {
            Some(index) => WindowDocument::Tab(index),
            None => WindowDocument::NoTab,
        })
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
    ) -> Option<(CommandOutcome, Task<Message>)> {
        let id = window?;
        // Close Document in a detached panel or modal does nothing (#1792):
        // closing the main window's tab from a tool window is surprising.
        if matches!(
            self.ui_state.windows.get(&id),
            Some(WindowKind::DetachedModal(_) | WindowKind::DetachedPanel(_))
        ) && self.ui_state.main_window_id != Some(id)
        {
            return Some((
                CommandOutcome::disabled(DETACHED_WINDOW_CLOSE_MESSAGE),
                Task::none(),
            ));
        }
        Some(match self.document_of_window(window)? {
            WindowDocument::Tab(index) => {
                (CommandOutcome::Ran, self.close_tab_parking_dirty(index))
            }
            WindowDocument::NoTab
                if matches!(
                    self.ui_state.windows.get(&id),
                    Some(WindowKind::ComponentEditor { .. })
                ) =>
            {
                (
                    CommandOutcome::Ran,
                    self.dispatch_update(Message::Window(
                        crate::app::WindowMsg::WindowCloseRequested(id),
                    )),
                )
            }
            WindowDocument::NoTab => explain_document_gone(),
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
        args: &super::CommandArgs,
        window: Option<iced::window::Id>,
    ) -> Option<(CommandOutcome, Task<Message>)> {
        if !is_window_document_command(command) {
            return None;
        }
        Some(match self.document_of_window(window)? {
            WindowDocument::Tab(index) if self.tab_takes_window_command(index, command) => {
                let window = window?;
                let mut outcome = CommandOutcome::Ran;
                let task = self.run_in_window_document(window, |app| {
                    let (inner, task) = app.dispatch_command_reporting(command, args.clone(), None);
                    outcome = inner;
                    task
                });
                (outcome, task)
            }
            WindowDocument::Tab(index) => (
                CommandOutcome::disabled(self.explain_window_refusal(index, command)),
                Task::none(),
            ),
            WindowDocument::NoTab => explain_document_gone(),
        })
    }

    /// Say why a window-document command did nothing instead of leaving the
    /// keystroke silent: a Component Preview has no file for Save, Save As or
    /// Print, and a single-document window has no tab to step to.
    fn explain_window_refusal(&self, index: usize, command: &AppCommandId) -> &'static str {
        let is_preview = self
            .document_state
            .tabs
            .get(index)
            .is_some_and(|tab| tab.kind.is_component_editor());
        if is_tab_stepper(command) {
            let reason = "This window shows one document, so there is no tab to step to; \
                 use the main window's tab strip.";
            crate::diagnostics::log_info(reason);
            reason
        } else if is_preview {
            let reason = "A Component Preview has no file of its own: Save, Save As and Print \
                 do nothing here. Save the component from the library table.";
            crate::diagnostics::log_info(reason);
            reason
        } else {
            "The window's document does not take this command."
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

    /// The undocked window showing `path` that has a canvas to run a
    /// follow-up on. A window whose canvas is not there yet (or is gone)
    /// cannot take it, and `run_in_window` would drop it without a word.
    pub(crate) fn undocked_canvas_window(
        &self,
        path: &std::path::Path,
    ) -> Option<iced::window::Id> {
        self.undocked_schematic_window(path)
            .filter(|window| self.interaction_state.canvases.contains_key(window))
    }

    /// `window` when it is an undocked window, `None` for the main window or
    /// an unknown id: the owner an overlay records when it opens (#1910).
    pub(crate) fn overlay_owner(&self, window: iced::window::Id) -> Option<iced::window::Id> {
        (self.ui_state.main_window_id != Some(window)
            && self.ui_state.windows.contains_key(&window))
        .then_some(window)
    }

    /// Drop the Active Bar dropdown and context menu that `window` opened,
    /// when it closes or reattaches: nothing else would clear them (#1910).
    pub(crate) fn close_overlays_owned_by(&mut self, window: iced::window::Id) {
        self.retire_window_panel_ctx(window);
        self.close_surfaces_owned_by(window);
        if self
            .library
            .components_panel
            .drag
            .as_ref()
            .is_some_and(|d| d.window == Some(window))
        {
            self.library.components_panel.drag = None;
        }
        if self
            .library
            .components_panel
            .row_menu
            .as_ref()
            .is_some_and(|menu| menu.window == Some(window))
        {
            self.library.components_panel.row_menu = None;
        }
        let interaction = &mut self.interaction_state;
        if interaction.active_bar_menu_window == Some(window) {
            interaction.active_bar_menu = None;
            interaction.active_bar_menu_window = None;
        }
        if interaction
            .context_menu
            .as_ref()
            .is_some_and(|menu| menu.window == Some(window))
        {
            interaction.context_menu = None;
            interaction.context_submenu = None;
            interaction.pending_submenu = None;
            interaction.submenu_launcher_hovered = None;
            interaction.submenu_panel_hovered = false;
            interaction.submenu_unhovered_since = None;
        }
    }

    /// Whether an overlay opened by `owner` (see [`Self::overlay_owner`])
    /// paints in `window`: only the window it was opened in shows it.
    pub(crate) fn overlay_belongs_to(
        &self,
        owner: Option<iced::window::Id>,
        window: iced::window::Id,
    ) -> bool {
        owner == self.overlay_owner(window)
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
        let target = match self.ui_state.windows.get(&window) {
            Some(WindowKind::UndockedTab { path, .. }) => Some(path.clone()),
            _ => None,
        };
        let revision_of = |app: &Self| {
            target
                .as_ref()
                .and_then(|path| app.document_state.engines.get(path))
                .map(signex_engine::Engine::revision)
        };
        let before = revision_of(self);
        let task = self.run_in_window(window, true, handler);
        self.refresh_main_panel_ctx();
        // The main window may show the sheet the window just edited: its
        // selection panel must not keep, and later write back, the old value.
        if target.is_some()
            && self.document_state.active_path == target
            && revision_of(self) != before
        {
            self.update_selection_info();
        }
        self.refresh_window_panel_ctxs(Some(window));
        task
    }
}

#[cfg(test)]
#[path = "window_document_tests.rs"]
mod tests;
