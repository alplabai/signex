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
use iced::{Color, Point, Rectangle, Renderer, Theme};

use crate::library::messages::{EditorMsg, LibraryMessage};
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
        // First-draw camera placement.
        // - With content: fit-to-bounds so every pad / sketch entity
        //   is visible.
        // - Without content (fresh `.snxfpt` from "Add New ▸
        //   Footprint"): centre world origin in the viewport so the
        //   user lands on (0, 0) rather than the screen's top-left.
        //   Without this, the default offset (0, 0) renders world
        //   (0, 0) at screen pixel (0, 0) — the user's drawing area
        //   starts in the corner and they have to pan to find the
        //   centre.
        if !cstate.did_initial_fit && bounds.width > 0.0 && bounds.height > 0.0 {
            if let Some(bbox) = self.state.content_bbox_mm() {
                cstate.fit_to_bounds(bbox, bounds);
            } else {
                cstate.offset = Point::new(bounds.width / 2.0, bounds.height / 2.0);
                // Keep DEFAULT_PX_PER_MM scale.
            }
            cstate.did_initial_fit = true;
        }

        // v0.26-C — one-shot Fit-to-Window from the right-click menu.
        // The dispatcher set `state.fit_pending = true`; we honour it
        // on the very next event tick (any mouse motion / scroll /
        // press over the canvas) and publish `FitConsumed` so the
        // flag clears. Bounds-guard mirrors the first-draw branch.
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
                msg: EditorMsg::FootprintFitConsumed,
            }));
        }

        cstate.last_known_selected = self.state.selected_pad;

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
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
                return Some(
                    canvas::Action::publish(LibraryMessage::EditorEvent {
                        library_path: self.address.library_path.clone(),
                        table: self.address.table.clone(),
                        row_id: self.address.row_id,
                        msg: EditorMsg::FootprintCursorAt {
                            x_mm: world.0,
                            y_mm: world.1,
                        },
                    })
                    .and_capture(),
                );
            }
            Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                if matches!(button, mouse::Button::Right | mouse::Button::Middle) {
                    // v0.15 — schematic-parity tool cancel. Right-
                    // click while a non-Select tool is active (Pads
                    // mode PlacePad, or Sketch mode Line/Circle/Arc/
                    // tool_pending != Idle) cancels the tool back to
                    // Select instead of starting a pan. Middle-click
                    // always pans (matches Altium).
                    use crate::library::editor::footprint::state::{
                        EditorMode, PadsTool, SketchTool, ToolPending,
                    };
                    if *button == mouse::Button::Right {
                        // v0.27 — Lasso Select right-click commits
                        // the polygon. Pre-empts the generic tool-
                        // cancel logic below.
                        if self.state.lasso_mode_active {
                            return Some(
                                canvas::Action::publish(LibraryMessage::EditorEvent {
                                    library_path: self.address.library_path.clone(),
                                    table: self.address.table.clone(),
                                    row_id: self.address.row_id,
                                    msg: EditorMsg::FootprintLassoCommit,
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
                                    msg: EditorMsg::FootprintTouchingLineCancel,
                                })
                                .and_capture(),
                            );
                        }
                        let cancel_msg: Option<EditorMsg> = match self.state.mode {
                            EditorMode::Normal => {
                                if self.state.pads_tool != PadsTool::Select {
                                    Some(EditorMsg::FootprintSetPadsTool(PadsTool::Select))
                                } else {
                                    None
                                }
                            }
                            EditorMode::Sketch => {
                                let tool_active = self.state.active_tool != SketchTool::Select;
                                let pending_active =
                                    !matches!(self.state.tool_pending, ToolPending::Idle);
                                if tool_active || pending_active {
                                    Some(EditorMsg::FootprintSketchSetTool(SketchTool::Select))
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
                    // Otherwise (Middle, or Right with no active tool)
                    // → start a pan as usual.
                    cstate.panning = true;
                    cstate.last_pan_pos = cursor.position_in(bounds);
                    // v0.26 — track motion so right-release without
                    // pan motion opens the context menu instead.
                    cstate.pan_moved = false;
                    return Some(canvas::Action::capture());
                }
                if *button == mouse::Button::Left
                    && let Some(cursor_pos) = cursor.position_in(bounds)
                {
                    // v0.27 — Lasso Select intercept. While the
                    // lasso tool is armed (set from the active-bar
                    // Selection Mode dropdown), each left-click adds
                    // a vertex to the in-flight polygon. Right-click
                    // commits / Esc cancels are handled in their own
                    // arms below.
                    if self.state.lasso_mode_active {
                        let world = cstate.screen_to_world(cursor_pos);
                        return Some(
                            canvas::Action::publish(LibraryMessage::EditorEvent {
                                library_path: self.address.library_path.clone(),
                                table: self.address.table.clone(),
                                row_id: self.address.row_id,
                                msg: EditorMsg::FootprintLassoAddVertex {
                                    x_mm: world.0,
                                    y_mm: world.1,
                                },
                            })
                            .and_capture(),
                        );
                    }
                    // v0.27 — Touching Line intercept. First click
                    // stashes the start point; second click commits
                    // by publishing FootprintTouchingLineCommit, the
                    // dispatcher walks pads + selects everything the
                    // segment intersects.
                    if self.state.touching_line_active {
                        let world = cstate.screen_to_world(cursor_pos);
                        let msg = if self.state.touching_line_first.is_none() {
                            EditorMsg::FootprintTouchingLineFirst {
                                x_mm: world.0,
                                y_mm: world.1,
                            }
                        } else {
                            EditorMsg::FootprintTouchingLineCommit {
                                x_mm: world.0,
                                y_mm: world.1,
                            }
                        };
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
                    let raw_world = cstate.screen_to_world(cursor_pos);
                    // v0.18.8 — Snap Options (Point / H/V / Angle /
                    // Grid) apply in BOTH Sketch and Pads modes. Each
                    // priority is gated by `state.snap_options.<flag>`
                    // inside `snap::snap_cursor`, so unchecking every
                    // box restores the v0.16.1 raw-cursor Pads-mode
                    // behaviour. Sketch-mode point-hit lookup still
                    // requires an existing sketch; in Pads mode the
                    // sketch is usually `None` and snap falls through
                    // to the H/V + Grid priorities.
                    use crate::library::editor::footprint::state::{
                        EditorMode as _EM, SketchTool as _ST,
                    };
                    // v0.27 — Select tool gets the raw cursor so a
                    // click can target a specific entity without
                    // jumping to the grid. Placement tools still go
                    // through `snap::snap_cursor` so Line / Circle /
                    // Arc etc. land on snap targets.
                    let select_mode = matches!(self.state.mode, _EM::Sketch)
                        && self.state.active_tool == _ST::Select;
                    let world = if select_mode {
                        cstate.last_snap = None;
                        raw_world
                    } else {
                        let point_hit = sketch_snap(self.sketch, cstate, raw_world);
                        let result =
                            snap::snap_cursor(raw_world, self.sketch, self.state, point_hit);
                        cstate.last_snap = Some(result);
                        result.pos
                    };
                    // v0.27 — Sketch mode + Select tool: hit-test the
                    // east-edge cyan diameter handle of any Round pad
                    // before the generic Point-snap. The handle is
                    // drawn at (pad.position + (pad.size_x/2, 0)) with
                    // a 4 px radius, so allow a 6 px hit slop.
                    if matches!(self.state.mode, _EM::Sketch)
                        && self.state.active_tool == _ST::Select
                    {
                        const HANDLE_HIT_RADIUS_PX: f32 = 6.0;
                        for (idx, pad) in self.state.pads.iter().enumerate() {
                            if !matches!(pad.shape, signex_library::PadShape::Round) {
                                continue;
                            }
                            if pad.sketch_entity_id.is_none() {
                                continue;
                            }
                            let handle_world = (
                                pad.position_mm.0 + pad.size_mm.0 / 2.0,
                                pad.position_mm.1,
                            );
                            let handle_screen = cstate.world_to_screen(handle_world);
                            let dx = cursor_pos.x - handle_screen.x;
                            let dy = cursor_pos.y - handle_screen.y;
                            if dx * dx + dy * dy
                                <= HANDLE_HIT_RADIUS_PX * HANDLE_HIT_RADIUS_PX
                            {
                                cstate.round_resize_drag = Some(idx);
                                cstate.drag = Some(DragState {
                                    pad_idx: usize::MAX,
                                    sketch_point: None,
                                    sketch_line: None,
                                    grab_offset_mm: pad.position_mm,
                                    last_world: world,
                                    press_screen: cursor_pos,
                                    moved: false,
                                });
                                return Some(canvas::Action::capture());
                            }
                        }
                    }
                    if matches!(self.state.mode, _EM::Sketch)
                        && self.state.active_tool == _ST::Select
                        && let Some(point_id) = sketch_snap(self.sketch, cstate, raw_world)
                    {
                        // Defensive: clear any stale rubber-band
                        // anchor so it can't render alongside the
                        // Point drag.
                        cstate.box_select_anchor_screen = None;
                        cstate.box_select_current_screen = None;
                        cstate.drag = Some(DragState {
                            pad_idx: usize::MAX,
                            sketch_point: Some(point_id),
                            sketch_line: None,
                            grab_offset_mm: (0.0, 0.0),
                            last_world: world,
                            press_screen: cursor_pos,
                            moved: false,
                        });
                        // Publish a select so the inspector + DOF
                        // overlay highlight this Point immediately.
                        return Some(
                            canvas::Action::publish(LibraryMessage::EditorEvent {
                                library_path: self.address.library_path.clone(),
                                table: self.address.table.clone(),
                                row_id: self.address.row_id,
                                msg: EditorMsg::FootprintSketchSelect {
                                    id: Some(point_id),
                                    shift: false,
                                },
                            })
                            .and_capture(),
                        );
                    }
                    // v0.27 — Fusion-style Line drag. In Sketch mode +
                    // Select tool, a click within ~6 px of a Line's
                    // stroke (but missing the snap radius for both
                    // endpoints) starts a Line-drag gesture: the
                    // dispatcher translates BOTH endpoints by the
                    // per-tick delta in one solver pass so an edge
                    // of a closed shape can be pushed without having
                    // to grab a corner.
                    if matches!(self.state.mode, _EM::Sketch)
                        && self.state.active_tool == _ST::Select
                        && let Some(sketch_ref) = self.sketch
                    {
                        const LINE_HIT_TOL_PX: f32 = 10.0;
                        let tol_mm =
                            (LINE_HIT_TOL_PX / cstate.scale.max(1.0)) as f64;
                        let mut best_line: Option<(f64, signex_sketch::id::SketchEntityId)> =
                            None;
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
                                    signex_sketch::entity::EntityKind::Point { x, y } => {
                                        Some((x, y))
                                    }
                                    _ => None,
                                })
                        };
                        for ent in &sketch_ref.entities {
                            if let signex_sketch::entity::EntityKind::Line { start, end } =
                                ent.kind
                                && let (Some(a), Some(b)) = (pos_of(start), pos_of(end))
                            {
                                let dx = b.0 - a.0;
                                let dy = b.1 - a.1;
                                let llen2 = dx * dx + dy * dy;
                                if llen2 <= 1e-12 {
                                    continue;
                                }
                                let t = ((world.0 - a.0) * dx + (world.1 - a.1) * dy)
                                    / llen2;
                                let tc = t.clamp(0.0, 1.0);
                                let px = a.0 + tc * dx;
                                let py = a.1 + tc * dy;
                                let d2 = (px - world.0).powi(2) + (py - world.1).powi(2);
                                if d2 <= tol_mm * tol_mm
                                    && best_line.as_ref().is_none_or(|(b2, _)| d2 < *b2)
                                {
                                    best_line = Some((d2, ent.id));
                                }
                            }
                        }
                        if let Some((_, line_id)) = best_line {
                            // Make sure no stale rubber-band anchor
                            // is left over from a prior gesture.
                            cstate.box_select_anchor_screen = None;
                            cstate.box_select_current_screen = None;
                            cstate.drag = Some(DragState {
                                pad_idx: usize::MAX,
                                sketch_point: None,
                                sketch_line: Some(line_id),
                                grab_offset_mm: (0.0, 0.0),
                                last_world: world,
                                press_screen: cursor_pos,
                                moved: false,
                            });
                            return Some(
                                canvas::Action::publish(LibraryMessage::EditorEvent {
                                    library_path: self.address.library_path.clone(),
                                    table: self.address.table.clone(),
                                    row_id: self.address.row_id,
                                    msg: EditorMsg::FootprintSketchSelect {
                                        id: Some(line_id),
                                        shift: false,
                                    },
                                })
                                .and_capture(),
                            );
                        }
                    }
                    // v0.27 — Fusion-style "click the fill, select the
                    // closed shape." Only in Sketch mode + Select tool,
                    // and only when the Point-snap path missed. We
                    // walk the same closed-loop adjacency the fill
                    // renderer uses (`find_closed_loops`), and on a
                    // point-in-polygon hit we dispatch a SelectMany
                    // carrying every Line + Point in the loop. This
                    // gives the user the "closed shape is selected as
                    // a single profile" experience the rubber-band
                    // path was failing to deliver, and feeds straight
                    // into Make Pad from Profile.
                    if matches!(self.state.mode, _EM::Sketch)
                        && self.state.active_tool == _ST::Select
                        && let Some(sketch_ref) = self.sketch
                    {
                        let loops = find_closed_loops(sketch_ref, self.state);
                        let mut hit: Option<&ClosedLoop> = None;
                        for lp in &loops {
                            if point_in_polygon(world.0, world.1, &lp.polygon) {
                                hit = Some(lp);
                                break;
                            }
                        }
                        if let Some(lp) = hit {
                            let mut ids: Vec<signex_sketch::id::SketchEntityId> =
                                lp.lines.clone();
                            ids.extend(lp.points.iter().copied());
                            return Some(
                                canvas::Action::publish(LibraryMessage::EditorEvent {
                                    library_path: self.address.library_path.clone(),
                                    table: self.address.table.clone(),
                                    row_id: self.address.row_id,
                                    msg: EditorMsg::FootprintSketchSelectMany(ids),
                                })
                                .and_capture(),
                            );
                        }
                    }
                    // v0.21 — Pad hit is gated on the Selection Filter
                    // pad bit. When `pads` is off, pads stay
                    // unselectable and clicks fall through to the
                    // silk-hit / empty-canvas branches below.
                    //
                    // v0.23 — also gate on Pads-mode (Normal). In
                    // Sketch mode the pad's bbox is just construction
                    // chrome; clicks inside it must fall through to
                    // the sketch-tool click handler so Line / Circle /
                    // etc. can snap to the pad's corner Points. The
                    // Select tool's pad-grab path (v0.16) lived
                    // strictly in Pads mode and that contract is
                    // preserved.
                    if matches!(self.state.mode, _EM::Normal) && self.state.selection_filter.pads {
                        if let Some(pad_idx) = self.state.pad_at(world.0, world.1) {
                            let pad = &self.state.pads[pad_idx];
                            cstate.drag = Some(DragState {
                                pad_idx,
                                sketch_point: None,
                                sketch_line: None,
                                grab_offset_mm: (
                                    world.0 - pad.position_mm.0,
                                    world.1 - pad.position_mm.1,
                                ),
                                last_world: world,
                                press_screen: cursor_pos,
                                moved: false,
                            });
                            // v0.27 — Altium-parity modifier handling
                            // on the pad-hit branch. Ctrl/Cmd toggles
                            // the pad in/out of the multi-select set;
                            // Shift extends without removal; bare
                            // click replaces the selection.
                            let cmd = cstate.current_modifiers.command();
                            let shift = cstate.current_modifiers.shift();
                            let select_msg = if cmd || shift {
                                let mut current: Vec<usize> =
                                    self.state.selected_pad.into_iter().collect();
                                current.extend(self.state.selected_pads_extra.iter().copied());
                                if cmd {
                                    if let Some(pos) = current.iter().position(|&i| i == pad_idx) {
                                        current.remove(pos);
                                    } else {
                                        current.push(pad_idx);
                                    }
                                } else if !current.contains(&pad_idx) {
                                    current.push(pad_idx);
                                }
                                EditorMsg::FootprintSelectPads(current)
                            } else {
                                EditorMsg::FootprintSelectPad(Some(pad_idx))
                            };
                            return Some(
                                canvas::Action::publish(LibraryMessage::EditorEvent {
                                    library_path: self.address.library_path.clone(),
                                    table: self.address.table.clone(),
                                    row_id: self.address.row_id,
                                    msg: select_msg,
                                })
                                .and_capture(),
                            );
                        }
                    }
                    // v0.18.18 — Silk-front graphic hit, filter-gated
                    // per kind. v0.21 maps each FpGraphicKind to its
                    // matching `selection_filter.*` bit so the user
                    // can disable Tracks / Arcs / Texts / Regions /
                    // Fills independently.
                    {
                        use crate::library::editor::footprint::state::{
                            EditorMode as Em2, PadsTool as Pt2,
                        };
                        if matches!(self.state.mode, Em2::Normal)
                            && self.state.pads_tool == Pt2::Select
                        {
                            let tolerance = 4.0_f64 / (cstate.scale.max(1.0) as f64);
                            if let Some(silk_idx) =
                                silk_f_hit_at(self.silk_f, world.0, world.1, tolerance)
                            {
                                use signex_library::primitive::footprint::FpGraphicKind;
                                let g = &self.silk_f[silk_idx];
                                let allowed = match &g.kind {
                                    FpGraphicKind::Line { .. } => {
                                        self.state.selection_filter.tracks
                                    }
                                    FpGraphicKind::Arc { .. }
                                    | FpGraphicKind::Circle { .. } => {
                                        self.state.selection_filter.arcs
                                    }
                                    FpGraphicKind::Rectangle { .. } => {
                                        if g.filled {
                                            self.state.selection_filter.fills
                                        } else {
                                            self.state.selection_filter.tracks
                                        }
                                    }
                                    FpGraphicKind::Polygon { .. } => {
                                        if g.filled {
                                            self.state.selection_filter.regions
                                        } else {
                                            self.state.selection_filter.tracks
                                        }
                                    }
                                    FpGraphicKind::Text { .. } => {
                                        self.state.selection_filter.texts
                                    }
                                };
                                if allowed {
                                    return Some(
                                        canvas::Action::publish(LibraryMessage::EditorEvent {
                                            library_path: self.address.library_path.clone(),
                                            table: self.address.table.clone(),
                                            row_id: self.address.row_id,
                                            msg: EditorMsg::FootprintSelectSilkF(Some(silk_idx)),
                                        })
                                        .and_capture(),
                                    );
                                }
                            }
                        }
                    }
                    // Empty area → pending click-add. We can't add yet
                    // because a drag may follow; commit on release.
                    cstate.drag = Some(DragState {
                        pad_idx: usize::MAX,
                        sketch_point: None,
                        sketch_line: None,
                        grab_offset_mm: (world.0, world.1),
                        last_world: world,
                        press_screen: cursor_pos,
                        moved: false,
                    });
                    // v0.26-I + v0.27 — Select-tool empty-canvas
                    // press arms the rubber-band rectangle. Fires
                    // for Pads mode (PadsTool::Select) + Sketch mode
                    // (SketchTool::Select). Each mode's release
                    // picker walks the appropriate entity list.
                    let arm_rubber = matches!(
                        self.state.mode,
                        super::state::EditorMode::Normal
                    ) && self.state.pads_tool == super::state::PadsTool::Select
                        || matches!(self.state.mode, super::state::EditorMode::Sketch)
                            && self.state.active_tool == super::state::SketchTool::Select;
                    if arm_rubber {
                        cstate.box_select_anchor_screen = Some(cursor_pos);
                        cstate.box_select_current_screen = Some(cursor_pos);
                    }
                    return Some(canvas::Action::capture());
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                if matches!(button, mouse::Button::Right | mouse::Button::Middle) {
                    // v0.26-J — only treat the release as a "context
                    // menu trigger" when the matching press actually
                    // started a pan. The press handler short-circuits
                    // before setting `panning = true` when a non-Select
                    // tool is active (right-click cancels the tool),
                    // so without this gate the cancel-then-release
                    // sequence falls through to "no motion → menu" and
                    // opens the menu after every tool cancel.
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
                        // Window-absolute screen coords for the
                        // overlay: bounds.x/y is the canvas's
                        // top-left within the window, cursor_pos is
                        // relative to bounds.
                        let screen_x = bounds.x + cursor_pos.x;
                        let screen_y = bounds.y + cursor_pos.y;
                        // v0.26-B/C — hit-test the pad list first
                        // (top z-order in PCB convention), then fall
                        // through to silk graphics, then Empty. Pads
                        // win because they''re drawn on top of silk
                        // in the rendered footprint.
                        use crate::library::editor::footprint::state
                            ::FootprintContextTarget;
                        let world = cstate.screen_to_world(cursor_pos);
                        let target = if let Some(idx) =
                            self.state.pad_at(world.0, world.1)
                        {
                            FootprintContextTarget::Pad(idx)
                        } else {
                            // Same tolerance the left-click silk
                            // pick uses (4 px in screen → world via
                            // current zoom).
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
                                msg: EditorMsg::FootprintShowContextMenu {
                                    x: screen_x,
                                    y: screen_y,
                                    target,
                                },
                            })
                            .and_capture(),
                        );
                    }
                    return None;
                }
                if *button == mouse::Button::Left
                    && let Some(drag) = cstate.drag.take()
                {
                    // v0.27 — round-pad resize drag releases here.
                    // The CursorMoved handler has been streaming
                    // FootprintSketchResizeRoundPad per tick; release
                    // just clears the drag state.
                    if cstate.round_resize_drag.take().is_some() {
                        self.cache.clear();
                        return None;
                    }
                    // v0.16 — sketch-Point drag releases here. The
                    // press handler already published the select and
                    // CursorMoved has been streaming
                    // FootprintSketchMovePoint per tick; release just
                    // ends the drag, no commit needed. Clear the
                    // cache so the final solved frame renders.
                    if drag.sketch_point.is_some() {
                        self.cache.clear();
                        return None;
                    }
                    if drag.pad_idx == usize::MAX {
                        if drag.moved {
                            // v0.26-I — empty-canvas drag with motion.
                            // If the press armed a rubber-band (Select
                            // tool + Pads mode), commit it now: walk
                            // the pad list and pick the first pad
                            // whose bbox lies fully inside the
                            // dragged rectangle. Multi-select is
                            // queued. Without box-select armed (other
                            // tool / Sketch mode), the empty drag is
                            // a cancelled click-add — return None.
                            //
                            // v0.27 — the rubber-band logic used to
                            // live below in an `if drag.moved` block
                            // that this early-return prevented from
                            // executing. Box-select would arm on
                            // press but never fire on release. Moved
                            // up here so the empty-drag-moved path
                            // actually reaches the picker.
                            if let (Some(a), Some(c)) = (
                                cstate.box_select_anchor_screen.take(),
                                cstate.box_select_current_screen.take(),
                            ) {
                                use super::state::{EditorMode, FpSelectionMode};
                                let world_a = cstate.screen_to_world(a);
                                let world_c = cstate.screen_to_world(c);
                                let (x0, x1) = if world_a.0 <= world_c.0 {
                                    (world_a.0, world_c.0)
                                } else {
                                    (world_c.0, world_a.0)
                                };
                                let (y0, y1) = if world_a.1 <= world_c.1 {
                                    (world_a.1, world_c.1)
                                } else {
                                    (world_c.1, world_a.1)
                                };
                                // v0.27 — Sketch-mode rubber-band:
                                // pick every entity whose bbox is
                                // inside the rect. Bbox per kind:
                                //   Point — single point (snap to inside)
                                //   Line — bbox of (start, end)
                                //   Arc — bbox of (centre±radius), pruned
                                //         by sweep, approximated as
                                //         centre ± radius for simplicity
                                //   Circle — bbox of (centre±radius)
                                if matches!(self.state.mode, EditorMode::Sketch) {
                                    if let Some(sketch) = self.sketch {
                                        use signex_sketch::entity::EntityKind;
                                        let resolve =
                                            |id: signex_sketch::id::SketchEntityId|
                                                -> Option<(f64, f64)> {
                                                sketch
                                                    .entities
                                                    .iter()
                                                    .find(|e| e.id == id)
                                                    .and_then(|e| match e.kind {
                                                        EntityKind::Point { x, y } => Some((x, y)),
                                                        _ => None,
                                                    })
                                            };
                                        let bbox_of = |e: &signex_sketch::entity::Entity|
                                            -> Option<(f64, f64, f64, f64)> {
                                            match e.kind {
                                                EntityKind::Point { x, y } => Some((x, y, x, y)),
                                                EntityKind::Line { start, end } => {
                                                    let s = resolve(start)?;
                                                    let f = resolve(end)?;
                                                    Some((
                                                        s.0.min(f.0),
                                                        s.1.min(f.1),
                                                        s.0.max(f.0),
                                                        s.1.max(f.1),
                                                    ))
                                                }
                                                EntityKind::Circle { center, radius } => {
                                                    let c = resolve(center)?;
                                                    Some((
                                                        c.0 - radius,
                                                        c.1 - radius,
                                                        c.0 + radius,
                                                        c.1 + radius,
                                                    ))
                                                }
                                                EntityKind::Arc {
                                                    center, start, ..
                                                } => {
                                                    let c = resolve(center)?;
                                                    let s = resolve(start)?;
                                                    let r = ((s.0 - c.0).powi(2)
                                                        + (s.1 - c.1).powi(2))
                                                    .sqrt();
                                                    Some((c.0 - r, c.1 - r, c.0 + r, c.1 + r))
                                                }
                                            }
                                        };
                                        let mut hits: Vec<signex_sketch::id::SketchEntityId> =
                                            Vec::new();
                                        for e in &sketch.entities {
                                            let Some((bx0, by0, bx1, by1)) = bbox_of(e) else {
                                                continue;
                                            };
                                            let fully_inside = bx0 >= x0
                                                && bx1 <= x1
                                                && by0 >= y0
                                                && by1 <= y1;
                                            if fully_inside {
                                                hits.push(e.id);
                                            }
                                        }
                                        self.cache.clear();
                                        return Some(canvas::Action::publish(
                                            LibraryMessage::EditorEvent {
                                                library_path: self.address.library_path.clone(),
                                                table: self.address.table.clone(),
                                                row_id: self.address.row_id,
                                                msg: EditorMsg::FootprintSketchSelectMany(hits),
                                            },
                                        ));
                                    }
                                }
                                // v0.27 — honour the active-bar
                                // Selection picker. Inside (default):
                                // pad bbox fully inside rect.
                                // Touching: pad bbox intersects rect.
                                // Outside: pad bbox fully outside.
                                let mode = self.state.selection_mode_2d;
                                // v0.27 — Altium-parity rubber-band
                                // multi-select. Collect EVERY pad
                                // matching the active mode rather
                                // than breaking at the first hit.
                                let mut hits: Vec<usize> = Vec::new();
                                for (idx, pad) in self.state.pads.iter().enumerate() {
                                    let (px0, py0, px1, py1) = pad.bbox_mm();
                                    let fully_inside = px0 >= x0
                                        && px1 <= x1
                                        && py0 >= y0
                                        && py1 <= y1;
                                    let fully_outside = px1 < x0
                                        || px0 > x1
                                        || py1 < y0
                                        || py0 > y1;
                                    let touching = !fully_outside;
                                    let keep = match mode {
                                        FpSelectionMode::Inside => fully_inside,
                                        FpSelectionMode::Touching => touching,
                                        FpSelectionMode::Outside => fully_outside,
                                    };
                                    if keep {
                                        hits.push(idx);
                                    }
                                }
                                // v0.27 — Ctrl/Shift modifier on
                                // release combines the rubber-band
                                // hits with the existing selection.
                                // Ctrl toggles each hit in/out;
                                // Shift unions; no modifier replaces.
                                let cmd = cstate.current_modifiers.command();
                                let shift = cstate.current_modifiers.shift();
                                let combined: Vec<usize> = if cmd || shift {
                                    let mut acc: Vec<usize> =
                                        self.state.selected_pad.into_iter().collect();
                                    acc.extend(
                                        self.state.selected_pads_extra.iter().copied(),
                                    );
                                    if cmd {
                                        for h in &hits {
                                            if let Some(p) = acc.iter().position(|i| i == h) {
                                                acc.remove(p);
                                            } else {
                                                acc.push(*h);
                                            }
                                        }
                                    } else {
                                        for h in &hits {
                                            if !acc.contains(h) {
                                                acc.push(*h);
                                            }
                                        }
                                    }
                                    acc
                                } else {
                                    hits
                                };
                                self.cache.clear();
                                return Some(canvas::Action::publish(
                                    LibraryMessage::EditorEvent {
                                        library_path: self.address.library_path.clone(),
                                        table: self.address.table.clone(),
                                        row_id: self.address.row_id,
                                        msg: EditorMsg::FootprintSelectPads(combined),
                                    },
                                ));
                            }
                            return None;
                        }
                        // v0.13.1 Phase 6.3 — Sketch mode redirects
                        // empty-canvas click-add to the Place Point
                        // sketch-tool path. Normal mode keeps the
                        // existing FootprintAddPad behaviour.
                        // v0.13.2 Phase 6.4 — also routes Line /
                        // Circle / Arc multi-click gestures with
                        // snap-to-existing-point detection. The
                        // dispatcher advances tool_pending per click.
                        if matches!(self.state.mode, super::state::EditorMode::Sketch) {
                            use super::state::SketchTool;
                            let click_world = drag.grab_offset_mm;
                            let snap_id = sketch_snap(self.sketch, cstate, click_world);
                            // v0.13.3 — also try to hit-test Lines /
                            // Arcs / Circles (not just snap-to-Point)
                            // for the Select tool.
                            let select_id = snap_id
                                .or_else(|| sketch_hit_other(self.sketch, cstate, click_world));
                            // v0.16.1 — TAB pause/resume also applies
                            // to sketch placement tools. While paused,
                            // suppress click-publish for non-Select
                            // tools so the user can adjust defaults
                            // without accidentally minting geometry.
                            // Select-tool clicks still resolve so the
                            // user can re-pick a different anchor.
                            if self.state.placement_paused
                                && self.state.active_tool != SketchTool::Select
                            {
                                return None;
                            }
                            let msg = match self.state.active_tool {
                                SketchTool::Select => EditorMsg::FootprintSketchSelect {
                                    id: select_id,
                                    shift: false,
                                },
                                SketchTool::Point => EditorMsg::FootprintSketchPlacePoint {
                                    x_mm: click_world.0,
                                    y_mm: click_world.1,
                                },
                                SketchTool::Line
                                | SketchTool::Rectangle
                                | SketchTool::RoundedRectangle
                                | SketchTool::Circle
                                | SketchTool::Arc
                                | SketchTool::Mirror
                                | SketchTool::Offset
                                | SketchTool::RectPattern
                                | SketchTool::CircularPattern
                                | SketchTool::TangentArc
                                | SketchTool::Fillet
                                | SketchTool::Trim => EditorMsg::FootprintSketchToolClick {
                                    x_mm: click_world.0,
                                    y_mm: click_world.1,
                                    snap_id,
                                },
                            };
                            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                                library_path: self.address.library_path.clone(),
                                table: self.address.table.clone(),
                                row_id: self.address.row_id,
                                msg,
                            }));
                        }
                        // v0.15 — gate empty-click pad-add on
                        // PadsTool::PlacePad. The default Select tool
                        // no longer auto-adds a pad on every empty
                        // click, which removes the "I clicked
                        // somewhere by accident and now have a stray
                        // pad" footgun.
                        // v0.16.1 — also gate on `placement_paused`
                        // so TAB-pause suppresses click-add until the
                        // user resumes (TAB again).
                        use crate::library::editor::footprint::state::PadsTool;
                        if self.state.pads_tool == PadsTool::PlacePad
                            && !self.state.placement_paused
                        {
                            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                                library_path: self.address.library_path.clone(),
                                table: self.address.table.clone(),
                                row_id: self.address.row_id,
                                msg: EditorMsg::FootprintAddPad {
                                    x_mm: drag.grab_offset_mm.0,
                                    y_mm: drag.grab_offset_mm.1,
                                },
                            }));
                        }
                        // v0.27 — PlaceVia drops a canonical via (Round
                        // 0.6 mm copper + 0.3 mm drill + Multi-Layer)
                        // via a dedicated dispatcher path. Bypasses
                        // `next_pad_defaults` so the via geometry is
                        // always correct regardless of Pads-mode dial-in.
                        if self.state.pads_tool == PadsTool::PlaceVia
                            && !self.state.placement_paused
                        {
                            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                                library_path: self.address.library_path.clone(),
                                table: self.address.table.clone(),
                                row_id: self.address.row_id,
                                msg: EditorMsg::FootprintAddVia {
                                    x_mm: drag.grab_offset_mm.0,
                                    y_mm: drag.grab_offset_mm.1,
                                },
                            }));
                        }
                        // v0.18.12 — Place Hole drops a non-plated
                        // through hole at the cursor. Same gating as
                        // PlacePad (TAB-pause suppresses click-add).
                        if self.state.pads_tool == PadsTool::PlaceHole
                            && !self.state.placement_paused
                        {
                            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                                library_path: self.address.library_path.clone(),
                                table: self.address.table.clone(),
                                row_id: self.address.row_id,
                                msg: EditorMsg::FootprintAddHole {
                                    x_mm: drag.grab_offset_mm.0,
                                    y_mm: drag.grab_offset_mm.1,
                                },
                            }));
                        }
                        // v0.18.15 — Place String drops a silk-layer
                        // text label at the cursor.
                        if self.state.pads_tool == PadsTool::PlaceString
                            && !self.state.placement_paused
                        {
                            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                                library_path: self.address.library_path.clone(),
                                table: self.address.table.clone(),
                                row_id: self.address.row_id,
                                msg: EditorMsg::FootprintAddText {
                                    x_mm: drag.grab_offset_mm.0,
                                    y_mm: drag.grab_offset_mm.1,
                                },
                            }));
                        }
                        // v0.18.15.1 — Place Track is a 2-click
                        // gesture. The dispatcher uses
                        // `state.track_first` to decide whether the
                        // current click is the start (first click)
                        // or the commit (second click + chain).
                        if self.state.pads_tool == PadsTool::PlaceTrack
                            && !self.state.placement_paused
                        {
                            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                                library_path: self.address.library_path.clone(),
                                table: self.address.table.clone(),
                                row_id: self.address.row_id,
                                msg: EditorMsg::FootprintTrackClick {
                                    x_mm: drag.grab_offset_mm.0,
                                    y_mm: drag.grab_offset_mm.1,
                                },
                            }));
                        }
                        // v0.18.15.3 — Place Arc is a 3-click
                        // gesture (centre / radius / sweep). The
                        // dispatcher reads `state.place_arc_pending`
                        // to decide which click stage this is.
                        if self.state.pads_tool == PadsTool::PlaceArc
                            && !self.state.placement_paused
                        {
                            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                                library_path: self.address.library_path.clone(),
                                table: self.address.table.clone(),
                                row_id: self.address.row_id,
                                msg: EditorMsg::FootprintArcClick {
                                    x_mm: drag.grab_offset_mm.0,
                                    y_mm: drag.grab_offset_mm.1,
                                },
                            }));
                        }
                        // v0.18.15.4 / v0.18.17 — Place Polygon
                        // and Place Region share the same gesture
                        // (vertex click stash). The dispatcher
                        // reads `pads_tool` at commit time to
                        // decide `filled` on the resulting
                        // `Polygon` FpGraphic.
                        if (self.state.pads_tool == PadsTool::PlacePolygon
                            || self.state.pads_tool == PadsTool::PlaceRegion)
                            && !self.state.placement_paused
                        {
                            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                                library_path: self.address.library_path.clone(),
                                table: self.address.table.clone(),
                                row_id: self.address.row_id,
                                msg: EditorMsg::FootprintPolygonClick {
                                    x_mm: drag.grab_offset_mm.0,
                                    y_mm: drag.grab_offset_mm.1,
                                },
                            }));
                        }
                        // v0.20 — Select tool: empty-area left-click
                        // clears the current selection. Mirrors Altium
                        // and the schematic canvas; the previous
                        // behaviour silently no-op'd, leaving the user
                        // unable to deselect a pad without picking
                        // another. Only fires for the Select tool —
                        // every other pads_tool handles the click
                        // above (place / drop / etc.).
                        if self.state.pads_tool == PadsTool::Select {
                            // Clean the box-select arming we set on
                            // press — un-moved click stays a click,
                            // not a rubber band.
                            cstate.box_select_anchor_screen = None;
                            cstate.box_select_current_screen = None;
                            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                                library_path: self.address.library_path.clone(),
                                table: self.address.table.clone(),
                                row_id: self.address.row_id,
                                msg: EditorMsg::FootprintSelectPad(None),
                            }));
                        }
                        return None;
                    }
                    if drag.moved {
                        // Final pad-drag move position is already
                        // published per CursorMoved tick — release
                        // just clears the cache so the next frame
                        // settles. v0.27 — the empty-canvas rubber-
                        // band branch lives in the `pad_idx == MAX`
                        // block above; only the pad-drag fall-through
                        // remains here.
                        self.cache.clear();
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let Some(cursor_pos) = cursor.position_in(bounds) else {
                    return None;
                };
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
                                    msg: EditorMsg::FootprintCloseContextMenu,
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
                            msg: EditorMsg::FootprintCursorAt {
                                x_mm: world.0,
                                y_mm: world.1,
                            },
                        })
                        .and_capture(),
                    );
                }
                let raw_world = cstate.screen_to_world(cursor_pos);
                // v0.18.8 — Snap (Point / H/V / Angle / Grid) applies
                // in both Sketch and Pads modes; per-priority gating
                // lives in `snap::snap_cursor` via `state.snap_options`.
                // v0.27 — Select tool sees the raw cursor so the
                // click hit-test isn't pulled to the grid. EXCEPT
                // while a Line drag is in flight — Fusion-style edge
                // resize should respect the user's Snap Options
                // (grid / point / H/V) so the moved edge lands on
                // grid as the user expects.
                use crate::library::editor::footprint::state::{
                    EditorMode as _EMt, SketchTool as _STt,
                };
                let select_mode_tick = matches!(self.state.mode, _EMt::Sketch)
                    && self.state.active_tool == _STt::Select;
                let drag_active_for_snap = cstate
                    .drag
                    .as_ref()
                    .map(|d| d.sketch_line.is_some() || d.sketch_point.is_some())
                    .unwrap_or(false);
                let world = if select_mode_tick && !drag_active_for_snap {
                    cstate.last_snap = None;
                    raw_world
                } else {
                    let point_hit = sketch_snap(self.sketch, cstate, raw_world);
                    let result = snap::snap_cursor(raw_world, self.sketch, self.state, point_hit);
                    cstate.last_snap = Some(result);
                    result.pos
                };
                // v0.27 — round-pad diameter handle drag: take the
                // armed pad index, compute the new diameter from the
                // cursor's world distance to the pad centre, publish
                // a resize message. Bypass the generic drag handler
                // so the Point-drag / pad-drag paths don't fire.
                if let Some(pad_idx) = cstate.round_resize_drag {
                    let centre = self
                        .state
                        .pads
                        .get(pad_idx)
                        .map(|p| p.position_mm);
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
                            msg: EditorMsg::FootprintSketchResizeRoundPad {
                                pad_idx,
                                diameter_mm,
                            },
                        }));
                    }
                }
                if let Some(drag) = cstate.drag.as_mut() {
                    let dx = (cursor_pos.x - drag.press_screen.x).abs();
                    let dy = (cursor_pos.y - drag.press_screen.y).abs();
                    if !drag.moved && dx.max(dy) >= DRAG_THRESHOLD_PX {
                        drag.moved = true;
                    }
                    // v0.26-I — keep the box-select endpoint in lock-
                    // step with the cursor so the draw pass paints a
                    // live rubber band. Cleared on release alongside
                    // the anchor.
                    if cstate.box_select_anchor_screen.is_some() {
                        cstate.box_select_current_screen = Some(cursor_pos);
                        self.cache.clear();
                    }
                    // v0.16 — sketch Point drag (Sketch mode + Select
                    // tool with a hit on a Point entity). Publish a
                    // per-tick `FootprintSketchMovePoint` with the
                    // delta since the previous tick, then advance
                    // `last_world` so successive ticks accumulate
                    // correctly. The dispatcher's handler also drags
                    // the matching pad if `sketch_entity_id` is set,
                    // keeping the bidirectional link in sync.
                    if drag.moved
                        && let Some(point_id) = drag.sketch_point
                    {
                        let dx_mm = world.0 - drag.last_world.0;
                        let dy_mm = world.1 - drag.last_world.1;
                        drag.last_world = world;
                        self.cache.clear();
                        return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                            library_path: self.address.library_path.clone(),
                            table: self.address.table.clone(),
                            row_id: self.address.row_id,
                            msg: EditorMsg::FootprintSketchMovePoint {
                                id: point_id,
                                dx: dx_mm,
                                dy: dy_mm,
                            },
                        }));
                    }
                    // v0.27 — Line drag tick. Translate both endpoints
                    // in one solver pass via FootprintSketchMoveLine.
                    // The cursor delta is projected onto the line's
                    // perpendicular so dragging an edge only pushes
                    // it in its natural resize direction (Fusion-
                    // style). Without this, a horizontal line would
                    // also slide left/right under the user's mouse,
                    // which doesn't match what the cursor cue (↕)
                    // promises.
                    if drag.moved
                        && let Some(line_id) = drag.sketch_line
                        && let Some(sketch_ref) = self.sketch
                    {
                        let raw_dx = world.0 - drag.last_world.0;
                        let raw_dy = world.1 - drag.last_world.1;
                        // Resolve the line's current direction. Falls
                        // back to the raw delta if the lookup fails
                        // (entity vanished mid-drag, etc.).
                        let endpoints = sketch_ref
                            .entities
                            .iter()
                            .find(|e| e.id == line_id)
                            .and_then(|e| match e.kind {
                                signex_sketch::entity::EntityKind::Line { start, end } => {
                                    Some((start, end))
                                }
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
                                    signex_sketch::entity::EntityKind::Point { x, y } => {
                                        Some((x, y))
                                    }
                                    _ => None,
                                })
                        };
                        let (dx_mm, dy_mm) = match endpoints
                            .and_then(|(s, e)| pos_of(s).zip(pos_of(e)))
                        {
                            Some(((ax, ay), (bx, by))) => {
                                let lx = bx - ax;
                                let ly = by - ay;
                                let llen = (lx * lx + ly * ly).sqrt();
                                if llen <= 1e-9 {
                                    (raw_dx, raw_dy)
                                } else {
                                    // Unit perpendicular (rotate
                                    // tangent +90°): (-ly, lx)/llen.
                                    let nx = -ly / llen;
                                    let ny = lx / llen;
                                    let proj = raw_dx * nx + raw_dy * ny;
                                    (proj * nx, proj * ny)
                                }
                            }
                            None => (raw_dx, raw_dy),
                        };
                        // Advance last_world by the CONSTRAINED delta
                        // so the cursor's parallel motion accumulates
                        // (instead of dropping silently each tick and
                        // forcing the user to overshoot).
                        drag.last_world = (
                            drag.last_world.0 + dx_mm,
                            drag.last_world.1 + dy_mm,
                        );
                        self.cache.clear();
                        return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                            library_path: self.address.library_path.clone(),
                            table: self.address.table.clone(),
                            row_id: self.address.row_id,
                            msg: EditorMsg::FootprintSketchMoveLine {
                                id: line_id,
                                dx: dx_mm,
                                dy: dy_mm,
                            },
                        }));
                    }
                    if drag.moved && drag.pad_idx != usize::MAX {
                        let new_x = world.0 - drag.grab_offset_mm.0;
                        let new_y = world.1 - drag.grab_offset_mm.1;
                        return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                            library_path: self.address.library_path.clone(),
                            table: self.address.table.clone(),
                            row_id: self.address.row_id,
                            msg: EditorMsg::FootprintMovePad {
                                idx: drag.pad_idx,
                                x_mm: new_x,
                                y_mm: new_y,
                            },
                        }));
                    }
                }
                // v0.14.2: when the user is in Sketch mode AND a
                // multi-click tool has placed its anchor (line first
                // endpoint / circle centre / arc centre / arc start),
                // clear the canvas cache on every cursor tick so the
                // dashed ghost preview drawn by
                // `draw_sketch_tool_preview` re-renders against the
                // new cursor position. Without this the ghost stayed
                // frozen at the position when the cache was last
                // cleared.
                // v0.16.1: also clear in Pads mode + PlacePad so the
                // pad-placement ghost rectangle re-renders at the
                // moving cursor.
                use crate::library::editor::footprint::state::{EditorMode, PadsTool, ToolPending};
                let in_sketch_with_anchor = matches!(self.state.mode, EditorMode::Sketch)
                    && !matches!(self.state.tool_pending, ToolPending::Idle);
                let in_pads_place = matches!(self.state.mode, EditorMode::Normal)
                    && matches!(
                        self.state.pads_tool,
                        PadsTool::PlacePad | PadsTool::PlaceVia
                    );
                // v0.27 — re-render lasso ghost as the cursor moves
                // so the open polygon edge tracks live to the cursor.
                let in_lasso = self.state.lasso_mode_active
                    && !self.state.lasso_vertices.is_empty();
                let in_touching_line =
                    self.state.touching_line_active && self.state.touching_line_first.is_some();
                // v0.27 — Sketch mode also redraws every cursor tick
                // so the custom black crosshair tracks the cursor
                // smoothly. Without this the crosshair stays at the
                // last cached position until some other event fires.
                let in_sketch_mode_for_cursor = matches!(self.state.mode, EditorMode::Sketch);
                if in_sketch_with_anchor
                    || in_pads_place
                    || in_lasso
                    || in_touching_line
                    || in_sketch_mode_for_cursor
                {
                    self.cache.clear();
                }

                // Background hover — push the cursor position so the
                // footer readout updates.
                return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                    library_path: self.address.library_path.clone(),
                    table: self.address.table.clone(),
                    row_id: self.address.row_id,
                    msg: EditorMsg::FootprintCursorAt {
                        x_mm: world.0,
                        y_mm: world.1,
                    },
                }));
            }
            // v0.27 — track the live modifier state so the press
            // handler can branch on Ctrl/Cmd (toggle pad in
            // selection) and Shift (extend selection). Mouse events
            // on iced 0.14 don't carry modifiers, so we mirror
            // ModifiersChanged into cstate. Returns None so the rest
            // of the app still receives the event.
            Event::Keyboard(keyboard::Event::ModifiersChanged(mods)) => {
                cstate.current_modifiers = *mods;
                return None;
            }
            // v0.24 Track D — keyboard intercept for the live numeric
            // placement input. Active only while a multi-click sketch
            // tool (Line / Circle / Arc) has its first click pending,
            // so digit keys outside Sketch mode never get swallowed.
            // Modifiers must be empty so global shortcuts (Ctrl+Z,
            // Ctrl+S, …) still reach the app dispatcher.
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) => {
                use crate::library::editor::footprint::state::{
                    EditorMode, PlacementInputKind, ToolPending,
                };

                // v0.26-F — Ctrl+X / Ctrl+C / Ctrl+V clipboard
                // shortcuts. Mode-agnostic (works in Normal AND
                // Sketch) so they behave the same as the right-click
                // menu items they mirror. Publishes via EditorEvent
                // so the standalone-editor translation in
                // `editor_msg_to_primitive_msg` carries them through.
                // Canvas captures the event so iced''s global key
                // subscription doesn''t fire a duplicate handler.
                if modifiers.command()
                    && !modifiers.shift()
                    && !modifiers.alt()
                    && let keyboard::Key::Character(c) = key.as_ref()
                {
                    let cb_msg = match c {
                        "x" | "X" => Some(EditorMsg::FootprintCutPad),
                        "c" | "C" => Some(EditorMsg::FootprintCopyPad),
                        "v" | "V" => Some(EditorMsg::FootprintPastePad),
                        _ => None,
                    };
                    if let Some(msg) = cb_msg {
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

                // v0.26-G — Space (rotate 90°) / X (flip layer) on
                // the selected pad. Altium parity. Mode-agnostic; only
                // fires when there''s a pad to act on so the canvas
                // doesn''t swallow Space / X away from sketch tools
                // that need them. Captures so the global subscription
                // doesn''t double-fire (Space → schematic
                // RotateSelected, X → schematic MirrorSelectedY).
                if !modifiers.command() && !modifiers.alt() {
                    if matches!(key, keyboard::Key::Named(keyboard::key::Named::Space))
                        && self.state.selected_pad.is_some()
                    {
                        return Some(
                            canvas::Action::publish(LibraryMessage::EditorEvent {
                                library_path: self.address.library_path.clone(),
                                table: self.address.table.clone(),
                                row_id: self.address.row_id,
                                msg: EditorMsg::FootprintActiveBarRotateSelection,
                            })
                            .and_capture(),
                        );
                    }
                    if let keyboard::Key::Character(c) = key.as_ref()
                        && (c == "x" || c == "X")
                        && self.state.selected_pad.is_some()
                    {
                        return Some(
                            canvas::Action::publish(LibraryMessage::EditorEvent {
                                library_path: self.address.library_path.clone(),
                                table: self.address.table.clone(),
                                row_id: self.address.row_id,
                                msg: EditorMsg::FootprintActiveBarFlipSelection,
                            })
                            .and_capture(),
                        );
                    }
                }

                if !matches!(self.state.mode, EditorMode::Sketch) {
                    return None;
                }
                // Only intercept when there's either an open buffer or
                // an in-progress gesture that could accept one — both
                // are required so a stray digit press at idle (no
                // first click yet) doesn't open an overlay against an
                // empty tool state.
                let has_open_buffer = self.state.placement_input.is_some();
                let kind_for_active = PlacementInputKind::from_active_tool(
                    self.state.active_tool,
                    &self.state.tool_pending,
                );
                if !has_open_buffer && kind_for_active.is_none() {
                    return None;
                }
                if matches!(self.state.tool_pending, ToolPending::Idle) && !has_open_buffer {
                    return None;
                }
                if modifiers.command() || modifiers.alt() || modifiers.logo() {
                    return None;
                }
                let publish = |msg: EditorMsg| -> Option<canvas::Action<LibraryMessage>> {
                    Some(
                        canvas::Action::publish(LibraryMessage::EditorEvent {
                            library_path: self.address.library_path.clone(),
                            table: self.address.table.clone(),
                            row_id: self.address.row_id,
                            msg,
                        })
                        .and_capture(),
                    )
                };
                match key {
                    keyboard::Key::Named(keyboard::key::Named::Backspace) => {
                        if has_open_buffer {
                            return publish(EditorMsg::FootprintSketchPlacementInputBackspace);
                        }
                    }
                    keyboard::Key::Named(keyboard::key::Named::Enter) => {
                        if has_open_buffer {
                            return publish(EditorMsg::FootprintSketchPlacementInputEnter);
                        }
                    }
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        // v0.27 — Lasso Select cancel via Esc.
                        // Pre-empts the placement-input Esc which
                        // also fires here; lasso mode wins because
                        // it's the more recent intent (the user
                        // armed it from the active bar).
                        if self.state.lasso_mode_active {
                            return publish(EditorMsg::FootprintLassoCancel);
                        }
                        // v0.27 — Touching Line cancel via Esc.
                        if self.state.touching_line_active {
                            return publish(EditorMsg::FootprintTouchingLineCancel);
                        }
                        if has_open_buffer {
                            return publish(EditorMsg::FootprintSketchPlacementInputEscape);
                        }
                    }
                    _ => {
                        // Use the platform-supplied `text` so we get
                        // exactly the codepoint the user typed
                        // (handles Numpad digits, decimal-point
                        // localisation pre-conversion, etc.). Only
                        // forward digits / `.` / `-`; everything else
                        // falls through to the generic catch-all.
                        if let Some(s) = text.as_ref() {
                            if let Some(ch) = s.chars().next() {
                                let useful = ch.is_ascii_digit()
                                    || ch == '.'
                                    || (ch == '-'
                                        && kind_for_active
                                            .or_else(|| {
                                                self.state.placement_input.as_ref().map(|p| p.kind)
                                            })
                                            .map(|k| k.allows_negative())
                                            .unwrap_or(false));
                                if useful {
                                    return publish(EditorMsg::FootprintSketchPlacementInputChar(
                                        ch,
                                    ));
                                }
                            }
                        }
                    }
                }
                return None;
            }
            _ => {}
        }
        None
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
            let (fine_alpha, coarse_alpha) = if in_sketch { (0.50, 0.55) } else { (0.10, 0.30) };
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
                            ArrayKind::Linear { source, count_expr, .. } => {
                                (*source, count_expr.trim().parse::<usize>().unwrap_or(0))
                            }
                            ArrayKind::Grid { source, nx_expr, ny_expr, .. } => {
                                let nx = nx_expr.trim().parse::<usize>().unwrap_or(0);
                                let ny = ny_expr.trim().parse::<usize>().unwrap_or(0);
                                (*source, nx * ny)
                            }
                            ArrayKind::Polar { source, count_expr, .. } => {
                                (*source, count_expr.trim().parse::<usize>().unwrap_or(0))
                            }
                        };
                        if count > 0 {
                            Some((source, count))
                        } else {
                            None
                        }
                    })
                    .fold(
                        std::collections::HashMap::new(),
                        |mut acc, (id, count)| {
                            *acc.entry(id).or_insert(0) += count;
                            acc
                        },
                    );

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
                        let badge_rect = Path::rectangle(
                            Point::new(bx, by),
                            iced::Size::new(badge_w, badge_h),
                        );
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
                    Color { r: 0.55, g: 0.55, b: 0.55, a: 1.0 }
                } else {
                    Color { r: 0.85, g: 0.20, b: 0.20, a: 1.0 }
                };
                let ghost_stroke = if paused {
                    Color { r: 0.40, g: 0.40, b: 0.40, a: 1.0 }
                } else {
                    Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 }
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
                    PS::Chamfered { chamfer_ratio, corners } => {
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
                if let Some(d) = defaults.drill_diameter_mm.filter(|d| *d > f32::EPSILON as f64)
                {
                    let r_px = (d / 2.0) as f32 * cstate.scale;
                    if r_px > 0.5 {
                        let hole_color = if paused {
                            Color { r: 0.10, g: 0.10, b: 0.10, a: 0.85 }
                        } else {
                            Color { r: 0.05, g: 0.05, b: 0.05, a: 0.95 }
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
                    &Path::line(
                        Point::new(p.x - arm, p.y),
                        Point::new(p.x + arm, p.y),
                    ),
                    stroke,
                );
                frame.stroke(
                    &Path::line(
                        Point::new(p.x, p.y - arm),
                        Point::new(p.x, p.y + arm),
                    ),
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
            if in_sketch
                && in_select_tool
                && !context_menu_open
                && let Some(p) = cursor_screen
            {
                let arm = 5.0_f32;
                let near_black = Color::from_rgba(0.10, 0.10, 0.10, 0.90);
                let stroke = Stroke::default().with_width(1.0).with_color(near_black);
                frame.stroke(
                    &Path::line(
                        Point::new(p.x - arm, p.y),
                        Point::new(p.x + arm, p.y),
                    ),
                    stroke,
                );
                frame.stroke(
                    &Path::line(
                        Point::new(p.x, p.y - arm),
                        Point::new(p.x, p.y + arm),
                    ),
                    stroke,
                );
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
                    let rect_path = Path::rectangle(
                        Point::new(x0, y0),
                        iced::Size::new(w, h),
                    );
                    // Altium pen: cyan-ish translucent fill +
                    // dashed-look outline at 1 px.
                    frame.fill(
                        &rect_path,
                        Color::from_rgba(0.30, 0.55, 0.90, 0.18),
                    );
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
                                // v0.27 — bucket the line's angle
                                // into the four cardinal cursors.
                                // The cursor points along the
                                // perpendicular (the direction the
                                // user drags to push the edge).
                                //
                                //   horizontal line  →   ↕  (drag up/down)
                                //   vertical line    →   ↔  (drag left/right)
                                //   "/" diagonal     →   ↘↖ ResizingDiagonallyDown
                                //   "\" diagonal     →   ↗↙ ResizingDiagonallyUp
                                //
                                // World coords share screen Y-down
                                // (world_to_screen does no flip), so
                                // dy/dx > 0 reads as "going down-and-
                                // right" on screen, which is the "\"
                                // slope visually.
                                let angle_deg = dy
                                    .atan2(dx)
                                    .to_degrees()
                                    .rem_euclid(180.0);
                                return if angle_deg < 22.5 || angle_deg >= 157.5 {
                                    mouse::Interaction::ResizingVertically
                                } else if angle_deg < 67.5 {
                                    mouse::Interaction::ResizingDiagonallyUp
                                } else if angle_deg < 112.5 {
                                    mouse::Interaction::ResizingHorizontally
                                } else {
                                    mouse::Interaction::ResizingDiagonallyDown
                                };
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

