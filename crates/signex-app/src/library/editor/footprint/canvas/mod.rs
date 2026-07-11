//! Footprint editor 2D canvas — pure CPU rendering via
//! `iced::widget::Canvas`. Pads are drawn as axis-aligned rectangles
//! coloured by their primary layer; courtyard renders as a yellow
//! outline; graphics (silk/fab) trace through their stored layer
//! colour.
//!
//! Input model — middle/right-drag pans, scroll-wheel zooms (cursor
//! anchored), left-click on a pad selects it, left-drag moves the
//! selected pad, left-click on empty canvas adds a pad. Delete-key
//! handling lives in `library/editor/footprint/mod.rs`'s key event
//! since Canvas doesn't surface keyboard events.
//!
//! Submodules:
//! - [`geometry`] — pure helpers (point-in-poly, segment distance).
//! - [`hit_test`] — sketch entity hit-test + `sketch_snap`.
//! - [`input`] — the `Program::update` event handlers.
//! - [`draw`] — the `Program::draw` layer methods plus the free
//!   renderers they call: grid (fine + coarse), pad (copper / hole /
//!   number + Pads-mode tool preview), silk (front + back graphics),
//!   and sketch (entity overlay, DOF arrows, snap glyph, constraint
//!   icons, filled closed loops, and the multi-click ghost preview).

mod geometry;
mod hit_test;
mod input;
mod draw;

#[cfg(test)]
mod tests;

// `draw_pads_tool_preview` is the one sub-draw still invoked inline by
// the `draw` dispatcher; the rest of the `draw_*` / `geometry` /
// `hit_test` helpers are called from the `input` / `draw` submodules,
// which import them directly. `geometry` helpers are still used by
// `silk_f_hit_at` below.
use draw::draw_pads_tool_preview;
use geometry::{point_in_polygon, point_to_segment_dist, polygon_outline_hit};

use iced::event::Event;
use iced::keyboard;
use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Point, Rectangle, Renderer, Theme};

use crate::library::messages::LibraryMessage;
use crate::library::state::EditorAddress;

use super::snap::SnapResult;
use super::state::FootprintEditorState;

/// Drag threshold in screen pixels — below this we treat the press
/// as a click, above this as a drag.
const DRAG_THRESHOLD_PX: f32 = 3.0;

/// Pixel-per-mm at the canvas's "100%" zoom — picked so a 5×5 mm
/// SOT-23 fits comfortably in a 600px-wide tab.
const DEFAULT_PX_PER_MM: f32 = 30.0;

const MIN_SCALE: f32 = 5.0;
const MAX_SCALE: f32 = 400.0;
const ZOOM_FACTOR: f32 = 1.15;

