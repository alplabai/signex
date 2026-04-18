use iced::Task;

use super::super::*;

impl Signex {
    /// On TAB during placement: commit the ghost at the current cursor
    /// world position as a real engine object and select it, so the normal
    /// selection-aware Properties panel (with full per-kind fields) renders.
    /// Returns without committing for kinds that have no in-place properties
    /// (wire / bus / bus entry / no-connect / component fallback).
    #[allow(dead_code)]
    fn commit_ghost_for_pre_placement(
        &mut self,
        kind: crate::panels::PrePlacementKind,
        label_text: &str,
    ) {
        use crate::panels::PrePlacementKind;
        use signex_types::schematic as sch;
        // Snap the cursor to grid so the new object lands on a grid dot.
        let (wx, wy) = {
            let x = self.ui_state.cursor_x;
            let y = self.ui_state.cursor_y;
            if self.ui_state.snap_enabled && self.ui_state.grid_size_mm > 0.0 {
                let g = self.ui_state.grid_size_mm as f64;
                ((x / g).round() * g, (y / g).round() * g)
            } else {
                (x, y)
            }
        };
        match kind {
            PrePlacementKind::NetLabel
            | PrePlacementKind::GlobalPort
            | PrePlacementKind::HierPort => {
                let (label_type, shape) = match kind {
                    PrePlacementKind::GlobalPort => {
                        (sch::LabelType::Global, "bidirectional".to_string())
                    }
                    PrePlacementKind::HierPort => (sch::LabelType::Hierarchical, String::new()),
                    _ => (sch::LabelType::Net, String::new()),
                };
                let uuid = uuid::Uuid::new_v4();
                let label = sch::Label {
                    uuid,
                    text: if label_text.is_empty() {
                        "NET".to_string()
                    } else {
                        label_text.to_string()
                    },
                    position: sch::Point::new(wx, wy),
                    rotation: 0.0,
                    label_type,
                    shape,
                    font_size: 1.8,
                    justify: sch::HAlign::Left,
                };
                self.apply_engine_command(
                    signex_engine::Command::PlaceLabel { label },
                    false,
                    false,
                );
                self.interaction_state.canvas.selected = vec![sch::SelectedItem {
                    uuid,
                    kind: sch::SelectedKind::Label,
                }];
                // Keep the ghost armed so Resume picks up where the user
                // left off — just hidden while paused.
                self.update_selection_info();
            }
            PrePlacementKind::TextNote => {
                let uuid = uuid::Uuid::new_v4();
                let tn = sch::TextNote {
                    uuid,
                    text: if label_text.is_empty() {
                        "Text".to_string()
                    } else {
                        label_text.to_string()
                    },
                    position: sch::Point::new(wx, wy),
                    rotation: 0.0,
                    font_size: 1.8,
                    justify_h: sch::HAlign::Left,
                    justify_v: sch::VAlign::default(),
                };
                self.apply_engine_command(
                    signex_engine::Command::PlaceTextNote { text_note: tn },
                    false,
                    false,
                );
                self.interaction_state.canvas.selected = vec![sch::SelectedItem {
                    uuid,
                    kind: sch::SelectedKind::TextNote,
                }];
                self.update_selection_info();
            }
            PrePlacementKind::PowerPort => {
                let Some((net_name, lib_id)) = self.interaction_state.pending_power.clone() else {
                    return;
                };
                let uuid = uuid::Uuid::new_v4();
                let symbol = sch::Symbol {
                    uuid,
                    lib_id,
                    reference: String::new(),
                    value: if label_text.is_empty() {
                        net_name
                    } else {
                        label_text.to_string()
                    },
                    footprint: String::new(),
                    datasheet: String::new(),
                    position: sch::Point::new(wx, wy),
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
                };
                self.apply_engine_command(
                    signex_engine::Command::PlaceSymbol { symbol },
                    false,
                    false,
                );
                self.interaction_state.canvas.selected = vec![sch::SelectedItem {
                    uuid,
                    kind: sch::SelectedKind::Symbol,
                }];
                self.update_selection_info();
            }
            _ => {
                // Wire / bus / bus entry / no-connect / component —
                // properties are edited post-placement via the regular
                // selection flow. TAB just pauses the ghost here.
            }
        }
    }

