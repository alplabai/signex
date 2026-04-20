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
        // Modal drag — accumulate delta into the per-modal offset so the
        // dialog slides under the cursor.
        if let Some((modal, last_x, last_y)) = self.ui_state.modal_dragging {
            let dx = x - last_x;
            let dy = y - last_y;
            let entry = self
                .ui_state
                .modal_offsets
                .entry(modal)
                .or_insert((0.0, 0.0));
            entry.0 += dx;
            entry.1 += dy;
            self.ui_state.modal_dragging = Some((modal, x, y));
        }
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
            // Only undock when the drag clearly exits the tab strip.
            // Tabs sit at the top/bottom of each region at ~28 px tall,
            // so requiring vertical movement > 28 px keeps horizontal
            // drags within the same bar free for reorder. Also bump
            // the overall threshold so a slightly-wobbly intra-strip
            // reorder doesn't undock by accident.
            let moved_far = (dx * dx + dy * dy).sqrt() > 60.0;
            let left_strip = dy.abs() > 28.0;
            if moved_far && left_strip {
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

    /// Returns the tab index to undock when the user is dragging a
    /// document tab and the cursor crosses the main window boundary.
    pub(crate) fn check_tab_auto_detach(&self, cursor_x: f32, cursor_y: f32) -> Option<usize> {
        let (idx, _, _) = self.ui_state.tab_dragging?;
        // Skip if this tab is already undocked (owned by another window).
        let tab = self.document_state.tabs.get(idx)?;
        if self.ui_state.windows.values().any(
            |k| matches!(k, super::super::state::WindowKind::UndockedTab { path, .. } if path == &tab.path),
        ) {
            return None;
        }
        let (ww, wh) = self.ui_state.window_size;
        const EDGE_THRESHOLD: f32 = 12.0;
        let past = cursor_x < -EDGE_THRESHOLD
            || cursor_x > ww + EDGE_THRESHOLD
            || cursor_y < -EDGE_THRESHOLD
            || cursor_y > wh + EDGE_THRESHOLD;
        if past { Some(idx) } else { None }
    }

    /// Scan the floating-panel list for one whose drag just crossed the
    /// main window boundary. Returns the index into `dock.floating` so
    /// the dispatcher can chain a `DetachFloatingPanel(idx)` task.
    pub(crate) fn check_floating_panel_auto_detach(
        &self,
        cursor_x: f32,
        cursor_y: f32,
    ) -> Option<usize> {
        let (ww, wh) = self.ui_state.window_size;
        const EDGE_THRESHOLD: f32 = 12.0;
        let past = cursor_x < -EDGE_THRESHOLD
            || cursor_x > ww + EDGE_THRESHOLD
            || cursor_y < -EDGE_THRESHOLD
            || cursor_y > wh + EDGE_THRESHOLD;
        if !past {
            return None;
        }
        self.document_state
            .dock
            .floating
            .iter()
            .position(|fp| fp.dragging)
    }

    /// Altium-style auto-detach. While the user drags a modal's title
    /// bar, watch the cursor; if it crosses the main window boundary by
    /// more than `EDGE_THRESHOLD`, pop the modal out into its own OS
    /// window. Returns the modal that should detach, if any, so the
    /// dispatcher can chain a `DetachModal` task onto the DragMove path.
    pub(crate) fn check_modal_auto_detach(
        &self,
        cursor_x: f32,
        cursor_y: f32,
    ) -> Option<super::super::state::ModalId> {
        let (modal, _, _) = self.ui_state.modal_dragging?;
        // Skip if it's already detached — another path owns it now.
        if self
            .ui_state
            .windows
            .values()
            .any(|k| matches!(k, super::super::state::WindowKind::DetachedModal(m) if *m == modal))
        {
            return None;
        }
        let (ww, wh) = self.ui_state.window_size;
        // Dead zone so a brief accidental graze doesn't flip the modal out.
        const EDGE_THRESHOLD: f32 = 12.0;
        let past_left = cursor_x < -EDGE_THRESHOLD;
        let past_right = cursor_x > ww + EDGE_THRESHOLD;
        let past_top = cursor_y < -EDGE_THRESHOLD;
        let past_bottom = cursor_y > wh + EDGE_THRESHOLD;
        if past_left || past_right || past_top || past_bottom {
            Some(modal)
        } else {
            None
        }
    }

    pub(crate) fn handle_layout_drag_finished(&mut self) {
        if self.interaction_state.dragging.is_some() {
            crate::diagnostics::log_debug("[drag] END");
            self.interaction_state.dragging = None;
            self.interaction_state.drag_start_pos = None;
            self.ui_state.tab_dragging = None;
            return;
        }

        self.document_state.dock.tab_drag = None;
        self.interaction_state.tab_drag_origin = None;
        self.ui_state.tab_dragging = None;
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
                // Lasso auto-sample: while a lasso is anchored (>= 1
                // vertex already committed), sample a new vertex each
                // time the cursor moves more than SAMPLE_MIN from the
                // last recorded point. Produces a freehand polyline
                // between the two clicks (Altium's behaviour).
                // Sample at a constant ~6 screen pixels so the lasso
                // tracks the cursor at all zoom levels. At 100% zoom
                // (scale=3 px/mm) that's 2.0 mm world; at 50% zoom
                // we need 4.0 mm world for the same screen distance.
                // `zoom_pct` comes from `camera.zoom_percent()` which
                // is `scale/3 * 100`, so mm-per-6px = 200 / zoom_pct.
                const SAMPLE_MIN_PX: f64 = 6.0;
                let sample_min_mm = if zoom_pct > 1.0 {
                    (SAMPLE_MIN_PX * 100.0) / (zoom_pct * 3.0)
                } else {
                    2.0
                };
                let sampled = if let Some(pts) = self.ui_state.lasso_polygon.as_mut()
                    && let Some(&last) = pts.last()
                {
                    let dx = x as f64 - last.x;
                    let dy = y as f64 - last.y;
                    if (dx * dx + dy * dy).sqrt() >= sample_min_mm {
                        pts.push(signex_types::schematic::Point::new(x as f64, y as f64));
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if sampled {
                    self.sync_lasso_polygon_to_canvas();
                }
            }
            CanvasEvent::Clicked { world_x, world_y } => {
                // Altium-style lasso: first click anchors the start,
                // the cursor path auto-samples vertices, a second
                // click closes the polygon and commits. Escape /
                // right-click cancels.
                if self.ui_state.lasso_polygon.is_some() {
                    let (vx, vy) = if self.interaction_state.canvas.snap_enabled
                        && self.interaction_state.canvas.snap_grid_mm > 0.0
                    {
                        let g = self.interaction_state.canvas.snap_grid_mm;
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
                            self.interaction_state.canvas.selected =
                                signex_render::schematic::hit_test::hit_test_polygon(
                                    snapshot, &poly,
                                )
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
                // .kicad_sch round-trips unchanged — KiCad has no
                // notion of per-wire override colours.
                if let Some(pending) = self.ui_state.pending_net_color {
                    // Snap the click point to the grid before hit
                    // testing so the click lands where the pen ghost
                    // previewed. Without this the cursor's raw world
                    // coords can land a full grid cell off the nearest
                    // wire — the visible pen snaps, but the hit target
                    // was being missed.
                    let (hit_x, hit_y) = if self.interaction_state.canvas.snap_enabled
                        && self.interaction_state.canvas.snap_grid_mm > 0.0
                    {
                        let g = self.interaction_state.canvas.snap_grid_mm;
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
                                if point_on_segment(
                                    hit_x, hit_y, w.start.x, w.start.y, w.end.x, w.end.y,
                                ) {
                                    Some(signex_types::schematic::SelectedItem::new(
                                        w.uuid,
                                        signex_types::schematic::SelectedKind::Wire,
                                    ))
                                } else {
                                    None
                                }
                            });
                            match hit {
                                Some(h)
                                    if h.kind == signex_types::schematic::SelectedKind::Wire =>
                                {
                                    fn q(p: &signex_types::schematic::Point) -> (i64, i64) {
                                        ((p.x * 100.0).round() as i64, (p.y * 100.0).round() as i64)
                                    }
                                    fn find(
                                        parent: &mut std::collections::HashMap<
                                            (i64, i64),
                                            (i64, i64),
                                        >,
                                        x: (i64, i64),
                                    ) -> (i64, i64) {
                                        let p = *parent.entry(x).or_insert(x);
                                        if p == x {
                                            x
                                        } else {
                                            let r = find(parent, p);
                                            parent.insert(x, r);
                                            r
                                        }
                                    }
                                    fn union(
                                        parent: &mut std::collections::HashMap<
                                            (i64, i64),
                                            (i64, i64),
                                        >,
                                        a: (i64, i64),
                                        b: (i64, i64),
                                    ) {
                                        let ra = find(parent, a);
                                        let rb = find(parent, b);
                                        if ra != rb {
                                            parent.insert(ra, rb);
                                        }
                                    }
                                    let mut parent: std::collections::HashMap<
                                        (i64, i64),
                                        (i64, i64),
                                    > = std::collections::HashMap::new();
                                    for w in &snapshot.wires {
                                        union(&mut parent, q(&w.start), q(&w.end));
                                    }
                                    match snapshot.wires.iter().find(|w| w.uuid == h.uuid) {
                                        None => Vec::new(),
                                        Some(hw) => {
                                            let root = find(&mut parent, q(&hw.start));
                                            let mut ids: Vec<uuid::Uuid> = snapshot
                                                .wires
                                                .iter()
                                                .filter(|w| find(&mut parent, q(&w.start)) == root)
                                                .map(|w| w.uuid)
                                                .collect();
                                            // Junctions on this net should follow
                                            // the colour — a junction's position
                                            // is always a wire endpoint, so the
                                            // union-find root test works with the
                                            // same parent map.
                                            for j in &snapshot.junctions {
                                                let jq = q(&j.position);
                                                if parent.contains_key(&jq)
                                                    && find(&mut parent, jq) == root
                                                {
                                                    ids.push(j.uuid);
                                                }
                                            }
                                            ids
                                        }
                                    }
                                }
                                _ => Vec::new(),
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
                            self.interaction_state.canvas.wire_color_overrides =
                                self.ui_state.wire_color_overrides.clone();
                            self.interaction_state.canvas.clear_content_cache();
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
                        let hit = signex_render::schematic::hit_test::hit_test(
                            snapshot, world_x, world_y,
                        );
                        if let Some(reference) = hit {
                            let direction = match picker {
                                super::super::state::ReorderPicker::Above => {
                                    signex_engine::ReorderDirection::JustAbove(reference.uuid)
                                }
                                super::super::state::ReorderPicker::Below => {
                                    signex_engine::ReorderDirection::JustBelow(reference.uuid)
                                }
                            };
                            let items = self.interaction_state.canvas.selected.clone();
                            self.apply_engine_command(
                                signex_engine::Command::ReorderObjects { items, direction },
                                false,
                                true,
                            );
                        }
                    }
                    self.ui_state.reorder_picker = None;
                    self.interaction_state.canvas.reorder_picker_armed = false;
                    self.interaction_state.canvas.clear_overlay_cache();
                    return Task::none();
                }

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
                                    font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
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
                    Tool::Line => {
                        // Two-click: anchor → commit. First click
                        // seeds shape_anchor; second click emits
                        // SchDrawing::Line and re-arms for the next
                        // line (matches Altium's stay-in-tool flow).
                        let p = signex_types::schematic::Point::new(wx, wy);
                        match self.interaction_state.shape_anchor.take() {
                            None => {
                                self.interaction_state.shape_anchor = Some(p);
                                self.interaction_state.canvas.shape_anchor =
                                    Some((p, crate::canvas::ShapePreviewKind::Line));
                                self.interaction_state.canvas.clear_overlay_cache();
                            }
                            Some(start) => {
                                let drawing = signex_types::schematic::SchDrawing::Line {
                                    uuid: uuid::Uuid::new_v4(),
                                    start,
                                    end: p,
                                    width: 0.0,
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
                                self.interaction_state.canvas.shape_anchor =
                                    Some((p, crate::canvas::ShapePreviewKind::Line));
                                self.interaction_state.canvas.clear_overlay_cache();
                            }
                        }
                    }
                    Tool::Rectangle => {
                        // Two-click: corner → opposite-corner →
                        // SchDrawing::Rect. Rearms blank for the
                        // next rect.
                        let p = signex_types::schematic::Point::new(wx, wy);
                        match self.interaction_state.shape_anchor.take() {
                            None => {
                                self.interaction_state.shape_anchor = Some(p);
                                self.interaction_state.canvas.shape_anchor =
                                    Some((p, crate::canvas::ShapePreviewKind::Rect));
                                self.interaction_state.canvas.clear_overlay_cache();
                            }
                            Some(start) => {
                                let drawing = signex_types::schematic::SchDrawing::Rect {
                                    uuid: uuid::Uuid::new_v4(),
                                    start,
                                    end: p,
                                    width: 0.0,
                                    fill: signex_types::schematic::FillType::default(),
                                };
                                self.apply_engine_command(
                                    signex_engine::Command::PlaceSchDrawing { drawing },
                                    false,
                                    false,
                                );
                                self.interaction_state.shape_anchor = None;
                                self.interaction_state.canvas.shape_anchor = None;
                                self.interaction_state.canvas.clear_overlay_cache();
                            }
                        }
                    }
                    Tool::Circle => {
                        // Two-click: center → edge-point → commit as
                        // SchDrawing::Circle with radius = |edge-center|.
                        let p = signex_types::schematic::Point::new(wx, wy);
                        match self.interaction_state.shape_anchor.take() {
                            None => {
                                self.interaction_state.shape_anchor = Some(p);
                                self.interaction_state.canvas.shape_anchor =
                                    Some((p, crate::canvas::ShapePreviewKind::Circle));
                                self.interaction_state.canvas.clear_overlay_cache();
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
                                        width: 0.0,
                                        fill: signex_types::schematic::FillType::default(),
                                    };
                                    self.apply_engine_command(
                                        signex_engine::Command::PlaceSchDrawing { drawing },
                                        false,
                                        false,
                                    );
                                }
                                self.interaction_state.shape_anchor = None;
                                self.interaction_state.canvas.shape_anchor = None;
                                self.interaction_state.canvas.clear_overlay_cache();
                            }
                        }
                    }
                    Tool::Arc => {
                        // 3-click arc: start → mid → end.
                        let p = signex_types::schematic::Point::new(wx, wy);
                        self.interaction_state.arc_points.push(p);
                        if self.interaction_state.arc_points.len() >= 3 {
                            let pts = std::mem::take(&mut self.interaction_state.arc_points);
                            let drawing = signex_types::schematic::SchDrawing::Arc {
                                uuid: uuid::Uuid::new_v4(),
                                start: pts[0],
                                mid: pts[1],
                                end: pts[2],
                                width: 0.0,
                                fill: signex_types::schematic::FillType::default(),
                            };
                            self.apply_engine_command(
                                signex_engine::Command::PlaceSchDrawing { drawing },
                                false,
                                false,
                            );
                        }
                        self.interaction_state.canvas.arc_points =
                            self.interaction_state.arc_points.clone();
                        self.interaction_state.canvas.clear_overlay_cache();
                    }
                    Tool::Polyline => {
                        // Click-by-click polyline. Enter / double-click commits.
                        let p = signex_types::schematic::Point::new(wx, wy);
                        self.interaction_state.polyline_points.push(p);
                        self.interaction_state.canvas.polyline_points =
                            self.interaction_state.polyline_points.clone();
                        self.interaction_state.canvas.clear_overlay_cache();
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
                            .map(|pp| {
                                (
                                    pp.rotation,
                                    pp.font_size_pt as f64
                                        * signex_types::schematic::SCHEMATIC_PT_TO_MM,
                                    pp.justify_h,
                                )
                            })
                            .unwrap_or((
                                0.0,
                                signex_types::schematic::SCHEMATIC_TEXT_MM,
                                signex_types::schematic::HAlign::Left,
                            ));
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
                // Lasso already commits on the second single-click
                // (see CanvasEvent::Clicked above), so by the time
                // a DoubleClicked fires the polygon is already
                // consumed. Fall through to the wire-drawing /
                // inline-edit branches below.
                //
                // Polyline closes on double-click: the first click of
                // the double already appended a vertex (see Clicked
                // handler), so we just commit whatever's in the buffer
                // — minimum 2 points for a valid polyline.
                if self.interaction_state.current_tool == Tool::Polyline
                    && self.interaction_state.polyline_points.len() >= 2
                {
                    let pts = std::mem::take(&mut self.interaction_state.polyline_points);
                    let drawing = signex_types::schematic::SchDrawing::Polyline {
                        uuid: uuid::Uuid::new_v4(),
                        points: pts,
                        width: 0.0,
                        fill: signex_types::schematic::FillType::default(),
                    };
                    self.apply_engine_command(
                        signex_engine::Command::PlaceSchDrawing { drawing },
                        false,
                        false,
                    );
                    self.interaction_state.canvas.polyline_points.clear();
                    self.interaction_state.canvas.clear_overlay_cache();
                    return Task::none();
                }
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
