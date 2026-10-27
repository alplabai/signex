use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_active_bar_placement_preset(
        &mut self,
        action: crate::active_bar::ActiveBarAction,
    ) -> Task<Message> {
        use crate::active_bar::ActiveBarAction;

        // Follow-up task from any re-entrant tool-select / power-port arm
        // (each `self.update(...)` returns a `finish_update` task that must
        // be propagated, not dropped). Only one match arm runs, so a single
        // slot suffices.
        let mut follow = Task::none();
        match action {
            ActiveBarAction::PlacePowerGND => {
                follow = self.set_pending_power_port("GND", "power:GND");
            }
            ActiveBarAction::PlacePowerVCC => {
                follow = self.set_pending_power_port("VCC", "power:VCC");
            }
            ActiveBarAction::PlacePowerPlus12 => {
                follow = self.set_pending_power_port("+12V", "power:+12V");
            }
            ActiveBarAction::PlacePowerPlus5 => {
                follow = self.set_pending_power_port("+5V", "power:+5V");
            }
            ActiveBarAction::PlacePowerMinus5 => {
                follow = self.set_pending_power_port("-5V", "power:-5V");
            }
            // Styled ports carry the style keyword the built-in power renderer
            // recognizes. They must never use `power:PWR_FLAG`: a flag does not
            // name a net, so a styled port with that lib_id would stop merging
            // by value in the netlist and ERC.
            ActiveBarAction::PlacePowerArrow => {
                follow = self.set_pending_power_port("PWR", "power:arrow");
            }
            ActiveBarAction::PlacePowerWave => {
                follow = self.set_pending_power_port("PWR", "power:wave");
            }
            ActiveBarAction::PlacePowerBar => {
                follow = self.set_pending_power_port("PWR", "power:bar");
            }
            ActiveBarAction::PlacePowerCircle => {
                follow = self.set_pending_power_port("PWR", "power:circle");
            }
            ActiveBarAction::PlacePowerSignalGND => {
                follow = self.set_pending_power_port("GNDREF", "power:GNDREF");
            }
            ActiveBarAction::PlacePowerEarth => {
                follow = self.set_pending_power_port("Earth", "power:Earth");
            }
            ActiveBarAction::PlacePort => {
                follow = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Label)));
                self.interaction_state.pending_port = Some((
                    signex_types::schematic::LabelType::Global,
                    "bidirectional".to_string(),
                ));
                self.interaction_state.active_canvas_mut().ghost_label =
                    Some(signex_types::schematic::Label {
                        unknown_columns: Default::default(),
                        uuid: uuid::Uuid::new_v4(),
                        text: "PORT".to_string(),
                        position: signex_types::schematic::Point::new(0.0, 0.0),
                        rotation: 0.0,
                        label_type: signex_types::schematic::LabelType::Global,
                        shape: "bidirectional".to_string(),
                        font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
                        justify: signex_types::schematic::HAlign::Left,
                        justify_v: signex_types::schematic::VAlign::Bottom,
                        font_family: None,
                    });
            }
            ActiveBarAction::PlaceOffSheetConnector => {
                follow = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Label)));
                self.interaction_state.pending_port = Some((
                    signex_types::schematic::LabelType::Hierarchical,
                    String::new(),
                ));
                self.interaction_state.active_canvas_mut().ghost_label =
                    Some(signex_types::schematic::Label {
                        unknown_columns: Default::default(),
                        uuid: uuid::Uuid::new_v4(),
                        text: "SHEET".to_string(),
                        position: signex_types::schematic::Point::new(0.0, 0.0),
                        rotation: 0.0,
                        label_type: signex_types::schematic::LabelType::Hierarchical,
                        shape: String::new(),
                        font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
                        justify: signex_types::schematic::HAlign::Left,
                        justify_v: signex_types::schematic::VAlign::Bottom,
                        font_family: None,
                    });
            }
            ActiveBarAction::PlaceBusEntry => {
                follow = self.update(Message::Tool(ToolMessage::SelectTool(Tool::BusEntry)));
                self.interaction_state.pending_power = None;
            }
            ActiveBarAction::PlaceNoERC => {
                follow = self.update(Message::Tool(ToolMessage::SelectTool(Tool::NoErcDirective)));
            }
            // Sheet symbol has its own tool: a hierarchical sheet, not a
            // plain rectangle.
            ActiveBarAction::PlaceSheetSymbol => {
                follow = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Sheet)));
            }
            // Arc — dedicated 3-click tool.
            ActiveBarAction::DrawArc => {
                follow = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Arc)));
            }
            // Polygon — click-by-click freeform.
            ActiveBarAction::DrawPolygon => {
                follow = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Polyline)));
            }
            ActiveBarAction::PlaceParameterSet => {
                // Parameter Set — attaches named params to a net. Arm a
                // label-tool ghost with a "PARAM=VALUE" default so the
                // user drops + edits inline. Bulk edits go through the
                // Parameter Manager (Design menu).
                follow = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Label)));
                self.interaction_state.pending_port = None;
                self.interaction_state.active_canvas_mut().ghost_label =
                    Some(signex_types::schematic::Label {
                        unknown_columns: Default::default(),
                        uuid: uuid::Uuid::new_v4(),
                        text: "PARAM=VALUE".to_string(),
                        position: signex_types::schematic::Point::new(0.0, 0.0),
                        rotation: 0.0,
                        label_type: signex_types::schematic::LabelType::Net,
                        shape: String::new(),
                        font_size: 1.8,
                        justify: signex_types::schematic::HAlign::Left,
                        justify_v: signex_types::schematic::VAlign::Bottom,
                        font_family: None,
                    });
            }
            // Net-color palette — arms pending_net_color so the next
            // canvas click on a wire floods its whole net with the
            // selected colour. Colours stay in app state only; nothing
            // is written back to the .standard_sch so Standard round-trips
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
                self.ui_state.pending_net_color = Some(signex_types::theme::Color {
                    r: c.0,
                    g: c.1,
                    b: c.2,
                    a: 255,
                });
                // Sync to canvas so mouse_interaction can show a pen
                // cursor while armed.
                self.interaction_state.active_canvas_mut().pending_net_color =
                    self.ui_state.pending_net_color;
                crate::diagnostics::log_info("Net-color armed — click a wire to flood its net");
            }
            ActiveBarAction::NetColorCustom => {
                // Open the custom colour picker modal. Pre-seed the
                // draft from whatever is currently armed so reopening
                // the picker after a custom pick doesn't reset it.
                let seed = self
                    .ui_state
                    .pending_net_color
                    .filter(|c| c.a != 0)
                    .map(|c| iced::Color::from_rgb8(c.r, c.g, c.b))
                    .unwrap_or(iced::Color::from_rgb(0.40, 0.40, 0.93));
                self.ui_state.net_color_custom.draft = seed;
                self.ui_state.net_color_custom.show = true;
            }
            ActiveBarAction::ClearNetColor => {
                // Arm "clear one" — next click removes the override on
                // the wires of the clicked net.
                self.ui_state.pending_net_color = None;
                self.interaction_state.active_canvas_mut().pending_net_color = None;
                // A distinct armed state: use a sentinel color (alpha 0)
                // to mean "clear mode". Simpler than a second enum — we
                // still read pending_net_color at click time.
                self.ui_state.pending_net_color = Some(signex_types::theme::Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                });
                self.interaction_state.active_canvas_mut().pending_net_color =
                    self.ui_state.pending_net_color;
                crate::diagnostics::log_info("Click a wire to clear its net-color override");
            }
            ActiveBarAction::ClearAllNetColors => {
                // #631 — clearing `ui_state` is the whole operation now;
                // the canvas reads that map through `CanvasViewPrefs`
                // instead of keeping a cleared-in-parallel copy. #1005 —
                // each sheet keeps its own undo step for what this removes.
                self.clear_all_net_colors_undoably();
                self.ui_state.pending_net_color = None;
                self.interaction_state.active_canvas_mut().pending_net_color = None;
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                crate::diagnostics::log_info("Cleared all net-color overrides");
            }
            _ => {}
        }

        follow
    }

    fn set_pending_power_port(&mut self, net_name: &str, lib_id: &str) -> Task<Message> {
        // Go through the normal tool-switch path first so previous ghosts
        // and tool_preview get cleaned up; then override tool_preview to
        // the specific power-port name and arm the ghost_symbol.
        let task = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Component)));
        self.interaction_state.pending_power = Some((net_name.to_string(), lib_id.to_string()));
        self.interaction_state.active_canvas_mut().tool_preview = Some(net_name.to_string());
        // Live preview: build a ghost power-port symbol that follows the
        // cursor so the user sees the actual shape (bars / bar / triangle)
        // before committing to a click.
        self.interaction_state.active_canvas_mut().ghost_symbol =
            Some(signex_types::schematic::Symbol {
                unknown_columns: Default::default(),
                unknown: Default::default(),
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
                fields_user_placed: false,
                dnp: false,
                in_bom: true,
                on_board: true,
                exclude_from_sim: false,
                locked: false,
                fields: std::collections::HashMap::new(),
                custom_properties: Vec::new(),
                pin_uuids: std::collections::HashMap::new(),
                instances: Vec::new(),
                library_id: None,
                row_id: None,
                library_version: String::new(),
            });
        task
    }
}