    pub(super) fn dispatch_tool_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PrePlacementTab => {
                if self.interaction_state.current_tool != Tool::Select
                    && self.interaction_state.current_tool != Tool::Measure
                {
                    use crate::panels::PrePlacementKind;
                    use signex_types::schematic::LabelType;
                    // Figure out the exact placement flavor so the form
                    // shows fields relevant to what's being dropped. A
                    // Tool::Label with `pending_port` = Global is NOT the
                    // same as a plain net label; a Tool::Component with
                    // `pending_power` is a power port, not a regular part.
                    let (kind, tool_name, default_label_text, default_designator) =
                        match self.interaction_state.current_tool {
                            Tool::Wire => (
                                PrePlacementKind::Wire,
                                "Wire".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            Tool::Bus => (
                                PrePlacementKind::Bus,
                                "Bus".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            Tool::BusEntry => (
                                PrePlacementKind::BusEntry,
                                "Bus Entry".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            Tool::NoConnect => (
                                PrePlacementKind::NoConnect,
                                "No Connect".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            Tool::Text => (
                                PrePlacementKind::TextNote,
                                "Text".to_string(),
                                "Text".to_string(),
                                String::new(),
                            ),
                            Tool::Component => {
                                // Power port — armed via Active Bar — lives under
                                // Tool::Component but has `pending_power` set.
                                if let Some((net, _)) =
                                    self.interaction_state.pending_power.as_ref().cloned()
                                {
                                    (
                                        PrePlacementKind::PowerPort,
                                        format!("Power Port ({})", net),
                                        net,
                                        String::new(),
                                    )
                                } else {
                                    let (label, designator) = self
                                        .current_component_defaults()
                                        .unwrap_or_else(|| ("NET".to_string(), String::new()));
                                    (
                                        PrePlacementKind::Component,
                                        "Component".to_string(),
                                        label,
                                        designator,
                                    )
                                }
                            }
                            Tool::Label => match self.interaction_state.pending_port.as_ref() {
                                Some((LabelType::Global, _)) => (
                                    PrePlacementKind::GlobalPort,
                                    "Global Port".to_string(),
                                    "PORT".to_string(),
                                    String::new(),
                                ),
                                Some((LabelType::Hierarchical, _)) => (
                                    PrePlacementKind::HierPort,
                                    "Hierarchical Port".to_string(),
                                    "SHEET".to_string(),
                                    String::new(),
                                ),
                                _ => (
                                    PrePlacementKind::NetLabel,
                                    "Net Label".to_string(),
                                    "NET".to_string(),
                                    String::new(),
                                ),
                            },
                            _ => (
                                PrePlacementKind::Other,
                                format!("{}", self.interaction_state.current_tool),
                                String::new(),
                                String::new(),
                            ),
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
                    let prev_rotation = self
                        .document_state
                        .panel_ctx
                        .pre_placement
                        .as_ref()
                        .map(|pp| pp.rotation)
                        .unwrap_or(0.0);
                    let prev_font = self
                        .document_state
                        .panel_ctx
                        .pre_placement
                        .as_ref()
                        .map(|pp| pp.font.clone())
                        .unwrap_or_else(|| "Iosevka".to_string());
                    let prev_font_size = self
                        .document_state
                        .panel_ctx
                        .pre_placement
                        .as_ref()
                        .map(|pp| pp.font_size_pt)
                        .unwrap_or(10);
                    let prev_jh = self
                        .document_state
                        .panel_ctx
                        .pre_placement
                        .as_ref()
                        .map(|pp| pp.justify_h)
                        .unwrap_or(signex_types::schematic::HAlign::Left);
                    let prev_jv = self
                        .document_state
                        .panel_ctx
                        .pre_placement
                        .as_ref()
                        .map(|pp| pp.justify_v)
                        .unwrap_or_default();
                    let label_text_for_commit = label_text.clone();
                    self.document_state.panel_ctx.pre_placement =
                        Some(crate::panels::PrePlacementData {
                            tool_name,
                            kind,
                            label_text,
                            designator,
                            rotation: prev_rotation,
                            font: prev_font,
                            font_size_pt: prev_font_size,
                            justify_h: prev_jh,
                            justify_v: prev_jv,
                            bold: false,
                            italic: false,
                            underline: false,
                            cursor_x_mm: self.ui_state.cursor_x,
                            cursor_y_mm: self.ui_state.cursor_y,
                        });
                    self.document_state
                        .dock
                        .add_panel(PanelPosition::Right, crate::panels::PanelKind::Properties);
                    // Freeze ghost + suppress canvas clicks until OK. TAB
                    // does NOT place the object — it pauses placement and
                    // the pre-placement form edits the properties used by
                    // the next click. Placement resumes when the user hits
                    // Resume or Enter.
                    self.interaction_state.canvas.placement_paused = true;
                    let _ = label_text_for_commit;
                }
                self.finish_update()
            }
            Message::ResumePlacement => {
                // Big on-canvas "Resume" button (Altium-style pause overlay).
                // Keep `pre_placement` alive so the next click consumes the
                // values the user just edited — only un-pause the canvas and
                // close any transient selection the dock picked up.
                self.interaction_state.canvas.placement_paused = false;
                self.interaction_state.canvas.selected.clear();
                self.update_selection_info();
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
                // Arm a ghost text-note preview for the Text tool so the
                // user sees a sample "Text" glyph that will be placed on
                // the next click. Cleared on tool switch / Escape.
                if tool == Tool::Text {
                    self.interaction_state.canvas.ghost_text =
                        Some(signex_types::schematic::TextNote {
                            uuid: uuid::Uuid::new_v4(),
                            text: "Text".to_string(),
                            position: signex_types::schematic::Point::new(0.0, 0.0),
                            rotation: 0.0,
                            font_size: 1.8,
                            justify_h: signex_types::schematic::HAlign::Left,
                            justify_v: signex_types::schematic::VAlign::default(),
                        });
                } else {
                    self.interaction_state.canvas.ghost_text = None;
                }
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
