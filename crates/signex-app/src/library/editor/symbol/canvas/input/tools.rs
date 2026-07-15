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
                if let Some(sel) = state::hit_test(self.symbol, ux, uy, self.active_part) {
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
            SymbolTool::PlaceRectangle => Some(
                canvas::Action::publish(CanvasAction::AddRectangle { x: wx, y: wy }).and_capture(),
            ),
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
        }
    }
}
