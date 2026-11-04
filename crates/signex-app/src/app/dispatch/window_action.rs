//! Panels and commands of an undocked document window act on the sheet that
//! window shows (#1966).
//!
//! It builds on the #1910 mechanism: a window's messages are wrapped
//! (`DockMessage::ScopedToWindow`, like `ActiveBarMsg::ScopedToWindow`) and run
//! through `run_in_window_document`, which swaps the window's canvas, sheet and
//! panel context in for the duration of the handler.

use iced::Task;

use crate::app::state::WindowKind;
use crate::app::{Message, Signex};

/// Whether a dock message edits the sheet an undocked window shows, so it
/// runs in the window's swap. This is an allow-list: everything else (the
/// dock layout, Projects, Navigator, ERC focus and quick fixes, History
/// restore, library rows, anything that opens or activates a document, and
/// any variant added later) runs on the main window, where the swap cannot
/// load one sheet into another's canvas or write one sheet's history over
/// another sheet. The Components row press is scoped to stamp its window.
fn dock_message_edits_shown_sheet(message: &crate::dock::DockMessage) -> bool {
    use crate::dock::DockMessage;
    use crate::library::LibraryMessage;
    use crate::panels::PanelMsg as P;
    use signex_widgets::tree_view::TreeMsg;
    match message {
        DockMessage::Library(
            LibraryMessage::ComponentsPanelRowPressed(_)
            | LibraryMessage::ComponentsPanelRowRightClicked(_)
            | LibraryMessage::ComponentsPanelRowMenuPicked(_),
        ) => true,
        // Expanding a row only changes the window's own tree.
        DockMessage::Panel(P::Tree(TreeMsg::Toggle(_))) => true,
        DockMessage::Panel(panel) => matches!(
            panel,
            P::PropertiesTab(_)
                | P::ToggleSection(_)
                | P::EditSymbolDesignator(..)
                | P::EditSymbolValue(..)
                | P::EditSymbolFootprint(..)
                | P::SwitchSymbolChoice(..)
                | P::ToggleSymbolMirrorX(_)
                | P::ToggleSymbolMirrorY(_)
                | P::ToggleSymbolLocked(_)
                | P::ToggleItemLocked(..)
                | P::ToggleSymbolDnp(_)
                | P::SymbolBodyStylePicked(..)
                | P::EditSymbolRotation(..)
                | P::EditSymbolValueFontSizePt(..)
                | P::EditPowerPortStyle { .. }
                | P::EditLabelText(..)
                | P::EditLabelJustifyH(..)
                | P::EditLabelDirection(..)
                | P::EditLabelRotation(..)
                | P::EditLabelFontSizePt(..)
                | P::EditTextNoteText(..)
                | P::EditTextFontFamily(..)
                | P::SetDocumentFontFamily(_)
                | P::SetPageFormatMode(_)
                | P::SetPaperSize(_)
                | P::SetSheetOrientation(_)
                | P::SetPageOrigin(_)
                | P::SetSheetColor(_)
                | P::EditSheetFrame(..)
                | P::CustomPaperEdgeTyping(..)
                | P::CommitCustomPaperEdge(..)
                | P::TitleBlockTyping(..)
                | P::CommitTitleBlockField(..)
                | P::NumericFieldTyped(..)
                | P::NumericFieldSubmitted(..)
                | P::UpdateDrawingEdit(..)
                | P::DrawingFieldTyping(..)
                | P::MultiEdit(..)
        ),
        _ => false,
    }
}

/// Whether the message a command resolved to edits the sheet an undocked
/// window shows, so it runs in the window's swap. An allow-list: sheet edits,
/// selection, tools, in-place text edits, the Active Bar and the view zoom.
/// Everything else (sheet stepping, project opens, net jumps, Find with
/// Project scope, Annotate, ERC, export, app-wide toggles, and any message
/// added later) runs on the main window, where it cannot load one sheet into
/// another's canvas.
fn command_edits_shown_sheet(message: &Message) -> bool {
    use crate::menu_bar::MenuMessage;
    matches!(
        message,
        Message::Edit(_)
            | Message::Selection(_)
            | Message::Tool(_)
            | Message::TextEdit(_)
            | Message::ActiveBar(_)
            | Message::Menu(MenuMessage::ZoomIn | MenuMessage::ZoomOut)
    )
}

/// Tag a pick from the Components row menu of the undocked `window` so it
/// acts on that window's sheet; anything else passes through.
pub(in crate::app) fn scope_row_menu_message(
    window: iced::window::Id,
    message: Message,
) -> Message {
    use crate::library::LibraryMessage;
    match message {
        Message::Library(inner @ LibraryMessage::ComponentsPanelRowMenuPicked(_)) => {
            Message::Dock(crate::dock::DockMessage::ScopedToWindow(
                window,
                Box::new(crate::dock::DockMessage::Library(inner)),
            ))
        }
        other => other,
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

    /// `message` as the dock of `window` sends it: wrapped when it edits the
    /// sheet that undocked window shows, unchanged otherwise (and for the
    /// main window).
    pub(crate) fn scope_dock_message(&self, window: iced::window::Id, message: Message) -> Message {
        match message {
            Message::Dock(dock)
                if self.is_document_window(window) && dock_message_edits_shown_sheet(&dock) =>
            {
                Message::Dock(crate::dock::DockMessage::ScopedToWindow(
                    window,
                    Box::new(dock),
                ))
            }
            other => other,
        }
    }

    /// Run a dock message against the sheet `window` shows. A window that has
    /// closed since the click drops the action rather than let it fall
    /// through to the main window.
    pub(crate) fn handle_dock_message_in_window(
        &mut self,
        window: iced::window::Id,
        message: crate::dock::DockMessage,
    ) -> Task<Message> {
        if !self.is_document_window(window) {
            crate::diagnostics::log_warning(
                "A panel action for a window that has closed was dropped; repeat it in the window that shows the sheet.",
            );
            return Task::none();
        }
        use crate::dock::DockMessage;
        use crate::library::LibraryMessage;
        let presses_row = matches!(
            message,
            DockMessage::Library(LibraryMessage::ComponentsPanelRowPressed(_))
        );
        let opens_row_menu = matches!(
            message,
            DockMessage::Library(LibraryMessage::ComponentsPanelRowRightClicked(_))
        );
        let task = self.run_in_window_document(window, |app| app.handle_dock_message(message));
        // The row's drag and menu are owned by the window they started in
        // (#1910's `overlay_owner`), recorded only once the action ran.
        let owner = self.overlay_owner(window);
        if presses_row && let Some(drag) = self.library.components_panel.drag.as_mut() {
            drag.window = owner;
        }
        if opens_row_menu && let Some(menu) = self.library.components_panel.row_menu.as_mut() {
            menu.window = owner;
        }
        task
    }

    /// Run the message a command resolved to against the sheet `window`
    /// shows when it edits that sheet; anything else runs on the main window.
    pub(crate) fn run_command_in_window(
        &mut self,
        window: iced::window::Id,
        message: Message,
    ) -> Task<Message> {
        if !command_edits_shown_sheet(&message) {
            return self.dispatch_update(message);
        }
        self.run_in_window_document(window, |app| app.dispatch_update(message))
    }
}

#[cfg(test)]
#[path = "window_action_tests.rs"]
mod tests;