/// Canvas-only state owned by `iced::widget::Canvas`. The editor's
/// model lives in `FootprintEditorState`; this struct only holds
/// per-instance interaction state (camera, drag flags).
#[derive(Debug)]
pub struct FootprintCanvasState {
    /// World→screen affine: `screen = world * scale + offset`.
    /// `scale` is in pixels-per-mm.
    pub scale: f32,
    pub offset: Point,
    /// Auto-fit on the first draw — toggled false once we've seen
    /// non-zero bounds at least once.
    pub did_initial_fit: bool,
    panning: bool,
    last_pan_pos: Option<Point>,
    /// v0.26 — `true` once the right-button drag has moved further than
    /// 2 px from the press point. Drives the right-release branch
    /// between "show context menu" (no motion) and "this was a pan,
    /// stay quiet" (motion crossed the threshold). Reset on every
    /// right-press.
    pan_moved: bool,
    /// v0.26-I — rubber-band selection state. `Some` while the user
    /// is left-dragging on empty canvas with the Select tool active.
    /// Stores the press position (screen-space) so the draw pass can
    /// render the rectangle from press → current cursor. Cleared on
    /// release.
    box_select_anchor_screen: Option<Point>,
    /// v0.26-I — current cursor screen position during a box-select
    /// drag. Updated per CursorMoved tick so the draw pass can
    /// render the rubber-band rectangle to the live cursor.
    box_select_current_screen: Option<Point>,
    /// Drag state — `Some` while the user is mid-drag on a pad.
    drag: Option<DragState>,
    /// The pad index reported as `selected_pad` on the model the
    /// last time we drew. Used so the press handler can tell whether
    /// the click was on the already-selected pad.
    last_known_selected: Option<usize>,
    /// v0.16.1 — last snap outcome at the cursor. `Some` when in
    /// Sketch mode and a snap fired; drives the snap-kind badge in
    /// `draw_sketch_tool_preview`. Cleared each tick before the
    /// next compute.
    last_snap: Option<SnapResult>,
    /// v0.27 — `Some(pad_idx)` while the user is mid-drag on a Round
    /// pad's diameter handle (cyan disc on the east edge of the
    /// minted Circle entity). Per-tick cursor moves publish
    /// `FootprintSketchResizeRoundPad`. Cleared on release.
    round_resize_drag: Option<usize>,
    /// v0.27 — most recent modifier state from the iced
    /// ModifiersChanged event. Mouse events don't carry modifiers
    /// on iced 0.14, so we track them out-of-band and read on press
    /// for Ctrl/Cmd-click toggle + Shift-click extend semantics.
    current_modifiers: keyboard::Modifiers,
}

#[derive(Debug, Clone, Copy)]
struct DragState {
    /// `usize::MAX` for "click on empty canvas" (used by tools that
    /// need a release event to commit, like Place Pad in Pads mode
    /// and the click-add-Point fallback in Sketch mode). Otherwise
    /// the index into `state.pads`.
    pad_idx: usize,
    /// v0.16 — `Some(id)` when the drag originated on a sketch
    /// `Point` entity. Active in Sketch mode + Select tool;
    /// per-tick CursorMoved publishes `FootprintSketchMovePoint`
    /// with the world-mm delta.
    sketch_point: Option<signex_sketch::id::SketchEntityId>,
    /// v0.27 — `Some(id)` when the drag originated on a sketch
    /// `Line` entity. Per-tick CursorMoved publishes
    /// `FootprintSketchMoveLine` with the world-mm delta; the
    /// dispatcher translates both endpoints in one solver pass.
    sketch_line: Option<signex_sketch::id::SketchEntityId>,
    /// World-mm offset between the drag origin and the pad/Point
    /// centre. Subtract from cursor position to get the pad's new
    /// centre OR (for sketch Point drags) compute the per-tick
    /// delta `(world - grab_offset_mm) - last_pos`.
    grab_offset_mm: (f64, f64),
    /// World-mm position from the previous CursorMoved tick — used
    /// by sketch-Point drags to compute the per-tick delta the
    /// dispatcher's `FootprintSketchMovePoint` handler expects.
    last_world: (f64, f64),
    /// Screen-pixel position the press started at. Used to gate
    /// "did this drag actually move?".
    press_screen: Point,
    moved: bool,
}

impl Default for FootprintCanvasState {
    fn default() -> Self {
        Self {
            scale: DEFAULT_PX_PER_MM,
            offset: Point::new(0.0, 0.0),
            did_initial_fit: false,
            panning: false,
            last_pan_pos: None,
            pan_moved: false,
            box_select_anchor_screen: None,
            box_select_current_screen: None,
            drag: None,
            last_known_selected: None,
            last_snap: None,
            round_resize_drag: None,
            current_modifiers: keyboard::Modifiers::empty(),
        }
    }
}

impl FootprintCanvasState {
    fn world_to_screen(&self, world: (f64, f64)) -> Point {
        Point::new(
            world.0 as f32 * self.scale + self.offset.x,
            world.1 as f32 * self.scale + self.offset.y,
        )
    }

    fn screen_to_world(&self, screen: Point) -> (f64, f64) {
        (
            ((screen.x - self.offset.x) / self.scale) as f64,
            ((screen.y - self.offset.y) / self.scale) as f64,
        )
    }

