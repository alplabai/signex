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
//! - [`draw_grid`] — fine + coarse grid rendering.
//! - [`draw_pad`] — pad copper / hole / number + Pads-mode tool preview.
//! - [`draw_silk`] — silk-front + silk-back graphics renderer.
//! - [`draw_sketch`] — sketch entity overlay, DOF arrows, snap glyph,
//!   constraint icons, filled closed loops, and the multi-click ghost
//!   preview for sketch drawing tools.

mod draw_grid;
mod draw_pad;
mod draw_silk;
mod draw_sketch;
mod geometry;
mod hit_test;
mod input;

#[cfg(test)]
mod tests;

use draw_grid::{draw_grid, draw_grid_dots};
use draw_pad::{draw_pad, draw_pads_tool_preview};
use draw_silk::draw_silk_graphics;
use draw_sketch::{
    ClosedLoop, draw_dof_direction_arrows, draw_sketch_overlay, draw_sketch_snap_glyph,
    draw_sketch_tool_preview, find_closed_loops,
};
use geometry::{point_in_polygon, point_to_segment_dist, polygon_outline_hit};
use hit_test::{sketch_hit_other, sketch_snap};

use iced::event::Event;
use iced::keyboard;
use iced::mouse;
use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Radians, Rectangle, Renderer, Theme, Vector};

use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage};
use crate::library::state::EditorAddress;

