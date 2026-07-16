//! Left-press per-tool gesture arms — the `ButtonPressed(Left)` branch
//! of the canvas `Program::update` god-function, extracted verbatim as
//! an `impl SymbolCanvas` method. Select-tool hit-testing (resize
//! handles, objects, rubber-band start) and every placement tool keep
//! identical conditions, coordinate math, and `Action` capture/publish
//! sites.

use super::super::*;
use iced::Rectangle;
use iced::mouse;
use iced::widget::canvas;

impl SymbolCanvas<'_> {
    /// Handle a left mouse-button press: Select-tool hit-testing or a
    /// placement-tool gesture, depending on `self.tool`.
    pub(in crate::library::editor::symbol::canvas) fn on_left_press(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasAction>> {
        let pos = cursor.position_in(bounds)?;
        // Placement tools need grid-snapped coordinates; hit-testing for
        // the Select tool must use the raw (unsnapped) cursor position so
        // that objects not sitting exactly on the snap grid can still be
        // clicked. We compute both up-front and choose per tool arm.
        let (wx, wy) = world_for(self, pos.x, pos.y, bounds);
        let (ux, uy) = world_unsnapped(self, pos.x, pos.y, bounds);
        match self.tool {
            SymbolTool::Select => {
                // Resize handles win over everything else.
                // Use a screen-pixel-based tolerance so handles are
                // equally easy to hit at any zoom level.
                let tol_mm = (8.0_f32 / self.camera.scale.max(0.01)).clamp(0.5, 4.0) as f64;
                if let Some((idx, handle)) =
                    state::hit_test_graphic_handle(self.symbol, ux, uy, tol_mm, self.active_part)
                {
                    state.dragging_handle = Some((idx, handle));
                    state.dragging = false;
                    state.drag_anchor_offset = None;
                    state.last_drag_world_pos = None;
                    state.box_select_origin = None;
                    state.box_select_current = None;
                    return Some(canvas::Action::capture());
                }
                // Pin tip + graphic hit-test wins; a click on a pin's
                // name/number label is a fallback so the pin is grabbable
                // by its text, not only its ~1.5 mm tip.
                let sel = state::hit_test(self.symbol, ux, uy, self.active_part).or_else(|| {
                    self.pin_hit_by_label(ux, uy)
                        .map(state::SymbolSelection::Pin)
                });
                if let Some(sel) = sel {
                    state.box_select_origin = None;
                    state.box_select_current = None;

                    // If the clicked item is inside the current Multiple /
                    // All selection, drag the whole group as a unit.
                    let in_group = self
                        .selected
                        .as_ref()
                        .map_or(false, |s| item_in_selection(s, &sel));
                    if in_group {
                        state.dragging = true;
                        state.last_drag_world_pos = Some((wx, wy));
                        state.drag_anchor_offset = None;
                        return Some(canvas::Action::capture());
                    }

                    let effective_sel = sel;

                    let is_delta = matches!(effective_sel, SymbolSelection::All);
                    state.dragging = true;
                    state.last_drag_world_pos = if is_delta { Some((wx, wy)) } else { None };
                    state.drag_anchor_offset = selection_anchor(self.symbol, &effective_sel)
                        .map(|(ax, ay)| (ax - wx, ay - wy));

                    if self.selected.as_ref() == Some(&effective_sel) {
                        return Some(canvas::Action::capture());
                    }
                    Some(canvas::Action::publish(CanvasAction::Select(effective_sel)).and_capture())
                } else {
                    // Empty space — start a rubber-band box selection.
                    // Use unsnapped coords so box corners track the pointer exactly.
                    state.dragging = false;
                    state.drag_anchor_offset = None;
                    state.last_drag_world_pos = None;
                    state.box_select_origin = Some((ux, uy));
                    state.box_select_current = Some((ux, uy));
                    Some(canvas::Action::capture())
                }
            }
            SymbolTool::AddPin => {
                Some(canvas::Action::publish(CanvasAction::AddPin { x: wx, y: wy }).and_capture())
            }
            SymbolTool::PlaceRectangle => {
                if let Some((from_x, from_y)) = state.rect_from.take() {
                    // Second click — commit the rectangle.
                    state.rect_cursor = None;
                    Some(
                        canvas::Action::publish(CanvasAction::AddRectangle {
                            from_x,
                            from_y,
                            to_x: wx,
                            to_y: wy,
                        })
                        .and_capture(),
                    )
                } else {
                    // First click — set the first corner and wait.
                    state.rect_from = Some((wx, wy));
                    state.rect_cursor = Some((wx, wy));
                    Some(canvas::Action::capture())
                }
            }
            SymbolTool::PlaceLine => {
                if let Some((from_x, from_y)) = state.line_from.take() {
                    // Second click — commit the line.
                    state.line_cursor = None;
                    Some(
                        canvas::Action::publish(CanvasAction::AddLine {
                            from_x,
                            from_y,
                            to_x: wx,
                            to_y: wy,
                        })
                        .and_capture(),
                    )
                } else {
                    // First click — set the start point and wait.
                    state.line_from = Some((wx, wy));
                    state.line_cursor = Some((wx, wy));
                    Some(canvas::Action::capture())
                }
            }
            SymbolTool::PlaceCircle => {
                if let Some((center_x, center_y)) = state.circle_center.take() {
                    // Second click — commit the circle.
                    let dx = wx - center_x;
                    let dy = wy - center_y;
                    let radius = (dx * dx + dy * dy).sqrt().max(0.1);
                    state.circle_cursor = None;
                    Some(
                        canvas::Action::publish(CanvasAction::AddCircle {
                            cx: center_x,
                            cy: center_y,
                            radius,
                        })
                        .and_capture(),
                    )
                } else {
                    // First click — set the center and wait.
                    state.circle_center = Some((wx, wy));
                    state.circle_cursor = Some((wx, wy));
                    Some(canvas::Action::capture())
                }
            }
            SymbolTool::PlaceArc => {
                if let Some((radius, start_deg)) = state.arc_radius_start.take() {
                    // Third click — commit the arc.
                    let (cx, cy) = state.arc_center.take().unwrap_or((wx, wy));
                    state.arc_cursor = None;
                    // Use the unwrapped end angle so arcs that swept
                    // past ±180° are stored correctly. Fall back to a
                    // fresh atan2 only if the cursor never moved after
                    // the second click.
                    let end_deg = state.arc_end_deg_unwrapped.take().unwrap_or_else(|| {
                        let dx = wx - cx;
                        let dy = wy - cy;
                        dy.atan2(dx).to_degrees()
                    });
                    Some(
                        canvas::Action::publish(CanvasAction::AddArc {
                            cx,
                            cy,
                            radius,
                            start_deg,
                            end_deg,
                        })
                        .and_capture(),
                    )
                } else if let Some((cx, cy)) = state.arc_center {
                    // Second click — define radius + start angle.
                    let dx = wx - cx;
                    let dy = wy - cy;
                    let radius = (dx * dx + dy * dy).sqrt().max(0.1);
                    let start_deg = dy.atan2(dx).to_degrees();
                    state.arc_radius_start = Some((radius, start_deg));
                    // Seed the unwrapped tracker at the start angle so
                    // the first CursorMoved won't produce a large jump.
                    state.arc_end_deg_unwrapped = Some(start_deg);
                    Some(canvas::Action::capture())
                } else {
                    // First click — set the center and wait.
                    state.arc_center = Some((wx, wy));
                    state.arc_cursor = Some((wx, wy));
                    Some(canvas::Action::capture())
                }
            }
            SymbolTool::PlaceText => {
                Some(canvas::Action::publish(CanvasAction::AddText { x: wx, y: wy }).and_capture())
            }
            SymbolTool::PlacePolygon => self.on_polygon_click(state, wx, wy, ux, uy),
        }
    }

    /// Click-collect gesture for the `PlacePolygon` tool. Each click
    /// normally appends a snapped vertex to `state.polygon_vertices`;
    /// two close gestures are checked first so they don't append a
    /// duplicate vertex:
    ///
    /// 1. A click on the already-placed first vertex (hit-tolerance
    ///    based, using the unsnapped cursor so it feels the same at
    ///    every zoom level — mirrors `hit_test_graphic_handle`'s
    ///    tolerance derivation).
    /// 2. A double-click (300 ms / 3 mm — the same heuristic the
    ///    schematic canvas uses for its own double-click detection).
    ///
    /// Both close gestures commit only once >= 3 vertices are
    /// collected; below that they're a no-op (kept collecting)
    /// rather than a discard — Esc / tool-switch are the discard
    /// paths (see `on_key_pressed` / `sync_polygon_tool`).
    fn on_polygon_click(
        &self,
        state: &mut CanvasState,
        wx: f64,
        wy: f64,
        ux: f64,
        uy: f64,
    ) -> Option<canvas::Action<CanvasAction>> {
        // Close gesture 1: click on the first vertex.
        if state.polygon_vertices.len() >= 3
            && let Some(&(fx, fy)) = state.polygon_vertices.first()
        {
            let tol_mm = (8.0_f32 / self.camera.scale.max(0.01)).clamp(0.5, 4.0) as f64;
            let dx = ux - fx;
            let dy = uy - fy;
            if dx * dx + dy * dy <= tol_mm * tol_mm {
                return Some(commit_polygon(state));
            }
        }

        // Close gesture 2: double-click. Detected before appending so
        // the second click of the pair never becomes a duplicate
        // vertex; below 3 vertices this still consumes the click
        // (capture) without committing or appending — the stash is
        // too short to close yet.
        let now = std::time::Instant::now();
        if let (Some(last_time), Some((lx, ly))) =
            (state.polygon_last_click_time, state.polygon_last_click_pos)
        {
            let dt = now.duration_since(last_time);
            let dist_sq = (wx - lx).powi(2) + (wy - ly).powi(2);
            if dt.as_millis() < 300 && dist_sq < 3.0 * 3.0 {
                state.polygon_last_click_time = None;
                state.polygon_last_click_pos = None;
                if state.polygon_vertices.len() >= 3 {
                    return Some(commit_polygon(state));
                }
                return Some(canvas::Action::capture());
            }
        }

        // Plain click — append a vertex and keep collecting.
        state.polygon_last_click_time = Some(now);
        state.polygon_last_click_pos = Some((wx, wy));
        state.polygon_vertices.push((wx, wy));
        state.polygon_cursor = Some((wx, wy));
        Some(canvas::Action::capture())
    }

    /// Footprint parity: "leaving Place Polygon commits the in-flight
    /// stash (>= 3 vertices) / discards it (< 3)". Since the Symbol
    /// canvas's stash lives in `CanvasState` rather than the editor
    /// model, there is no message handler this can hook into the way
    /// the footprint editor's `SetTool` dispatcher does — a toolbar
    /// click that changes `editor.tool` never reaches this Program's
    /// `update`. Instead this runs at the top of every canvas event
    /// and compares the live tool against the tool seen on the
    /// previous event; on a transition away from `PlacePolygon` it
    /// flushes the stash before the triggering event is processed
    /// further (that one event is otherwise a harmless miss — most
    /// commonly a `CursorMoved` as the pointer re-enters the canvas).
    pub(in crate::library::editor::symbol::canvas) fn sync_polygon_tool(
        &self,
        state: &mut CanvasState,
    ) -> Option<canvas::Action<CanvasAction>> {
        let previous = state.last_tool.replace(self.tool);
        let leaving_polygon =
            previous == Some(SymbolTool::PlacePolygon) && self.tool != SymbolTool::PlacePolygon;
        if leaving_polygon && !state.polygon_vertices.is_empty() {
            // `commit_polygon` always empties the stash; the < 3 case
            // still publishes, but the updates/mod.rs handler silently
            // drops an under-count payload (no undo snapshot, no
            // pushed graphic) — that's the "discard" half of
            // "commits (>= 3) / discards it (< 3)" for free, with one
            // code path instead of two.
            return Some(commit_polygon(state));
        }
        None
    }
}

/// Take the vertex stash and publish the commit action, resetting the
/// preview cursor. Callers are responsible for checking the stash is
/// non-empty first; an under-3-vertex payload is a safe no-op at the
/// `SymbolEditorMsg::AddPolygon` handler (silently dropped, no undo
/// snapshot) — see `sync_polygon_tool` for the case that relies on
/// that.
pub(super) fn commit_polygon(state: &mut CanvasState) -> canvas::Action<CanvasAction> {
    let vertices = std::mem::take(&mut state.polygon_vertices);
    state.polygon_cursor = None;
    state.polygon_last_click_time = None;
    state.polygon_last_click_pos = None;
    canvas::Action::publish(CanvasAction::AddPolygon {
        vertices: vertices.into_iter().map(|(x, y)| [x, y]).collect(),
    })
    .and_capture()
}
