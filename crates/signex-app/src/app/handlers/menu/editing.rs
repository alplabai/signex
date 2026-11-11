use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_menu_editing_command(
        &mut self,
        msg: &MenuMessage,
    ) -> Option<Task<Message>> {
        match msg {
            MenuMessage::BringDrawingsToFront => Some(self.update(Message::Edit(
                EditMsg::ArrangeDrawings(signex_engine::DrawingArrange::ToFront),
            ))),
            MenuMessage::SendDrawingsToBack => Some(self.update(Message::Edit(
                EditMsg::ArrangeDrawings(signex_engine::DrawingArrange::ToBack),
            ))),
            MenuMessage::BringDrawingsForward => Some(self.update(Message::Edit(
                EditMsg::ArrangeDrawings(signex_engine::DrawingArrange::Forward),
            ))),
            MenuMessage::SendDrawingsBackward => Some(self.update(Message::Edit(
                EditMsg::ArrangeDrawings(signex_engine::DrawingArrange::Backward),
            ))),
            MenuMessage::Annotate => Some(self.update(Message::Annotate(AnnotateMsg::OpenDialog))),
            MenuMessage::AnnotateReset => {
                Some(self.update(Message::Annotate(AnnotateMsg::OpenResetConfirm)))
            }
            // Reset Duplicate Designators — find references that appear on
            // more than one part across the project, reset just those to
            // `{prefix}?`. Other designators keep their current value. The
            // sheets are read and the unopened ones written on background
            // jobs (`handlers/erc/annotate_job.rs`), so the fix is project-wide.
            MenuMessage::AnnotateResetDuplicates => Some(self.handle_reset_duplicate_designators()),
            _ => None,
        }
    }
}
