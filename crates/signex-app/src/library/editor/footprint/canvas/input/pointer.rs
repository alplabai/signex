//! Pointer mechanics — button-press / button-release / cursor-move
//! dispatchers plus the classification helpers they share (snap
//! resolution, empty-press arming, drag-tick move publishing, and the
//! cursor-move cache/hover tail).
//!
//! The press/release dispatchers walk their per-tool arms (in
//! `tools.rs` / `release.rs`) in the original top-to-bottom order; the
//! secondary-button (pan / context-menu) handling lives here.

use iced::mouse;
use iced::widget::canvas;
use iced::{Point, Rectangle};

use crate::library::editor::footprint::snap;
use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage};

use super::super::hit_test::sketch_snap;
use super::super::{
    DRAG_THRESHOLD_PX, DragState, FootprintCanvas, FootprintCanvasState, silk_f_hit_at,
};

impl FootprintCanvas<'_> {
    // ---- Button pressed ---------------------------------------------

    pub(in crate::library::editor::footprint::canvas) fn on_button_pressed(
        &self,
        cstate: &mut FootprintCanvasState,
        button: &mouse::Button,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<LibraryMessage>> {
        if matches!(button, mouse::Button::Right | mouse::Button::Middle) {
            return self.on_secondary_pressed(cstate, button, bounds, cursor);
        }
        if *button == mouse::Button::Left
            && let Some(cursor_pos) = cursor.position_in(bounds)
        {
            return self.on_primary_pressed(cstate, cursor_pos);
        }
        None
    }

    /// Right/Middle press — right-click tool cancel (schematic parity),
    /// otherwise start a pan.
    fn on_secondary_pressed(
        &self,
        cstate: &mut FootprintCanvasState,
        button: &mouse::Button,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<LibraryMessage>> {
        // v0.15 — schematic-parity tool cancel. Right-click while a
        // non-Select tool is active cancels the tool back to Select
        // instead of starting a pan. Middle-click always pans.
        use crate::library::editor::footprint::state::{
            EditorMode, PadsTool, SketchTool, ToolPending,
        };
        if *button == mouse::Button::Right {
            // v0.27 — Lasso Select right-click commits the polygon.
            if self.state.lasso_mode_active {
                return Some(
                    canvas::Action::publish(LibraryMessage::EditorEvent {
                        library_path: self.address.library_path.clone(),
                        table: self.address.table.clone(),
                        row_id: self.address.row_id,
                        msg: EditorMsg::Footprint(FootprintEditorMsg::LassoCommit),
                    })
                    .and_capture(),
                );
            }
            // v0.27 — Touching Line right-click cancels.
            if self.state.touching_line_active {
                return Some(
                    canvas::Action::publish(LibraryMessage::EditorEvent {
                        library_path: self.address.library_path.clone(),
                        table: self.address.table.clone(),
                        row_id: self.address.row_id,
                        msg: EditorMsg::Footprint(FootprintEditorMsg::TouchingLineCancel),
                    })
                    .and_capture(),
                );
            }
            let cancel_msg: Option<EditorMsg> = match self.state.mode {
                EditorMode::Normal => {
                    if self.state.pads_tool != PadsTool::Select {
                        Some(EditorMsg::Footprint(FootprintEditorMsg::SetPadsTool(
                            PadsTool::Select,
                        )))
                    } else {
                        None
                    }
                }
                EditorMode::Sketch => {
                    let tool_active = self.state.active_tool != SketchTool::Select;
                    let pending_active = !matches!(self.state.tool_pending, ToolPending::Idle);
                    if tool_active || pending_active {
                        Some(EditorMsg::Footprint(FootprintEditorMsg::SketchSetTool(
                            SketchTool::Select,
                        )))
                    } else {
                        None
                    }
                }
                EditorMode::View3d => None,
            };
            if let Some(msg) = cancel_msg {
                return Some(
                    canvas::Action::publish(LibraryMessage::EditorEvent {
                        library_path: self.address.library_path.clone(),
                        table: self.address.table.clone(),
                        row_id: self.address.row_id,
                        msg,
                    })
                    .and_capture(),
                );
            }
        }
        // Otherwise (Middle, or Right with no active tool) → start a
        // pan as usual.
        cstate.panning = true;
        cstate.last_pan_pos = cursor.position_in(bounds);
        // v0.26 — track motion so right-release without pan motion
        // opens the context menu instead.
        cstate.pan_moved = false;
        Some(canvas::Action::capture())
    }

    /// Left press — walk the per-tool click arms in order. First one
    /// to claim the click returns; the empty-canvas tail arms the
    /// pending drag + rubber-band.
    fn on_primary_pressed(
        &self,
        cstate: &mut FootprintCanvasState,
        cursor_pos: Point,
    ) -> Option<canvas::Action<LibraryMessage>> {
        if let Some(a) = self.try_lasso_add_vertex(cstate, cursor_pos) {
            return Some(a);
        }
        if let Some(a) = self.try_touching_line_click(cstate, cursor_pos) {
            return Some(a);
        }
        let raw_world = cstate.screen_to_world(cursor_pos);
        let world = self.primary_press_snap(cstate, raw_world);
        if let Some(a) = self.try_round_handle_grab(cstate, cursor_pos, world) {
            return Some(a);
        }
        // #361 — the armed "Drag Track End" tool wins the click before
        // the generic Point / Line grabs so it can bias to the segment's
        // nearer endpoint (see `try_drag_track_end_grab`).
        if let Some(a) = self.try_drag_track_end_grab(cstate, cursor_pos, world) {
            return Some(a);
        }
        if let Some(a) = self.try_sketch_point_grab(cstate, cursor_pos, raw_world, world) {
            return Some(a);
        }
        if let Some(a) = self.try_sketch_line_grab(cstate, cursor_pos, world) {
            return Some(a);
        }
        if let Some(a) = self.try_closed_loop_select(world) {
            return Some(a);
        }
        if let Some(a) = self.try_pad_grab(cstate, cursor_pos, world) {
            return Some(a);
        }
        if let Some(a) = self.try_silk_select(cstate, world) {
            return Some(a);
        }
        self.primary_press_empty(cstate, cursor_pos, world)
    }

    /// v0.18.8 / v0.27 — resolve the press-time world position: Select
    /// tools use the raw cursor (so a click can target a specific
    /// entity), placement tools go through `snap::snap_cursor`. Also
    /// updates `cstate.last_snap` for the snap-kind badge.
    fn primary_press_snap(
        &self,
        cstate: &mut FootprintCanvasState,
        raw_world: (f64, f64),
    ) -> (f64, f64) {
        use crate::library::editor::footprint::state::{EditorMode, PadsTool, SketchTool};
        // #361 — DragTrackEnd is a grab gesture, not a placement tool:
        // at press time it must hit-test the RAW cursor (so the line
        // under the cursor is found, not a snapped position) and leave
        // the snap badge clear, exactly like Select. Vertex snapping is
        // re-enabled once the endpoint drag is in flight — see
        // `pointer_move_world`, whose drag-active branch already keeps
        // snapping alive for a `sketch_point` drag.
        let select_mode = (matches!(self.state.mode, EditorMode::Sketch)
            && matches!(
                self.state.active_tool,
                SketchTool::Select | SketchTool::DragTrackEnd
            ))
            || (matches!(self.state.mode, EditorMode::Normal)
                && self.state.pads_tool == PadsTool::Select);
        if select_mode {
            cstate.last_snap = None;
            raw_world
        } else {
            let point_hit = sketch_snap(self.sketch, cstate, raw_world);
            let result = snap::snap_cursor(raw_world, self.sketch, self.state, point_hit);
            cstate.last_snap = Some(result);
            result.pos
        }
    }

    /// Empty-area press — stash a pending click-add drag (commit on
    /// release) and, for the Select tool, arm the rubber-band anchor.
    fn primary_press_empty(
        &self,
        cstate: &mut FootprintCanvasState,
        cursor_pos: Point,
        world: (f64, f64),
    ) -> Option<canvas::Action<LibraryMessage>> {
        use crate::library::editor::footprint::state::{EditorMode, PadsTool, SketchTool};
        // Empty area → pending click-add. We can't add yet because a
        // drag may follow; commit on release.
        cstate.drag = Some(DragState {
            pad_idx: usize::MAX,
            sketch_point: None,
            sketch_line: None,
            grab_offset_mm: (world.0, world.1),
            last_world: world,
            press_screen: cursor_pos,
            moved: false,
        });
        // v0.26-I + v0.27 — Select-tool empty-canvas press arms the
        // rubber-band rectangle for Pads mode + Sketch mode.
        let arm_rubber = matches!(self.state.mode, EditorMode::Normal)
            && self.state.pads_tool == PadsTool::Select
            || matches!(self.state.mode, EditorMode::Sketch)
                && self.state.active_tool == SketchTool::Select;
        if arm_rubber {
            cstate.box_select_anchor_screen = Some(cursor_pos);
            cstate.box_select_current_screen = Some(cursor_pos);
        }
        Some(canvas::Action::capture())
    }

    // ---- Button released --------------------------------------------

    pub(in crate::library::editor::footprint::canvas) fn on_button_released(
        &self,
        cstate: &mut FootprintCanvasState,
        button: &mouse::Button,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<LibraryMessage>> {
        if matches!(button, mouse::Button::Right | mouse::Button::Middle) {
            return self.on_secondary_released(cstate, button, bounds, cursor);
        }
        if *button == mouse::Button::Left {
            return self.on_primary_released(cstate, bounds, cursor);
        }
        None
    }

    /// Right/Middle release — a right-release that did not pan opens
    /// the context menu (pad → silk → empty hit priority).
    fn on_secondary_released(
        &self,
        cstate: &mut FootprintCanvasState,
        button: &mouse::Button,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<LibraryMessage>> {
        // v0.26-J — only treat the release as a context-menu trigger
        // when the matching press actually started a pan.
        let was_panning = cstate.panning;
        let did_pan = cstate.pan_moved;
        cstate.panning = false;
        cstate.last_pan_pos = None;
        cstate.pan_moved = false;
        if was_panning
            && !did_pan
            && *button == mouse::Button::Right
            && let Some(cursor_pos) = cursor.position_in(bounds)
        {
            // Window-absolute screen coords for the overlay.
            let screen_x = bounds.x + cursor_pos.x;
            let screen_y = bounds.y + cursor_pos.y;
            // v0.26-B/C — hit-test pads first (top z-order), then silk,
            // then Empty.
            use crate::library::editor::footprint::state::FootprintContextTarget;
            let world = cstate.screen_to_world(cursor_pos);
            let target = if let Some(idx) = self.state.pad_at(world.0, world.1) {
                FootprintContextTarget::Pad(idx)
            } else {
                let tol = 4.0_f64 / (cstate.scale.max(1.0) as f64);
                match silk_f_hit_at(self.silk_f, world.0, world.1, tol) {
                    Some(idx) => FootprintContextTarget::SilkF(idx),
                    None => FootprintContextTarget::Empty,
                }
            };
            return Some(
                canvas::Action::publish(LibraryMessage::EditorEvent {
                    library_path: self.address.library_path.clone(),
                    table: self.address.table.clone(),
                    row_id: self.address.row_id,
                    msg: EditorMsg::Footprint(FootprintEditorMsg::ShowContextMenu {
                        x: screen_x,
                        y: screen_y,
                        target,
                    }),
                })
                .and_capture(),
            );
        }
        None
    }

    /// Left release — take the drag, dispatch by its kind (round-pad
    /// resize / sketch-point / empty-or-tool / pad-drag settle).
    fn on_primary_released(
        &self,
        cstate: &mut FootprintCanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<LibraryMessage>> {
        // Original: `if *button == Left && let Some(drag) =
        // cstate.drag.take() { … }` — kept as an `if let` (not a
        // `let … else` early return) so the drag `take()` and the
        // subsequent field mutations match the pre-split control flow.
        if let Some(drag) = cstate.drag.take() {
            // v0.27 — round-pad resize drag releases here; the
            // CursorMoved handler streamed the resize per tick, release
            // just clears.
            if cstate.round_resize_drag.take().is_some() {
                self.cache.clear();
                return None;
            }
            // v0.16 — sketch-Point drag releases here; CursorMoved
            // streamed FootprintSketchMovePoint per tick, release just
            // ends it.
            if drag.sketch_point.is_some() {
                self.cache.clear();
                return None;
            }
            if drag.pad_idx == usize::MAX {
                return self.released_empty_or_tool(cstate, &drag, bounds, cursor);
            }
            if drag.moved {
                // Final pad-drag move position is already published per
                // CursorMoved tick — release just clears the cache so
                // the next frame settles.
                self.cache.clear();
            }
        }
        None
    }

    // ---- Cursor moved -----------------------------------------------

    pub(in crate::library::editor::footprint::canvas) fn on_cursor_moved(
        &self,
        cstate: &mut FootprintCanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<LibraryMessage>> {
        let Some(cursor_pos) = cursor.position_in(bounds) else {
            return None;
        };
        if let Some(action) = self.pan_on_cursor_moved(cstate, cursor_pos) {
            return Some(action);
        }
        let world = self.pointer_move_world(cstate, cursor_pos);
        if let Some(a) = self.try_round_resize_tick(cstate, world) {
            return Some(a);
        }
        if let Some(a) = self.on_pointer_drag_tick(cstate, cursor_pos, world) {
            return Some(a);
        }
        self.cursor_move_cache_and_hover(world)
    }

    /// v0.18.8 / v0.27 — resolve the move-tick world position. Select
    /// tools read the raw cursor EXCEPT while a drag is in flight
    /// (edge-resize should respect Snap Options).
    fn pointer_move_world(
        &self,
        cstate: &mut FootprintCanvasState,
        cursor_pos: Point,
    ) -> (f64, f64) {
        let raw_world = cstate.screen_to_world(cursor_pos);
        use crate::library::editor::footprint::state::{EditorMode, PadsTool, SketchTool};
        let select_mode_tick = (matches!(self.state.mode, EditorMode::Sketch)
            && self.state.active_tool == SketchTool::Select)
            || (matches!(self.state.mode, EditorMode::Normal)
                && self.state.pads_tool == PadsTool::Select);
        let drag_active_for_snap = cstate
            .drag
            .as_ref()
            .map(|d| d.sketch_line.is_some() || d.sketch_point.is_some() || d.pad_idx != usize::MAX)
            .unwrap_or(false);
        if select_mode_tick && !drag_active_for_snap {
            cstate.last_snap = None;
            raw_world
        } else {
            // A whole-pad drag must NOT snap the cursor to sketch
            // Points. A sketch-profile pad carries its outline as
            // sketch geometry that moves WITH the pad, so those Points
            // sit right under the cursor mid-drag; snapping to them
            // makes the cursor lock onto the pad's own outline for a
            // tick and jump — a visible flicker. Grid / pad snapping
            // via `snap_cursor` still applies. Sketch-point and
            // sketch-line drags keep vertex snapping (that's the whole
            // point of dragging a vertex).
            let pad_drag = cstate
                .drag
                .as_ref()
                .map(|d| {
                    d.pad_idx != usize::MAX && d.sketch_point.is_none() && d.sketch_line.is_none()
                })
                .unwrap_or(false);
            let point_hit = if pad_drag {
                None
            } else {
                sketch_snap(self.sketch, cstate, raw_world)
            };
            let result = snap::snap_cursor(raw_world, self.sketch, self.state, point_hit);
            cstate.last_snap = Some(result);
            result.pos
        }
    }

    /// v0.27 — round-pad diameter handle drag tick.
    fn try_round_resize_tick(
        &self,
        cstate: &FootprintCanvasState,
        world: (f64, f64),
    ) -> Option<canvas::Action<LibraryMessage>> {
        if let Some(pad_idx) = cstate.round_resize_drag {
            let centre = self.state.pads.get(pad_idx).map(|p| p.position_mm);
            if let Some(centre) = centre {
                let dx_mm = world.0 - centre.0;
                let dy_mm = world.1 - centre.1;
                let r_mm = (dx_mm * dx_mm + dy_mm * dy_mm).sqrt();
                let diameter_mm = (2.0 * r_mm).max(0.05);
                self.cache.clear();
                return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                    library_path: self.address.library_path.clone(),
                    table: self.address.table.clone(),
                    row_id: self.address.row_id,
                    msg: EditorMsg::Footprint(FootprintEditorMsg::SketchResizeRoundPad {
                        pad_idx,
                        diameter_mm,
                    }),
                }));
            }
        }
        None
    }

    /// Generic pad / sketch-point / sketch-line drag tick — updates the
    /// drag-moved threshold, keeps the rubber-band endpoint in sync,
    /// and publishes the per-tick move message.
    fn on_pointer_drag_tick(
        &self,
        cstate: &mut FootprintCanvasState,
        cursor_pos: Point,
        world: (f64, f64),
    ) -> Option<canvas::Action<LibraryMessage>> {
        // Kept as `if let Some(drag) = cstate.drag.as_mut()` (not a
        // `let … else` early return) so it matches the pre-split
        // control flow exactly.
        if let Some(drag) = cstate.drag.as_mut() {
            let dx = (cursor_pos.x - drag.press_screen.x).abs();
            let dy = (cursor_pos.y - drag.press_screen.y).abs();
            if !drag.moved && dx.max(dy) >= DRAG_THRESHOLD_PX {
                drag.moved = true;
            }
            // v0.26-I — keep the box-select endpoint in lock-step with
            // the cursor so the draw pass paints a live rubber band.
            if cstate.box_select_anchor_screen.is_some() {
                cstate.box_select_current_screen = Some(cursor_pos);
                self.cache.clear();
            }
            // v0.16 — sketch Point drag tick.
            if drag.moved
                && let Some(point_id) = drag.sketch_point
            {
                return self.drag_tick_point(drag, point_id, world);
            }
            // v0.27 — Line drag tick (translate both endpoints).
            if drag.moved
                && let Some(line_id) = drag.sketch_line
                && let Some(sketch_ref) = self.sketch
            {
                return self.drag_tick_line(drag, line_id, sketch_ref, world);
            }
            if drag.moved && drag.pad_idx != usize::MAX {
                let new_x = world.0 - drag.grab_offset_mm.0;
                let new_y = world.1 - drag.grab_offset_mm.1;
                return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                    library_path: self.address.library_path.clone(),
                    table: self.address.table.clone(),
                    row_id: self.address.row_id,
                    msg: EditorMsg::Footprint(FootprintEditorMsg::MovePad {
                        idx: drag.pad_idx,
                        x_mm: new_x,
                        y_mm: new_y,
                    }),
                }));
            }
        }
        None
    }

    /// Sketch Point drag tick — per-tick delta since the last tick.
    fn drag_tick_point(
        &self,
        drag: &mut DragState,
        point_id: signex_sketch::id::SketchEntityId,
        world: (f64, f64),
    ) -> Option<canvas::Action<LibraryMessage>> {
        let dx_mm = world.0 - drag.last_world.0;
        let dy_mm = world.1 - drag.last_world.1;
        drag.last_world = world;
        self.cache.clear();
        Some(canvas::Action::publish(LibraryMessage::EditorEvent {
            library_path: self.address.library_path.clone(),
            table: self.address.table.clone(),
            row_id: self.address.row_id,
            msg: EditorMsg::Footprint(FootprintEditorMsg::SketchMovePoint {
                id: point_id,
                dx: dx_mm,
                dy: dy_mm,
            }),
        }))
    }

    /// Sketch Line drag tick — the cursor delta is projected onto the
    /// line's perpendicular so an edge only pushes in its natural
    /// resize direction (Fusion-style).
    fn drag_tick_line(
        &self,
        drag: &mut DragState,
        line_id: signex_sketch::id::SketchEntityId,
        sketch_ref: &signex_sketch::SketchData,
        world: (f64, f64),
    ) -> Option<canvas::Action<LibraryMessage>> {
        let raw_dx = world.0 - drag.last_world.0;
        let raw_dy = world.1 - drag.last_world.1;
        // Resolve the line's current direction. Falls back to the raw
        // delta if the lookup fails (entity vanished mid-drag, etc.).
        let endpoints = sketch_ref
            .entities
            .iter()
            .find(|e| e.id == line_id)
            .and_then(|e| match e.kind {
                signex_sketch::entity::EntityKind::Line { start, end } => Some((start, end)),
                _ => None,
            });
        let pos_of = |id: signex_sketch::id::SketchEntityId| -> Option<(f64, f64)> {
            if let Some(solve) = self.state.last_solve.as_ref()
                && let Some(p) = signex_sketch::solver::state::point_xy(
                    id,
                    &solve.result.state,
                    &solve.result.index,
                    sketch_ref,
                )
            {
                return Some(p);
            }
            sketch_ref
                .entities
                .iter()
                .find(|e| e.id == id)
                .and_then(|e| match e.kind {
                    signex_sketch::entity::EntityKind::Point { x, y } => Some((x, y)),
                    _ => None,
                })
        };
        let (dx_mm, dy_mm) = match endpoints.and_then(|(s, e)| pos_of(s).zip(pos_of(e))) {
            Some(((ax, ay), (bx, by))) => {
                let lx = bx - ax;
                let ly = by - ay;
                let llen = (lx * lx + ly * ly).sqrt();
                if llen <= 1e-9 {
                    (raw_dx, raw_dy)
                } else {
                    // Unit perpendicular (rotate tangent +90°):
                    // (-ly, lx)/llen.
                    let nx = -ly / llen;
                    let ny = lx / llen;
                    let proj = raw_dx * nx + raw_dy * ny;
                    (proj * nx, proj * ny)
                }
            }
            None => (raw_dx, raw_dy),
        };
        // Advance last_world by the CONSTRAINED delta so the cursor's
        // parallel motion accumulates.
        drag.last_world = (drag.last_world.0 + dx_mm, drag.last_world.1 + dy_mm);
        self.cache.clear();
        Some(canvas::Action::publish(LibraryMessage::EditorEvent {
            library_path: self.address.library_path.clone(),
            table: self.address.table.clone(),
            row_id: self.address.row_id,
            msg: EditorMsg::Footprint(FootprintEditorMsg::SketchMoveLine {
                id: line_id,
                dx: dx_mm,
                dy: dy_mm,
            }),
        }))
    }

    /// Cursor-move tail — clear the cache for the tools whose ghost
    /// preview tracks the cursor, then publish the footer readout.
    fn cursor_move_cache_and_hover(
        &self,
        world: (f64, f64),
    ) -> Option<canvas::Action<LibraryMessage>> {
        use crate::library::editor::footprint::state::{EditorMode, PadsTool, ToolPending};
        let in_sketch_with_anchor = matches!(self.state.mode, EditorMode::Sketch)
            && !matches!(self.state.tool_pending, ToolPending::Idle);
        let in_pads_place = matches!(self.state.mode, EditorMode::Normal)
            && matches!(
                self.state.pads_tool,
                PadsTool::PlacePad | PadsTool::PlaceVia
            );
        // v0.14 — Place Text Frame redraws on every cursor tick while
        // armed so the drag-rect ghost tracks the live cursor.
        let in_text_frame_place = matches!(self.state.mode, EditorMode::Normal)
            && self.state.pads_tool == PadsTool::PlaceTextFrame;
        // v0.27 — re-render lasso ghost as the cursor moves.
        let in_lasso = self.state.lasso_mode_active && !self.state.lasso_vertices.is_empty();
        let in_touching_line =
            self.state.touching_line_active && self.state.touching_line_first.is_some();
        // v0.27 — Sketch mode also redraws every cursor tick so the
        // custom crosshair tracks the cursor smoothly.
        let in_sketch_mode_for_cursor = matches!(self.state.mode, EditorMode::Sketch);
        if in_sketch_with_anchor
            || in_pads_place
            || in_text_frame_place
            || in_lasso
            || in_touching_line
            || in_sketch_mode_for_cursor
        {
            self.cache.clear();
        }

        // Background hover — push the cursor position so the footer
        // readout updates.
        Some(canvas::Action::publish(LibraryMessage::EditorEvent {
            library_path: self.address.library_path.clone(),
            table: self.address.table.clone(),
            row_id: self.address.row_id,
            msg: EditorMsg::Footprint(FootprintEditorMsg::CursorAt {
                x_mm: world.0,
                y_mm: world.1,
            }),
        }))
    }
}