    fn fit_to_bounds(&mut self, world_bbox: (f64, f64, f64, f64), viewport: Rectangle) {
        let (min_x, min_y, max_x, max_y) = world_bbox;
        let w = (max_x - min_x).max(1e-3);
        let h = (max_y - min_y).max(1e-3);
        let pad = 12.0_f32;
        let avail_w = (viewport.width - pad * 2.0).max(1.0);
        let avail_h = (viewport.height - pad * 2.0).max(1.0);
        let scale_x = avail_w / w as f32;
        let scale_y = avail_h / h as f32;
        self.scale = scale_x.min(scale_y).clamp(MIN_SCALE, MAX_SCALE);
        let cx = ((min_x + max_x) / 2.0) as f32;
        let cy = ((min_y + max_y) / 2.0) as f32;
        self.offset = Point::new(
            viewport.width / 2.0 - cx * self.scale,
            viewport.height / 2.0 - cy * self.scale,
        );
    }
}

/// The Canvas program. Holds a snapshot of the model — `view()`
/// rebuilds this every frame, so we only need a borrowed reference.
pub struct FootprintCanvas<'a> {
    pub state: &'a FootprintEditorState,
    pub address: EditorAddress,
    pub bg_color: Color,
    pub grid_color: Color,
    /// Pre-allocated empty cache so `draw` can reuse iced's caching
    /// layer if profiling later motivates it.
    pub cache: &'a canvas::Cache,
    /// v0.13.1 Phase 6.2 — sketch entities are read-only here so the
    /// canvas can render them when [`EditorMode::Sketch`] is active.
    /// `None` for footprints with no sketch field set (legacy v1).
    pub sketch: Option<&'a signex_sketch::SketchData>,
    /// v0.18.16 — silk-front graphics (`Line` / `Arc` / `Text` /
    /// `Rectangle` / `Circle`). Read-only on the canvas side; the
    /// active-bar tools (Place String / Track / Arc / Polygon)
    /// commit through the dispatcher into
    /// `editor.primitive_mut().silk_f`.
    pub silk_f: &'a [signex_library::primitive::footprint::FpGraphic],
    /// v0.18.16 — silk-back graphics (mirror layer for B.SilkS).
    pub silk_b: &'a [signex_library::primitive::footprint::FpGraphic],
}

impl<'a> canvas::Program<LibraryMessage> for FootprintCanvas<'a> {
    type State = FootprintCanvasState;

