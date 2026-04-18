use iced::Task;

use super::*;

mod document;
mod overlay;
mod routed;
mod text_edit;
mod tool;
mod ui;

impl Signex {
    pub(crate) fn dispatch_update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Menu(_) | Message::Tab(_) | Message::Dock(_) | Message::Selection(_) => {
                self.dispatch_routed_message(message)
            }
            Message::ThemeChanged(_)
            | Message::UnitCycled
            | Message::GridToggle
            | Message::CanvasEvent(_)
            | Message::DragStart(_)
            | Message::DragMove(_, _)
            | Message::WindowResized(_, _)
            | Message::DragEnd
            | Message::GridCycle
            | Message::StatusBar(_) => self.dispatch_ui_message(message),
            Message::TextEditChanged(_) | Message::TextEditSubmit => {
                self.dispatch_text_edit_message(message)
            }
            Message::PrePlacementTab
            | Message::ResumePlacement
            | Message::CycleDrawMode
            | Message::CancelDrawing
            | Message::Tool(_) => self.dispatch_tool_message(message),
            Message::FileOpened(_)
            | Message::DeleteSelected
            | Message::Undo
            | Message::Redo
            | Message::RotateSelected
            | Message::MirrorSelectedX
            | Message::MirrorSelectedY
            | Message::Cut
            | Message::Copy
            | Message::Paste
            | Message::SmartPaste
            | Message::Duplicate
            | Message::SaveFile
            | Message::SaveFileAs(_)
            | Message::SchematicLoaded(_) => self.dispatch_document_message(message),
            Message::TogglePanelList
            | Message::OpenPanel(_)
            | Message::OpenFind
            | Message::OpenReplace
            | Message::OpenPreferences
            | Message::ClosePreferences
            | Message::PreferencesNav(_)
            | Message::PreferencesMsg(_)
            | Message::FindReplaceMsg(_)
            | Message::ActiveBar(_)
            | Message::ShowContextMenu(_, _)
            | Message::CloseContextMenu
            | Message::ContextAction(_)
            | Message::CloseTabConfirm(_)
            | Message::RunErc
            | Message::Annotate(_)
            | Message::FocusAt { .. }
            | Message::ToggleAutoFocus => self.dispatch_overlay_message(message),
            Message::Noop => Task::none(),
        }
    }
}
