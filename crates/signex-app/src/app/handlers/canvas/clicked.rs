use iced::Task;

use super::super::super::helpers::constrain_segments;
use super::super::super::*;
use super::pre_placement_shape;

impl Signex {
    pub(super) fn handle_canvas_clicked(&mut self, world_x: f64, world_y: f64) -> Task<Message> {
        // Altium-style lasso: first click anchors the start,
        // the cursor path auto-samples vertices, a second
        // click closes the polygon and commits. Escape /
        // right-click cancels.
        if self.ui_state.lasso_polygon.is_some() {
            let (vx, vy) = if self.interaction_state.active_canvas_mut().snap_enabled
                && self.interaction_state.active_canvas_mut().snap_grid_mm > 0.0
            {
                let g = self.interaction_state.active_canvas_mut().snap_grid_mm;
                ((world_x / g).round() * g, (world_y / g).round() * g)
            } else {
                (world_x, world_y)
            };
            let has_start = self
                .ui_state
                .lasso_polygon
                .as_ref()
                .is_some_and(|v| !v.is_empty());
            if !has_start {
                // First click — anchor vertex 0.
                if let Some(poly) = self.ui_state.lasso_polygon.as_mut() {
                    poly.push(signex_types::schematic::Point::new(vx, vy));
                }
                self.sync_lasso_polygon_to_canvas();
                return Task::none();
            }
            // Second click — close and commit. Append the
            // click position as the final vertex so the
            // polygon lands exactly where the user clicked.
            let mut pts = self.ui_state.lasso_polygon.take().unwrap_or_default();
            pts.push(signex_types::schematic::Point::new(vx, vy));
            if pts.len() >= 3 {
                let poly: Vec<(f64, f64)> = pts.iter().map(|p| (p.x, p.y)).collect();
                if let Some(snapshot) = self.active_render_snapshot() {
                    let filters = self.interaction_state.selection_filters.clone();
                    self.interaction_state.active_canvas_mut().selected =
                        crate::schematic_runtime::hit_test::hit_test_polygon(snapshot, &poly)
                            .into_iter()
                            .filter(|h| {
                                crate::app::handlers::selection_workflow::passes_filter(
                                    h, snapshot, &filters,
                                )
                            })
                            .collect();
                    self.update_selection_info();
                }
            }
            // `lasso_polygon` already drained via `.take()` above;
            // mirror the None into the canvas copy + invalidate
            // the overlay cache.
            self.sync_lasso_polygon_to_canvas();
            return Task::none();
        }
        // Net-colour flood: the user picked a swatch from the
        // Active Bar and is now clicking a wire. Union-find the
        // whole connected net and apply the colour (or clear it
        // if alpha == 0). Colours stay in app state so the
        // .standard_sch round-trips unchanged — Standard has no
        // notion of per-wire override colours.
        if let Some(pending) = self.ui_state.pending_net_color {
            // Snap the click point to the grid before hit
            // testing so the click lands where the pen ghost
            // previewed. Without this the cursor's raw world
            // coords can land a full grid cell off the nearest
            // wire — the visible pen snaps, but the hit target
            // was being missed.
            let (hit_x, hit_y) = if self.interaction_state.active_canvas_mut().snap_enabled
                && self.interaction_state.active_canvas_mut().snap_grid_mm > 0.0
            {
                let g = self.interaction_state.active_canvas_mut().snap_grid_mm;
                ((world_x / g).round() * g, (world_y / g).round() * g)
            } else {
                (world_x, world_y)
            };
            // Compute the net's wire uuids while only holding an
            // immutable snapshot borrow, then release it before
            // mutating ui_state.
            let net_wire_uuids: Vec<uuid::Uuid> = match self.active_render_snapshot() {
                None => Vec::new(),
                Some(snapshot) => {
                    // Strict on-wire test: the snapped click
                    // point must lie on a wire segment (point-
                    // to-segment distance < 0.05 mm). The
                    // general `hit_test` uses a 1.5 mm
                    // tolerance so parallel wires on adjacent
                    // grid points could each register; for
                    // net-colour painting we only want the
                    // wire directly under the pen.
                    fn point_on_segment(
                        px: f64,
                        py: f64,
                        ax: f64,
                        ay: f64,
                        bx: f64,
                        by: f64,
                    ) -> bool {
                        let dx = bx - ax;
                        let dy = by - ay;
                        let len2 = dx * dx + dy * dy;
                        if len2 == 0.0 {
                            let ddx = px - ax;
                            let ddy = py - ay;
                            return (ddx * ddx + ddy * ddy).sqrt() < 0.05;
                        }
                        let t = ((px - ax) * dx + (py - ay) * dy) / len2;
                        let t = t.clamp(0.0, 1.0);
                        let cx = ax + t * dx;
                        let cy = ay + t * dy;
                        let ddx = px - cx;
                        let ddy = py - cy;
                        (ddx * ddx + ddy * ddy).sqrt() < 0.05
                    }
                    let hit = snapshot.wires.iter().find_map(|w| {
                        if point_on_segment(hit_x, hit_y, w.start.x, w.start.y, w.end.x, w.end.y) {
                            Some(w.uuid)
                        } else {
                            None
                        }
                    });
                    // Net membership comes from the authoritative
                    // connectivity core (signex-net), the same one
                    // build_netlist / ERC read. It buckets at 1 µm and
                    // resolves T-junctions by an interior on-segment
                    // test, so the highlight follows the real net —
                    // unlike the old inline union-find here, which
                    // bucketed at 0.01 mm (bleeding across nearly
                    // coincident nets) and only tied junctions sitting
                    // exactly on a wire endpoint (missing true Ts).
                    match hit {
                        Some(wire_uuid) => {
                            match signex_net::flood_net_elements(snapshot, wire_uuid) {
                                Some(f) => f.wires.into_iter().chain(f.junctions).collect(),
                                None => Vec::new(),
                            }
                        }
                        None => Vec::new(),
                    }
                }
            };
            if !net_wire_uuids.is_empty() {
                // Dry-run the flood to see if anything will
                // actually change; only snapshot + apply when
                // there's a real diff. Clicking the same net
                // twice with the same colour was double-pushing
                // identical snapshots, forcing the user to hit
                // Ctrl+Z twice for a single visible change.
                let mut diff = false;
                for uuid in &net_wire_uuids {
                    if pending.a == 0 {
                        if self.ui_state.wire_color_overrides.contains_key(uuid) {
                            diff = true;
                            break;
                        }
                    } else {
                        match self.ui_state.wire_color_overrides.get(uuid) {
                            Some(c) if *c == pending => {}
                            _ => {
                                diff = true;
                                break;
                            }
                        }
                    }
                }
                if diff {
                    self.ui_state
                        .net_color_undo
                        .push(self.ui_state.wire_color_overrides.clone());
                    for uuid in net_wire_uuids {
                        if pending.a == 0 {
                            self.ui_state.wire_color_overrides.remove(&uuid);
                        } else {
                            self.ui_state.wire_color_overrides.insert(uuid, pending);
                        }
                    }
                    self.interaction_state
                        .active_canvas_mut()
                        .wire_color_overrides = self.ui_state.wire_color_overrides.clone();
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_content_cache();
                }
            }
            // Altium-style continuous placement: stay armed
            // until the user right-clicks or presses Escape.
            // Empty-canvas clicks are no-ops; wire clicks keep
            // the pen ready for the next net. Return early so
            // the normal selection path doesn't run and pick
            // up the clicked wire as a selection.
            return Task::none();
        }
        // Z-order reference picker: the user previously chose
        // Bring-To-Front-Of / Send-To-Back-Of on the Active Bar
        // and is now clicking a reference item. Resolve the hit,
        // emit a Reorder command, and clear the picker — matches
        // Altium's "click the object to reference" convention.
        if let Some(picker) = self.ui_state.reorder_picker {
            if let Some(snapshot) = self.active_render_snapshot() {
                let hit = crate::schematic_runtime::hit_test::hit_test(snapshot, world_x, world_y);
                if let Some(reference) = hit {
                    let direction = match picker {
                        super::super::super::state::ReorderPicker::Above => {
                            signex_engine::ReorderDirection::JustAbove(reference.uuid)
                        }
                        super::super::super::state::ReorderPicker::Below => {
                            signex_engine::ReorderDirection::JustBelow(reference.uuid)
                        }
                    };
                    let items = self.interaction_state.active_canvas_mut().selected.clone();
                    self.apply_engine_command(
                        signex_engine::Command::ReorderObjects { items, direction },
                        false,
                        true,
                    );
                }
            }
            self.ui_state.reorder_picker = None;
            self.interaction_state
                .active_canvas_mut()
                .reorder_picker_armed = false;
            self.interaction_state
                .active_canvas_mut()
                .clear_overlay_cache();
            return Task::none();
        }

        // Altium-style placement pause: while the ghost is frozen
        // (TAB pressed, pre-placement form open), canvas clicks are
        // suppressed so the user can edit properties without a
        // stray click dropping an object. `pre_placement` lives on
        // past Resume so the first click after Resume inherits the
        // edited defaults — so key the gate on `placement_paused`,
        // not on `pre_placement.is_some()`.
        if self.interaction_state.active_canvas_mut().placement_paused {
            return Task::none();
        }
        // A click outside the inline editor commits its current value
        // and returns to normal selection — same convention as most
        // IDEs (Escape cancels, click elsewhere confirms).
        if let Some(state) = self.interaction_state.editing_text.take() {
            if state.text != state.original_text {
                let stored = crate::schematic_runtime::text::escape_for_standard(&state.text);
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
            Tool::Wire => {
                let pt = signex_types::schematic::Point::new(wx, wy);
                if !self.interaction_state.wire_drawing {
                    self.interaction_state.wire_drawing = true;
                    self.interaction_state.wire_points.clear();
                    self.interaction_state.wire_points.push(pt);
                    self.interaction_state.active_canvas_mut().wire_preview =
                        self.interaction_state.wire_points.clone();
                    self.interaction_state.active_canvas_mut().drawing_mode = true;
                    self.interaction_state.active_canvas_mut().draw_mode =
                        self.interaction_state.draw_mode;
                    self.interaction_state.active_canvas_mut().tool_preview = None;
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
                    self.interaction_state.active_canvas_mut().wire_preview = vec![end_pt];
                }
            }
            Tool::Bus => {
                let pt = signex_types::schematic::Point::new(wx, wy);
                if !self.interaction_state.wire_drawing {
                    self.interaction_state.wire_drawing = true;
                    self.interaction_state.wire_points.clear();
                    self.interaction_state.wire_points.push(pt);
                    self.interaction_state.active_canvas_mut().wire_preview =
                        self.interaction_state.wire_points.clone();
                    self.interaction_state.active_canvas_mut().drawing_mode = true;
                    self.interaction_state.active_canvas_mut().draw_mode =
                        self.interaction_state.draw_mode;
                    self.interaction_state.active_canvas_mut().tool_preview = None;
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
                    self.interaction_state.active_canvas_mut().wire_preview = vec![end_pt];
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
                            font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
                            justify_h: signex_types::schematic::HAlign::Center,
                            justify_v: signex_types::schematic::VAlign::default(),
                            hidden: false,
                        }),
                        fields_autoplaced: true,
                        fields_user_placed: false,
                        dnp: false,
                        in_bom: false,
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
            Tool::Line => {
                // Two-click: anchor → commit. First click
                // seeds shape_anchor; second click emits
                // SchDrawing::Line and re-arms for the next
                // line (matches Altium's stay-in-tool flow).
                let (pp_w, _) = pre_placement_shape(&self.document_state);
                let p = signex_types::schematic::Point::new(wx, wy);
                match self.interaction_state.shape_anchor.take() {
                    None => {
                        self.interaction_state.shape_anchor = Some(p);
                        self.interaction_state.active_canvas_mut().shape_anchor =
                            Some((p, crate::canvas::ShapePreviewKind::Line));
                        self.interaction_state
                            .active_canvas_mut()
                            .clear_overlay_cache();
                    }
                    Some(start) => {
                        let drawing = signex_types::schematic::SchDrawing::Line {
                            uuid: uuid::Uuid::new_v4(),
                            start,
                            end: p,
                            width: pp_w,
                            stroke_color: None,
                        };
                        self.apply_engine_command(
                            signex_engine::Command::PlaceSchDrawing { drawing },
                            false,
                            false,
                        );
                        // Re-arm with the just-placed endpoint
                        // as the new anchor so chained segments
                        // share vertices.
                        self.interaction_state.shape_anchor = Some(p);
                        self.interaction_state.active_canvas_mut().shape_anchor =
                            Some((p, crate::canvas::ShapePreviewKind::Line));
                        self.interaction_state
                            .active_canvas_mut()
                            .clear_overlay_cache();
                    }
                }
            }
            Tool::Rectangle => {
                // Two-click: corner → opposite-corner →
                // SchDrawing::Rect. Rearms blank for the
                // next rect.
                let (pp_w, pp_fill) = pre_placement_shape(&self.document_state);
                let p = signex_types::schematic::Point::new(wx, wy);
                match self.interaction_state.shape_anchor.take() {
                    None => {
                        self.interaction_state.shape_anchor = Some(p);
                        self.interaction_state.active_canvas_mut().shape_anchor =
                            Some((p, crate::canvas::ShapePreviewKind::Rect));
                        self.interaction_state
                            .active_canvas_mut()
                            .clear_overlay_cache();
                    }
                    Some(start) => {
                        let drawing = signex_types::schematic::SchDrawing::Rect {
                            uuid: uuid::Uuid::new_v4(),
                            start,
                            end: p,
                            width: pp_w,
                            fill: pp_fill,
                            stroke_color: None,
                        };
                        self.apply_engine_command(
                            signex_engine::Command::PlaceSchDrawing { drawing },
                            false,
                            false,
                        );
                        self.interaction_state.shape_anchor = None;
                        self.interaction_state.active_canvas_mut().shape_anchor = None;
                        self.interaction_state
                            .active_canvas_mut()
                            .clear_overlay_cache();
                    }
                }
            }
            Tool::Circle => {
                // Two-click: center → edge-point → commit as
                // SchDrawing::Circle with radius = |edge-center|.
                let (pp_w, pp_fill) = pre_placement_shape(&self.document_state);
                let p = signex_types::schematic::Point::new(wx, wy);
                match self.interaction_state.shape_anchor.take() {
                    None => {
                        self.interaction_state.shape_anchor = Some(p);
                        self.interaction_state.active_canvas_mut().shape_anchor =
                            Some((p, crate::canvas::ShapePreviewKind::Circle));
                        self.interaction_state
                            .active_canvas_mut()
                            .clear_overlay_cache();
                    }
                    Some(center) => {
                        let dx = p.x - center.x;
                        let dy = p.y - center.y;
                        let radius = (dx * dx + dy * dy).sqrt();
                        if radius > 0.01 {
                            let drawing = signex_types::schematic::SchDrawing::Circle {
                                uuid: uuid::Uuid::new_v4(),
                                center,
                                radius,
                                width: pp_w,
                                fill: pp_fill,
                                stroke_color: None,
                            };
                            self.apply_engine_command(
                                signex_engine::Command::PlaceSchDrawing { drawing },
                                false,
                                false,
                            );
                        }
                        self.interaction_state.shape_anchor = None;
                        self.interaction_state.active_canvas_mut().shape_anchor = None;
                        self.interaction_state
                            .active_canvas_mut()
                            .clear_overlay_cache();
                    }
                }
            }
            Tool::Arc => {
                // 3-click arc: start → mid → end.
                let (pp_w, pp_fill) = pre_placement_shape(&self.document_state);
                let p = signex_types::schematic::Point::new(wx, wy);
                self.interaction_state.arc_points.push(p);
                if self.interaction_state.arc_points.len() >= 3 {
                    let pts = std::mem::take(&mut self.interaction_state.arc_points);
                    let drawing = signex_types::schematic::SchDrawing::Arc {
                        uuid: uuid::Uuid::new_v4(),
                        start: pts[0],
                        mid: pts[1],
                        end: pts[2],
                        width: pp_w,
                        fill: pp_fill,
                        stroke_color: None,
                    };
                    self.apply_engine_command(
                        signex_engine::Command::PlaceSchDrawing { drawing },
                        false,
                        false,
                    );
                }
                self.interaction_state.active_canvas_mut().arc_points =
                    self.interaction_state.arc_points.clone();
                self.interaction_state
                    .active_canvas_mut()
                    .clear_overlay_cache();
            }
            Tool::Polyline => {
                // Click-by-click polyline. Enter / double-click commits.
                let p = signex_types::schematic::Point::new(wx, wy);
                self.interaction_state.polyline_points.push(p);
                self.interaction_state.active_canvas_mut().polyline_points =
                    self.interaction_state.polyline_points.clone();
                self.interaction_state
                    .active_canvas_mut()
                    .clear_overlay_cache();
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
                    font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
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
                let (label_type, shape, default_text) =
                    if let Some((lt, shape)) = self.interaction_state.pending_port.clone() {
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
                let (pp_rot, pp_fs_mm, pp_justify_h, pp_justify_v) = self
                    .document_state
                    .panel_ctx
                    .pre_placement
                    .as_ref()
                    .map(|pp| {
                        (
                            pp.rotation,
                            pp.font_size_pt as f64 * signex_types::schematic::SCHEMATIC_PT_TO_MM,
                            pp.justify_h,
                            pp.justify_v,
                        )
                    })
                    .unwrap_or((
                        0.0,
                        signex_types::schematic::SCHEMATIC_TEXT_MM,
                        signex_types::schematic::HAlign::Left,
                        signex_types::schematic::VAlign::Bottom,
                    ));
                let label = signex_types::schematic::Label {
                    uuid: uuid::Uuid::new_v4(),
                    text,
                    position: signex_types::schematic::Point::new(wx, wy),
                    rotation: pp_rot,
                    label_type,
                    shape,
                    font_size: pp_fs_mm,
                    justify: pp_justify_h,
                    justify_v: pp_justify_v,
                };
                self.apply_engine_command(
                    signex_engine::Command::PlaceLabel { label },
                    false,
                    false,
                );
            }
            _ => {
                return self.handle_selection_request(selection_request::SelectionRequest::HitAt {
                    world_x,
                    world_y,
                });
            }
        }
        Task::none()
    }
}
