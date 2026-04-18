use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_menu_editing_command(
        &mut self,
        msg: &MenuMessage,
    ) -> Option<Task<Message>> {
        match msg {
            MenuMessage::Undo => Some(self.update(Message::Undo)),
            MenuMessage::Redo => Some(self.update(Message::Redo)),
            MenuMessage::Cut => Some(self.update(Message::Cut)),
            MenuMessage::Copy => Some(self.update(Message::Copy)),
            MenuMessage::Paste => Some(self.update(Message::Paste)),
            MenuMessage::SmartPaste => Some(self.update(Message::SmartPaste)),
            MenuMessage::Delete => Some(self.update(Message::DeleteSelected)),
            MenuMessage::SelectAll => Some(self.update(Message::Selection(
                selection_request::SelectionRequest::SelectAll,
            ))),
            MenuMessage::Duplicate => Some(self.update(Message::Duplicate)),
            MenuMessage::Find => Some(self.update(Message::OpenFind)),
            MenuMessage::Replace => Some(self.update(Message::OpenReplace)),
            MenuMessage::Annotate => Some(self.update(Message::OpenAnnotateDialog)),
            MenuMessage::AnnotateReset => {
                Some(self.update(Message::OpenAnnotateResetConfirm))
            }
            MenuMessage::Erc => Some(self.update(Message::OpenErcDialog)),
            MenuMessage::ToggleAutoFocus => Some(self.update(Message::ToggleAutoFocus)),
            MenuMessage::GenerateBom => {
                crate::diagnostics::log_info("Generate BOM is v0.8 scope");
                Some(Task::none())
            }
            _ => None,
        }
    }
}
