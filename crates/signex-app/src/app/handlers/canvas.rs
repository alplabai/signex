use iced::Task;
use signex_types::coord::Unit;

use crate::dock::DockMessage;

use super::super::helpers::constrain_segments;
use super::super::*;

impl Signex {
    pub(crate) fn handle_layout_drag_started(&mut self, target: DragTarget) {
        crate::diagnostics::log_debug(format!("[drag] START {target:?}"));
        self.interaction_state.dragging = Some(target);
        self.interaction_state.drag_start_pos = None;
        self.interaction_state.drag_start_size = match target {
            DragTarget::LeftPanel => self.ui_state.left_width,
            DragTarget::RightPanel => self.ui_state.right_width,
            DragTarget::BottomPanel => self.ui_state.bottom_height,
            DragTarget::ComponentsSplit => self.document_state.panel_ctx.components_split,
        };
    }

    pub(crate) fn handle_layout_drag_moved(&mut self, x: f32, y: f32) {
        self.interaction_state.last_mouse_pos = (x, y);
        if let Some(target) = self.interaction_state.dragging {
            let pos = match target {
                DragTarget::LeftPanel | DragTarget::RightPanel => x,
                DragTarget::BottomPanel | DragTarget::ComponentsSplit => y,
            };
            if self.interaction_state.drag_start_pos.is_none() {
                self.interaction_state.drag_start_pos = Some(pos);
            }
            if let Some(start) = self.interaction_state.drag_start_pos {
                let delta = pos - start;
                let (current, new_val) = match target {
                    DragTarget::LeftPanel => (
                        self.ui_state.left_width,
                        (self.interaction_state.drag_start_size + delta).clamp(100.0, 500.0),
                    ),
                    DragTarget::RightPanel => (
                        self.ui_state.right_width,
                        (self.interaction_state.drag_start_size - delta).clamp(100.0, 500.0),
                    ),
                    DragTarget::BottomPanel => (
                        self.ui_state.bottom_height,
                        (self.interaction_state.drag_start_size - delta).clamp(60.0, 400.0),
                    ),
                    DragTarget::ComponentsSplit => (
                        self.document_state.panel_ctx.components_split,
                        (self.interaction_state.drag_start_size + delta).clamp(80.0, 600.0),
                    ),
                };
                let new_val = new_val.round();
                if (current - new_val).abs() >= 1.0 {
                    match target {
                        DragTarget::LeftPanel => self.ui_state.left_width = new_val,
                        DragTarget::RightPanel => self.ui_state.right_width = new_val,
                        DragTarget::BottomPanel => self.ui_state.bottom_height = new_val,
                        DragTarget::ComponentsSplit => {
                            self.document_state.panel_ctx.components_split = new_val
                        }
                    }
                }
            }
        }

        if let (Some((pos, idx)), Some((ox, oy))) = (
            self.document_state.dock.tab_drag,
            self.interaction_state.tab_drag_origin,
        ) {
            let dx = x - ox;
            let dy = y - oy;
            if (dx * dx + dy * dy).sqrt() > 20.0 {
                self.document_state.dock.update(DockMessage::UndockPanel(pos, idx));
                self.interaction_state.tab_drag_origin = None;
            }
        }

        for fp in &mut self.document_state.dock.floating {
            if fp.dragging {
                fp.x = x - fp.width / 2.0;
                fp.y = y - 15.0;
            }
        }
    }

    pub(crate) fn handle_layout_drag_finished(&mut self) {
        if self.interaction_state.dragging.is_some() {
            crate::diagnostics::log_debug("[drag] END");
            self.interaction_state.dragging = None;
            self.interaction_state.drag_start_pos = None;
            return;
        }

        self.document_state.dock.tab_drag = None;
        self.interaction_state.tab_drag_origin = None;
        let (mx, my) = self.interaction_state.last_mouse_pos;
        let (ww, wh) = self.ui_state.window_size;
        let dock_zone = 120.0;
        let has_dragging = self.document_state.dock.floating.iter().any(|fp| fp.dragging);
        crate::diagnostics::log_debug(format!(
            "[dock-end] mouse=({mx:.0},{my:.0}) win=({ww:.0},{wh:.0}) floating={} dragging={has_dragging}",
            self.document_state.dock.floating.len()
        ));
        if let Some(drag_idx) = self
            .document_state
            .dock
            .floating
            .iter()
            .position(|fp| fp.dragging)
        {
            let target = if mx < dock_zone {
                Some(PanelPosition::Left)
            } else if mx > ww - dock_zone {
                Some(PanelPosition::Right)
            } else if my > wh - dock_zone {
                Some(PanelPosition::Bottom)
            } else {
                None
            };
            crate::diagnostics::log_debug(format!("[dock-end] target={target:?}"));
            if let Some(pos) = target {
                self.document_state
                    .dock
                    .update(DockMessage::DockFloatingTo(drag_idx, pos));
            } else {
                self.document_state.dock.floating[drag_idx].dragging = false;
            }
        } else {
            for fp in &mut self.document_state.dock.floating {
                fp.dragging = false;
            }
        }
    }