    fn update(
        &self,
        cstate: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<LibraryMessage>> {
        // First-draw camera placement + one-shot Fit-to-Window (see
        // `input::camera`). `apply_initial_fit` only mutates the
        // camera; `apply_pending_fit` consumes the event tick when a
        // Fit-to-Window is queued.
        self.apply_initial_fit(cstate, bounds);
        if let Some(action) = self.apply_pending_fit(cstate, bounds) {
            return Some(action);
        }

        cstate.last_known_selected = self.state.selected_pad;

        // Dispatch each event kind to its extracted concern method
        // (see the `input` submodule). The order + conditions are
        // identical to the pre-split god-function.
        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                self.on_wheel_scrolled(cstate, delta, bounds, cursor)
            }
            Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                self.on_button_pressed(cstate, button, bounds, cursor)
            }
            Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                self.on_button_released(cstate, button, bounds, cursor)
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                self.on_cursor_moved(cstate, bounds, cursor)
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(mods)) => {
                self.on_modifiers_changed(cstate, mods)
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) => self.on_key_pressed(key, modifiers, text.as_ref().and_then(|s| s.chars().next())),
            _ => None,
        }
    }

    fn draw(
        &self,
        cstate: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let cursor_screen = cursor.position_in(bounds);
        let geom = self.cache.draw(renderer, bounds.size(), |frame| {
            // Layers are painted bottom-to-top; this z-order is
            // load-bearing and identical to the pre-split god-function.
            // Each layer's body lives in the `draw` submodule.
            self.draw_background_and_grid(frame, cstate, bounds);
            self.draw_guides(frame, cstate, bounds);
            self.draw_origin_crosshair(frame, cstate);
            self.draw_silk_layers(frame, cstate);
            self.draw_courtyard(frame, cstate);
            self.draw_pads_layer(frame, cstate);
            self.draw_array_badges(frame, cstate);
            self.draw_place_pad_ghost(frame, cstate);
            self.draw_place_via_ghost(frame, cstate);
            // v0.18.16 — Pads-mode multi-click gesture previews
            // (Track / Arc / Polygon ghost lines).
            if matches!(self.state.mode, super::state::EditorMode::Normal) {
                draw_pads_tool_preview(frame, cstate, self.state);
            }
            self.draw_sketch_reticle(frame, cstate, cursor_screen);
            self.draw_select_cursor_mark(frame, cstate, cursor_screen);
            self.draw_touching_line_ghost(frame, cstate);
            self.draw_lasso_ghost(frame, cstate);
            self.draw_rubber_band(frame, cstate);
            self.draw_sketch_overlays(frame, cstate);
        });

        vec![geom]
    }

    fn mouse_interaction(
        &self,
        cstate: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cstate.panning {
            return mouse::Interaction::Grabbing;
        }
        if cstate.drag.is_some() {
            return mouse::Interaction::Grab;
        }

        // v0.16.3 — drag-corner cursor indicators in Sketch mode.
        // Hover a pad-outline corner / edge to see the matching
        // resize cursor (↘/↗ for corners, ↔ for vertical edges, ↕
        // for horizontal edges). Drag actually works since v0.16.1.1
        // — this is the visual hint Caner asked for.
        use crate::library::editor::footprint::state::EditorMode;
        if self.state.mode == EditorMode::Sketch {
            if let Some(c) = cursor.position_in(bounds) {
                const CORNER_TOL: f32 = 6.0;
                const EDGE_TOL: f32 = 4.0;
                for pad in &self.state.pads {
                    if pad.corner_entity_ids.is_none() {
                        continue;
                    }
                    let (cx, cy) = pad.position_mm;
                    let (w, h) = pad.size_mm;
                    let half_w = w * 0.5;
                    let half_h = h * 0.5;
                    let nw = cstate.world_to_screen((cx - half_w, cy - half_h));
                    let ne = cstate.world_to_screen((cx + half_w, cy - half_h));
                    let se = cstate.world_to_screen((cx + half_w, cy + half_h));
                    let sw = cstate.world_to_screen((cx - half_w, cy + half_h));

                    // Corners take priority over edges. Diagonal sign
                    // (relative to the centre) picks the cursor:
                    // (dx * dy > 0) => NW or SE => DiagonallyDown (↘)
                    // (dx * dy < 0) => NE or SW => DiagonallyUp (↗)
                    let corners = [(nw, true), (ne, false), (se, true), (sw, false)];
                    for (corner_pt, is_down) in corners {
                        let dx = (c.x - corner_pt.x).abs();
                        let dy = (c.y - corner_pt.y).abs();
                        if dx <= CORNER_TOL && dy <= CORNER_TOL {
                            return if is_down {
                                mouse::Interaction::ResizingDiagonallyDown
                            } else {
                                mouse::Interaction::ResizingDiagonallyUp
                            };
                        }
                    }

                    // Edges — point lies within the bbox AND within
                    // EDGE_TOL of one of the four edge lines.
                    let inside_x = c.x >= nw.x - EDGE_TOL && c.x <= se.x + EDGE_TOL;
                    let inside_y = c.y >= nw.y - EDGE_TOL && c.y <= se.y + EDGE_TOL;
                    if inside_x && inside_y {
                        let near_top = (c.y - nw.y).abs() <= EDGE_TOL;
                        let near_bottom = (c.y - se.y).abs() <= EDGE_TOL;
                        let near_left = (c.x - nw.x).abs() <= EDGE_TOL;
                        let near_right = (c.x - se.x).abs() <= EDGE_TOL;
                        if near_top || near_bottom {
                            return mouse::Interaction::ResizingVertically;
                        }
                        if near_left || near_right {
                            return mouse::Interaction::ResizingHorizontally;
                        }
                    }
                }
            }
        }

        if cursor.is_over(bounds) {
            // v0.27 — in Sketch mode + Select tool, hover-test the
            // sketch's Line entities and surface a Resize cursor
            // (↔ for vertical lines that resize horizontally, ↕
            // for horizontal lines that resize vertically, ✥ for
            // diagonals). This is the visible cue that the edge is
            // draggable. Placement tools still hide the cursor so
            // the reticle is the only visual marker.
            if self.state.mode == EditorMode::Sketch {
                if self.state.active_tool == super::state::SketchTool::Select
                    && let Some(c) = cursor.position_in(bounds)
                    && let Some(sketch_ref) = self.sketch
                {
                    const LINE_HIT_TOL_PX: f32 = 6.0;
                    let world = cstate.screen_to_world(c);
                    let tol_mm = (LINE_HIT_TOL_PX / cstate.scale.max(1.0)) as f64;
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
                    for ent in &sketch_ref.entities {
                        if let signex_sketch::entity::EntityKind::Line { start, end } = ent.kind
                            && let (Some(a), Some(b)) = (pos_of(start), pos_of(end))
                        {
                            let dx = b.0 - a.0;
                            let dy = b.1 - a.1;
                            let llen2 = dx * dx + dy * dy;
                            if llen2 <= 1e-12 {
                                continue;
                            }
                            let t = ((world.0 - a.0) * dx + (world.1 - a.1) * dy) / llen2;
                            let tc = t.clamp(0.0, 1.0);
                            let px = a.0 + tc * dx;
                            let py = a.1 + tc * dy;
                            let d2 = (px - world.0).powi(2) + (py - world.1).powi(2);
                            if d2 <= tol_mm * tol_mm {
                                // v0.27 — line-hover resize cue is
                                // drawn in-canvas as a rotated
                                // double-headed arrow (see draw()).
                                // Returning Hidden here keeps the
                                // OS cursor invisible so the canvas
                                // glyph is the only indicator at
                                // the exact line angle. The
                                // previous four-cardinal bucket
                                // (↕↔↗↘) only matched line angles
                                // within ±22.5° of an axis or
                                // diagonal — for arbitrary angles
                                // it read as "off".
                                return mouse::Interaction::Hidden;
                            }
                        }
                    }
                }
                // Sketch mode hides the OS cursor in BOTH Select
                // and Placement tools. Select draws its own dark
                // "+" at the raw cursor position (see draw); the
                // OS Crosshair was rendering as a pale "ghost" on
                // the white canvas with the user's cursor scheme
                // and breaking the dark-on-white expectation.
                mouse::Interaction::Hidden
            } else {
                mouse::Interaction::Crosshair
            }
        } else {
            mouse::Interaction::default()
        }
    }
}

