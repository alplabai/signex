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

/// Whether a dock message runs in the document of the undocked window it
/// came from. Every panel message does unless [`PanelMsg::runs_in_window`]
/// names it app-wide; the dock's own layout never does. No wildcard: a new
/// variant must be classified to compile.
///
/// [`PanelMsg::runs_in_window`]: crate::panels::PanelMsg::runs_in_window
fn dock_message_runs_in_window(message: &crate::dock::DockMessage) -> bool {
    use crate::dock::DockMessage as D;
    use crate::library::LibraryMessage;
    match message {
        D::Panel(panel) => panel.runs_in_window(),
        D::Library(library) => matches!(
            library,
            LibraryMessage::ComponentsPanelRowPressed(_)
                | LibraryMessage::ComponentsPanelRowRightClicked(_)
                | LibraryMessage::ComponentsPanelRowMenuPicked(_)
        ),
        D::SelectTab(..)
        | D::ToggleCollapse(..)
        | D::ClosePanel(..)
        | D::UndockPanel(..)
        | D::TabDragStart(..)
        | D::TabClick(..)
        | D::TabHoverEnter(..)
        | D::TabHoverExit(..)
        | D::MoveFloating(..)
        | D::StartDragFloating(..)
        | D::FloatingDragEnd(..)
        | D::DockFloating(..)
        | D::DockFloatingTo(..)
        | D::TabContextMenuRequested(..)
        | D::TabMenuDismissed
        | D::TabMenuActionPicked(..)
        | D::ScopedToWindow(..) => false,
    }
}

/// Whether the message a command resolved to edits the sheet an undocked
/// window shows. An explicit allow-list of variants (never a whole enum):
/// sheet edits, selection, tools, in-place text edits, Active Bar actions,
/// the view zoom and opening Find or Find Similar (the dialog then works on
/// that window's sheet).
fn command_edits_shown_sheet(message: &Message) -> bool {
    use crate::active_bar::ActiveBarMsg;
    use crate::app::selection_request::SelectionRequest as Sel;
    use crate::app::{EditMsg, InputMsg, OverlayMsg, TextEditMsg, ToolMessage};
    use crate::library::{LibraryMessage, PlacedPartMsg};
    use crate::menu_bar::MenuMessage;
    match message {
        Message::Edit(edit) => matches!(
            edit,
            EditMsg::DeleteSelected
                | EditMsg::RotateSelectedClockwise
                | EditMsg::RotateSelectedCounterclockwise
                | EditMsg::MirrorSelectedX
                | EditMsg::MirrorSelectedY
                | EditMsg::FlipSelection
                | EditMsg::Copy
                | EditMsg::Cut
                | EditMsg::Paste
                | EditMsg::SmartPaste
                | EditMsg::Duplicate
                | EditMsg::RepeatLastItem
                | EditMsg::RubberStampCopy
                | EditMsg::PasteArrayRequested
                | EditMsg::ArrangeDrawings(_)
                | EditMsg::Extras(
                    crate::app::PlacementExtrasMsg::AutoplaceFieldsRequested
                        | crate::app::PlacementExtrasMsg::CopyAttributesRequested { .. }
                )
        ),
        Message::Selection(selection) => matches!(
            selection,
            Sel::SelectAll
                | Sel::ClearAll
                | Sel::StoreSlot { .. }
                | Sel::RecallSlot { .. }
                | Sel::SelectConnected { .. }
                | Sel::ConnectionItemAtCursorRequested
                | Sel::CycleMode
                | Sel::LassoCommit
                | Sel::FindSimilar(crate::find_similar::FindSimilarMsg::OpenRequested { .. })
        ),
        Message::Tool(tool) => matches!(
            tool,
            ToolMessage::SelectTool(_)
                | ToolMessage::PrePlacementTab
                | ToolMessage::ResumePlacement
                | ToolMessage::CycleDrawMode
                | ToolMessage::CornerFlipRequested
                | ToolMessage::DragWiringCycleRequested
                | ToolMessage::CancelDrawing
                | ToolMessage::RemoveLastVertex
                | ToolMessage::LastSegmentUndoRequested
        ),
        Message::ActiveBar(ActiveBarMsg::Action(_)) => true,
        // Placed-part actions act on the selected part of the shown sheet.
        Message::Library(LibraryMessage::PlacedPart(PlacedPartMsg::ActionRequested(_))) => true,
        // AutoFocus dims by the shown sheet's selection; Esc resets the tool;
        // Find opens bound to the window, so it searches the shown sheet.
        Message::Overlay(
            OverlayMsg::ToggleAutoFocus
            | OverlayMsg::OpenFind
            | OverlayMsg::OpenReplace
            | OverlayMsg::FindToggled,
        ) => true,
        Message::Input(InputMsg::EscapePressed { .. }) => true,
        Message::TextEdit(TextEditMsg::Requested(_)) => true,
        Message::Menu(menu) => matches!(menu, MenuMessage::ZoomIn | MenuMessage::ZoomOut),
        _ => false,
    }
}

/// Whether a command's message acts on the sheet or board an undocked
/// window shows, so it runs in the window's swap: it edits that sheet, reads
/// it, or edits that board. A canvas menu row picked in the window resolves
/// to one of these (#2267), as do the window's keys and menu bar. Everything
/// else (sheet stepping, project opens, net jumps, Find next / previous,
/// Annotate, ERC, export, app-wide toggles, and any message added later)
/// runs on the main window, where it cannot load one sheet into another's
/// canvas.
fn command_runs_in_shown_document(message: &Message) -> bool {
    command_edits_shown_sheet(message)
        || command_reads_shown_sheet(message)
        || command_edits_shown_board(message)
        || command_steps_shown_grid(message)
}