    pub(crate) fn handle_canvas_interaction_event(&mut self, event: CanvasEvent) -> Task<Message> {
        match event {
            CanvasEvent::CursorAt { x, y, zoom_pct } => {
                self.ui_state.cursor_x = x as f64;
                self.ui_state.cursor_y = y as f64;
                self.ui_state.zoom = zoom_pct;
                if self.interaction_state.current_tool == Tool::Measure
                    && self.interaction_state.canvas.measure_start.is_some()
                    && !self.interaction_state.canvas.measure_locked
                {
                    let (mx, my) = if self.ui_state.snap_enabled {
                        let gs = self.ui_state.grid_size_mm as f64;
                        ((x as f64 / gs).round() * gs, (y as f64 / gs).round() * gs)
                    } else {
                        (x as f64, y as f64)
                    };
                    self.interaction_state.canvas.measure_end =
                        Some(signex_types::schematic::Point::new(mx, my));
                }
            }
            CanvasEvent::Clicked { world_x, world_y } => {
                let (wx, wy) = if self.ui_state.snap_enabled {
                    let gs = self.ui_state.grid_size_mm as f64;
                    ((world_x / gs).round() * gs, (world_y / gs).round() * gs)
                } else {
                    (world_x, world_y)
                };

                match self.interaction_state.current_tool {
                    Tool::Measure => {
                        let point = signex_types::schematic::Point::new(wx, wy);
                        if self.interaction_state.canvas.measure_start.is_some()
                            && !self.interaction_state.canvas.measure_locked
                        {
                            self.interaction_state.canvas.measure_end = Some(point);
                            self.interaction_state.canvas.measure_locked = true;
                        } else {
                            self.interaction_state.canvas.measure_start = Some(point);
                            self.interaction_state.canvas.measure_end = Some(point);
                            self.interaction_state.canvas.measure_locked = false;
                        }
                    }
                    Tool::Wire => {
                        let pt = signex_types::schematic::Point::new(wx, wy);
                        if !self.interaction_state.wire_drawing {
                            self.interaction_state.wire_drawing = true;
                            self.interaction_state.wire_points.clear();
                            self.interaction_state.wire_points.push(pt);
                            self.interaction_state.canvas.wire_preview =
                                self.interaction_state.wire_points.clone();
                            self.interaction_state.canvas.drawing_mode = true;
                            self.interaction_state.canvas.draw_mode = self.interaction_state.draw_mode;
                            self.interaction_state.canvas.tool_preview = None;
                        } else if let Some(&start) = self.interaction_state.wire_points.last() {
                            let segments = constrain_segments(start, pt, self.interaction_state.draw_mode);
                            let mut wire_commands = Vec::new();
                            for seg in &segments {
                                let wire = signex_types::schematic::Wire {
                                    uuid: uuid::Uuid::new_v4(),
                                    start: seg.0,
                                    end: seg.1,
                                    stroke_width: 0.0,
                                };
                                wire_commands.push(signex_engine::Command::PlaceWireSegment { wire });
                            }
                            if !wire_commands.is_empty() {
                                self.apply_engine_commands(wire_commands, false, false);
                            }
                            let end_pt = segments.last().map(|s| s.1).unwrap_or(pt);
                            self.interaction_state.wire_points = vec![end_pt];
                            self.interaction_state.canvas.wire_preview = vec![end_pt];
                        }
                    }
                    Tool::Bus => {
                        let pt = signex_types::schematic::Point::new(wx, wy);
                        if !self.interaction_state.wire_drawing {
                            self.interaction_state.wire_drawing = true;
                            self.interaction_state.wire_points.clear();
                            self.interaction_state.wire_points.push(pt);
                            self.interaction_state.canvas.wire_preview =
                                self.interaction_state.wire_points.clone();
                            self.interaction_state.canvas.drawing_mode = true;
                            self.interaction_state.canvas.draw_mode = self.interaction_state.draw_mode;
                            self.interaction_state.canvas.tool_preview = None;
                        } else if let Some(&start) = self.interaction_state.wire_points.last() {
                            let segments = constrain_segments(start, pt, self.interaction_state.draw_mode);
                            let mut bus_commands = Vec::new();
                            for seg in &segments {
                                let bus = signex_types::schematic::Bus {
                                    uuid: uuid::Uuid::new_v4(),
                                    start: seg.0,
                                    end: seg.1,
                                };
                                bus_commands.push(signex_engine::Command::PlaceBus { bus });
                            }
                            if !bus_commands.is_empty() {
                                self.apply_engine_commands(bus_commands, false, false);
                            }
                            let end_pt = segments.last().map(|s| s.1).unwrap_or(pt);
                            self.interaction_state.wire_points = vec![end_pt];
                            self.interaction_state.canvas.wire_preview = vec![end_pt];
                        }
                    }
                    Tool::Component if self.interaction_state.pending_power.is_some() => {
                        if let Some((ref net_name, ref lib_id)) = self.interaction_state.pending_power {
                            let sym = signex_types::schematic::Symbol {
                                uuid: uuid::Uuid::new_v4(),
                                lib_id: lib_id.clone(),
                                reference: "#PWR?".to_string(),
                                value: net_name.clone(),
                                footprint: String::new(),
                                datasheet: String::new(),
                                position: signex_types::schematic::Point::new(wx, wy),
                                rotation: 0.0,
                                mirror_x: false,
                                mirror_y: false,
                                unit: 1,
                                is_power: true,
                                ref_text: None,
                                val_text: Some(signex_types::schematic::TextProp {
                                    position: signex_types::schematic::Point::new(wx, wy - 1.27),
                                    rotation: 0.0,
                                    font_size: 1.27,
                                    justify_h: signex_types::schematic::HAlign::Center,
                                    justify_v: signex_types::schematic::VAlign::default(),
                                    hidden: false,
                                }),
                                fields_autoplaced: true,
                                dnp: false,
                                in_bom: false,
                                on_board: true,
                                exclude_from_sim: false,
                                locked: false,
                                fields: std::collections::HashMap::new(),
                                pin_uuids: std::collections::HashMap::new(),
                                instances: Vec::new(),
                            };
                            self.apply_engine_command(
                                signex_engine::Command::PlaceSymbol { symbol: sym },
                                false,
                                false,
                            );
                        }
                    }
                    Tool::Component => {
                        let _ = self.place_selected_component(wx, wy);
                    }
                    Tool::NoConnect => {
                        let nc = signex_types::schematic::NoConnect {
                            uuid: uuid::Uuid::new_v4(),
                            position: signex_types::schematic::Point::new(wx, wy),
                        };
                        self.apply_engine_command(
                            signex_engine::Command::PlaceNoConnect { no_connect: nc },
                            false,
                            false,
                        );
                    }
                    Tool::BusEntry => {
                        let be = signex_types::schematic::BusEntry {
                            uuid: uuid::Uuid::new_v4(),
                            position: signex_types::schematic::Point::new(wx, wy),
                            size: (2.54, 2.54),
                        };
                        self.apply_engine_command(
                            signex_engine::Command::PlaceBusEntry { bus_entry: be },
                            false,
                            false,
                        );
                    }
                    Tool::Text => {
                        let note_text = self
                            .document_state
                            .panel_ctx
                            .pre_placement
                            .as_ref()
                            .map(|pp| pp.label_text.clone())
                            .unwrap_or_else(|| "Text".to_string());
                        let tn = signex_types::schematic::TextNote {
                            uuid: uuid::Uuid::new_v4(),
                            text: note_text,
                            position: signex_types::schematic::Point::new(wx, wy),
                            rotation: 0.0,
                            font_size: 1.27,
                            justify_h: signex_types::schematic::HAlign::Left,
                            justify_v: signex_types::schematic::VAlign::default(),
                        };
                        self.apply_engine_command(
                            signex_engine::Command::PlaceTextNote { text_note: tn },
                            false,
                            false,
                        );
                        self.interaction_state.current_tool = Tool::Select;
                    }
                    _ => {
                        return self.handle_selection_request(selection_request::SelectionRequest::HitAt {
                            world_x,
                            world_y,
                        });
                    }
                }
            }
            CanvasEvent::MoveSelected { dx, dy } => {
                let (dx, dy) = if self.ui_state.snap_enabled {
                    let gs = self.ui_state.grid_size_mm as f64;
                    ((dx / gs).round() * gs, (dy / gs).round() * gs)
                } else {
                    (dx, dy)
                };
                if (dx.abs() > 0.001 || dy.abs() > 0.001)
                    && !self.interaction_state.canvas.selected.is_empty()
                {
                    self.apply_engine_command(
                        signex_engine::Command::MoveSelection {
                            items: self.interaction_state.canvas.selected.clone(),
                            dx,
                            dy,
                        },
                        true,
                        true,
                    );
                }
            }
            CanvasEvent::DoubleClicked {
                world_x,
                world_y,
                screen_x,
                screen_y,
            } => {
                if self.interaction_state.wire_drawing {
                    self.interaction_state.wire_drawing = false;
                    self.interaction_state.wire_points.clear();
                    self.interaction_state.canvas.wire_preview.clear();
                    self.interaction_state.canvas.drawing_mode = false;
                } else if let Some(snapshot) = self.active_render_snapshot() {
                    use signex_types::schematic::SelectedKind;
                    if let Some(hit) =
                        signex_render::schematic::hit_test::hit_test(snapshot, world_x, world_y)
                    {
                        let edit_info = match hit.kind {
                            SelectedKind::Label => snapshot
                                .labels
                                .iter()
                                .find(|l| l.uuid == hit.uuid)
                                .map(|l| (l.text.clone(), SelectedKind::Label)),
                            SelectedKind::TextNote => snapshot
                                .text_notes
                                .iter()
                                .find(|t| t.uuid == hit.uuid)
                                .map(|t| (t.text.clone(), SelectedKind::TextNote)),
                            _ => None,
                        };
                        if let Some((text, kind)) = edit_info {
                            self.interaction_state.editing_text = Some(TextEditState {
                                uuid: hit.uuid,
                                kind,
                                original_text: text.clone(),
                                text,
                                screen_x,
                                screen_y,
                            });
                        }
                    }
                }
            }
            CanvasEvent::BoxSelect { x1, y1, x2, y2 } => {
                return self.handle_selection_request(selection_request::SelectionRequest::BoxSelect {
                    x1,
                    y1,
                    x2,
                    y2,
                });
            }
            CanvasEvent::CursorMoved => {
                self.interaction_state.canvas.clear_bg_cache();
                self.interaction_state.canvas.clear_content_cache();
                self.interaction_state.canvas.clear_overlay_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
                self.interaction_state.pcb_canvas.clear_content_cache();
                self.interaction_state.canvas.pending_fit.set(None);
                self.interaction_state.pcb_canvas.pending_fit.set(None);
            }
            CanvasEvent::FitAll => {
                if self.has_active_schematic() {
                    self.interaction_state.canvas.fit_to_paper();
                    self.interaction_state.canvas.clear_bg_cache();
                    self.interaction_state.canvas.clear_content_cache();
                } else if self.has_active_pcb() {
                    self.interaction_state.pcb_canvas.fit_to_board();
                    self.interaction_state.pcb_canvas.clear_bg_cache();
                    self.interaction_state.pcb_canvas.clear_content_cache();
                }
            }
            CanvasEvent::CtrlClicked { world_x, world_y } => {
                if let Some(snapshot) = self.active_render_snapshot()
                    && let Some(hit) =
                        signex_render::schematic::hit_test::hit_test(snapshot, world_x, world_y)
                {
                    if let Some(pos) = self
                        .interaction_state
                        .canvas
                        .selected
                        .iter()
                        .position(|s| s.uuid == hit.uuid)
                    {
                        self.interaction_state.canvas.selected.remove(pos);
                    } else {
                        self.interaction_state.canvas.selected.push(hit);
                    }
                    self.interaction_state.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
        }

        Task::none()
    }

    pub(crate) fn handle_unit_cycle_request(&mut self) {
        self.ui_state.unit = match self.ui_state.unit {
            Unit::Mm => Unit::Mil,
            Unit::Mil => Unit::Inch,
            Unit::Inch => Unit::Micrometer,
            Unit::Micrometer => Unit::Mm,
        };
    }
}