use super::layers::FpLayer;
use super::snap::{self, SnapKind, SnapResult};
use super::state::{EditorPad, FootprintEditorState};

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
            // v0.27 — Sketch mode flips to a Fusion-style white
            // canvas so the geometric primitives (Points / Lines /
            // Arcs / Circles) read against a familiar high-contrast
            // background. Pads-mode keeps the dark theme that
            // matches Altium's PCB Library editor. The grid colour
            // also follows so coarse / fine grid stays visible
            // against either backdrop.
            use crate::library::editor::footprint::state::EditorMode;
            let in_sketch = matches!(self.state.mode, EditorMode::Sketch);
            let bg = if in_sketch {
                Color::WHITE
            } else {
                self.bg_color
            };
            // v0.27 — Fusion-style sketch grid: darker base so the
            // grid reads cleanly against the white canvas. Pure
            // grey (0.55) at higher alpha lands at ~#BFBFBF —
            // matches Fusion's mid-grey grid weight rather than the
            // washed-out near-white we had before.
            let grid = if in_sketch {
                Color::from_rgba(0.55, 0.55, 0.55, 1.0)
            } else {
                self.grid_color
            };
            frame.fill_rectangle(Point::ORIGIN, bounds.size(), bg);

            // v0.18.19 — fine + coarse grid display follows the
            // Cartesian Grid Editor's per-style picker. Step is
            // driven by `snap_options.grid_step_mm`; the coarse
            // overlay sits at `multiplier × fine_step`.
            use crate::library::editor::footprint::state::GridDisplay as Gd;
            let fine_step = (self.state.snap_options.grid_step_mm as f32) * cstate.scale;
            let multiplier = self.state.snap_options.coarse_multiplier.max(1) as f32;
            let coarse_step = fine_step * multiplier;
            let fine_style = self.state.snap_options.fine_grid_display;
            let coarse_style = self.state.snap_options.coarse_grid_display;
            // Fine pass — only when each cell is at least 6 px wide
            // so we don't chew tessellation budget on dense grids
            // at low zoom.
            // v0.27 — Sketch mode runs on a white background. The
            // grid's base RGB is darker (0.55 grey) so the alpha
            // multiplier can stay modest while still reading
            // visibly. Fine = 50% alpha → ~#BFBFBF, matches the
            // Fusion sketch grid weight.
            let (fine_alpha, coarse_alpha) = if in_sketch {
                (0.50, 0.55)
            } else {
                (0.10, 0.30)
            };
            if fine_step >= 6.0 {
                let fine_color = Color {
                    a: fine_alpha,
                    ..grid
                };
                match fine_style {
                    Gd::Lines => {
                        draw_grid(frame, bounds, cstate.offset, fine_step, fine_color);
                    }
                    Gd::Dots => {
                        draw_grid_dots(frame, bounds, cstate.offset, fine_step, fine_color);
                    }
                    Gd::Hidden => {}
                }
            }
            // v0.27 — Fusion-style sketch grid uses ONLY a single
            // fine grid (no coarse overlay). The coarse pass below
            // is skipped while in sketch mode; Pads-mode keeps the
            // 2-tier grid for Altium parity.
            if !in_sketch && coarse_step >= 8.0 {
                let coarse_color = Color {
                    a: coarse_alpha,
                    ..grid
                };
                match coarse_style {
                    Gd::Lines => {
                        draw_grid(frame, bounds, cstate.offset, coarse_step, coarse_color);
                    }
                    Gd::Dots => {
                        draw_grid_dots(frame, bounds, cstate.offset, coarse_step, coarse_color);
                    }
                    Gd::Hidden => {}
                }
            }

            // v0.18.20 — Altium-style guide lines. Each enabled guide
            // is a full-bleed line at the user-set world coordinate on
            // its axis. Cyan dashed look mirrors Altium's snap-guide
            // UX (snap-to-guides hook lands separately).
            {
                use crate::library::editor::footprint::state::GuideAxis;
                let guide_color = Color::from_rgba(0.30, 0.85, 0.95, 0.55);
                let dash_segments: &[f32] = &[6.0, 4.0];
                for g in self.state.guides.iter().filter(|g| g.enabled) {
                    let stroke = Stroke {
                        line_dash: canvas::LineDash {
                            segments: dash_segments,
                            offset: 0,
                        },
                        ..Stroke::default().with_width(1.0).with_color(guide_color)
                    };
                    match g.axis {
                        GuideAxis::Vertical => {
                            let p = cstate.world_to_screen((g.position_mm, 0.0));
                            if p.x >= 0.0 && p.x <= bounds.width {
                                frame.stroke(
                                    &Path::line(
                                        Point::new(p.x, 0.0),
                                        Point::new(p.x, bounds.height),
                                    ),
                                    stroke,
                                );
                            }
                        }
                        GuideAxis::Horizontal => {
                            let p = cstate.world_to_screen((0.0, g.position_mm));
                            if p.y >= 0.0 && p.y <= bounds.height {
                                frame.stroke(
                                    &Path::line(
                                        Point::new(0.0, p.y),
                                        Point::new(bounds.width, p.y),
                                    ),
                                    stroke,
                                );
                            }
                        }
                    }
                }
            }

            // Origin crosshair — Altium-style yellow on the dark
            // Pads canvas, dark slate grey on the Fusion-style
            // white sketch canvas. v0.27 — bright yellow read as
            // a stray stray cursor on white; the slate grey keeps
            // the origin marker visible without dominating.
            let origin = cstate.world_to_screen((0.0, 0.0));
            let origin_color = if matches!(self.state.mode, EditorMode::Sketch) {
                Color::from_rgba(0.20, 0.25, 0.32, 0.85)
            } else {
                Color::from_rgba(1.0, 0.95, 0.30, 0.85)
            };
            frame.stroke(
                &Path::line(
                    Point::new(origin.x - 8.0, origin.y),
                    Point::new(origin.x + 8.0, origin.y),
                ),
                Stroke::default().with_width(1.5).with_color(origin_color),
            );
            frame.stroke(
                &Path::line(
                    Point::new(origin.x, origin.y - 8.0),
                    Point::new(origin.x, origin.y + 8.0),
                ),
                Stroke::default().with_width(1.5).with_color(origin_color),
            );

            // v0.18.16 — silk-layer graphics rendering. The
            // active-bar tools (Place Track / Arc / String /
            // Polygon) commit `FpGraphic` entries to
            // `primitive.silk_f`; without this draw pass the user's
            // placements were invisible on the canvas.
            if self.state.layer_visibility.get(FpLayer::FSilks) {
                draw_silk_graphics(
                    frame,
                    cstate,
                    self.silk_f,
                    FpLayer::FSilks,
                    self.state.selected_silk_f,
                );
            }
            if self.state.layer_visibility.get(FpLayer::BSilks) {
                draw_silk_graphics(frame, cstate, self.silk_b, FpLayer::BSilks, None);
            }

            // Courtyard — outline-following polygon takes
            // precedence over the bbox rectangle when present
            // (v0.27); fall back to the bbox for legacy state.
            if self.state.layer_visibility.get(FpLayer::EdgeCuts) {
                let edge_color = FpLayer::EdgeCuts.color();
                if let Some(outline) = self.state.courtyard_outline_mm.as_ref() {
                    if outline.len() >= 3 {
                        let path = Path::new(|b| {
                            let first = cstate.world_to_screen(outline[0]);
                            b.move_to(first);
                            for v in outline.iter().skip(1) {
                                b.line_to(cstate.world_to_screen(*v));
                            }
                            b.line_to(first);
                        });
                        frame.stroke(
                            &path,
                            Stroke::default().with_width(1.5).with_color(edge_color),
                        );
                    }
                } else if let Some(c) = self.state.courtyard_mm {
                    let p0 = cstate.world_to_screen((c.min_x, c.min_y));
                    let p1 = cstate.world_to_screen((c.max_x, c.max_y));
                    let rect = Path::rectangle(
                        Point::new(p0.x, p0.y),
                        iced::Size::new(p1.x - p0.x, p1.y - p0.y),
                    );
                    frame.stroke(
                        &rect,
                        Stroke::default().with_width(1.5).with_color(edge_color),
                    );
                }
            }

            // Pads — render last so they sit on top.
            for (idx, pad) in self.state.pads.iter().enumerate() {
                if !self.state.layer_visibility.get(pad.primary_layer()) {
                    continue;
                }
                // v0.27 — multi-select highlight: primary OR extras.
                let is_selected = self.state.selected_pad == Some(idx)
                    || self.state.selected_pads_extra.contains(&idx);
                draw_pad(frame, cstate, pad, is_selected);
            }

            // v0.25 polish — Source-pad indicator. When a pad is the
            // `source` of an Array (Linear / Grid / Polar), render a
            // small "+N" badge at the top-right corner of its bbox
            // showing the replica count. Makes the linkage visible
            // at a glance — without it, users can't tell which pad
            // is the authoring source vs a baked replica.
            if let Some(sketch) = self.sketch {
                let array_source_counts: std::collections::HashMap<
                    signex_sketch::id::SketchEntityId,
                    usize,
                > = sketch
                    .arrays
                    .iter()
                    .filter_map(|a| {
                        use signex_sketch::array::ArrayKind;
                        let (source, count) = match &a.kind {
                            ArrayKind::Linear {
                                source, count_expr, ..
                            } => (*source, count_expr.trim().parse::<usize>().unwrap_or(0)),
                            ArrayKind::Grid {
                                source,
                                nx_expr,
                                ny_expr,
                                ..
                            } => {
                                let nx = nx_expr.trim().parse::<usize>().unwrap_or(0);
                                let ny = ny_expr.trim().parse::<usize>().unwrap_or(0);
                                (*source, nx * ny)
                            }
                            ArrayKind::Polar {
                                source, count_expr, ..
                            } => (*source, count_expr.trim().parse::<usize>().unwrap_or(0)),
                        };
                        if count > 0 {
                            Some((source, count))
                        } else {
                            None
                        }
                    })
                    .fold(std::collections::HashMap::new(), |mut acc, (id, count)| {
                        *acc.entry(id).or_insert(0) += count;
                        acc
                    });

                if !array_source_counts.is_empty() && cstate.scale >= 12.0 {
                    for pad in self.state.pads.iter() {
                        let Some(entity_id) = pad.sketch_entity_id else {
                            continue;
                        };
                        let Some(replica_count) = array_source_counts.get(&entity_id) else {
                            continue;
                        };
                        if !self.state.layer_visibility.get(pad.primary_layer()) {
                            continue;
                        }
                        let (_, _, x1, y1) = pad.bbox_mm();
                        let p1 = cstate.world_to_screen((x1, y1));
                        // Badge: 18×12 px rounded rect, accent fill,
                        // white "+N" text. Positioned 6 px above + to
                        // the right of the bbox top-right corner so
                        // it doesn't overlap the pad outline.
                        let badge_w: f32 = 22.0;
                        let badge_h: f32 = 12.0;
                        let bx = p1.x + 4.0;
                        let by = p1.y - 6.0 - badge_h;
                        let badge_rect =
                            Path::rectangle(Point::new(bx, by), iced::Size::new(badge_w, badge_h));
                        // Altium-orange accent fill.
                        frame.fill(&badge_rect, Color::from_rgba(0.96, 0.62, 0.18, 0.95));
                        frame.stroke(
                            &badge_rect,
                            Stroke::default()
                                .with_width(0.8)
                                .with_color(Color::from_rgba(0.0, 0.0, 0.0, 0.6)),
                        );
                        frame.fill_text(canvas::Text {
                            content: format!("+{replica_count}"),
                            position: Point::new(bx + badge_w / 2.0, by + 1.0),
                            color: Color::WHITE,
                            size: 9.5.into(),
                            align_x: iced::alignment::Horizontal::Center.into(),
                            align_y: iced::alignment::Vertical::Top,
                            ..canvas::Text::default()
                        });
                    }
                }
            }

            // v0.16.1 — Pads-mode placement ghost. When PlacePad is
            // active, render a solid 1×1 mm rectangle at the cursor
            // showing where the next pad will land. Mirrors the
            // schematic placement-tool's pre-placement preview.
            // While `placement_paused` (TAB), the ghost is hidden
            // entirely so the cursor position no longer implies a
            // placement target — the user adjusts properties first,
            // then TAB resumes.
            use crate::library::editor::footprint::state::PadsTool;
            if matches!(self.state.mode, EditorMode::Normal)
                && self.state.pads_tool == PadsTool::PlacePad
                && !self.state.placement_paused
                && let Some((cx, cy)) = self.state.cursor_mm
            {
                use signex_library::PadShape as PS;
                // v0.20 — ghost reflects `next_pad_defaults`: actual
                // size_x / size_y / shape from the Properties panel
                // form, so the cursor preview shows what the click
                // will mint. Round/Oval/RoundRect/Chamfered render
                // their proper outline; everything else falls back
                // to a rectangle.
                let defaults = &self.state.next_pad_defaults;
                let half_w = (defaults.size_x_mm.max(0.05) / 2.0) as f32 * cstate.scale;
                let half_h = (defaults.size_y_mm.max(0.05) / 2.0) as f32 * cstate.scale;
                let centre = cstate.world_to_screen((cx, cy));
                let paused = self.state.placement_paused;
                let ghost_fill = if paused {
                    Color {
                        r: 0.55,
                        g: 0.55,
                        b: 0.55,
                        a: 1.0,
                    }
                } else {
                    Color {
                        r: 0.85,
                        g: 0.20,
                        b: 0.20,
                        a: 1.0,
                    }
                };
                let ghost_stroke = if paused {
                    Color {
                        r: 0.40,
                        g: 0.40,
                        b: 0.40,
                        a: 1.0,
                    }
                } else {
                    Color {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    }
                };

                let path = match &defaults.shape {
                    PS::Round | PS::Oval => Path::new(|b| {
                        let segments = 36;
                        for i in 0..=segments {
                            let t = i as f32 / segments as f32 * std::f32::consts::TAU;
                            let x = centre.x + half_w * t.cos();
                            let y = centre.y + half_h * t.sin();
                            if i == 0 {
                                b.move_to(Point::new(x, y));
                            } else {
                                b.line_to(Point::new(x, y));
                            }
                        }
                        b.close();
                    }),
                    PS::RoundRect { radius_ratio } => {
                        let r = (half_w.min(half_h) * (*radius_ratio as f32 * 2.0)).max(0.5);
                        Path::new(|b| {
                            b.move_to(Point::new(centre.x - half_w + r, centre.y - half_h));
                            b.line_to(Point::new(centre.x + half_w - r, centre.y - half_h));
                            b.arc_to(
                                Point::new(centre.x + half_w, centre.y - half_h),
                                Point::new(centre.x + half_w, centre.y - half_h + r),
                                r,
                            );
                            b.line_to(Point::new(centre.x + half_w, centre.y + half_h - r));
                            b.arc_to(
                                Point::new(centre.x + half_w, centre.y + half_h),
                                Point::new(centre.x + half_w - r, centre.y + half_h),
                                r,
                            );
                            b.line_to(Point::new(centre.x - half_w + r, centre.y + half_h));
                            b.arc_to(
                                Point::new(centre.x - half_w, centre.y + half_h),
                                Point::new(centre.x - half_w, centre.y + half_h - r),
                                r,
                            );
                            b.line_to(Point::new(centre.x - half_w, centre.y - half_h + r));
                            b.arc_to(
                                Point::new(centre.x - half_w, centre.y - half_h),
                                Point::new(centre.x - half_w + r, centre.y - half_h),
                                r,
                            );
                            b.close();
                        })
                    }
                    PS::Chamfered {
                        chamfer_ratio,
                        corners,
                    } => {
                        let c = (half_w.min(half_h) * (*chamfer_ratio as f32 * 2.0)).max(0.5);
                        Path::new(|b| {
                            let tl = Point::new(centre.x - half_w, centre.y - half_h);
                            let tr = Point::new(centre.x + half_w, centre.y - half_h);
                            let br = Point::new(centre.x + half_w, centre.y + half_h);
                            let bl = Point::new(centre.x - half_w, centre.y + half_h);
                            b.move_to(Point::new(tl.x + c, tl.y));
                            if corners.top_right {
                                b.line_to(Point::new(tr.x - c, tr.y));
                                b.line_to(Point::new(tr.x, tr.y + c));
                            } else {
                                b.line_to(tr);
                            }
                            if corners.bottom_right {
                                b.line_to(Point::new(br.x, br.y - c));
                                b.line_to(Point::new(br.x - c, br.y));
                            } else {
                                b.line_to(br);
                            }
                            if corners.bottom_left {
                                b.line_to(Point::new(bl.x + c, bl.y));
                                b.line_to(Point::new(bl.x, bl.y - c));
                            } else {
                                b.line_to(bl);
                            }
                            if corners.top_left {
                                b.line_to(Point::new(tl.x, tl.y + c));
                                b.line_to(Point::new(tl.x + c, tl.y));
                            } else {
                                b.line_to(tl);
                                b.line_to(Point::new(tl.x + c, tl.y));
                            }
                            b.close();
                        })
                    }
                    _ => {
                        let p0 = Point::new(centre.x - half_w, centre.y - half_h);
                        let size = iced::Size::new(half_w * 2.0, half_h * 2.0);
                        Path::rectangle(p0, size)
                    }
                };
                frame.fill(&path, ghost_fill);
                frame.stroke(
                    &path,
                    Stroke::default().with_width(1.0).with_color(ghost_stroke),
                );

                // v0.26 — drill hole on the ghost so the user sees
                // the THT punch BEFORE clicking. Drawn as a dark disc
                // at the pad centre so it reads against the red /
                // grey pad fill without a stroke ring (the rendered
                // pad uses the same convention).
                if let Some(d) = defaults
                    .drill_diameter_mm
                    .filter(|d| *d > f32::EPSILON as f64)
                {
                    let r_px = (d / 2.0) as f32 * cstate.scale;
                    if r_px > 0.5 {
                        let hole_color = if paused {
                            Color {
                                r: 0.10,
                                g: 0.10,
                                b: 0.10,
                                a: 0.85,
                            }
                        } else {
                            Color {
                                r: 0.05,
                                g: 0.05,
                                b: 0.05,
                                a: 0.95,
                            }
                        };
                        frame.fill(&Path::circle(centre, r_px), hole_color);
                        frame.stroke(
                            &Path::circle(centre, r_px),
                            Stroke::default().with_width(0.75).with_color(ghost_stroke),
                        );
                    }
                }
            }

            // v0.27 — PlaceVia ghost preview. Vias have canonical
            // geometry (Round 0.6 mm copper / 0.3 mm drill) so the
            // ghost reads off hardcoded constants rather than
            // `next_pad_defaults`. Renders a small translucent green
            // disc with a black drilled hole — visually distinct
            // from the red PlacePad ghost so the user can tell the
            // tools apart at a glance.
            if matches!(self.state.mode, EditorMode::Normal)
                && self.state.pads_tool == PadsTool::PlaceVia
                && !self.state.placement_paused
                && let Some((cx, cy)) = self.state.cursor_mm
            {
                const VIA_DIAMETER_MM: f64 = 0.6;
                const VIA_DRILL_MM: f64 = 0.3;
                let centre = cstate.world_to_screen((cx, cy));
                let r_px = (VIA_DIAMETER_MM / 2.0) as f32 * cstate.scale;
                let drill_r_px = (VIA_DRILL_MM / 2.0) as f32 * cstate.scale;
                let copper = Color::from_rgba(0.20, 0.75, 0.40, 0.85);
                let outline = Color::from_rgba(0.10, 0.95, 0.50, 1.0);
                let hole = Color::from_rgba(0.05, 0.05, 0.05, 0.95);
                frame.fill(&Path::circle(centre, r_px), copper);
                frame.stroke(
                    &Path::circle(centre, r_px),
                    Stroke::default().with_width(1.0).with_color(outline),
                );
                if drill_r_px > 0.5 {
                    frame.fill(&Path::circle(centre, drill_r_px), hole);
                }
            }

            // v0.18.16 — Pads-mode multi-click gesture previews
            // (Track / Arc / Polygon ghost lines). Reads in-flight
            // state + cursor; no-op for tools without a multi-click
            // gesture (Select / PlacePad / PlaceHole / PlaceString).
            if matches!(self.state.mode, super::state::EditorMode::Normal) {
                draw_pads_tool_preview(frame, cstate, self.state);
            }

            // v0.27 — Fusion-style sketch reticle. A small square
            // with an inner crosshair painted at the SNAP target
            // (state.cursor_mm), so the user sees exactly where
            // the next click will commit even when the OS cursor
            // is hovering a few pixels away because snap fired.
            // The OS cursor stays as the raw mouse position; the
            // reticle is the "click here" indicator. Hidden while
            // the right-click context menu is open (the menu owns
            // the interaction).
            let context_menu_open = self.state.context_menu.is_some();
            // v0.27 — only draw the reticle while a placement tool
            // is active; the Select tool wants the OS cursor (now
            // re-enabled in mouse_interaction) so click hit-testing
            // reads as direct pointer interaction, not a snap-target
            // glyph hovering near the cursor.
            let in_select_tool = self.state.active_tool == super::state::SketchTool::Select;
            if in_sketch
                && !in_select_tool
                && !context_menu_open
                && let Some((cx, cy)) = self.state.cursor_mm
                && cursor_screen.is_some()
            {
                let p = cstate.world_to_screen((cx, cy));
                let half = 7.0_f32;
                let arm = 4.5_f32;
                // v0.27 — darker reticle: solid dark-blue square +
                // black crosshair, no white halo. The white halo we
                // had was reading as a "ghost cursor" against the
                // white sketch canvas. Pure dark stroke is enough.
                let dark_blue = Color::from_rgba(0.04, 0.18, 0.36, 1.0);
                let near_black = Color::from_rgba(0.08, 0.08, 0.08, 1.0);
                let square_path = Path::rectangle(
                    Point::new(p.x - half, p.y - half),
                    iced::Size::new(half * 2.0, half * 2.0),
                );
                frame.stroke(
                    &square_path,
                    Stroke::default().with_width(1.5).with_color(dark_blue),
                );
                let stroke = Stroke::default().with_width(1.0).with_color(near_black);
                frame.stroke(
                    &Path::line(Point::new(p.x - arm, p.y), Point::new(p.x + arm, p.y)),
                    stroke,
                );
                frame.stroke(
                    &Path::line(Point::new(p.x, p.y - arm), Point::new(p.x, p.y + arm)),
                    stroke,
                );
            }

            // v0.27 — Select-tool cursor mark. Sketch mode hides
            // the OS cursor (the Crosshair variant rendered as a
            // pale ghost on white with the user's cursor scheme),
            // so we draw our own dark "+" at the raw cursor
            // position when the Select tool is active. Placement
            // tools have the dark-blue reticle above; Select gets
            // this lighter mark since it isn't pinned to a snap
            // target.
            //
            // When the cursor hovers a sketch Line the "+" is
            // replaced by a double-headed arrow rotated to the
            // line's perpendicular. OS resize cursors only carry
            // four cardinal shapes (↔ ↕ ↗ ↘) — for arbitrary line
            // angles they bucket and look "off". This in-canvas
            // glyph matches the exact angle.
            if in_sketch
                && in_select_tool
                && !context_menu_open
                && let Some(p) = cursor_screen
            {
                let arm = 5.0_f32;
                let near_black = Color::from_rgba(0.10, 0.10, 0.10, 0.90);
                let stroke = Stroke::default().with_width(1.0).with_color(near_black);

                // Hit-test the cursor against sketch Lines to find
                // the one we'd resize on drag. Same tolerance as
                // mouse_interaction's hover test so the cursor
                // glyph swaps in/out at the same threshold the
                // grab gesture activates.
                let hovered_line_angle: Option<f32> = if let Some(sketch_ref) = self.sketch {
                    const LINE_HIT_TOL_PX: f32 = 6.0;
                    let world = cstate.screen_to_world(p);
                    let tol_mm = (LINE_HIT_TOL_PX / cstate.scale.max(1.0)) as f64;
                    let pos_of = |pid: signex_sketch::id::SketchEntityId| -> Option<(f64, f64)> {
                        if let Some(solve) = self.state.last_solve.as_ref()
                            && let Some(q) = signex_sketch::solver::state::point_xy(
                                pid,
                                &solve.result.state,
                                &solve.result.index,
                                sketch_ref,
                            )
                        {
                            return Some(q);
                        }
                        sketch_ref
                            .entities
                            .iter()
                            .find(|e| e.id == pid)
                            .and_then(|e| match e.kind {
                                signex_sketch::entity::EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            })
                    };
                    let mut hit: Option<f32> = None;
                    for ent in &sketch_ref.entities {
                        if let signex_sketch::entity::EntityKind::Line { start, end } = ent.kind
                            && let (Some(a), Some(b)) = (pos_of(start), pos_of(end))
                        {
                            let line_dx = b.0 - a.0;
                            let line_dy = b.1 - a.1;
                            let llen2 = line_dx * line_dx + line_dy * line_dy;
                            if llen2 <= 1e-12 {
                                continue;
                            }
                            let t = ((world.0 - a.0) * line_dx + (world.1 - a.1) * line_dy) / llen2;
                            let tc = t.clamp(0.0, 1.0);
                            let px = a.0 + tc * line_dx;
                            let py = a.1 + tc * line_dy;
                            let d2 = (px - world.0).powi(2) + (py - world.1).powi(2);
                            if d2 <= tol_mm * tol_mm {
                                // World coords use the same Y-down
                                // mapping as screen (cstate.world_to_screen
                                // does no Y flip), so atan2 in
                                // world space matches the screen-
                                // space angle the user sees.
                                let a_screen = cstate.world_to_screen(a);
                                let b_screen = cstate.world_to_screen(b);
                                let sdx = b_screen.x - a_screen.x;
                                let sdy = b_screen.y - a_screen.y;
                                hit = Some(sdy.atan2(sdx));
                                break;
                            }
                        }
                    }
                    hit
                } else {
                    None
                };

                if let Some(line_angle) = hovered_line_angle {
                    // Perpendicular to the line — that's the drag
                    // axis. The glyph is a double-headed arrow with
                    // filled triangle heads (cleaner than the
                    // earlier stroked V's) plus a small white halo
                    // underneath so it reads against both the dark
                    // pad copper and the white sketch canvas.
                    let perp = line_angle + std::f32::consts::FRAC_PI_2;
                    let length = 16.0_f32; // shaft + head tip extent
                    let head_len = 7.0_f32;
                    let head_half = 5.0_f32;
                    let shaft_stroke = Stroke::default().with_width(2.2).with_color(near_black);
                    let halo_stroke = Stroke::default()
                        .with_width(4.5)
                        .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.85));
                    frame.with_save(|inner| {
                        inner.translate(Vector::new(p.x, p.y));
                        inner.rotate(Radians(perp));
                        // Halo behind the shaft so the arrow stays
                        // legible over silk / pad fills.
                        inner.stroke(
                            &Path::line(
                                Point::new(-length + head_len, 0.0),
                                Point::new(length - head_len, 0.0),
                            ),
                            halo_stroke,
                        );
                        // Shaft.
                        inner.stroke(
                            &Path::line(
                                Point::new(-length + head_len, 0.0),
                                Point::new(length - head_len, 0.0),
                            ),
                            shaft_stroke,
                        );
                        // Right arrowhead — filled triangle.
                        let right_head = Path::new(|b| {
                            b.move_to(Point::new(length, 0.0));
                            b.line_to(Point::new(length - head_len, -head_half));
                            b.line_to(Point::new(length - head_len, head_half));
                            b.close();
                        });
                        // Left arrowhead — filled triangle.
                        let left_head = Path::new(|b| {
                            b.move_to(Point::new(-length, 0.0));
                            b.line_to(Point::new(-length + head_len, -head_half));
                            b.line_to(Point::new(-length + head_len, head_half));
                            b.close();
                        });
                        // White outline pass first so a thin halo
                        // wraps each filled head.
                        inner.stroke(&right_head, halo_stroke);
                        inner.stroke(&left_head, halo_stroke);
                        inner.fill(&right_head, near_black);
                        inner.fill(&left_head, near_black);
                    });
                } else {
                    frame.stroke(
                        &Path::line(Point::new(p.x - arm, p.y), Point::new(p.x + arm, p.y)),
                        stroke,
                    );
                    frame.stroke(
                        &Path::line(Point::new(p.x, p.y - arm), Point::new(p.x, p.y + arm)),
                        stroke,
                    );
                }
            }

            // v0.27 — Touching Line ghost. After the first click
            // armed the line, the second-endpoint preview tracks
            // the cursor live. Same cyan accent as Lasso so the
            // selection-tool ghosts read as one family.
            if self.state.touching_line_active {
                if let Some((sx, sy)) = self.state.touching_line_first {
                    let p0 = cstate.world_to_screen((sx, sy));
                    let p1 = match self.state.cursor_mm {
                        Some(c) => cstate.world_to_screen(c),
                        None => p0,
                    };
                    let line_col = Color::from_rgba(0.10, 0.55, 0.85, 1.00);
                    frame.stroke(
                        &Path::line(p0, p1),
                        Stroke::default().with_width(1.5).with_color(line_col),
                    );
                    frame.fill(&Path::circle(p0, 3.5), line_col);
                    frame.fill(&Path::circle(p1, 3.5), line_col);
                }
            }

            // v0.27 — Lasso Select polygon ghost. Renders the
            // captured vertices as cyan dots + a closed-loop
            // outline back to the live cursor so the user sees
            // the polygon they're stroking. Right-click commits;
            // Esc cancels.
            if self.state.lasso_mode_active && !self.state.lasso_vertices.is_empty() {
                let lasso_col = Color::from_rgba(0.10, 0.55, 0.85, 1.00);
                let lasso_fill = Color::from_rgba(0.30, 0.55, 0.90, 0.18);
                let path = Path::new(|builder| {
                    let first = cstate.world_to_screen(self.state.lasso_vertices[0]);
                    builder.move_to(first);
                    for v in self.state.lasso_vertices.iter().skip(1) {
                        builder.line_to(cstate.world_to_screen(*v));
                    }
                    if let Some(cur_world) = self.state.cursor_mm {
                        builder.line_to(cstate.world_to_screen(cur_world));
                    }
                    if self.state.lasso_vertices.len() >= 2 {
                        builder.line_to(first);
                    }
                });
                if self.state.lasso_vertices.len() >= 2 {
                    frame.fill(&path, lasso_fill);
                }
                frame.stroke(
                    &path,
                    Stroke::default().with_width(1.2).with_color(lasso_col),
                );
                for v in &self.state.lasso_vertices {
                    let p = cstate.world_to_screen(*v);
                    frame.fill(&Path::circle(p, 3.0), lasso_col);
                }
            }

            // v0.26-I — rubber-band selection rectangle. Drawn only
            // when both anchor + current are set and they''re at
            // least the drag threshold apart (so an un-moved click
            // doesn''t flash a 0×0 rect on screen).
            if let (Some(a), Some(c)) = (
                cstate.box_select_anchor_screen,
                cstate.box_select_current_screen,
            ) {
                let dx = (c.x - a.x).abs();
                let dy = (c.y - a.y).abs();
                if dx.max(dy) >= DRAG_THRESHOLD_PX {
                    let x0 = a.x.min(c.x);
                    let y0 = a.y.min(c.y);
                    let w = (c.x - a.x).abs();
                    let h = (c.y - a.y).abs();
                    let rect_path = Path::rectangle(Point::new(x0, y0), iced::Size::new(w, h));
                    // Altium pen: cyan-ish translucent fill +
                    // dashed-look outline at 1 px.
                    frame.fill(&rect_path, Color::from_rgba(0.30, 0.55, 0.90, 0.18));
                    frame.stroke(
                        &rect_path,
                        Stroke::default()
                            .with_width(1.0)
                            .with_color(Color::from_rgba(0.50, 0.75, 1.00, 0.95)),
                    );
                }
            }

            // v0.13.1 Phase 6.2 — sketch entities overlay. Only drawn
            // when the editor is in Sketch mode AND a sketch exists.
            // DOF colour pulls from `state.last_solve.colours` so an
            // under-constrained Point lights up blue, fully pinned
            // black, over-constrained red.
            if matches!(self.state.mode, super::state::EditorMode::Sketch)
                && let Some(sketch) = self.sketch
            {
                draw_sketch_overlay(frame, cstate, sketch, self.state);
                // v0.22 Phase E2 — DOF direction-arrow overlay for
                // under-constrained Points. Sits on top of the entity
                // layer so the cyan arrow is visible against any
                // backing colour.
                draw_dof_direction_arrows(frame, cstate, sketch, self.state);
                draw_sketch_tool_preview(frame, cstate, sketch, self.state);
                // v0.22 Phase A6 — Inferred-constraint snap glyph
                // sits on top of the entity layer so the badge
                // doesn't get hidden under filled regions or
                // dashed-line ghosts.
                draw_sketch_snap_glyph(frame, cstate, self.state);
            }
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
