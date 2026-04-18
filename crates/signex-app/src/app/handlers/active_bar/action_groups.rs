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
            ActiveBarAction::DrawBus => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Bus)))
            }
            ActiveBarAction::PlaceNetLabel => {
                // Switch to the Label tool, clear any pending port state
                // (so this is a plain Net label not a Global/Hier port),
                // and arm a ghost preview that follows the cursor.
                self.interaction_state.current_tool = Tool::Label;
                self.interaction_state.pending_port = None;
                self.interaction_state.canvas.ghost_label = Some(signex_types::schematic::Label {
                    uuid: uuid::Uuid::new_v4(),
                    text: "NET".to_string(),
                    position: signex_types::schematic::Point::new(0.0, 0.0),
                    rotation: 0.0,
                    label_type: signex_types::schematic::LabelType::Net,
                    shape: String::new(),
                    font_size: 1.8,
                    justify: signex_types::schematic::HAlign::Left,
                });
                Task::none()
            }
            ActiveBarAction::PlaceComponent => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Component)))
            }
            ActiveBarAction::PlaceTextString => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Text)))
            }
            ActiveBarAction::DrawLine => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Line)))
            }
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
            // "Select → Connection": if a wire/bus/junction/label is already
            // selected, expand immediately. Otherwise fall through to Select
            // tool so the next click triggers the net walk via the hit-test
            // dispatched with SelectConnected.
            ActiveBarAction::SelectConnection => {
                use signex_types::schematic::SelectedKind;
                let has_net_seed = self
                    .interaction_state
                    .canvas
                    .selected
                    .iter()
                    .any(|i| matches!(i.kind,
                        SelectedKind::Wire | SelectedKind::Bus
                        | SelectedKind::Junction | SelectedKind::Label));
                if has_net_seed {
                    if let (Some(snapshot), Some(seed)) = (
                        self.active_render_snapshot(),
                        self.interaction_state.canvas.selected.first().cloned(),
                    ) {
                        let (wx, wy) = match seed.kind {
                            SelectedKind::Wire => snapshot
                                .wires
                                .iter()
                                .find(|w| w.uuid == seed.uuid)
                                .map(|w| (w.start.x, w.start.y))
                                .unwrap_or((0.0, 0.0)),
                            SelectedKind::Bus => snapshot
                                .buses
                                .iter()
                                .find(|b| b.uuid == seed.uuid)
                                .map(|b| (b.start.x, b.start.y))
                                .unwrap_or((0.0, 0.0)),
                            SelectedKind::Junction => snapshot
                                .junctions
                                .iter()
                                .find(|j| j.uuid == seed.uuid)
                                .map(|j| (j.position.x, j.position.y))
                                .unwrap_or((0.0, 0.0)),
                            SelectedKind::Label => snapshot
                                .labels
                                .iter()
                                .find(|l| l.uuid == seed.uuid)
                                .map(|l| (l.position.x, l.position.y))
                                .unwrap_or((0.0, 0.0)),
                            _ => (0.0, 0.0),
                        };
                        self.update(Message::Selection(
                            selection_request::SelectionRequest::SelectConnected {
                                world_x: wx,
                                world_y: wy,
                            },
                        ))
                    } else {
                        Task::none()
                    }
                } else {
                    self.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)))
                }
            }
            // Other select-mode variants currently enter the normal Select
            // tool; distinct box/lasso modes land with a later selection
            // rewrite.
            ActiveBarAction::LassoSelect
            | ActiveBarAction::InsideArea
            | ActiveBarAction::OutsideArea
            | ActiveBarAction::TouchingRectangle
            | ActiveBarAction::TouchingLine
            | ActiveBarAction::ToggleSelection => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)))
            }
            // "Drag" / "Drag Selection" — arm the next canvas click so it
            // begins a move-drag whether or not it lands on an already-
            // selected item. Requires a prior selection.
            ActiveBarAction::Drag | ActiveBarAction::DragSelection => {
                self.update(Message::Selection(
                    selection_request::SelectionRequest::ArmDrag,
                ))
            }
            // Other move / z-order variants not yet implemented — fall
            // through to Select tool so at least the user can move with the
            // mouse.
            ActiveBarAction::MoveSelection
            | ActiveBarAction::MoveSelectionXY
            | ActiveBarAction::MoveToFront
            | ActiveBarAction::BringToFront
            | ActiveBarAction::SendToBack
            | ActiveBarAction::BringToFrontOf
            | ActiveBarAction::SendToBackOf => {
                crate::diagnostics::log_info(
                    "Move / z-order variants are deferred — using plain Select for now",
                );
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
