use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_active_bar_action(
        &mut self,
        action: crate::active_bar::ActiveBarAction,
    ) -> Task<Message> {
        use crate::active_bar::ActiveBarAction;

        self.interaction_state.active_bar_menu = None;
        self.remember_active_bar_group(&action);

        match action {
            ActiveBarAction::ToolSelect => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)))
            }
            ActiveBarAction::DrawWire => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)))
            }
            ActiveBarAction::DrawBus => self.update(Message::Tool(ToolMessage::SelectTool(Tool::Bus))),
            ActiveBarAction::PlaceNetLabel => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Label)))
            }
            ActiveBarAction::PlaceComponent => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Component)))
            }
            ActiveBarAction::PlaceTextString => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Text)))
            }
            ActiveBarAction::DrawLine => self.update(Message::Tool(ToolMessage::SelectTool(Tool::Line))),
            ActiveBarAction::DrawRectangle => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Rectangle)))
            }
            ActiveBarAction::DrawFullCircle => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Circle)))
            }
            ActiveBarAction::RotateSelection => self.update(Message::RotateSelected),
            ActiveBarAction::FlipSelectedX => self.update(Message::MirrorSelectedX),
            ActiveBarAction::FlipSelectedY => self.update(Message::MirrorSelectedY),
            ActiveBarAction::AlignLeft
            | ActiveBarAction::AlignRight
            | ActiveBarAction::AlignTop
            | ActiveBarAction::AlignBottom
            | ActiveBarAction::AlignHorizontalCenters
            | ActiveBarAction::AlignVerticalCenters
            | ActiveBarAction::DistributeHorizontally
            | ActiveBarAction::DistributeVertically
            | ActiveBarAction::AlignToGrid => {
                self.align_selected(&action);
                Task::none()
            }
            ActiveBarAction::SelectAll => self.update(Message::Selection(
                selection_request::SelectionRequest::SelectAll,
            )),
            _ => self.handle_active_bar_placement_preset(action),
        }
    }
}