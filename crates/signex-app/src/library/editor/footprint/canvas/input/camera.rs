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

use super::super::{FootprintCanvas, FootprintCanvasState, MIN_SCALE};

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
    ///
    /// Returns the scale the fit settled on as a published message, since
    /// the camera lives in the widget and the status bar's zoom reads the
    /// model (#1808).
    pub(in crate::library::editor::footprint::canvas) fn apply_initial_fit(
        &self,
        cstate: &mut FootprintCanvasState,
        bounds: Rectangle,
    ) -> Option<canvas::Action<LibraryMessage>> {
        if cstate.did_initial_fit || bounds.width <= 0.0 || bounds.height <= 0.0 {
            return None;
        }
        if let Some(bbox) = self.state.content_bbox_mm() {
            fit_clear_of_chrome(cstate, bbox, bounds);
        } else {
            cstate.offset = Point::new(bounds.width / 2.0, bounds.height / 2.0);
            // Keep DEFAULT_PX_PER_MM scale.
        }
        cstate.did_initial_fit = true;
        // A new camera starts from the fit, not from a zoom the pill asked
        // of an earlier camera.
        cstate.applied_zoom_seq = self.state.zoom_request.seq;
        Some(canvas::Action::publish(LibraryMessage::EditorEvent {
            library_path: self.address.library_path.clone(),
            table: self.address.table.clone(),
            row_id: self.address.row_id,
            msg: EditorMsg::Footprint(FootprintEditorMsg::ViewScaleChanged {
                px_per_mm: cstate.scale,
            }),
        }))
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
                fit_clear_of_chrome(cstate, bbox, bounds);
            } else {
                cstate.offset = Point::new(bounds.width / 2.0, bounds.height / 2.0);
            }
            cstate.did_initial_fit = true;
            self.cache.clear();
            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                library_path: self.address.library_path.clone(),
                table: self.address.table.clone(),
                row_id: self.address.row_id,
                msg: EditorMsg::Footprint(FootprintEditorMsg::FitConsumed {
                    px_per_mm: cstate.scale,
                }),
            }));
        }
        None
    }

    /// The zoom pill's `-` / `+`: apply a [`ZoomRequest`] this camera has
    /// not applied yet, about the canvas centre, and publish the new scale
    /// for the pill's readout.
    ///
    /// [`ZoomRequest`]: crate::library::editor::footprint::state::ZoomRequest
    pub(in crate::library::editor::footprint::canvas) fn apply_pending_zoom(
        &self,
        cstate: &mut FootprintCanvasState,
        bounds: Rectangle,
    ) -> Option<canvas::Action<LibraryMessage>> {
        let request = self.state.zoom_request;
        if request.seq == cstate.applied_zoom_seq || bounds.width <= 0.0 || bounds.height <= 0.0 {
            return None;
        }
        cstate.applied_zoom_seq = request.seq;
        let centre = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        cstate.zoom_about(centre, request.factor, MIN_SCALE);
        self.cache.clear();
        Some(canvas::Action::publish(LibraryMessage::EditorEvent {
            library_path: self.address.library_path.clone(),
            table: self.address.table.clone(),
            row_id: self.address.row_id,
            msg: EditorMsg::Footprint(FootprintEditorMsg::ViewScaleChanged {
                px_per_mm: cstate.scale,
            }),
        }))
    }

    /// Scroll-wheel zoom, anchored on the cursor, or a pan: see
    /// [`crate::canvas::wheel_intent`] for the preferences.
    pub(in crate::library::editor::footprint::canvas) fn on_wheel_scrolled(
        &self,
        cstate: &mut FootprintCanvasState,
        delta: &mouse::ScrollDelta,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<LibraryMessage>> {
        let intent = crate::canvas::wheel_intent(
            delta,
            50.0,
            self.trackpad_pan,
            self.wheel_mode,
            crate::canvas::WheelKeys {
                zoom: cstate.current_modifiers.command() || cstate.current_modifiers.control(),
                shift: cstate.current_modifiers.shift(),
            },
        );
        let cursor_pos = cursor.position_in(bounds)?;
        match intent {
            crate::canvas::ScrollIntent::Zoom { notches } => {
                if notches == 0.0 {
                    return None;
                }
                let factor = crate::canvas::Camera::zoom_factor_for_scroll(self.zoom_step, notches);
                if !factor.is_finite() || factor == 1.0 {
                    return None;
                }
                cstate.zoom_about(cursor_pos, factor, MIN_SCALE);
            }
            crate::canvas::ScrollIntent::Pan { dx, dy } => {
                if dx == 0.0 && dy == 0.0 {
                    return None;
                }
                cstate.pan_by(iced::Vector::new(dx, dy));
            }
        }
        Some(self.publish_view_changed(cstate, cursor_pos))
    }

    /// Trackpad pinch zoom, anchored on the cursor (ignored outside the
    /// canvas).
    pub(in crate::library::editor::footprint::canvas) fn on_pinch_gesture(
        &self,
        cstate: &mut FootprintCanvasState,
        delta: f32,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<LibraryMessage>> {
        let cursor_pos = cursor.position_in(bounds)?;
        let factor = crate::canvas::Camera::pinch_factor(delta);
        if factor == 1.0 {
            return None;
        }
        cstate.zoom_about(cursor_pos, factor, MIN_SCALE);
        Some(self.publish_view_changed(cstate, cursor_pos))
    }

    /// Redraw after a zoom or pan, and publish the cursor world position.
    fn publish_view_changed(
        &self,
        cstate: &FootprintCanvasState,
        cursor_pos: Point,
    ) -> canvas::Action<LibraryMessage> {
        self.cache.clear();
        // v0.14.2: same as the panning fix — publish a
        // lightweight cursor-position message + capture so
        // iced renders the new zoom immediately. `capture()`
        // alone froze the canvas at the pre-zoom scale until
        // some unrelated frame fired.
        let world = cstate.screen_to_world(cursor_pos);
        canvas::Action::publish(LibraryMessage::EditorEvent {
            library_path: self.address.library_path.clone(),
            table: self.address.table.clone(),
            row_id: self.address.row_id,
            msg: EditorMsg::Footprint(FootprintEditorMsg::CursorAt {
                x_mm: world.0,
                y_mm: world.1,
                px_per_mm: cstate.scale,
            }),
        })
        .and_capture()
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
                        px_per_mm: cstate.scale,
                    }),
                })
                .and_capture(),
            );
        }
        None
    }
}

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;
/// iced's default text line height, as a multiple of the font size.
const TEXT_LINE_HEIGHT: f32 = 1.3;

