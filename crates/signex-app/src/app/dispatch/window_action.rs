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
            | LibraryMessage::ComponentsPanelRowRightClicked(_),
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

/// Whether the message a command resolved to opens, activates or steps to
/// another sheet, or clears every canvas's caches: it runs on the main
/// window, never inside a window's swap (which would show the other sheet in
/// the window's canvas while the window keeps its own path).
fn command_stays_on_main(message: &Message) -> bool {
    use crate::app::{NetNavMsg, UiMsg};
    use crate::menu_bar::MenuMessage;
    matches!(
        message,
        Message::Menu(
            MenuMessage::AscendToParentSheet
                | MenuMessage::NextSheet
                | MenuMessage::PreviousSheet
                | MenuMessage::NewProject
                | MenuMessage::OpenProject
                | MenuMessage::OpenRecentProject(_)
                | MenuMessage::OpenSampleProject
        ) | Message::Ui(
            UiMsg::GridToggle
                | UiMsg::NetNav(
                    NetNavMsg::PartnerJumpRequested
                        | NetNavMsg::SheetBackRequested
                        | NetNavMsg::SheetForwardRequested
                        | NetNavMsg::HighlightStepRequested { .. }
                )
        )
    )
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
            || matches!(&message, Message::Dock(dock) if !dock_message_edits_shown_sheet(dock))
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
            crate::diagnostics::log_warning(
                "An action for a window that has closed was dropped; repeat it in the window that shows the sheet.",
            );
            return Task::none();
        }
        if command_stays_on_main(&message) {
            return self.update(message);
        }
        let task = self.run_in_window_document(window, |app| app.update(message));
        task.map(move |followup| tag_followup(window, followup))
    }

    /// Close what was opened in `window`, which is gone: a Find, a Move
    /// Selection, a menu or a Components drag stamped with its id would
    /// otherwise stay open where nothing paints them.
    pub(crate) fn close_surfaces_of_window(&mut self, window: iced::window::Id) {
        self.retire_window_panel_ctx(window);
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
