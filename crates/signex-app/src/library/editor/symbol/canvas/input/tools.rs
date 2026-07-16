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
                if let Some((idx, handle)) = state::hit_test_graphic_handle(
                    self.symbol,
                    ux,
                    uy,
                    tol_mm,
                    self.active_part,
                    &self.selected,
                ) {
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
                if let Some((radius, start_deg)) = state.arc_radius_start {
                    // Third click — peek (don't take yet) so a
                    // rejected commit (see below) leaves the gesture
                    // exactly where it was, ready for another click.
                    let (cx, cy) = state.arc_center.unwrap_or((wx, wy));
                    // Use the unwrapped end angle so arcs that swept
                    // past ±180° are stored correctly. Fall back to a
                    // fresh atan2 only if the cursor never moved after
                    // the second click.
                    let end_deg = state.arc_end_deg_unwrapped.unwrap_or_else(|| {
                        let dx = wx - cx;
                        let dy = wy - cy;
                        dy.atan2(dx).to_degrees()
                    });
                    if arc_sweep_exceeds_full_turn(start_deg, end_deg) {
                        // A full turn or more: the commit-normalization
                        // swap (`normalize_arc_commit_deg`) would
                        // collapse this to `start == end` — an
                        // invisible, unselectable, un-deletable point-
                        // arc that still saves to disk. Reject instead
                        // of committing; gesture state is untouched
                        // (no `.take()` above), so the third click is
                        // effectively ignored and dragging can continue.
                        return Some(
                            canvas::Action::publish(CanvasAction::ArcSweepRejected).and_capture(),
                        );
                    }
                    state.arc_radius_start = None;
                    state.arc_center = None;
                    state.arc_cursor = None;
                    state.arc_end_deg_unwrapped = None;
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

    /// Click-collect gesture for the `PlacePolygon` tool. The vertex
    /// stash itself (`self.polygon_vertices`) lives on
    /// `SymbolEditorState`, not this Program's `CanvasState` — a
    /// document-scoped Vec, not per-widget-slot ephemeral state, so a
    /// tab switch mid-placement can't leak an in-flight vertex list
    /// into a different document (see `CanvasAction::PolygonClick`'s
    /// doc comment). Each plain click publishes `PolygonClick` so the
    /// dispatcher appends it there; two close gestures are checked
    /// first so they don't append a duplicate vertex:
    ///
    /// 1. A click on the already-placed first vertex (hit-tolerance
    ///    based, using the unsnapped cursor so it feels the same at
    ///    every zoom level — mirrors `hit_test_graphic_handle`'s
    ///    tolerance derivation).
    /// 2. A double-click — both clicks must land on the exact same
    ///    snapped grid vertex within 300 ms (not a fixed mm radius,
    ///    which at a fine snap grid could misread two adjacent-but-
    ///    distinct clicks as a double-click).
    ///
    /// Both close gestures require `>= 3` collected vertices before
    /// they publish `PolygonCommit` — the dispatcher's `mem::take`
    /// discards an invalid ring, so committing early would silently
    /// WIPE the in-progress stash. Below 3 vertices a matched double-
    /// click is swallowed (captured, no vertex appended, no commit)
    /// and collection continues.
    fn on_polygon_click(
        &self,
        state: &mut CanvasState,
        wx: f64,
        wy: f64,
        ux: f64,
        uy: f64,
    ) -> Option<canvas::Action<CanvasAction>> {
        // Close gesture 1: click on the first vertex.
        if self.polygon_vertices.len() >= 3
            && let Some(&(fx, fy)) = self.polygon_vertices.first()
        {
            let tol_mm = (8.0_f32 / self.camera.scale.max(0.01)).clamp(0.5, 4.0) as f64;
            let dx = ux - fx;
            let dy = uy - fy;
            if dx * dx + dy * dy <= tol_mm * tol_mm {
                return Some(self.publish_polygon_commit(state));
            }
        }

        // Close gesture 2: double-click on the same snapped vertex.
        // Checked before appending so the second click of the pair
        // never becomes a near-duplicate vertex.
        let now = std::time::Instant::now();
        if let (Some(last_time), Some(last_pos)) =
            (state.polygon_last_click_time, state.polygon_last_click_pos)
        {
            let dt = now.duration_since(last_time);
            if dt.as_millis() < 300 && last_pos == (wx, wy) {
                state.polygon_last_click_time = None;
                state.polygon_last_click_pos = None;
                // Guard mirrors gesture 1 and the Enter arm: below 3
                // vertices there is nothing valid to close, and the
                // dispatcher's `mem::take` would wipe the stash — so
                // swallow the click (no duplicate vertex) and keep
                // collecting instead of committing.
                if self.polygon_vertices.len() < 3 {
                    return Some(canvas::Action::capture());
                }
                return Some(self.publish_polygon_commit(state));
            }
        }

        // Plain click — append a vertex and keep collecting.
        state.polygon_last_click_time = Some(now);
        state.polygon_last_click_pos = Some((wx, wy));
        state.polygon_cursor = Some((wx, wy));
        Some(canvas::Action::publish(CanvasAction::PolygonClick { x: wx, y: wy }).and_capture())
    }

    /// Publish the commit action + reset the ephemeral gesture-timing
    /// fields that live in `CanvasState`. The vertex-count / validity
    /// check happens at the dispatcher (`SymbolEditorMsg::PolygonCommit`
    /// handler) against the editor-owned stash, not here.
    fn publish_polygon_commit(&self, state: &mut CanvasState) -> canvas::Action<CanvasAction> {
        state.polygon_cursor = None;
        state.polygon_last_click_time = None;
        state.polygon_last_click_pos = None;
        canvas::Action::publish(CanvasAction::PolygonCommit).and_capture()
    }
}

/// `true` when the Place Arc gesture's raw, unwrapped drag delta
/// (`end_deg - start_deg`, BEFORE `normalize_arc_commit_deg`'s
/// swap-and-`rem_euclid`) covers a full turn or more. Committing such
/// a drag would collapse to `start_deg == end_deg` after
/// normalization — a zero-sweep point-arc that's invisible,
/// unselectable, and un-deletable via the canvas, yet still saves to
/// disk and still occupies its bounding box.
fn arc_sweep_exceeds_full_turn(start_deg: f64, end_deg: f64) -> bool {
    (end_deg - start_deg).abs() >= 360.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_sweep_exceeds_full_turn_at_exactly_360() {
        assert!(arc_sweep_exceeds_full_turn(0.0, 360.0));
        assert!(arc_sweep_exceeds_full_turn(30.0, -330.0));
    }

    #[test]
    fn arc_sweep_exceeds_full_turn_past_360() {
        assert!(arc_sweep_exceeds_full_turn(10.0, 400.0));
    }

    #[test]
    fn arc_sweep_within_a_turn_is_not_rejected() {
        assert!(!arc_sweep_exceeds_full_turn(30.0, -60.0));
        assert!(!arc_sweep_exceeds_full_turn(10.0, 350.0));
        assert!(!arc_sweep_exceeds_full_turn(0.0, 359.9));
    }
}