/// Room the editor's floating chrome takes at the canvas edges, which a fit
/// keeps content out of: the two state chips under the Active Bar at the
/// top (`standalone::dof_chip`) and the zoom pill at the bottom right
/// (`crate::canvas::zoom_pill`). Derived from the same type and spacing
/// scales those overlays are built from.
pub(in crate::library::editor::footprint::canvas) fn chrome_insets() -> iced::Padding {
    // A chip: one caption line, its padding and its 1 px border.
    let chip = TS.caption * TEXT_LINE_HEIGHT + 2.0 * SP.xs + 2.0;
    let (_, pill_h) = crate::canvas::zoom_pill::pill_size();
    iced::Padding {
        top: SP.xl + 2.0 * chip + SP.m + SP.m,
        bottom: SP.l + pill_h + SP.m,
        left: SP.xl,
        right: SP.xl,
    }
}

/// Fit `bbox` into the part of `bounds` the chrome leaves free. A canvas
/// too small for that gets the plain whole-canvas fit instead.
fn fit_clear_of_chrome(
    cstate: &mut FootprintCanvasState,
    bbox: (f64, f64, f64, f64),
    bounds: Rectangle,
) {
    let insets = chrome_insets();
    let free_w = bounds.width - insets.left - insets.right;
    let free_h = bounds.height - insets.top - insets.bottom;
    if free_w < bounds.width * 0.5 || free_h < bounds.height * 0.5 {
        cstate.fit_to_bounds(bbox, bounds);
        return;
    }
    let free = Rectangle::new(Point::ORIGIN, iced::Size::new(free_w, free_h));
    cstate.fit_to_bounds(bbox, free);
    cstate.offset.x += insets.left;
    cstate.offset.y += insets.top;
}