/// v0.18.18 — bounding-box hit test for silk-front graphics.
/// Returns the index of the first graphic whose hit-test contains
/// `(x, y)` (in mm), with a small tolerance to make thin shapes
/// reachable. Iterates in reverse so the topmost (most recently
/// placed) graphic wins on overlap.
///
/// v0.18.25 — Line/Arc/Circle/Rectangle/Polygon use shape-tight
/// hit-tests rather than AABB. Filled variants still match the
/// interior; outlined variants only match within `tolerance_mm` of
/// the stroke. Text continues to use AABB (the bake step doesn't
/// expose per-glyph geometry yet).
pub(super) fn silk_f_hit_at(
    silk_f: &[signex_library::primitive::footprint::FpGraphic],
    x: f64,
    y: f64,
    tolerance_mm: f64,
) -> Option<usize> {
    use signex_library::primitive::footprint::FpGraphicKind;
    let t = tolerance_mm.max(0.05);
    for (idx, g) in silk_f.iter().enumerate().rev() {
        let hit = match &g.kind {
            FpGraphicKind::Line { from, to } => {
                point_to_segment_dist(x, y, from[0], from[1], to[0], to[1]) <= t
            }
            FpGraphicKind::Rectangle { from, to } => {
                let min_x = from[0].min(to[0]);
                let min_y = from[1].min(to[1]);
                let max_x = from[0].max(to[0]);
                let max_y = from[1].max(to[1]);
                if g.filled {
                    x >= min_x - t && x <= max_x + t && y >= min_y - t && y <= max_y + t
                } else {
                    let inside_x = x >= min_x - t && x <= max_x + t;
                    let inside_y = y >= min_y - t && y <= max_y + t;
                    let near_left = (x - min_x).abs() <= t;
                    let near_right = (x - max_x).abs() <= t;
                    let near_top = (y - min_y).abs() <= t;
                    let near_bot = (y - max_y).abs() <= t;
                    (inside_y && (near_left || near_right)) || (inside_x && (near_top || near_bot))
                }
            }
            FpGraphicKind::Circle { center, radius } => {
                let dx = x - center[0];
                let dy = y - center[1];
                let dist = (dx * dx + dy * dy).sqrt();
                if g.filled {
                    dist <= *radius + t
                } else {
                    (dist - *radius).abs() <= t
                }
            }
            FpGraphicKind::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                let dx = x - center[0];
                let dy = y - center[1];
                let dist = (dx * dx + dy * dy).sqrt();
                if (dist - *radius).abs() > t {
                    false
                } else {
                    // Angular gating: cursor's angle relative to centre
                    // must lie within [start, end] (CCW). Angles are
                    // wrapped into [0, 360) before comparison so the
                    // arc can cross the 0° seam.
                    //
                    // The raw sweep `end_deg - start_deg` discriminates
                    // zero-sweep degenerates (`< 1e-6` → no hit) from
                    // full-circle arcs (`>= 359.999` → always hit on
                    // the radius ring). The intermediate case runs the
                    // CCW [a, b] containment test with a 360° unwrap
                    // so seam-crossing arcs work.
                    let raw_sweep = (end_deg - start_deg).abs();
                    if raw_sweep < 1e-6 {
                        // Zero-sweep degenerate — match the old AABB
                        // behaviour by failing the hit (a zero-sweep
                        // arc has no visible body).
                        false
                    } else if raw_sweep >= 359.999 {
                        // Full circle (or more). Hit anywhere on the
                        // radius ring (already gated above).
                        true
                    } else {
                        let mut angle_deg = dy.atan2(dx).to_degrees();
                        if angle_deg < 0.0 {
                            angle_deg += 360.0;
                        }
                        let a = start_deg.rem_euclid(360.0);
                        let mut b = end_deg.rem_euclid(360.0);
                        if b < a {
                            b += 360.0;
                        }
                        let mut p = angle_deg;
                        if p < a {
                            p += 360.0;
                        }
                        p >= a && p <= b
                    }
                }
            }
            FpGraphicKind::Text {
                position,
                content,
                size,
                ..
            } => {
                // Text stays AABB — the bake step doesn't expose
                // per-glyph metrics; the placeholder width estimate
                // is stable enough for selection purposes.
                let w = (content.chars().count() as f64) * size * 0.6;
                let h = *size;
                x >= position[0] - t
                    && x <= position[0] + w + t
                    && y >= position[1] - t
                    && y <= position[1] + h + t
            }
            FpGraphicKind::Polygon { vertices } => {
                if vertices.len() < 2 {
                    false
                } else if g.filled {
                    point_in_polygon(x, y, vertices)
                } else {
                    polygon_outline_hit(x, y, vertices, t)
                }
            }
        };
        if hit {
            return Some(idx);
        }
    }
    None
}
