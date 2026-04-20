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
                // Dispatch SelectTool so tool_preview, previous ghosts, and
                // other transient state get cleaned up centrally. Then arm
                // the label-specific ghost + clear any Global/Hier pending.
                let task = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Label)));
                self.interaction_state.pending_port = None;
                self.interaction_state.canvas.ghost_label = Some(signex_types::schematic::Label {
                    uuid: uuid::Uuid::new_v4(),
                    text: "NET".to_string(),
                    position: signex_types::schematic::Point::new(0.0, 0.0),
                    rotation: 0.0,
                    label_type: signex_types::schematic::LabelType::Net,
                    shape: String::new(),
                    font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
                    justify: signex_types::schematic::HAlign::Left,
                });
                task
            }
            ActiveBarAction::PlaceComponent => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Component)))
            }
            ActiveBarAction::PlaceTextString => {
                // Single-point text. Altium semantics = point glyphs.
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Text)))
            }
            ActiveBarAction::PlaceNote | ActiveBarAction::PlaceTextFrame => {
                // Altium: Text Frame = drag-rect bounded text (wraps),
                // Note = Text Frame with a default fill / border.
                // KiCad stores these as `(text_box ...)` (Graphic::TextBox),
                // which is parsed/written today but not yet rendered or
                // placeable interactively — see MASTER_PLAN.md v0.7 entry.
                // For now we fall back to placing a plain TextNote so the
                // user still gets something visible, and log the limitation.
                let default_text = if matches!(action, ActiveBarAction::PlaceNote) {
                    "Note"
                } else {
                    "Text Frame"
                };
                crate::diagnostics::log_info(format!(
                    "{default_text}: bounded text box (text_box) lands in v0.7 — placing a plain text note for now",
                ));
                let task = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Text)));
                self.interaction_state.canvas.ghost_text =
                    Some(signex_types::schematic::TextNote {
                        uuid: uuid::Uuid::new_v4(),
                        text: default_text.to_string(),
                        position: signex_types::schematic::Point::new(0.0, 0.0),
                        rotation: 0.0,
                        font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
                        justify_h: signex_types::schematic::HAlign::Left,
                        justify_v: signex_types::schematic::VAlign::default(),
                    });
                task
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
                let has_net_seed = self.interaction_state.canvas.selected.iter().any(|i| {
                    matches!(
                        i.kind,
                        SelectedKind::Wire
                            | SelectedKind::Bus
                            | SelectedKind::Junction
                            | SelectedKind::Label
                    )
                });
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
            // Lasso arms a multi-click polygon selection in
            // `ui_state.lasso_polygon`. The canvas handler grows it on
            // each click and commits on double-click / first-vertex
            // click. Escape / right-click cancels.
            ActiveBarAction::LassoSelect => {
                // Cleanly drop every prior tool's armed state (wire
                // drawing, pending power / port / net-colour, ghost
                // previews, editing_text, pre_placement) before
                // arming the lasso, THEN set lasso_polygon. The
                // helper also nulls lasso_polygon, so ordering
                // matters — arm after clearing.
                self.clear_transient_schematic_tool_state();
                self.interaction_state.current_tool = Tool::Select;
                self.ui_state.lasso_polygon = Some(Vec::new());
                self.sync_lasso_polygon_to_canvas();
                crate::diagnostics::log_info(
                    "Lasso: click to anchor, move freely, click again to close (Enter / Esc shortcuts)",
                );
                Task::none()
            }
            // Other select-mode variants currently enter the normal Select
            // tool; distinct box modes are handled via SelectionMode
            // (Shift+S cycle). ToggleSelection inverts the selection.
            ActiveBarAction::InsideArea
            | ActiveBarAction::OutsideArea
            | ActiveBarAction::TouchingRectangle
            | ActiveBarAction::TouchingLine
            | ActiveBarAction::ToggleSelection => {
                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)))
            }
            // "Drag" / "Drag Selection" — arm the next canvas click so it
            // begins a move-drag whether or not it lands on an already-
            // selected item. Requires a prior selection.
            ActiveBarAction::Drag | ActiveBarAction::DragSelection => self.update(
                Message::Selection(selection_request::SelectionRequest::ArmDrag),
            ),
            // Z-order: reorder the selection within its type vector.
            // Schematic render order = file order, so Bring-To-Front pushes
            // items to the end of their Vec; Send-To-Back moves them to the
            // front.
            ActiveBarAction::BringToFront | ActiveBarAction::MoveToFront => {
                if !self.interaction_state.canvas.selected.is_empty() {
                    let items = self.interaction_state.canvas.selected.clone();
                    self.apply_engine_command(
                        signex_engine::Command::ReorderObjects {
                            items,
                            direction: signex_engine::ReorderDirection::ToFront,
                        },
                        false,
                        true,
                    );
                }
                Task::none()
            }
            ActiveBarAction::SendToBack => {
                if !self.interaction_state.canvas.selected.is_empty() {
                    let items = self.interaction_state.canvas.selected.clone();
                    self.apply_engine_command(
                        signex_engine::Command::ReorderObjects {
                            items,
                            direction: signex_engine::ReorderDirection::ToBack,
                        },
                        false,
                        true,
                    );
                }
                Task::none()
            }
            // Reference picker — next canvas click on a qualifying item
            // becomes the z-order anchor. See `handle_canvas_left_click`
            // for the pick site.
            ActiveBarAction::BringToFrontOf => {
                if !self.interaction_state.canvas.selected.is_empty() {
                    self.ui_state.reorder_picker =
                        Some(super::super::super::state::ReorderPicker::Above);
                    self.interaction_state.canvas.reorder_picker_armed = true;
                    self.interaction_state.canvas.clear_overlay_cache();
                    crate::diagnostics::log_info(
                        "Click a reference object to bring the selection above it (Esc to cancel)",
                    );
                }
                Task::none()
            }
            ActiveBarAction::SendToBackOf => {
                if !self.interaction_state.canvas.selected.is_empty() {
                    self.ui_state.reorder_picker =
                        Some(super::super::super::state::ReorderPicker::Below);
                    self.interaction_state.canvas.reorder_picker_armed = true;
                    self.interaction_state.canvas.clear_overlay_cache();
                    crate::diagnostics::log_info(
                        "Click a reference object to send the selection below it (Esc to cancel)",
                    );
                }
                Task::none()
            }
            // MoveSelection / MoveSelectionXY → open the Move
            // Selection dialog (numeric ΔX / ΔY apply to every
            // selected item through Command::MoveSelection).
            ActiveBarAction::MoveSelection | ActiveBarAction::MoveSelectionXY => {
                self.update(Message::OpenMoveSelectionDialog)
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
