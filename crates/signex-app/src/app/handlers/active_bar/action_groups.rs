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
            ActiveBarAction::RotateSelection | ActiveBarAction::RotateSelectionCW => {
                self.update(Message::RotateSelected)
            }
            ActiveBarAction::FlipSelectedX => self.update(Message::MirrorSelectedX),
            ActiveBarAction::FlipSelectedY => self.update(Message::MirrorSelectedY),
            // Select-mode variants all currently enter the normal Select tool;
            // distinct box/lasso modes will land with a later selection rewrite.
            ActiveBarAction::LassoSelect
            | ActiveBarAction::InsideArea
            | ActiveBarAction::OutsideArea
            | ActiveBarAction::TouchingRectangle
            | ActiveBarAction::TouchingLine
            | ActiveBarAction::SelectConnection
            | ActiveBarAction::ToggleSelection => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)))
            }
            // Drag / Move actions — for now, simply switch to the Select tool so
            // the user can grab and move the current selection with the mouse.
            ActiveBarAction::Drag
            | ActiveBarAction::MoveSelection
            | ActiveBarAction::MoveSelectionXY
            | ActiveBarAction::DragSelection
            | ActiveBarAction::MoveToFront
            | ActiveBarAction::BringToFront
            | ActiveBarAction::SendToBack
            | ActiveBarAction::BringToFrontOf
            | ActiveBarAction::SendToBackOf => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)))
            }
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