/// The grid-step commands: on a board they edit that board's own grid.
fn command_steps_shown_grid(message: &Message) -> bool {
    use crate::app::UiMsg;
    matches!(
        message,
        Message::Ui(
            UiMsg::GridCycle
                | UiMsg::GridCycleBackward
                | UiMsg::FastGridPicked(_)
                | UiMsg::FastGridCycled
        )
    )
}

/// Commands that start from what the shown sheet holds: its net at the menu
/// point or the selection, the selection's cross probe, the selected sheet
/// symbol's child sheet and its entry sync, the sheet a new sheet symbol
/// lands on, and the sheet's project for Number Sheets.
fn command_reads_shown_sheet(message: &Message) -> bool {
    use crate::app::{
        ContextAction, ContextMenuMsg, CrossProbeMsg, EditMsg, NetNavMsg, ProjectMsg,
        SheetNumberingMsg, SheetSyncMsg, UiMsg,
    };
    match message {
        Message::Ui(UiMsg::NetNav(nav)) => matches!(
            nav,
            NetNavMsg::NetHighlightAtRequested { .. } | NetNavMsg::NetHighlightRequested
        ),
        Message::CrossProbe(CrossProbeMsg::ProbeRequested) => true,
        Message::ContextMenu(ContextMenuMsg::Action(action)) => matches!(
            action,
            ContextAction::OpenChildSheet
                | ContextAction::CreateChildSheet
                | ContextAction::SyncSheetPins
        ),
        Message::Project(ProjectMsg::SheetNumbering(SheetNumberingMsg::Requested)) => true,
        Message::Edit(EditMsg::SheetSync(
            SheetSyncMsg::Requested | SheetSyncMsg::CreateSymbolRequested,
        )) => true,
        _ => false,
    }
}

/// The board menu's edits and net rows, on the board the window shows.
fn command_edits_shown_board(message: &Message) -> bool {
    use crate::app::{
        BoardNetMsg, BoardSketchMsg, BoardToolsMsg, FootprintSwapMsg, PartSwapMsg, PcbMsg,
        ReuseBlockMsg,
    };
    let Message::Pcb(pcb) = message else {
        return false;
    };
    match pcb {
        PcbMsg::Tools(tool) => matches!(
            tool,
            BoardToolsMsg::TeardropsAddRequested
                | BoardToolsMsg::GlossRequested
                | BoardToolsMsg::BoardShapeRequested
                | BoardToolsMsg::KeepoutRequested
                | BoardToolsMsg::ArrangeRequested
        ),
        PcbMsg::Net(net) => matches!(
            net,
            BoardNetMsg::SelectRequested(_) | BoardNetMsg::HighlightRequested(_)
        ),
        PcbMsg::PlaceToolPicked(_)
        | PcbMsg::LockRequested { .. }
        | PcbMsg::FootprintSwap(FootprintSwapMsg::Requested)
        | PcbMsg::PartSwap(PartSwapMsg::Requested(_))
        | PcbMsg::ReuseBlock(ReuseBlockMsg::SaveRequested)
        | PcbMsg::Sketch(BoardSketchMsg::ModeToggled) => true,
        _ => false,
    }
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
    /// The one guard for everything that changes which sheet is shown (a tab
    /// switch, a sheet open, a document read). Inside an undocked window's
    /// swap the window's canvas is in the main slot, so a switch would load
    /// another sheet into it while the window keeps its own path. Such a call
    /// is refused with a Messages line; a descent through a sheet symbol is
    /// queued to the main window instead (`ProjectMsg::DescendRequested`).
    pub(crate) fn sheet_switch_refused_in_window(&self, what: &str) -> bool {
        if self.interaction_state.acting_window.is_none() {
            return false;
        }
        crate::diagnostics::log_warning(format!(
            "{what} was not done from inside an undocked window; do it in the main window."
        ));
        true
    }

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

    /// `message` as the dock of `window` sends it: wrapped when it runs in
    /// the document that undocked window shows, unchanged otherwise (and for
    /// the main window).
    pub fn scope_dock_message(&self, window: iced::window::Id, message: Message) -> Message {
        match message {
            Message::Dock(dock)
                if self.is_document_window(window) && dock_message_runs_in_window(&dock) =>
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
        // The palette is opened for this window: its rows and the command
        // picked follow the window's sheet (#2037).
        if matches!(
            message,
            Message::CommandPalette(crate::app::CommandPaletteMsg::Open)
        ) {
            return self.dispatch_update(Message::CommandPalette(
                crate::app::CommandPaletteMsg::OpenInWindow(window),
            ));
        }
        if matches!(
            message,
            Message::FindReplaceMsg(
                crate::find_replace::FindReplaceMsg::NextRequested
                    | crate::find_replace::FindReplaceMsg::PreviousRequested
            )
        ) {
            return self.step_find_in_window(window, message);
        }
        if !command_runs_in_shown_document(&message) {
            return self.dispatch_update(message);
        }
        self.run_in_window_document(window, |app| app.dispatch_update(message))
    }
}

#[cfg(test)]
#[path = "window_action_tests/mod.rs"]
mod tests;
