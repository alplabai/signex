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
                self.document_state
                    .dock
                    .update(DockMessage::UndockPanel(pos, idx));
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
        let has_dragging = self
            .document_state
            .dock
            .floating
            .iter()
            .any(|fp| fp.dragging);
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
                // Altium-style placement pause: while the ghost is frozen
                // (TAB pressed, pre-placement form open), canvas clicks are
                // suppressed so the user can edit properties without a
                // stray click dropping an object. `pre_placement` lives on
                // past Resume so the first click after Resume inherits the
                // edited defaults — so key the gate on `placement_paused`,
                // not on `pre_placement.is_some()`.
                if self.interaction_state.canvas.placement_paused {
                    return Task::none();
                }
                // A click outside the inline editor commits its current value
                // and returns to normal selection — same convention as most
                // IDEs (Escape cancels, click elsewhere confirms).
                if let Some(state) = self.interaction_state.editing_text.take() {
                    if state.text != state.original_text {
                        let stored = signex_render::schematic::text::escape_for_kicad(&state.text);
                        let cmd = match state.kind {
                            signex_types::schematic::SelectedKind::Label => {
                                Some(signex_engine::Command::UpdateText {
                                    target: signex_engine::TextTarget::Label(state.uuid),
                                    value: stored,
                                })
                            }
                            signex_types::schematic::SelectedKind::TextNote => {
                                Some(signex_engine::Command::UpdateText {
                                    target: signex_engine::TextTarget::TextNote(state.uuid),
                                    value: stored,
                                })
                            }
                            _ => None,
                        };
                        if let Some(cmd) = cmd {
                            self.apply_engine_command(cmd, false, true);
                        }
                    }
                    return Task::none();
                }
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
                            self.interaction_state.canvas.draw_mode =
                                self.interaction_state.draw_mode;
                            self.interaction_state.canvas.tool_preview = None;
                        } else if let Some(&start) = self.interaction_state.wire_points.last() {
                            let segments =
                                constrain_segments(start, pt, self.interaction_state.draw_mode);
                            let mut wire_commands = Vec::new();
                            for seg in &segments {
                                let wire = signex_types::schematic::Wire {
                                    uuid: uuid::Uuid::new_v4(),
                                    start: seg.0,
                                    end: seg.1,
                                    stroke_width: 0.0,
                                };
                                wire_commands
                                    .push(signex_engine::Command::PlaceWireSegment { wire });
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
                            self.interaction_state.canvas.draw_mode =
                                self.interaction_state.draw_mode;
                            self.interaction_state.canvas.tool_preview = None;
                        } else if let Some(&start) = self.interaction_state.wire_points.last() {
                            let segments =
                                constrain_segments(start, pt, self.interaction_state.draw_mode);
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
                        if let Some((ref net_name, ref lib_id)) =
                            self.interaction_state.pending_power
                        {
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
                                    font_size: 1.8,
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
                            font_size: 1.8,
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
                    Tool::Label => {
                        // Place net / global / hierarchical label depending on
                        // pending_port state. Default: net label named "NET".
                        let (label_type, shape, default_text) = if let Some((lt, shape)) =
                            self.interaction_state.pending_port.clone()
                        {
                            let default = match lt {
                                signex_types::schematic::LabelType::Global => "PORT",
                                signex_types::schematic::LabelType::Hierarchical => "SHEET",
                                _ => "NET",
                            };
                            (lt, shape, default.to_string())
                        } else {
                            (
                                signex_types::schematic::LabelType::Net,
                                String::new(),
                                "NET".to_string(),
                            )
                        };
                        let text = self
                            .document_state
                            .panel_ctx
                            .pre_placement
                            .as_ref()
                            .map(|pp| pp.label_text.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .unwrap_or(default_text);
                        // Pick up the rotation / font size / justification
                        // the user edited in the TAB pre-placement form so
                        // the first click matches what they configured.
                        let (pp_rot, pp_fs_mm, pp_justify) = self
                            .document_state
                            .panel_ctx
                            .pre_placement
                            .as_ref()
                            .map(|pp| (pp.rotation, pp.font_size_pt as f64 * 0.254, pp.justify_h))
                            .unwrap_or((0.0, 1.8, signex_types::schematic::HAlign::Left));
                        let label = signex_types::schematic::Label {
                            uuid: uuid::Uuid::new_v4(),
                            text,
                            position: signex_types::schematic::Point::new(wx, wy),
                            rotation: pp_rot,
                            label_type,
                            shape,
                            font_size: pp_fs_mm,
                            justify: pp_justify,
                        };
                        self.apply_engine_command(
                            signex_engine::Command::PlaceLabel { label },
                            false,
                            false,
                        );
                    }
                    _ => {
                        return self.handle_selection_request(
                            selection_request::SelectionRequest::HitAt { world_x, world_y },
                        );
                    }
                }
            }
            CanvasEvent::MoveSelected { dx, dy } => {
                // Snap so the PRIMARY selected item's connection point (its
                // stored `position`) lands on a grid dot after the move, not
                // just the drag delta. Snapping only the delta preserves an
                // off-grid origin; users expect the endpoint to be on-grid
                // like KiCad/Altium do.
                let (dx, dy) = if self.ui_state.snap_enabled {
                    let gs = self.ui_state.grid_size_mm as f64;
                    let primary = self
                        .interaction_state
                        .canvas
                        .selected
                        .first()
                        .and_then(|item| {
                            let snap = self.active_render_snapshot()?;
                            primary_anchor_world(snap, item)
                        });
                    if let Some((px, py)) = primary {
                        let target_x = px + dx;
                        let target_y = py + dy;
                        let snapped_x = (target_x / gs).round() * gs;
                        let snapped_y = (target_y / gs).round() * gs;
                        (snapped_x - px, snapped_y - py)
                    } else {
                        ((dx / gs).round() * gs, (dy / gs).round() * gs)
                    }
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
                screen_x: _,
                screen_y: _,
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
                        use signex_render::schematic::text::expand_char_escapes;
                        let edit_info = match hit.kind {
                            SelectedKind::Label => snapshot
                                .labels
                                .iter()
                                .find(|l| l.uuid == hit.uuid)
                                .map(|l| {
                                    (
                                        l.text.clone(),
                                        SelectedKind::Label,
                                        l.position.x,
                                        l.position.y,
                                    )
                                }),
                            SelectedKind::TextNote => snapshot
                                .text_notes
                                .iter()
                                .find(|t| t.uuid == hit.uuid)
                                .map(|t| {
                                    (
                                        t.text.clone(),
                                        SelectedKind::TextNote,
                                        t.position.x,
                                        t.position.y,
                                    )
                                }),
                            _ => None,
                        };
                        if let Some((raw_text, kind, wx, wy)) = edit_info {
                            // Show the user the visible form (e.g. "/OE"), not
                            // the KiCad-escaped storage form ("{slash}OE").
                            let display_text = expand_char_escapes(&raw_text);
                            self.interaction_state.editing_text = Some(TextEditState {
                                uuid: hit.uuid,
                                kind,
                                original_text: display_text.clone(),
                                text: display_text,
                                world_x: wx,
                                world_y: wy,
                            });
                        }
                    }
                }
            }
            CanvasEvent::BoxSelect { x1, y1, x2, y2 } => {
                return self.handle_selection_request(
                    selection_request::SelectionRequest::BoxSelect { x1, y1, x2, y2 },
                );
            }
            CanvasEvent::CursorMoved => {
                self.interaction_state.canvas.clear_bg_cache();
                self.interaction_state.canvas.clear_overlay_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
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
                    && crate::app::handlers::selection_workflow::passes_filter(
                        &hit,
                        snapshot,
                        &self.interaction_state.selection_filters,
                    )
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

/// Resolve a selected item's primary anchor — the world point that should
/// snap to the grid (connection point for labels/wires/symbols, etc.).
fn primary_anchor_world(
    snap: &signex_render::schematic::SchematicRenderSnapshot,
    item: &signex_types::schematic::SelectedItem,
) -> Option<(f64, f64)> {
    use signex_types::schematic::SelectedKind;
    match item.kind {
        SelectedKind::Label => snap
            .labels
            .iter()
            .find(|l| l.uuid == item.uuid)
            .map(|l| (l.position.x, l.position.y)),
        SelectedKind::Symbol => snap
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .map(|s| (s.position.x, s.position.y)),
        SelectedKind::Wire => snap
            .wires
            .iter()
            .find(|w| w.uuid == item.uuid)
            .map(|w| (w.start.x, w.start.y)),
        SelectedKind::Bus => snap
            .buses
            .iter()
            .find(|b| b.uuid == item.uuid)
            .map(|b| (b.start.x, b.start.y)),
        SelectedKind::Junction => snap
            .junctions
            .iter()
            .find(|j| j.uuid == item.uuid)
            .map(|j| (j.position.x, j.position.y)),
        SelectedKind::NoConnect => snap
            .no_connects
            .iter()
            .find(|n| n.uuid == item.uuid)
            .map(|n| (n.position.x, n.position.y)),
        SelectedKind::TextNote => snap
            .text_notes
            .iter()
            .find(|t| t.uuid == item.uuid)
            .map(|t| (t.position.x, t.position.y)),
        SelectedKind::ChildSheet => snap
            .child_sheets
            .iter()
            .find(|c| c.uuid == item.uuid)
            .map(|c| (c.position.x, c.position.y)),
        SelectedKind::SymbolRefField => snap
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .and_then(|s| s.ref_text.as_ref().map(|rt| (rt.position.x, rt.position.y))),
        SelectedKind::SymbolValField => snap
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .and_then(|s| s.val_text.as_ref().map(|vt| (vt.position.x, vt.position.y))),
        _ => None,
    }
}
