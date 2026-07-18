//! Pointer input — pan start/stop, cursor-move (handle drag, item
//! drag, rubber-band + multi-click preview tracking, idle readout),
//! and left-release commit (box-select / drag-commit). Each method is
//! the corresponding `Program::update` branch extracted verbatim;
//! conditions, coordinate math, and `Action` capture/publish sites are
//! unchanged.

use super::super::*;
use iced::Rectangle;
use iced::mouse;
use iced::widget::canvas;

/// Screen-pixel motion a right/middle-button drag must cross before
/// it counts as a real pan (as opposed to 1px cursor jitter) — mirrors
/// the footprint canvas's `pan_on_cursor_moved` threshold, so the same
/// jitter can't wrongly suppress the context menu opening on release
/// or wrongly close one that's already open.
const PAN_MOVE_THRESHOLD_PX: f32 = 2.0;

impl SymbolCanvas<'_> {
    /// Right/Middle press: **Right** cancels an in-progress multi-click
    /// draw (Place Polygon's stash included), else starts a pan.
    /// **Middle** never cancels Place Polygon — it always proceeds
    /// straight to arming a pan with the stash left intact. The
    /// vertex stash has no undo, so treating a middle-button pan
    /// attempt as a cancel would silently destroy every click the
    /// user had placed so far.
    pub(in crate::library::editor::symbol::canvas) fn on_secondary_press(
        &self,
        state: &mut CanvasState,
        button: &mouse::Button,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasAction>> {
        // Right-click cancels Place Polygon's editor-owned stash —
        // this needs its own publish (unlike the other multi-click
        // tools below, whose in-progress state is purely local to
        // `CanvasState` and never needs to notify the dispatcher).
        if should_cancel_polygon_placement(*button, self.tool, self.polygon_vertices.is_empty()) {
            state.polygon_cursor = None;
            state.polygon_last_click_time = None;
            state.polygon_last_click_pos = None;
            return Some(canvas::Action::publish(CanvasAction::PolygonCancel).and_capture());
        }
        // Right-click cancels any other in-progress multi-click draw;
        // otherwise starts a pan (same as schematic canvas).
        let draw_in_progress = match self.tool {
            SymbolTool::PlaceRectangle => state.rect_from.is_some(),
            SymbolTool::PlaceLine => state.line_from.is_some(),
            SymbolTool::PlaceCircle => state.circle_center.is_some(),
            SymbolTool::PlaceArc => state.arc_center.is_some() || state.arc_radius_start.is_some(),
            _ => false,
        };
        if draw_in_progress {
            state.rect_from = None;
            state.rect_cursor = None;
            state.line_from = None;
            state.line_cursor = None;
            state.circle_center = None;
            state.circle_cursor = None;
            state.arc_center = None;
            state.arc_radius_start = None;
            state.arc_cursor = None;
            state.arc_end_deg_unwrapped = None;
            return Some(canvas::Action::capture());
        }
        let pos = cursor.position_in(bounds)?;
        state.panning = true;
        state.last_pan_pos = Some(pos);
        // Fixed origin for the cumulative pan-motion latch in
        // `on_cursor_moved` — a slow, deliberate drag whose individual
        // per-frame deltas never cross the threshold must still be
        // recognised as a real pan once its TOTAL displacement does.
        state.secondary_press_pos = Some(pos);
        // Track motion so a right-release without pan motion opens the
        // context menu instead (see `on_secondary_release`).
        state.pan_moved = false;
        Some(canvas::Action::capture())
    }

    /// Right/Middle release: end the pan. A **right**-release that did
    /// not pan opens the context menu (pin → graphic → empty hit
    /// priority), window-absolute coords, mirroring the footprint
    /// canvas's `on_secondary_released`. Middle-release never opens the
    /// menu. Note: when the matching press instead cancelled an
    /// in-progress Place Polygon / multi-click draw (see
    /// `on_secondary_press`), `state.panning` was never armed, so this
    /// release naturally falls through to a no-op — placement-cancel
    /// wins over opening the menu with no extra checks needed here.
    pub(in crate::library::editor::symbol::canvas) fn on_secondary_release(
        &self,
        state: &mut CanvasState,
        button: &mouse::Button,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasAction>> {
        let was_panning = state.panning;
        let did_pan = state.pan_moved;
        state.panning = false;
        state.last_pan_pos = None;
        state.secondary_press_pos = None;
        state.pan_moved = false;
        if was_panning
            && !did_pan
            && *button == mouse::Button::Right
            && let Some(cursor_pos) = cursor.position_in(bounds)
        {
            let screen_x = bounds.x + cursor_pos.x;
            let screen_y = bounds.y + cursor_pos.y;
            // Unsnapped, like every other hit-test site (see
            // `on_left_press`'s comment on why Select-tool hit-testing
            // must use the raw cursor position): the grid-snapped
            // `world_for` can be off by up to half a grid cell, which
            // exceeds `GRAPHIC_BODY_TOL` and both steals the selection
            // near an unrelated pin and misses an off-grid shape the
            // user right-clicked directly on.
            let (wx, wy) = world_unsnapped(self, cursor_pos.x, cursor_pos.y, bounds);
            let target = match state::hit_test(self.symbol, wx, wy, self.active_part) {
                Some(SymbolSelection::Pin(idx)) => state::SymbolContextTarget::Pin(idx),
                Some(SymbolSelection::Graphic(idx)) => state::SymbolContextTarget::Graphic(idx),
                _ => state::SymbolContextTarget::Empty,
            };
            return Some(
                canvas::Action::publish(CanvasAction::ShowContextMenu {
                    x: screen_x,
                    y: screen_y,
                    target,
                })
                .and_capture(),
            );
        }
        None
    }

    /// Cursor move: pan / handle-drag / item-drag / rubber-band +
    /// multi-click preview tracking / idle coordinate readout.
    pub(in crate::library::editor::symbol::canvas) fn on_cursor_moved(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasAction>> {
        let pos = cursor.position_in(bounds)?;
        // Pan first so panning while a handle is grabbed
        // doesn't accidentally reshape geometry.
        if state.panning {
            let last = state.last_pan_pos.unwrap_or(pos);
            let dx = pos.x - last.x;
            let dy = pos.y - last.y;
            state.last_pan_pos = Some(pos);
            // Cumulative displacement from the fixed press origin, NOT
            // the per-frame delta above — a slow, deliberate drag can
            // move well past the threshold in total while never
            // crossing it in any single frame.
            let origin = state.secondary_press_pos.unwrap_or(pos);
            if !state.pan_moved && pan_moved_past_threshold(origin, pos) {
                state.pan_moved = true;
                // The context menu and a pan can't coexist — the drag
                // just became real, so close any open menu instead of
                // publishing this frame's pan (see the dismiss-layer
                // contract in `app/view/overlays/mod.rs`'s
                // `dismiss_layer` doc comment: the canvas, not the
                // dismiss layer, owns closing the menu on a real pan).
                if self.context_menu_open {
                    return Some(canvas::Action::publish(CanvasAction::CloseContextMenu));
                }
            }
            if dx != 0.0 || dy != 0.0 {
                return Some(canvas::Action::publish(CanvasAction::Pan { dx, dy }));
            }
            return None;
        }
        let (wx, wy) = world_for(self, pos.x, pos.y, bounds);
        let (ux, uy) = world_unsnapped(self, pos.x, pos.y, bounds);
        if let Some((idx, handle)) = state.dragging_handle {
            return Some(canvas::Action::publish(CanvasAction::MoveGraphicHandle {
                idx,
                handle,
                x: wx,
                y: wy,
            }));
        }
        if state.dragging {
            // All or Multiple selection: delta-based drag.
            let is_delta_based = matches!(
                self.selected,
                Some(SymbolSelection::All) | Some(SymbolSelection::Multiple { .. })
            );
            if is_delta_based {
                if let Some((last_wx, last_wy)) = state.last_drag_world_pos {
                    let dx = wx - last_wx;
                    let dy = wy - last_wy;
                    state.last_drag_world_pos = Some((wx, wy));
                    if dx.abs() > f64::EPSILON || dy.abs() > f64::EPSILON {
                        return Some(canvas::Action::publish(CanvasAction::MoveAll { dx, dy }));
                    }
                } else {
                    state.last_drag_world_pos = Some((wx, wy));
                }
                return None;
            }
            // Single-item selection: absolute positioning with anchor offset.
            let (move_x, move_y) = state
                .drag_anchor_offset
                .map(|(dx, dy)| (wx + dx, wy + dy))
                .unwrap_or((wx, wy));
            return Some(canvas::Action::publish(CanvasAction::Move {
                x: move_x,
                y: move_y,
            }));
        }
        // Update rubber-band box while the user drags on empty space.
        // Publishing CursorAt forces iced to process the event as a
        // state-changing message, which triggers draw() on the next
        // frame so the rubber band animates in real time.
        if state.box_select_origin.is_some() {
            state.box_select_current = Some((ux, uy));
            return Some(
                canvas::Action::publish(CanvasAction::CursorAt {
                    x_mm: Some(ux),
                    y_mm: Some(uy),
                })
                .and_capture(),
            );
        }
        // While waiting for a subsequent click in a multi-click
        // draw flow, update the rubber-band cursor and force a
        // redraw so the preview animates in real time.
        if self.tool == SymbolTool::PlaceRectangle && state.rect_from.is_some() {
            state.rect_cursor = Some((wx, wy));
            return Some(
                canvas::Action::publish(CanvasAction::CursorAt {
                    x_mm: Some(ux),
                    y_mm: Some(uy),
                })
                .and_capture(),
            );
        }
        if self.tool == SymbolTool::PlaceLine && state.line_from.is_some() {
            state.line_cursor = Some((wx, wy));
            return Some(
                canvas::Action::publish(CanvasAction::CursorAt {
                    x_mm: Some(ux),
                    y_mm: Some(uy),
                })
                .and_capture(),
            );
        }
        if self.tool == SymbolTool::PlaceCircle && state.circle_center.is_some() {
            state.circle_cursor = Some((wx, wy));
            return Some(
                canvas::Action::publish(CanvasAction::CursorAt {
                    x_mm: Some(ux),
                    y_mm: Some(uy),
                })
                .and_capture(),
            );
        }
        if self.tool == SymbolTool::PlaceArc
            && (state.arc_center.is_some() || state.arc_radius_start.is_some())
        {
            state.arc_cursor = Some((wx, wy));
            // Phase 2: keep a continuous (unwrapped) end angle so
            // arcs that sweep past ±180° don't jump.
            if let Some((cx, cy)) = state.arc_center {
                if state.arc_radius_start.is_some() {
                    let raw = (wy - cy).atan2(wx - cx).to_degrees();
                    state.arc_end_deg_unwrapped = Some(match state.arc_end_deg_unwrapped {
                        Some(prev) => unwrap_angle(prev, raw),
                        None => raw,
                    });
                }
            }
            return Some(
                canvas::Action::publish(CanvasAction::CursorAt {
                    x_mm: Some(ux),
                    y_mm: Some(uy),
                })
                .and_capture(),
            );
        }
        if self.tool == SymbolTool::PlacePolygon && !self.polygon_vertices.is_empty() {
            state.polygon_cursor = Some((wx, wy));
            return Some(
                canvas::Action::publish(CanvasAction::CursorAt {
                    x_mm: Some(ux),
                    y_mm: Some(uy),
                })
                .and_capture(),
            );
        }
        // Idle cursor — publish the unsnapped world position
        // for the status footer X/Y readout.
        Some(canvas::Action::publish(CanvasAction::CursorAt {
            x_mm: Some(ux),
            y_mm: Some(uy),
        }))
    }

    /// Cursor left the canvas: end pan, clear the coordinate readout.
    pub(in crate::library::editor::symbol::canvas) fn on_cursor_left(
        &self,
        state: &mut CanvasState,
    ) -> Option<canvas::Action<CanvasAction>> {
        state.panning = false;
        state.last_pan_pos = None;
        state.secondary_press_pos = None;
        Some(canvas::Action::publish(CanvasAction::CursorAt {
            x_mm: None,
            y_mm: None,
        }))
    }

    /// Left release: commit a rubber-band box selection, or close the
    /// coalesced move-drag undo group.
    pub(in crate::library::editor::symbol::canvas) fn on_left_release(
        &self,
        state: &mut CanvasState,
    ) -> Option<canvas::Action<CanvasAction>> {
        let was_dragging = state.dragging || state.dragging_handle.is_some();
        state.dragging = false;
        state.dragging_handle = None;
        state.drag_anchor_offset = None;
        state.last_drag_world_pos = None;

        // Commit a rubber-band box selection if one was in progress.
        if let (Some((ox, oy)), Some((cx, cy))) = (
            state.box_select_origin.take(),
            state.box_select_current.take(),
        ) {
            let drag_dist_sq = (cx - ox).powi(2) + (cy - oy).powi(2);
            if drag_dist_sq > 0.5 * 0.5 {
                // Enough movement — commit as a box selection.
                let kind = if cx >= ox {
                    state::BoxSelectKind::Window
                } else {
                    state::BoxSelectKind::Crossing
                };
                let result =
                    state::select_in_box(self.symbol, ox, oy, cx, cy, kind, self.active_part);
                return Some(match result {
                    Some(sel) => canvas::Action::publish(CanvasAction::Select(sel)).and_capture(),
                    None => canvas::Action::publish(CanvasAction::Deselect).and_capture(),
                });
            } else {
                // Micro-drag treated as a click → deselect.
                return Some(canvas::Action::publish(CanvasAction::Deselect).and_capture());
            }
        }

        // Notify dispatcher that a move drag completed so it can
        // close the coalesced undo snapshot group.
        if was_dragging {
            return Some(canvas::Action::publish(CanvasAction::DragCommit));
        }

        None
    }
}

/// Whether a secondary-button press should cancel the Place Polygon
/// stash instead of arming a pan (see `on_secondary_press`). Only
/// `Right` cancels — the stash has no undo, so a `Middle`-button pan
/// attempt must never destroy it.
fn should_cancel_polygon_placement(
    button: mouse::Button,
    tool: SymbolTool,
    polygon_vertices_empty: bool,
) -> bool {
    button == mouse::Button::Right && tool == SymbolTool::PlacePolygon && !polygon_vertices_empty
}

/// Whether cumulative displacement from the fixed press `origin` to
/// `current` crosses the pan motion threshold that latches
/// `CanvasState::pan_moved` (see `on_cursor_moved`). Comparing against
/// the ORIGIN, not the per-frame delta, means a slow, deliberate drag
/// — many sub-threshold per-frame steps — is still recognised as a
/// real pan once its total displacement adds up past the threshold.
fn pan_moved_past_threshold(origin: iced::Point, current: iced::Point) -> bool {
    let dx = current.x - origin.x;
    let dy = current.y - origin.y;
    dx.abs() > PAN_MOVE_THRESHOLD_PX || dy.abs() > PAN_MOVE_THRESHOLD_PX
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_cancel_polygon_placement_only_for_right_button() {
        assert!(should_cancel_polygon_placement(
            mouse::Button::Right,
            SymbolTool::PlacePolygon,
            false,
        ));
        assert!(
            !should_cancel_polygon_placement(
                mouse::Button::Middle,
                SymbolTool::PlacePolygon,
                false,
            ),
            "middle-button pan must never destroy the vertex stash"
        );
    }

    #[test]
    fn should_cancel_polygon_placement_requires_a_non_empty_stash() {
        assert!(!should_cancel_polygon_placement(
            mouse::Button::Right,
            SymbolTool::PlacePolygon,
            true,
        ));
    }

    #[test]
    fn should_cancel_polygon_placement_requires_the_place_polygon_tool() {
        assert!(!should_cancel_polygon_placement(
            mouse::Button::Right,
            SymbolTool::Select,
            false,
        ));
    }

    /// Many per-frame steps each under the 2px threshold still latch
    /// once their CUMULATIVE displacement from the fixed origin
    /// crosses it — a slow, deliberate drag, not 1px jitter.
    #[test]
    fn pan_moved_past_threshold_uses_cumulative_not_per_frame_delta() {
        let origin = iced::Point::new(0.0, 0.0);
        assert!(!pan_moved_past_threshold(
            origin,
            iced::Point::new(1.0, 0.0)
        ));
        assert!(!pan_moved_past_threshold(
            origin,
            iced::Point::new(2.0, 0.0)
        ));
        assert!(pan_moved_past_threshold(origin, iced::Point::new(2.1, 0.0)));
    }

    #[test]
    fn pan_moved_past_threshold_false_within_threshold_of_a_nonzero_origin() {
        let origin = iced::Point::new(10.0, 10.0);
        assert!(!pan_moved_past_threshold(
            origin,
            iced::Point::new(11.0, 11.0)
        ));
        assert!(pan_moved_past_threshold(
            origin,
            iced::Point::new(13.0, 10.0)
        ));
    }
}
