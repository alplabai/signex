//! The routing decisions [`Signex::dispatch_command_reporting`] takes around
//! the bridge, as functions of the id, the message and the window's facts.
//! Dispatch calls them on live state; the parity table (`parity`, #2504)
//! calls the same functions to list the forms each id reaches.

use crate::app::document::{DocumentKey, UndoTarget};
use crate::app::{ErcMsg, Message, PcbMsg, Signex};
use crate::keymap::AppCommandId;

/// Undo and Redo: the ids the history routers take.
pub(super) fn is_history_step(command: &AppCommandId) -> bool {
    matches!(command.as_str(), "undo" | "redo")
}

/// Undo / Redo typed in an undocked sheet window run inside that window
/// when the window's history is one the swap reaches: not none and not a
/// board's (#1546).
pub(super) fn steps_through_window_swap(target: &UndoTarget) -> bool {
    !matches!(
        target,
        UndoTarget::None | UndoTarget::Document(DocumentKey::Pcb(_))
    )
}

/// Undo / Redo typed in a window that shows a document of its own step that
/// document directly (#1854); a sheet goes through the swap, the ERC rules
/// through the dialog's message.
pub(super) fn steps_own_document(target: &UndoTarget) -> bool {
    !matches!(
        target,
        UndoTarget::ProjectRules(_) | UndoTarget::Document(DocumentKey::Schematic(_))
    )
}

/// What Undo / Redo resolve to while the ERC dialog takes the input: the
/// project's rule history, never the sheet.
pub(super) fn erc_dialog_step(command: &AppCommandId) -> Option<Message> {
    match command.as_str() {
        "undo" => Some(Message::Erc(ErcMsg::RulesUndone)),
        "redo" => Some(Message::Erc(ErcMsg::RulesRedone)),
        _ => None,
    }
}

/// Close Document follows the window the stroke came from (#1787).
pub(super) fn closes_in_its_window(command: &AppCommandId) -> bool {
    command.as_str() == "close_active_document"
}

/// What the window a command was typed in shows, as the in-window rewrites
/// read it.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct WindowFacts {
    /// The window has a canvas of its own.
    pub has_canvas: bool,
    /// The window is an undocked PCB window.
    pub shows_undocked_board: bool,
    /// The window is an undocked document window.
    pub is_document_window: bool,
    /// No window, or the main one.
    pub is_main: bool,
}

/// Where a resolved message runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InWindowRoute {
    /// A canvas event goes to the canvas of the window it was typed in (#525).
    CanvasInWindow(iced::window::Id),
    /// A board-view command acts on the board the window shows (#2223).
    BoardViewInWindow(iced::window::Id),
    /// A view turn turns that window's board (#2316).
    BoardTurnInWindow(iced::window::Id),
    /// Any other command acts on the sheet the window shows (#1966).
    DocumentWindow(iced::window::Id),
    /// Find Next / Previous from main take over a Sheet-scope Find owned by
    /// an undocked window.
    FindFromMain,
    /// The message runs as sent.
    Plain,
}

/// Where `message`, typed in `window`, runs.
pub(super) fn in_window_route(
    message: &Message,
    window: Option<iced::window::Id>,
    facts: WindowFacts,
) -> InWindowRoute {
    match (message, window) {
        (Message::CanvasEvent(_), Some(id)) if facts.has_canvas => {
            InWindowRoute::CanvasInWindow(id)
        }
        (Message::Pcb(PcbMsg::View(_)), Some(id)) if facts.shows_undocked_board => {
            InWindowRoute::BoardViewInWindow(id)
        }
        (Message::Pcb(PcbMsg::ViewRotation(_)), Some(id)) if facts.shows_undocked_board => {
            InWindowRoute::BoardTurnInWindow(id)
        }
        (_, Some(id)) if facts.is_document_window => InWindowRoute::DocumentWindow(id),
        (message, _) if is_find_step(message) && facts.is_main => InWindowRoute::FindFromMain,
        _ => InWindowRoute::Plain,
    }
}

/// F3 / Shift+F3 resolved to a Find step.
fn is_find_step(message: &Message) -> bool {
    use crate::find_replace::FindReplaceMsg::{NextRequested, PreviousRequested};
    matches!(
        message,
        Message::FindReplaceMsg(NextRequested | PreviousRequested)
    )
}

impl Signex {
    /// The [`WindowFacts`] of `window` (`None`: the main window).
    pub(super) fn window_facts(&self, window: Option<iced::window::Id>) -> WindowFacts {
        WindowFacts {
            has_canvas: window.is_some_and(|id| self.interaction_state.canvases.contains_key(&id)),
            shows_undocked_board: window.is_some_and(|id| self.undocked_pcb_path(id).is_some()),
            is_document_window: window.is_some_and(|id| self.is_document_window(id)),
            is_main: window.is_none_or(|id| self.ui_state.main_window_id == Some(id)),
        }
    }
}
