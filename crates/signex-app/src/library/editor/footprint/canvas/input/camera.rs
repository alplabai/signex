//! Camera input handling — first-draw fit, one-shot Fit-to-Window,
//! scroll-wheel zoom (cursor-anchored), and middle/right-drag panning.
//!
//! Extracted verbatim from the canvas `Program::update` god-function;
//! behaviour is byte-identical — same conditions, same coordinate
//! math, same `Action::publish` / capture sites.

use iced::mouse;
use iced::widget::canvas;
use iced::{Point, Rectangle};

use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage};

use super::super::{FootprintCanvas, FootprintCanvasState, MAX_SCALE, MIN_SCALE, ZOOM_FACTOR};

impl FootprintCanvas<'_> {
    /// First-draw camera placement.
    /// - With content: fit-to-bounds so every pad / sketch entity
    ///   is visible.
    /// - Without content (fresh `.snxfpt` from "Add New ▸
    ///   Footprint"): centre world origin in the viewport so the
    ///   user lands on (0, 0) rather than the screen's top-left.
    ///   Without this, the default offset (0, 0) renders world
    ///   (0, 0) at screen pixel (0, 0) — the user's drawing area
    ///   starts in the corner and they have to pan to find the
    ///   centre.
    pub(in crate::library::editor::footprint::canvas) fn apply_initial_fit(
        &self,
        cstate: &mut FootprintCanvasState,
        bounds: Rectangle,
    ) {
        if !cstate.did_initial_fit && bounds.width > 0.0 && bounds.height > 0.0 {
            if let Some(bbox) = self.state.content_bbox_mm() {
                cstate.fit_to_bounds(bbox, bounds);
            } else {
                cstate.offset = Point::new(bounds.width / 2.0, bounds.height / 2.0);
                // Keep DEFAULT_PX_PER_MM scale.
            }
            cstate.did_initial_fit = true;
        }
    }

    /// v0.26-C — one-shot Fit-to-Window from the right-click menu.
    /// The dispatcher set `state.fit_pending = true`; we honour it
    /// on the very next event tick (any mouse motion / scroll /
    /// press over the canvas) and publish `FitConsumed` so the
    /// flag clears. Bounds-guard mirrors the first-draw branch.
    pub(in crate::library::editor::footprint::canvas) fn apply_pending_fit(
        &self,
        cstate: &mut FootprintCanvasState,
        bounds: Rectangle,
    ) -> Option<canvas::Action<LibraryMessage>> {
        if self.state.fit_pending && bounds.width > 0.0 && bounds.height > 0.0 {
            if let Some(bbox) = self.state.content_bbox_mm() {
                cstate.fit_to_bounds(bbox, bounds);
            } else {
                cstate.offset = Point::new(bounds.width / 2.0, bounds.height / 2.0);
            }
            cstate.did_initial_fit = true;
            self.cache.clear();
            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                library_path: self.address.library_path.clone(),
                table: self.address.table.clone(),
                row_id: self.address.row_id,
                msg: EditorMsg::Footprint(FootprintEditorMsg::FitConsumed),
            }));
        }
        None
    }

    /// Scroll-wheel zoom, anchored on the cursor.
    pub(in crate::library::editor::footprint::canvas) fn on_wheel_scrolled(
        &self,
        cstate: &mut FootprintCanvasState,
        delta: &mouse::ScrollDelta,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<LibraryMessage>> {
        let scroll_y = match delta {
            mouse::ScrollDelta::Lines { y, .. } => *y,
            mouse::ScrollDelta::Pixels { y, .. } => *y / 50.0,
        };
        let Some(cursor_pos) = cursor.position_in(bounds) else {
            return None;
        };
        if scroll_y == 0.0 {
            return None;
        }
        let factor = if scroll_y > 0.0 {
            ZOOM_FACTOR
        } else {
            1.0 / ZOOM_FACTOR
        };
        let new_scale = (cstate.scale * factor).clamp(MIN_SCALE, MAX_SCALE);
        let actual_factor = new_scale / cstate.scale;
        cstate.offset.x = cursor_pos.x - (cursor_pos.x - cstate.offset.x) * actual_factor;
        cstate.offset.y = cursor_pos.y - (cursor_pos.y - cstate.offset.y) * actual_factor;
        cstate.scale = new_scale;
        self.cache.clear();
        // v0.14.2: same as the panning fix — publish a
        // lightweight cursor-position message + capture so
        // iced renders the new zoom immediately. `capture()`
        // alone froze the canvas at the pre-zoom scale until
        // some unrelated frame fired.
        let world = cstate.screen_to_world(cursor_pos);
        Some(
            canvas::Action::publish(LibraryMessage::EditorEvent {
                library_path: self.address.library_path.clone(),
                table: self.address.table.clone(),
                row_id: self.address.row_id,
                msg: EditorMsg::Footprint(FootprintEditorMsg::CursorAt {
                    x_mm: world.0,
                    y_mm: world.1,
                }),
            })
            .and_capture(),
        )
    }

    /// Middle/right-drag pan step. Returns `Some(action)` when a pan
    /// was applied (mirrors the original always-returns behaviour of
    /// the pan branch), `None` when no pan is in flight so the caller
    /// falls through to the rest of the cursor-move handling.
    pub(in crate::library::editor::footprint::canvas) fn pan_on_cursor_moved(
        &self,
        cstate: &mut FootprintCanvasState,
        cursor_pos: Point,
    ) -> Option<canvas::Action<LibraryMessage>> {
        if cstate.panning
            && let Some(last) = cstate.last_pan_pos
        {
            let dx = cursor_pos.x - last.x;
            let dy = cursor_pos.y - last.y;
            cstate.offset.x += dx;
            cstate.offset.y += dy;
            cstate.last_pan_pos = Some(cursor_pos);
            self.cache.clear();
            // v0.26 — first time the right-button drag crosses
            // the 2 px threshold, mark `pan_moved` so the
            // matching ButtonReleased branch knows to skip
            // the context menu, and close any open menu. The
            // context menu and a pan can't coexist.
            if !cstate.pan_moved && (dx.abs() > 2.0 || dy.abs() > 2.0) {
                cstate.pan_moved = true;
                if self.state.context_menu.is_some() {
                    return Some(
                        canvas::Action::publish(LibraryMessage::EditorEvent {
                            library_path: self.address.library_path.clone(),
                            table: self.address.table.clone(),
                            row_id: self.address.row_id,
                            msg: EditorMsg::Footprint(FootprintEditorMsg::CloseContextMenu),
                        })
                        .and_capture(),
                    );
                }
            }
            // v0.14.2: publish the cursor world position
            // alongside .and_capture(). `capture()` alone is
            // not enough to make iced schedule a redraw, so
            // panning was visually frozen until the next
            // unrelated frame fired. Mirroring the schematic
            // canvas's pattern: publish a lightweight
            // FootprintCursorAt + capture, and iced renders
            // the new offset immediately.
            let world = cstate.screen_to_world(cursor_pos);
            return Some(
                canvas::Action::publish(LibraryMessage::EditorEvent {
                    library_path: self.address.library_path.clone(),
                    table: self.address.table.clone(),
                    row_id: self.address.row_id,
                    msg: EditorMsg::Footprint(FootprintEditorMsg::CursorAt {
                        x_mm: world.0,
                        y_mm: world.1,
                    }),
                })
                .and_capture(),
            );
        }
        None
    }
}
