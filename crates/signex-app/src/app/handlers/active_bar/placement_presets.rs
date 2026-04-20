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
                let _ = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Label)));
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
                    font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
                    justify: signex_types::schematic::HAlign::Left,
                });
            }
            ActiveBarAction::PlaceOffSheetConnector => {
                let _ = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Label)));
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
                    font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
                    justify: signex_types::schematic::HAlign::Left,
                });
            }
            ActiveBarAction::PlaceBusEntry => {
                let _ = self.update(Message::Tool(ToolMessage::SelectTool(Tool::BusEntry)));
                self.interaction_state.pending_power = None;
            }
            // No-ERC directive reuses the existing No-Connect tool (Altium's
            // "Place No ERC" also drops an X marker at the clicked pin).
            ActiveBarAction::PlaceNoERC => {
                let _ = self.update(Message::Tool(ToolMessage::SelectTool(Tool::NoConnect)));
            }
            // Sheet-symbol / rounded rect / graphic — all rectangle-dragged
            // shapes in Altium. Until each has a bespoke tool, use the
            // rectangle tool so the drag-to-size gesture produces a visible
            // shape. (Text Frame is handled in action_groups so the ghost
            // text preview kicks in.)
            ActiveBarAction::PlaceSheetSymbol
            | ActiveBarAction::PlaceSheetEntry
            | ActiveBarAction::PlaceDeviceSheetSymbol
            | ActiveBarAction::PlaceReuseBlock
            | ActiveBarAction::DrawRoundRectangle
            | ActiveBarAction::PlaceGraphic => {
                let _ = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Rectangle)));
            }
            // Arcs / ellipses fall back to the circle tool.
            ActiveBarAction::DrawArc
            | ActiveBarAction::DrawEllipticalArc
            | ActiveBarAction::DrawEllipse => {
                let _ = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Circle)));
            }
            // Polygon / bezier — multi-click line approximation.
            ActiveBarAction::DrawPolygon | ActiveBarAction::DrawBezier => {
                let _ = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Line)));
            }
            // Harness + signal integrity directives — not yet implemented.
            // Log so the user sees the click registered and knows it's pending.
            ActiveBarAction::PlaceSignalHarness
            | ActiveBarAction::PlaceHarnessConnector
            | ActiveBarAction::PlaceHarnessEntry => {
                crate::diagnostics::log_info(
                    "Harness tools are planned for v1.1 (Advanced Schematic)",
                );
            }
            ActiveBarAction::PlaceParameterSet
            | ActiveBarAction::PlaceDiffPair
            | ActiveBarAction::PlaceBlanket
            | ActiveBarAction::PlaceCompileMask => {
                crate::diagnostics::log_info(
                    "Directive tool not yet implemented — coming with v0.7 ERC",
                );
            }
            // Net-color palette (F5 / sidebar). The underlying net-color model
            // isn't in place yet, but surface feedback so clicks don't silently
            // swallow and the action shows up in diagnostics.
            ActiveBarAction::NetColorBlue
            | ActiveBarAction::NetColorLightGreen
            | ActiveBarAction::NetColorLightBlue
            | ActiveBarAction::NetColorRed
            | ActiveBarAction::NetColorFuchsia
            | ActiveBarAction::NetColorYellow
            | ActiveBarAction::NetColorDarkGreen
            | ActiveBarAction::ClearNetColor
            | ActiveBarAction::ClearAllNetColors => {
                crate::diagnostics::log_info(
                    "Net-color override not yet implemented — planned for v0.7",
                );
            }
            _ => {}
        }

        Task::none()
    }

    fn set_pending_power_port(&mut self, net_name: &str, lib_id: &str) {
        // Go through the normal tool-switch path first so previous ghosts
        // and tool_preview get cleaned up; then override tool_preview to
        // the specific power-port name and arm the ghost_symbol.
        let _ = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Component)));
        self.interaction_state.pending_power = Some((net_name.to_string(), lib_id.to_string()));
        self.interaction_state.canvas.tool_preview = Some(net_name.to_string());
        // Live preview: build a ghost power-port symbol that follows the
        // cursor so the user sees the actual shape (bars / bar / triangle)
        // before committing to a click.
        self.interaction_state.canvas.ghost_symbol = Some(signex_types::schematic::Symbol {
            uuid: uuid::Uuid::new_v4(),
            lib_id: lib_id.to_string(),
            reference: String::new(),
            value: net_name.to_string(),
            footprint: String::new(),
            datasheet: String::new(),
            position: signex_types::schematic::Point::new(0.0, 0.0),
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            unit: 1,
            is_power: true,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: std::collections::HashMap::new(),
            custom_properties: Vec::new(),
            pin_uuids: std::collections::HashMap::new(),
            instances: Vec::new(),
        });
    }
}
