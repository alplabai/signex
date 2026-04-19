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
                    font_size: 1.8,
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
                    font_size: 1.8,
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
            ActiveBarAction::PlaceParameterSet => {
                // Parameter Set — attaches named params to a net. Arm a
                // label-tool ghost with a "PARAM=VALUE" default so the user
                // drops + edits inline. Full parameter-manager dialog lands
                // in v0.7.1.
                let _ = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Label)));
                self.interaction_state.pending_port = None;
                self.interaction_state.canvas.ghost_label =
                    Some(signex_types::schematic::Label {
                        uuid: uuid::Uuid::new_v4(),
                        text: "PARAM=VALUE".to_string(),
                        position: signex_types::schematic::Point::new(0.0, 0.0),
                        rotation: 0.0,
                        label_type: signex_types::schematic::LabelType::Net,
                        shape: String::new(),
                        font_size: 1.8,
                        justify: signex_types::schematic::HAlign::Left,
                    });
            }
            ActiveBarAction::PlaceDiffPair => {
                // DiffPair directive — attaches a differential-pair rule.
                // Ships as a text note until the PCB router's constraint
                // model lands in v2.1.
                let _ = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Text)));
                self.interaction_state.canvas.ghost_text =
                    Some(signex_types::schematic::TextNote {
                        uuid: uuid::Uuid::new_v4(),
                        text: "DIFF_PAIR".to_string(),
                        position: signex_types::schematic::Point::new(0.0, 0.0),
                        rotation: 0.0,
                        font_size: 1.8,
                        justify_h: signex_types::schematic::HAlign::Left,
                        justify_v: signex_types::schematic::VAlign::default(),
                    });
            }
            ActiveBarAction::PlaceBlanket | ActiveBarAction::PlaceCompileMask => {
                // Blanket / Compile Mask — rectangular rule areas. Placement
                // is Tool::Rectangle for v0.7; the rule/mask semantics wire
                // into ERC in v0.7.1.
                let _ =
                    self.update(Message::Tool(ToolMessage::SelectTool(Tool::Rectangle)));
            }
            // Net-color palette — arms pending_net_color so the next
            // canvas click on a wire floods its whole net with the
            // selected colour. Colours stay in app state only; nothing
            // is written back to the .kicad_sch so KiCad round-trips
            // unchanged.
            ActiveBarAction::NetColorBlue
            | ActiveBarAction::NetColorLightGreen
            | ActiveBarAction::NetColorLightBlue
            | ActiveBarAction::NetColorRed
            | ActiveBarAction::NetColorFuchsia
            | ActiveBarAction::NetColorYellow
            | ActiveBarAction::NetColorDarkGreen => {
                let c = match action {
                    ActiveBarAction::NetColorBlue => (0x3B, 0x82, 0xF6),
                    ActiveBarAction::NetColorLightGreen => (0x86, 0xEF, 0xAC),
                    ActiveBarAction::NetColorLightBlue => (0x7D, 0xD3, 0xFC),
                    ActiveBarAction::NetColorRed => (0xEF, 0x44, 0x44),
                    ActiveBarAction::NetColorFuchsia => (0xEC, 0x48, 0x99),
                    ActiveBarAction::NetColorYellow => (0xFA, 0xCC, 0x15),
                    ActiveBarAction::NetColorDarkGreen => (0x16, 0xA3, 0x4A),
                    _ => (0xFF, 0xFF, 0xFF),
                };
                self.ui_state.pending_net_color =
                    Some(signex_types::theme::Color {
                        r: c.0,
                        g: c.1,
                        b: c.2,
                        a: 255,
                    });
                // Sync to canvas so mouse_interaction can show a pen
                // cursor while armed.
                self.interaction_state.canvas.pending_net_color =
                    self.ui_state.pending_net_color;
                crate::diagnostics::log_info(
                    "Net-color armed — click a wire to flood its net",
                );
            }
            ActiveBarAction::ClearNetColor => {
                // Arm "clear one" — next click removes the override on
                // the wires of the clicked net.
                self.ui_state.pending_net_color = None;
                self.interaction_state.canvas.pending_net_color = None;
                // A distinct armed state: use a sentinel color (alpha 0)
                // to mean "clear mode". Simpler than a second enum — we
                // still read pending_net_color at click time.
                self.ui_state.pending_net_color =
                    Some(signex_types::theme::Color { r: 0, g: 0, b: 0, a: 0 });
                self.interaction_state.canvas.pending_net_color =
                    self.ui_state.pending_net_color;
                crate::diagnostics::log_info(
                    "Click a wire to clear its net-color override",
                );
            }
            ActiveBarAction::ClearAllNetColors => {
                if !self.ui_state.wire_color_overrides.is_empty() {
                    self.ui_state
                        .net_color_undo
                        .push(self.ui_state.wire_color_overrides.clone());
                }
                self.ui_state.wire_color_overrides.clear();
                self.interaction_state.canvas.wire_color_overrides.clear();
                self.ui_state.pending_net_color = None;
                self.interaction_state.canvas.pending_net_color = None;
                self.interaction_state.canvas.clear_content_cache();
                crate::diagnostics::log_info("Cleared all net-color overrides");
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
            pin_uuids: std::collections::HashMap::new(),
            instances: Vec::new(),
        });
    }
}
