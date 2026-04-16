use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_active_bar_message(
        &mut self,
        msg: crate::active_bar::ActiveBarMsg,
    ) -> Task<Message> {
        use crate::active_bar::{ActiveBarAction, ActiveBarMsg};

        match msg {
            ActiveBarMsg::ToggleMenu(menu) => {
                self.interaction_state.active_bar_menu = Some(menu);
                self.interaction_state.context_menu = None;
            }
            ActiveBarMsg::CloseMenus => {
                self.interaction_state.active_bar_menu = None;
            }
            ActiveBarMsg::ToggleFilter(filter) => {
                if self.interaction_state.selection_filters.contains(&filter) {
                    self.interaction_state.selection_filters.remove(&filter);
                } else {
                    self.interaction_state.selection_filters.insert(filter);
                }
                return Task::none();
            }
            ActiveBarMsg::ToggleAllFilters => {
                if self.interaction_state.selection_filters.len()
                    == crate::active_bar::SelectionFilter::ALL.len()
                {
                    self.interaction_state.selection_filters.clear();
                } else {
                    self.interaction_state.selection_filters =
                        crate::active_bar::SelectionFilter::ALL.iter().copied().collect();
                }
                return Task::none();
            }
            ActiveBarMsg::Action(action) => {
                self.interaction_state.active_bar_menu = None;
                let group = match &action {
                    ActiveBarAction::DrawWire
                    | ActiveBarAction::DrawBus
                    | ActiveBarAction::PlaceBusEntry
                    | ActiveBarAction::PlaceNetLabel => Some("wiring"),
                    ActiveBarAction::PlacePowerGND
                    | ActiveBarAction::PlacePowerVCC
                    | ActiveBarAction::PlacePowerPlus12
                    | ActiveBarAction::PlacePowerPlus5
                    | ActiveBarAction::PlacePowerMinus5
                    | ActiveBarAction::PlacePowerArrow
                    | ActiveBarAction::PlacePowerWave
                    | ActiveBarAction::PlacePowerBar
                    | ActiveBarAction::PlacePowerCircle
                    | ActiveBarAction::PlacePowerSignalGND
                    | ActiveBarAction::PlacePowerEarth => Some("power"),
                    ActiveBarAction::PlaceTextString
                    | ActiveBarAction::PlaceTextFrame
                    | ActiveBarAction::PlaceNote => Some("text"),
                    ActiveBarAction::DrawArc
                    | ActiveBarAction::DrawFullCircle
                    | ActiveBarAction::DrawEllipticalArc
                    | ActiveBarAction::DrawEllipse
                    | ActiveBarAction::DrawLine
                    | ActiveBarAction::DrawRectangle
                    | ActiveBarAction::DrawRoundRectangle
                    | ActiveBarAction::DrawPolygon
                    | ActiveBarAction::DrawBezier
                    | ActiveBarAction::PlaceGraphic => Some("shapes"),
                    ActiveBarAction::PlaceSignalHarness
                    | ActiveBarAction::PlaceHarnessConnector
                    | ActiveBarAction::PlaceHarnessEntry => Some("harness"),
                    ActiveBarAction::PlacePort | ActiveBarAction::PlaceOffSheetConnector => {
                        Some("port")
                    }
                    ActiveBarAction::PlaceSheetSymbol
                    | ActiveBarAction::PlaceSheetEntry
                    | ActiveBarAction::PlaceDeviceSheetSymbol
                    | ActiveBarAction::PlaceReuseBlock => Some("sheet"),
                    ActiveBarAction::PlaceParameterSet
                    | ActiveBarAction::PlaceNoERC
                    | ActiveBarAction::PlaceDiffPair
                    | ActiveBarAction::PlaceBlanket
                    | ActiveBarAction::PlaceCompileMask => Some("directives"),
                    _ => None,
                };
                if let Some(g) = group {
                    self.interaction_state
                        .last_tool
                        .insert(g.to_string(), action.clone());
                }
                match action {
                    ActiveBarAction::ToolSelect => {
                        return self.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)));
                    }
                    ActiveBarAction::DrawWire => {
                        return self.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
                    }
                    ActiveBarAction::DrawBus => {
                        return self.update(Message::Tool(ToolMessage::SelectTool(Tool::Bus)));
                    }
                    ActiveBarAction::PlaceNetLabel => {
                        return self.update(Message::Tool(ToolMessage::SelectTool(Tool::Label)));
                    }
                    ActiveBarAction::PlaceComponent => {
                        return self.update(Message::Tool(ToolMessage::SelectTool(Tool::Component)));
                    }
                    ActiveBarAction::PlaceTextString => {
                        return self.update(Message::Tool(ToolMessage::SelectTool(Tool::Text)));
                    }
                    ActiveBarAction::DrawLine => {
                        return self.update(Message::Tool(ToolMessage::SelectTool(Tool::Line)));
                    }
                    ActiveBarAction::DrawRectangle => {
                        return self.update(Message::Tool(ToolMessage::SelectTool(Tool::Rectangle)));
                    }
                    ActiveBarAction::DrawFullCircle => {
                        return self.update(Message::Tool(ToolMessage::SelectTool(Tool::Circle)));
                    }
                    ActiveBarAction::RotateSelection => {
                        return self.update(Message::RotateSelected);
                    }
                    ActiveBarAction::FlipSelectedX => {
                        return self.update(Message::MirrorSelectedX);
                    }
                    ActiveBarAction::FlipSelectedY => {
                        return self.update(Message::MirrorSelectedY);
                    }
                    ActiveBarAction::PlacePowerGND => {
                        self.interaction_state.pending_power = Some(("GND".into(), "power:GND".into()));
                        self.interaction_state.current_tool = Tool::Component;
                        self.interaction_state.canvas.tool_preview = Some("GND".into());
                    }
                    ActiveBarAction::PlacePowerVCC => {
                        self.interaction_state.pending_power = Some(("VCC".into(), "power:VCC".into()));
                        self.interaction_state.current_tool = Tool::Component;
                        self.interaction_state.canvas.tool_preview = Some("VCC".into());
                    }
                    ActiveBarAction::PlacePowerPlus12 => {
                        self.interaction_state.pending_power = Some(("+12V".into(), "power:+12V".into()));
                        self.interaction_state.current_tool = Tool::Component;
                        self.interaction_state.canvas.tool_preview = Some("+12V".into());
                    }
                    ActiveBarAction::PlacePowerPlus5 => {
                        self.interaction_state.pending_power = Some(("+5V".into(), "power:+5V".into()));
                        self.interaction_state.current_tool = Tool::Component;
                        self.interaction_state.canvas.tool_preview = Some("+5V".into());
                    }
                    ActiveBarAction::PlacePowerMinus5 => {
                        self.interaction_state.pending_power = Some(("-5V".into(), "power:-5V".into()));
                        self.interaction_state.current_tool = Tool::Component;
                        self.interaction_state.canvas.tool_preview = Some("-5V".into());
                    }
                    ActiveBarAction::PlacePowerArrow
                    | ActiveBarAction::PlacePowerWave
                    | ActiveBarAction::PlacePowerBar
                    | ActiveBarAction::PlacePowerCircle => {
                        self.interaction_state.pending_power = Some(("PWR".into(), "power:PWR_FLAG".into()));
                        self.interaction_state.current_tool = Tool::Component;
                        self.interaction_state.canvas.tool_preview = Some("PWR".into());
                    }
                    ActiveBarAction::PlacePowerSignalGND => {
                        self.interaction_state.pending_power = Some(("GNDREF".into(), "power:GNDREF".into()));
                        self.interaction_state.current_tool = Tool::Component;
                        self.interaction_state.canvas.tool_preview = Some("GNDREF".into());
                    }
                    ActiveBarAction::PlacePowerEarth => {
                        self.interaction_state.pending_power = Some(("Earth".into(), "power:Earth".into()));
                        self.interaction_state.current_tool = Tool::Component;
                        self.interaction_state.canvas.tool_preview = Some("Earth".into());
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
                    }
                    ActiveBarAction::SelectAll => {
                        return self.update(Message::Selection(
                            selection_message::SelectionMessage::SelectAll,
                        ));
                    }
                    ActiveBarAction::PlacePort => {
                        self.interaction_state.current_tool = Tool::Label;
                        self.interaction_state.pending_port = Some((
                            signex_types::schematic::LabelType::Global,
                            "bidirectional".to_string(),
                        ));
                        self.interaction_state.canvas.ghost_label = Some(signex_types::schematic::Label {
                            uuid: uuid::Uuid::new_v4(),
                            text: "PORT".to_string(),
                            position: signex_types::schematic::Point::new(0.0, 0.0),
                            rotation: 0.0,
                            label_type: signex_types::schematic::LabelType::Global,
                            shape: "bidirectional".to_string(),
                            font_size: 1.27,
                            justify: signex_types::schematic::HAlign::Left,
                        });
                    }
                    ActiveBarAction::PlaceOffSheetConnector => {
                        self.interaction_state.current_tool = Tool::Label;
                        self.interaction_state.pending_port = Some((
                            signex_types::schematic::LabelType::Hierarchical,
                            String::new(),
                        ));
                        self.interaction_state.canvas.ghost_label = Some(signex_types::schematic::Label {
                            uuid: uuid::Uuid::new_v4(),
                            text: "SHEET".to_string(),
                            position: signex_types::schematic::Point::new(0.0, 0.0),
                            rotation: 0.0,
                            label_type: signex_types::schematic::LabelType::Hierarchical,
                            shape: String::new(),
                            font_size: 1.27,
                            justify: signex_types::schematic::HAlign::Left,
                        });
                    }
                    ActiveBarAction::PlaceBusEntry => {
                        self.interaction_state.current_tool = Tool::Component;
                        self.interaction_state.pending_power = None;
                    }
                    ActiveBarAction::PlaceSheetSymbol
                    | ActiveBarAction::PlaceSheetEntry
                    | ActiveBarAction::PlaceDeviceSheetSymbol
                    | ActiveBarAction::PlaceReuseBlock => {}
                    _ => {}
                }
            }
        }

        Task::none()
    }
}