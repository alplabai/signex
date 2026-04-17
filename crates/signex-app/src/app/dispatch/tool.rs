use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_tool_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PrePlacementTab => {
                if self.interaction_state.current_tool != Tool::Select
                    && self.interaction_state.current_tool != Tool::Measure
                {
                    let tool_name = format!("{}", self.interaction_state.current_tool);
                    let (default_label_text, default_designator) =
                        if self.interaction_state.current_tool == Tool::Component {
                            self.current_component_defaults()
                                .unwrap_or_else(|| ("NET".to_string(), String::new()))
                        } else {
                            ("NET".to_string(), String::new())
                        };
                    let label_text = self
                        .document_state
                        .panel_ctx
                        .pre_placement
                        .as_ref()
                        .map(|pp| pp.label_text.clone())
                        .filter(|text| !text.is_empty())
                        .unwrap_or(default_label_text);
                    let designator = self
                        .document_state
                        .panel_ctx
                        .pre_placement
                        .as_ref()
                        .map(|pp| pp.designator.clone())
                        .filter(|value| !value.is_empty())
                        .unwrap_or(default_designator);
                    self.document_state.panel_ctx.pre_placement = Some(crate::panels::PrePlacementData {
                        tool_name,
                        label_text,
                        designator,
                        rotation: 0.0,
                    });
                    self.document_state
                        .dock
                        .add_panel(PanelPosition::Right, crate::panels::PanelKind::Properties);
                }
                self.finish_update()
            }
            Message::CycleDrawMode => {
                self.interaction_state.draw_mode = self.interaction_state.draw_mode.next();
                self.interaction_state.canvas.draw_mode = self.interaction_state.draw_mode;
                self.finish_update()
            }
            Message::CancelDrawing => {
                if self.interaction_state.wire_drawing {
                    self.interaction_state.wire_drawing = false;
                    self.interaction_state.wire_points.clear();
                    self.interaction_state.canvas.wire_preview.clear();
                    self.interaction_state.canvas.drawing_mode = false;
                    self.interaction_state.current_tool = Tool::Select;
                    self.interaction_state.canvas.tool_preview = None;
                }
                self.finish_update()
            }
            Message::Tool(ToolMessage::SelectTool(tool)) => {
                self.interaction_state.current_tool = tool;
                // Re-label the floating cursor tag so the user sees which
                // placement mode they're in. Altium shows the tool name near
                // the crosshair until the first click.
                self.interaction_state.canvas.tool_preview = match tool {
                    Tool::Wire => Some("Wire".to_string()),
                    Tool::Bus => Some("Bus".to_string()),
                    Tool::BusEntry => Some("Bus Entry".to_string()),
                    Tool::Text => Some("Text".to_string()),
                    Tool::NoConnect => Some("No Connect".to_string()),
                    Tool::Measure => Some("Measure".to_string()),
                    _ => None,
                };
                if tool == Tool::Measure {
                    self.clear_transient_schematic_tool_state();
                } else {
                    self.clear_measurement();
                    if tool == Tool::Select {
                        self.clear_transient_schematic_tool_state();
                    }
                }
                self.finish_update()
            }
            _ => unreachable!("dispatch_tool_message received non-tool message"),
        }
    }
}