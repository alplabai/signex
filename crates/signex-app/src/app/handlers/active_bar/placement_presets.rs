use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_active_bar_placement_preset(
        &mut self,
        action: crate::active_bar::ActiveBarAction,
    ) -> Task<Message> {
        use crate::active_bar::ActiveBarAction;

        match action {
            ActiveBarAction::PlacePowerGND => {
                self.set_pending_power_port("GND", "power:GND");
            }
            ActiveBarAction::PlacePowerVCC => {
                self.set_pending_power_port("VCC", "power:VCC");
            }
            ActiveBarAction::PlacePowerPlus12 => {
                self.set_pending_power_port("+12V", "power:+12V");
            }
            ActiveBarAction::PlacePowerPlus5 => {
                self.set_pending_power_port("+5V", "power:+5V");
            }
            ActiveBarAction::PlacePowerMinus5 => {
                self.set_pending_power_port("-5V", "power:-5V");
            }
            ActiveBarAction::PlacePowerArrow
            | ActiveBarAction::PlacePowerWave
            | ActiveBarAction::PlacePowerBar
            | ActiveBarAction::PlacePowerCircle => {
                self.set_pending_power_port("PWR", "power:PWR_FLAG");
            }
            ActiveBarAction::PlacePowerSignalGND => {
                self.set_pending_power_port("GNDREF", "power:GNDREF");
            }
            ActiveBarAction::PlacePowerEarth => {
                self.set_pending_power_port("Earth", "power:Earth");
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
            | ActiveBarAction::PlaceReuseBlock
            | ActiveBarAction::PlaceTextFrame
            | ActiveBarAction::PlaceNote
            | ActiveBarAction::DrawArc
            | ActiveBarAction::DrawEllipticalArc
            | ActiveBarAction::DrawEllipse
            | ActiveBarAction::DrawRoundRectangle
            | ActiveBarAction::DrawPolygon
            | ActiveBarAction::DrawBezier
            | ActiveBarAction::PlaceGraphic
            | ActiveBarAction::PlaceSignalHarness
            | ActiveBarAction::PlaceHarnessConnector
            | ActiveBarAction::PlaceHarnessEntry
            | ActiveBarAction::PlaceParameterSet
            | ActiveBarAction::PlaceNoERC
            | ActiveBarAction::PlaceDiffPair
            | ActiveBarAction::PlaceBlanket
            | ActiveBarAction::PlaceCompileMask => {}
            _ => {}
        }

        Task::none()
    }

    fn set_pending_power_port(&mut self, net_name: &str, lib_id: &str) {
        self.interaction_state.pending_power = Some((net_name.to_string(), lib_id.to_string()));
        self.interaction_state.current_tool = Tool::Component;
        self.interaction_state.canvas.tool_preview = Some(net_name.to_string());
    }
}