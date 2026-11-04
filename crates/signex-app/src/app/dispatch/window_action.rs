//! Actions started in an undocked document window act on that window's
//! document, not the main window's active tab (#1910, #1966).
//!
//! The view tags a window's messages with [`Signex::scope_to_window`]; the
//! router runs them through [`Signex::run_action_in_window`], which swaps the
//! window's canvas and document in for the duration of the handler.

use iced::Task;

use crate::app::state::WindowKind;
use crate::app::{Message, Signex, WindowMsg};

/// A message a scoped handler returned as a follow-up `Task`. One that edits
/// or reads the document keeps the window it came from; the rest (opening a
/// library tab, file results) run on the main window as they always did.
/// Switching or opening tabs inside the window swap would load the wrong
/// canvas, so those are never tagged.
pub(super) fn tag_followup(window: iced::window::Id, message: Message) -> Message {
    let acts_on_document = matches!(
        message,
        Message::ActiveBar(_)
            | Message::ContextMenu(_)
            | Message::Tool(_)
            | Message::TextEdit(_)
            | Message::Edit(_)
            | Message::Selection(_)
            | Message::MoveSelection(_)
            | Message::FindReplaceMsg(_)
    );
    if !acts_on_document {
        return message;
    }
    Message::Window(WindowMsg::ActionTakenInWindow {
        window,
        message: Box::new(message),
    })
}

/// A dock message that is not run against the window's sheet: the dock layout
/// is shared, and anything that opens, selects or closes a document would load
/// it into the window's swapped canvas. Those run on the main window. Panel
/// edits of the sheet, and the Components row press that stamps its window,
/// are scoped.
fn dock_message_stays_on_main(message: &crate::dock::DockMessage) -> bool {
    use crate::dock::DockMessage;
    use crate::library::LibraryMessage;
    use crate::panels::PanelMsg;
    use signex_widgets::tree_view::TreeMsg;
    match message {
        // Expanding a row only changes the window's own tree.
        DockMessage::Panel(PanelMsg::Tree(TreeMsg::Toggle(_))) => false,
        DockMessage::Panel(
            PanelMsg::Tree(_) | PanelMsg::Navigator(_) | PanelMsg::PlacedPartAction(_),
        ) => true,
        DockMessage::Panel(_) => false,
        DockMessage::Library(
            LibraryMessage::ComponentsPanelRowPressed(_)
            | LibraryMessage::ComponentsPanelRowRightClicked(_),
        ) => false,
        _ => true,
    }
}

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
            || matches!(&message, Message::Dock(dock) if dock_message_stays_on_main(dock))
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
        let task = self.run_in_window_document(window, |app| app.update(message));
        task.map(move |followup| tag_followup(window, followup))
    }

    /// Close what was opened in `window`, which is gone: a Find, a Move
    /// Selection, a menu or a Components drag stamped with its id would
    /// otherwise stay open where nothing paints them.
    pub(crate) fn close_surfaces_of_window(&mut self, window: iced::window::Id) {
        self.document_state.window_panel_ctx.remove(&window);
        let stamped = Some(window);
        if self.ui_state.find_replace.window == stamped {
            self.ui_state.find_replace.open = false;
            self.ui_state.find_replace.window = None;
        }
        if self.ui_state.move_selection.window == stamped {
            self.ui_state.move_selection.open = false;
            self.ui_state.move_selection.window = None;
        }
        if self
            .library
            .components_panel
            .drag
            .as_ref()
            .is_some_and(|d| d.window == stamped)
        {
            self.library.components_panel.drag = None;
        }
        if self.interaction_state.menu_window == stamped {
            let interaction = &mut self.interaction_state;
            interaction.menu_window = None;
            interaction.active_bar_menu = None;
            interaction.context_menu = None;
            interaction.project_tree_context_menu = None;
            interaction.tab_context_menu = None;
            self.library.components_panel.row_menu = None;
        }
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
