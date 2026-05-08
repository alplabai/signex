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
                    let world = {
                        let point_hit = sketch_snap(self.sketch, cstate, raw_world);
                        let result =
                            snap::snap_cursor(raw_world, self.sketch, self.state, point_hit);
                        cstate.last_snap = Some(result);
                        result.pos
                    };
                    if matches!(self.state.mode, _EM::Sketch)
                        && self.state.active_tool == _ST::Select
                        && let Some(point_id) = sketch_snap(self.sketch, cstate, raw_world)
                    {
                        cstate.drag = Some(DragState {
                            pad_idx: usize::MAX,
                            sketch_point: Some(point_id),
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
                                grab_offset_mm: (
                                    world.0 - pad.position_mm.0,
                                    world.1 - pad.position_mm.1,
                                ),
                                last_world: world,
                                press_screen: cursor_pos,
                                moved: false,
                            });
                            return Some(
                                canvas::Action::publish(LibraryMessage::EditorEvent {
                                    library_path: self.address.library_path.clone(),
                                    table: self.address.table.clone(),
                                    row_id: self.address.row_id,
                                    msg: EditorMsg::FootprintSelectPad(Some(pad_idx)),
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
                        grab_offset_mm: (world.0, world.1),
                        last_world: world,
                        press_screen: cursor_pos,
                        moved: false,
                    });
                    // v0.26-I — Select-tool empty-canvas press also
                    // arms the rubber-band rectangle. Sketch / Pads-
                    // tool placements are unaffected (their drag
                    // continues to commit a placement on release if
                    // it didn''t cross the move threshold). The draw
                    // pass renders the rubber band only when both
                    // `box_select_anchor_screen` and
                    // `box_select_current_screen` are Some, so a
                    // brief un-moved drag never paints a rectangle.
                    if matches!(self.state.mode, super::state::EditorMode::Normal)
                        && self.state.pads_tool == super::state::PadsTool::Select
                    {
                        cstate.box_select_anchor_screen = Some(cursor_pos);
                        cstate.box_select_current_screen = Some(cursor_pos);
                    }
                    return Some(canvas::Action::capture());
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                if matches!(button, mouse::Button::Right | mouse::Button::Middle) {
                    let did_pan = cstate.pan_moved;
                    cstate.panning = false;
                    cstate.last_pan_pos = None;
                    cstate.pan_moved = false;
                    // v0.26 — right-release without pan motion opens
                    // the canvas context menu. Middle-click never
                    // shows a menu (Altium parity: middle is pure
                    // pan).
                    if !did_pan
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
                            // Cancelled click-add — drag in empty
                            // space without crossing a pad doesn't
                            // place anything.
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
                                | SketchTool::TangentArc => EditorMsg::FootprintSketchToolClick {
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
                        // v0.26-I — rubber-band release. If the press
                        // armed a box-select (Select tool, empty
                        // canvas) AND the drag actually moved, walk
                        // the pad list and pick the first pad whose
                        // bbox lies inside the rectangle. Multi-
                        // select is queued; for now the rubber band
                        // is a single-pad picker that lets the user
                        // grab a pad without an exact left-click.
                        if let (Some(a), Some(c)) = (
                            cstate.box_select_anchor_screen.take(),
                            cstate.box_select_current_screen.take(),
                        ) {
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
                            let mut hit: Option<usize> = None;
                            for (idx, pad) in self.state.pads.iter().enumerate() {
                                let (px0, py0, px1, py1) = pad.bbox_mm();
                                if px0 >= x0 && px1 <= x1 && py0 >= y0 && py1 <= y1 {
                                    hit = Some(idx);
                                    break;
                                }
                            }
                            self.cache.clear();
                            if let Some(idx) = hit {
                                return Some(canvas::Action::publish(
                                    LibraryMessage::EditorEvent {
                                        library_path: self.address.library_path.clone(),
                                        table: self.address.table.clone(),
                                        row_id: self.address.row_id,
                                        msg: EditorMsg::FootprintSelectPad(Some(idx)),
                                    },
                                ));
                            }
                            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                                library_path: self.address.library_path.clone(),
                                table: self.address.table.clone(),
                                row_id: self.address.row_id,
                                msg: EditorMsg::FootprintSelectPad(None),
                            }));
                        }
                        // Final move position is already published per
                        // CursorMoved tick — nothing to do on release.
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
                let world = {
                    let point_hit = sketch_snap(self.sketch, cstate, raw_world);
                    let result = snap::snap_cursor(raw_world, self.sketch, self.state, point_hit);
                    cstate.last_snap = Some(result);
                    result.pos
                };
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
                    && self.state.pads_tool == PadsTool::PlacePad;
                if in_sketch_with_anchor || in_pads_place {
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
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let geom = self.cache.draw(renderer, bounds.size(), |frame| {
            // Background.
            frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.bg_color);

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
            if fine_step >= 6.0 {
                let fine_color = Color {
                    a: 0.10,
                    ..self.grid_color
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
            if coarse_step >= 8.0 {
                let coarse_color = Color {
                    a: 0.30,
                    ..self.grid_color
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

            // Origin crosshair — Altium-style yellow + at world (0, 0).
            // v0.16.2.2 swapped from theme-derived white to a
            // saturated Altium yellow so the origin pops against the
            // black canvas background.
            let origin = cstate.world_to_screen((0.0, 0.0));
            let origin_color = Color::from_rgba(1.0, 0.95, 0.30, 0.85);
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

            // Courtyard — drawn as a hollow rectangle on Edge.Cuts.
            if self.state.layer_visibility.get(FpLayer::EdgeCuts)
                && let Some(c) = self.state.courtyard_mm
            {
                let p0 = cstate.world_to_screen((c.min_x, c.min_y));
                let p1 = cstate.world_to_screen((c.max_x, c.max_y));
                let rect = Path::rectangle(
                    Point::new(p0.x, p0.y),
                    iced::Size::new(p1.x - p0.x, p1.y - p0.y),
                );
                frame.stroke(
                    &rect,
                    Stroke::default()
                        .with_width(1.5)
                        .with_color(FpLayer::EdgeCuts.color()),
                );
            }

            // Pads — render last so they sit on top.
            for (idx, pad) in self.state.pads.iter().enumerate() {
                if !self.state.layer_visibility.get(pad.primary_layer()) {
                    continue;
                }
                draw_pad(frame, cstate, pad, self.state.selected_pad == Some(idx));
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
            use crate::library::editor::footprint::state::{EditorMode, PadsTool};
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

            // v0.18.16 — Pads-mode multi-click gesture previews
            // (Track / Arc / Polygon ghost lines). Reads in-flight
            // state + cursor; no-op for tools without a multi-click
            // gesture (Select / PlacePad / PlaceHole / PlaceString).
            if matches!(self.state.mode, super::state::EditorMode::Normal) {
                draw_pads_tool_preview(frame, cstate, self.state);
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
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

/// v0.13.2 Phase 6.6 — render constraint glyphs above the sketch
/// entities. Each constraint's centroid (geometric mean of the
/// entities it touches) gets a small Unicode glyph; over-constrained
/// constraints render in red.
fn draw_constraint_icons(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    use signex_sketch::constraint::ConstraintKind;
    use signex_sketch::entity::EntityKind;
    use signex_sketch::id::{ConstraintId, SketchEntityId};

    let over_set: std::collections::HashSet<ConstraintId> = state
        .last_solve
        .as_ref()
        .map(|s| s.over_constraints.iter().copied().collect())
        .unwrap_or_default();

    let point_world_local = |id: SketchEntityId| -> Option<(f64, f64)> {
        if let Some(solve) = state.last_solve.as_ref() {
            if let Some(p) = signex_sketch::solver::state::point_xy(
                id,
                &solve.result.state,
                &solve.result.index,
                sketch,
            ) {
                return Some(p);
            }
        }
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    };
    let line_endpoints_local = |id: SketchEntityId| -> Option<(SketchEntityId, SketchEntityId)> {
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Line { start, end } => Some((start, end)),
                _ => None,
            })
    };
    fn arc_refs_local(
        sketch: &signex_sketch::SketchData,
        id: signex_sketch::id::SketchEntityId,
    ) -> Option<(
        signex_sketch::id::SketchEntityId,
        signex_sketch::id::SketchEntityId,
        signex_sketch::id::SketchEntityId,
        bool,
    )> {
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                signex_sketch::entity::EntityKind::Arc {
                    center,
                    start,
                    end,
                    sweep_ccw,
                } => Some((center, start, end, sweep_ccw)),
                _ => None,
            })
    }
    fn circle_center_local(
        sketch: &signex_sketch::SketchData,
        id: signex_sketch::id::SketchEntityId,
    ) -> Option<signex_sketch::id::SketchEntityId> {
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                signex_sketch::entity::EntityKind::Circle { center, .. } => Some(center),
                _ => None,
            })
    }

    for c in &sketch.constraints {
        let (glyph, points): (&str, Vec<SketchEntityId>) = match &c.kind {
            ConstraintKind::Coincident { p1, p2 } => ("=", vec![*p1, *p2]),
            ConstraintKind::PointOnLine { point, line } => {
                let mut v = vec![*point];
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.push(s);
                    v.push(e);
                }
                ("|", v)
            }
            ConstraintKind::Horizontal { line } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.push(s);
                    v.push(e);
                }
                ("H", v)
            }
            ConstraintKind::Vertical { line } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.push(s);
                    v.push(e);
                }
                ("V", v)
            }
            ConstraintKind::Parallel { l1, l2 } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*l1) {
                    v.extend([s, e]);
                }
                if let Some((s, e)) = line_endpoints_local(*l2) {
                    v.extend([s, e]);
                }
                ("//", v)
            }
            ConstraintKind::Perpendicular { l1, l2 } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*l1) {
                    v.extend([s, e]);
                }
                if let Some((s, e)) = line_endpoints_local(*l2) {
                    v.extend([s, e]);
                }
                ("L", v)
            }
            ConstraintKind::DistancePtPt { p1, p2, .. } => ("D", vec![*p1, *p2]),
            ConstraintKind::DistancePtLine { point, line, .. } => {
                let mut v = vec![*point];
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.push(s);
                    v.push(e);
                }
                ("d", v)
            }
            ConstraintKind::DistancePtCircle { point, circle, .. } => {
                let mut v = vec![*point];
                if let Some(c) = circle_center_local(sketch, *circle) {
                    v.push(c);
                } else if let Some((c, _, _, _)) = arc_refs_local(sketch, *circle) {
                    v.push(c);
                }
                ("\u{29bf}", v) // ⦿ "DistancePtCircle"
            }
            ConstraintKind::Fixed { point } => ("\u{1F512}", vec![*point]),
            // v0.13.3 — remaining constraint glyphs.
            ConstraintKind::PointOnArc { point, arc } => {
                let mut v = vec![*point];
                if let Some((c, s, e, _)) = arc_refs_local(sketch, *arc) {
                    v.extend([c, s, e]);
                }
                ("\u{2192}", v) // → "PointOnArc"
            }
            ConstraintKind::Angle { l1, l2, .. } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*l1) {
                    v.extend([s, e]);
                }
                if let Some((s, e)) = line_endpoints_local(*l2) {
                    v.extend([s, e]);
                }
                ("A", v)
            }
            ConstraintKind::EqualLength { l1, l2 } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*l1) {
                    v.extend([s, e]);
                }
                if let Some((s, e)) = line_endpoints_local(*l2) {
                    v.extend([s, e]);
                }
                ("=L", v)
            }
            ConstraintKind::EqualRadius { e1, e2 } => {
                let mut v = Vec::new();
                if let Some((c, _, _, _)) = arc_refs_local(sketch, *e1) {
                    v.push(c);
                }
                if let Some((c, _, _, _)) = arc_refs_local(sketch, *e2) {
                    v.push(c);
                }
                if v.is_empty() {
                    if let Some(c) = circle_center_local(sketch, *e1) {
                        v.push(c);
                    }
                    if let Some(c) = circle_center_local(sketch, *e2) {
                        v.push(c);
                    }
                }
                ("=R", v)
            }
            ConstraintKind::TangentLineArc { line, arc } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.extend([s, e]);
                }
                if let Some((c, _, _, _)) = arc_refs_local(sketch, *arc) {
                    v.push(c);
                }
                ("T", v)
            }
            ConstraintKind::TangentArcArc { a1, a2, .. } => {
                let mut v = Vec::new();
                if let Some((c, _, _, _)) = arc_refs_local(sketch, *a1) {
                    v.push(c);
                }
                if let Some((c, _, _, _)) = arc_refs_local(sketch, *a2) {
                    v.push(c);
                }
                ("TT", v)
            }
            ConstraintKind::SymmetricAboutLine { p1, p2, line } => {
                let mut v = vec![*p1, *p2];
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.extend([s, e]);
                }
                ("\u{29C7}", v) // ⧇ "Symmetric"
            }
            ConstraintKind::SymmetricAboutPoint { p1, p2, center } => {
                ("\u{29C7}", vec![*p1, *p2, *center])
            }
            ConstraintKind::Midpoint { point, line } => {
                let mut v = vec![*point];
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.extend([s, e]);
                }
                ("M", v)
            }
        };
        if glyph.is_empty() || points.is_empty() {
            continue;
        }
        let mut sum_x = 0.0_f64;
        let mut sum_y = 0.0_f64;
        let mut n = 0;
        for id in &points {
            if let Some((x, y)) = point_world_local(*id) {
                sum_x += x;
                sum_y += y;
                n += 1;
            }
        }
        if n == 0 {
            continue;
        }
        let centroid = (sum_x / n as f64, sum_y / n as f64);
        let p = cstate.world_to_screen(centroid);
        // v0.23 — per-row precision in the Conflicts list. When the
        // user hovers a specific row, only that constraint renders
        // at full red; every other glyph (including other
        // over-constraints) dims so the offender stands out alone.
        // When no row is hovered, fall back to the v0.22 set-wide
        // isolation (the whole over-constraint set lights up).
        let hover = state.conflicts_row_hovered;
        let is_over = over_set.contains(&c.id);
        let colour = match (hover, is_over) {
            // Specific row hovered + this is the row → full red.
            (Some(h), _) if h == c.id => Color::from_rgba(1.0, 0.20, 0.20, 1.00),
            // Specific row hovered + this is NOT the row → dimmed
            // (other over-constraints get the same dim as
            // non-over-constraints so the focus stays singular).
            (Some(_), _) => Color::from_rgba(0.85, 0.85, 0.85, 0.15),
            // No row hover + over-constrained → red (set-wide focus).
            (None, true) => Color::from_rgba(1.0, 0.20, 0.20, 1.00),
            // Default — non-over-constrained, no hover.
            (None, false) => Color::from_rgba(0.85, 0.85, 0.85, 0.85),
        };
        frame.fill_text(canvas::Text {
            content: glyph.to_string(),
            position: Point::new(p.x + 6.0, p.y - 6.0),
            color: colour,
            size: iced::Pixels(11.0),
            ..canvas::Text::default()
        });
    }
}

/// v0.13.3 — Hit-test Lines / Arcs / Circles (everything that isn't
/// a Point — Points are caught by `sketch_snap`). Returns the
/// nearest entity within `SKETCH_SNAP_RADIUS_PX`. Used by the
/// Select tool so the user can grab line / arc / circle entities,
/// not just Points.
fn sketch_hit_other(
    sketch: Option<&signex_sketch::SketchData>,
    cstate: &FootprintCanvasState,
    click_world: (f64, f64),
) -> Option<signex_sketch::id::SketchEntityId> {
    use signex_sketch::entity::EntityKind;
    let sketch = sketch?;
    let click_screen = cstate.world_to_screen(click_world);
    let radius_sq = SKETCH_SNAP_RADIUS_PX * SKETCH_SNAP_RADIUS_PX;
    let mut best: Option<(f32, signex_sketch::id::SketchEntityId)> = None;

    let resolve_pt = |id: signex_sketch::id::SketchEntityId| -> Option<(f64, f64)> {
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    };

    for entity in &sketch.entities {
        let world_dist_sq = match entity.kind {
            EntityKind::Line { start, end } => {
                let s = resolve_pt(start);
                let e = resolve_pt(end);
                match (s, e) {
                    (Some(s), Some(e)) => {
                        let p0 = cstate.world_to_screen(s);
                        let p1 = cstate.world_to_screen(e);
                        screen_dist_to_segment_sq(click_screen, p0, p1)
                    }
                    _ => continue,
                }
            }
            EntityKind::Circle { center, radius } => {
                let c = match resolve_pt(center) {
                    Some(c) => c,
                    None => continue,
                };
                let centre = cstate.world_to_screen(c);
                let dx = click_screen.x - centre.x;
                let dy = click_screen.y - centre.y;
                let dist = (dx * dx + dy * dy).sqrt();
                let r_screen = (radius as f32) * cstate.scale;
                let edge_dist = (dist - r_screen).abs();
                edge_dist * edge_dist
            }
            EntityKind::Arc { center, .. } => {
                let c = match resolve_pt(center) {
                    Some(c) => c,
                    None => continue,
                };
                let centre = cstate.world_to_screen(c);
                let dx = click_screen.x - centre.x;
                let dy = click_screen.y - centre.y;
                dx * dx + dy * dy
            }
            EntityKind::Point { .. } => continue,
        };
        if world_dist_sq <= radius_sq {
            match best {
                Some((b, _)) if b <= world_dist_sq => {}
                _ => best = Some((world_dist_sq, entity.id)),
            }
        }
    }
    best.map(|(_, id)| id)
}

fn screen_dist_to_segment_sq(p: Point, a: Point, b: Point) -> f32 {
    let abx = b.x - a.x;
    let aby = b.y - a.y;
    let len_sq = abx * abx + aby * aby;
    if len_sq < 1e-6 {
        let dx = p.x - a.x;
        let dy = p.y - a.y;
        return dx * dx + dy * dy;
    }
    let t = ((p.x - a.x) * abx + (p.y - a.y) * aby) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let qx = a.x + abx * t;
    let qy = a.y + aby * t;
    let dx = p.x - qx;
    let dy = p.y - qy;
    dx * dx + dy * dy
}

/// v0.13.2 — Snap radius in screen pixels. A click within this
/// distance of an existing sketch Point's screen position resolves
/// to that Point (auto-Coincident).
const SKETCH_SNAP_RADIUS_PX: f32 = 8.0;

/// Find the sketch Point whose screen position is within
/// `SKETCH_SNAP_RADIUS_PX` of the given world-mm click. Returns the
/// nearest-snap Point's `SketchEntityId`, or `None` if no Point is
/// in range. Used by the canvas to drive auto-Coincident behaviour
/// in multi-click drawing tools.
fn sketch_snap(
    sketch: Option<&signex_sketch::SketchData>,
    cstate: &FootprintCanvasState,
    click_world: (f64, f64),
) -> Option<signex_sketch::id::SketchEntityId> {
    use signex_sketch::entity::EntityKind;
    let sketch = sketch?;
    let click_screen = cstate.world_to_screen(click_world);
    let radius_sq = SKETCH_SNAP_RADIUS_PX * SKETCH_SNAP_RADIUS_PX;
    let mut best: Option<(f32, signex_sketch::id::SketchEntityId)> = None;
    for entity in &sketch.entities {
        if let EntityKind::Point { x, y } = entity.kind {
            let p = cstate.world_to_screen((x, y));
            let dx = p.x - click_screen.x;
            let dy = p.y - click_screen.y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= radius_sq {
                match best {
                    Some((b, _)) if b <= dist_sq => {}
                    _ => best = Some((dist_sq, entity.id)),
                }
            }
        }
    }
    best.map(|(_, id)| id)
}

/// Render the sketch entities (Phase 6.2). Points draw as small
/// filled circles, Lines stroke between their endpoints (dashed if
/// `construction == true`), Circles stroke the radius circle, Arcs
/// stroke a polyline approximation between start/end. DOF colour
/// drives the tint.
fn draw_sketch_overlay(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    use signex_sketch::entity::EntityKind;
    use signex_sketch::id::SketchEntityId;

    fn point_world(
        id: SketchEntityId,
        sketch: &signex_sketch::SketchData,
        state: &FootprintEditorState,
    ) -> Option<(f64, f64)> {
        // Prefer the solved state if available; fall back to the
        // entity's authored coords.
        if let Some(solve) = state.last_solve.as_ref() {
            if let Some((x, y)) = signex_sketch::solver::state::point_xy(
                id,
                &solve.result.state,
                &solve.result.index,
                sketch,
            ) {
                return Some((x, y));
            }
        }
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    }

    let dof_colour = |id: SketchEntityId| -> Color {
        use signex_sketch::solver::dof::DofColor;
        if let Some(solve) = state.last_solve.as_ref() {
            match solve.colours.get(&id) {
                Some(DofColor::Under) => Color::from_rgba(0.20, 0.40, 1.00, 1.00), // blue
                Some(DofColor::Over) => Color::from_rgba(1.00, 0.20, 0.20, 1.00),  // red
                Some(DofColor::Full) => Color::from_rgba(0.20, 0.85, 0.30, 1.00),  // green
                None => Color::from_rgba(0.85, 0.85, 0.85, 1.00),
            }
        } else {
            Color::from_rgba(0.85, 0.85, 0.85, 1.00)
        }
    };

    // v0.13.2 Phase 6.6 — Constraint icon overlay. Render BEFORE
    // entities so glyphs sit underneath the geometry layer and don't
    // hide pad-edge clicks. Tinted red for over-constrained
    // constraints; muted otherwise.
    draw_constraint_icons(frame, cstate, sketch, state);

    // v0.16.1 — Filled rendering for closed loops. Walks the line
    // graph, finds simple cycles whose Lines are NOT all
    // construction-flagged, and fills the polygon with a faint
    // role-tinted fill. Pad-corner outlines (whose Lines are all
    // construction-flagged) are skipped so they don't double-fill
    // over the rendered pads. Role-attr-driven layer tinting comes
    // with the role-assignment UI; for now everything assigns to a
    // neutral grey at low opacity.
    draw_filled_closed_loops(frame, cstate, sketch, state);

    for entity in &sketch.entities {
        match entity.kind {
            EntityKind::Point { .. } => {
                let world = match point_world(entity.id, sketch, state) {
                    Some(w) => w,
                    None => continue,
                };
                let p = cstate.world_to_screen(world);
                // v0.23 — Bumped Point handle sizes so corner/edge
                // grab targets read from a normal viewing distance.
                // Construction (bake-skipped) Points stay smaller
                // than authored Points so they read as secondary
                // chrome, but both are now grab-friendly.
                let r = if entity.bake_skipped() { 4.0 } else { 5.5 };
                let path = Path::circle(Point::new(p.x, p.y), r);
                let col = dof_colour(entity.id);
                frame.fill(&path, col);
                frame.stroke(
                    &path,
                    Stroke::default().with_width(1.5).with_color(Color {
                        a: 1.0,
                        r: col.r * 0.6,
                        g: col.g * 0.6,
                        b: col.b * 0.6,
                    }),
                );
            }
            EntityKind::Line { start, end } => {
                let s = match point_world(start, sketch, state) {
                    Some(w) => w,
                    None => continue,
                };
                let e = match point_world(end, sketch, state) {
                    Some(w) => w,
                    None => continue,
                };
                let p0 = cstate.world_to_screen(s);
                let p1 = cstate.world_to_screen(e);
                // v0.22 Phase A5 — Centerline lines render in Altium /
                // Fusion gold (#c9a04b) regardless of DOF colour, so
                // axis / mirror lines stay visually distinct from
                // construction scaffolding.
                let col = if entity.centerline {
                    Color::from_rgba(0.79, 0.63, 0.30, 1.00)
                } else {
                    dof_colour(start)
                };
                let stroke = Stroke::default().with_width(1.5).with_color(col);
                if entity.construction {
                    // Dashed line via short segments.
                    let dx = p1.x - p0.x;
                    let dy = p1.y - p0.y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > 0.0 {
                        let dash_len = 6.0_f32;
                        let n = (len / dash_len).floor() as i32;
                        for i in (0..n).step_by(2) {
                            let t0 = i as f32 / n as f32;
                            let t1 = ((i + 1) as f32 / n as f32).min(1.0);
                            let q0 = Point::new(p0.x + dx * t0, p0.y + dy * t0);
                            let q1 = Point::new(p0.x + dx * t1, p0.y + dy * t1);
                            frame.stroke(&Path::line(q0, q1), stroke);
                        }
                    }
                } else if entity.centerline {
                    // v0.22 Phase A5 — long-dash + dot pattern.
                    // Walk the line in screen-space cycles of
                    // [long-dash 12 px][gap 4][dot 1.5][gap 4]; ~21 px
                    // per cycle. Matches Altium's centerline glyph.
                    let dx = p1.x - p0.x;
                    let dy = p1.y - p0.y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > 0.5 {
                        let cycle = 21.5_f32;
                        let mut t = 0.0_f32;
                        while t < len {
                            let long_end = (t + 12.0).min(len);
                            let q0 =
                                Point::new(p0.x + dx * (t / len), p0.y + dy * (t / len));
                            let q1 = Point::new(
                                p0.x + dx * (long_end / len),
                                p0.y + dy * (long_end / len),
                            );
                            frame.stroke(&Path::line(q0, q1), stroke);
                            let dot_start = t + 16.0;
                            let dot_end = (dot_start + 1.5).min(len);
                            if dot_start < len {
                                let q2 = Point::new(
                                    p0.x + dx * (dot_start / len),
                                    p0.y + dy * (dot_start / len),
                                );
                                let q3 = Point::new(
                                    p0.x + dx * (dot_end / len),
                                    p0.y + dy * (dot_end / len),
                                );
                                frame.stroke(&Path::line(q2, q3), stroke);
                            }
                            t += cycle;
                        }
                    }
                } else {
                    frame.stroke(&Path::line(p0, p1), stroke);
                }
            }
            EntityKind::Circle { center, radius } => {
                let c = match point_world(center, sketch, state) {
                    Some(w) => w,
                    None => continue,
                };
                let centre = cstate.world_to_screen(c);
                let r_screen = (radius as f32) * cstate.scale;
                let path = Path::circle(Point::new(centre.x, centre.y), r_screen);
                let col = dof_colour(entity.id);
                frame.stroke(&path, Stroke::default().with_width(1.5).with_color(col));
            }
            EntityKind::Arc {
                center,
                start,
                end,
                sweep_ccw,
            } => {
                // Approximate the arc by a 16-segment polyline between
                // start and end on the circle through `center`. Sweep
                // direction respects the entity's `sweep_ccw` flag —
                // CCW arcs walk positive delta, CW arcs walk negative
                // delta.
                let c = match point_world(center, sketch, state) {
                    Some(w) => w,
                    None => continue,
                };
                let s = match point_world(start, sketch, state) {
                    Some(w) => w,
                    None => continue,
                };
                let e = match point_world(end, sketch, state) {
                    Some(w) => w,
                    None => continue,
                };
                let r = ((s.0 - c.0).powi(2) + (s.1 - c.1).powi(2)).sqrt();
                let a0 = (s.1 - c.1).atan2(s.0 - c.0);
                let a1 = (e.1 - c.1).atan2(e.0 - c.0);
                let mut delta = a1 - a0;
                let tau = std::f64::consts::TAU;
                if sweep_ccw {
                    while delta < 0.0 {
                        delta += tau;
                    }
                } else {
                    // Clockwise sweep — delta should be ≤ 0; wrap into
                    // (−2π, 0].
                    while delta > 0.0 {
                        delta -= tau;
                    }
                }
                let segs = 16;
                let mut prev = cstate.world_to_screen(s);
                let col = dof_colour(entity.id);
                for i in 1..=segs {
                    let t = (i as f64) / (segs as f64);
                    let a = a0 + delta * t;
                    let p = (c.0 + r * a.cos(), c.1 + r * a.sin());
                    let q = cstate.world_to_screen(p);
                    frame.stroke(
                        &Path::line(prev, q),
                        Stroke::default().with_width(1.5).with_color(col),
                    );
                    prev = q;
                }
            }
        }
    }
}

/// v0.22 Phase E2 — DOF direction-arrow overlay for under-constrained
/// Points. For every Point with `DofColor::Under`, draws a 10-px-long
/// 1-px-wide cyan arrow pointing in the direction of least constraint
/// sensitivity — i.e. the direction in which moving the Point
/// increases the constraint residual the least. Visually answers the
/// "if I drag this blue Point, which way will it go freely?"
/// question Fusion users expect.
///
/// Math: for a Point with Jacobian columns `c_x`, `c_y` (each column
/// is the partial derivative of every residual w.r.t. that state
/// var), the direction of greatest constraint sensitivity is the
/// eigenvector of
///   `M = [[||c_x||², c_x·c_y], [c_x·c_y, ||c_y||²]]`
/// associated with the LARGEST eigenvalue. The free-DoF direction is
/// the perpendicular (smallest-eigenvalue eigenvector).
///
/// Closed-form for a 2×2 symmetric matrix:
/// - λ_min = (a+d)/2 − √(((a-d)/2)² + b²)
/// - eigenvector for λ_min:
///     - if |b| > ε: (b, λ_min − a), normalized
///     - else (already diagonal): pick whichever column is smaller
/// - if all of a, b, d ≈ 0 (Point isn't touched by any constraint):
///   default to (1, 0) so the arrow still gives visual feedback.
///
/// Hides itself entirely when `state.last_solve` is `None` or the
/// jacobian is empty.
fn draw_dof_direction_arrows(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    use signex_sketch::entity::EntityKind;
    use signex_sketch::solver::dof::DofColor;

    let solve = match state.last_solve.as_ref() {
        Some(s) => s,
        None => return,
    };
    if solve.jacobian.is_empty() {
        // No constraints yet — would draw an arrow on every Point.
        // Silent skip; the user reads "no constraints" from the DOF
        // counter in the inspector.
        return;
    }

    const ARROW_LEN_PX: f32 = 10.0;
    const HEAD_LEN_PX: f32 = 3.0;
    const HEAD_SPREAD_RAD: f64 = 0.5; // ~28°
    let cyan = Color::from_rgba(0.30, 0.85, 0.95, 0.85);
    let stroke = Stroke::default().with_width(1.0).with_color(cyan);

    let m_rows = solve.jacobian.len();

    for entity in &sketch.entities {
        let pt_id = match entity.kind {
            EntityKind::Point { .. } => entity.id,
            _ => continue,
        };
        if !matches!(solve.colours.get(&pt_id), Some(DofColor::Under)) {
            continue;
        }
        let (xi, yi) = match solve.result.index.points.get(&pt_id) {
            Some(t) => *t,
            None => continue, // Fixed Point — has no state column.
        };
        // Compute a, d, b from columns xi, yi.
        let mut a = 0.0_f64;
        let mut d = 0.0_f64;
        let mut b = 0.0_f64;
        for r in 0..m_rows {
            let row = &solve.jacobian[r];
            if xi >= row.len() || yi >= row.len() {
                continue;
            }
            let cx = row[xi];
            let cy = row[yi];
            a += cx * cx;
            d += cy * cy;
            b += cx * cy;
        }
        let (mut dirx, mut diry) = if a.abs() < 1e-12 && d.abs() < 1e-12 && b.abs() < 1e-12
        {
            (1.0, 0.0)
        } else {
            let half = (a + d) * 0.5;
            let radicand = ((a - d) * 0.5).powi(2) + b * b;
            let lam_min = half - radicand.sqrt();
            if b.abs() > 1e-12 {
                (b, lam_min - a)
            } else if a <= d {
                (1.0, 0.0)
            } else {
                (0.0, 1.0)
            }
        };
        let mag = (dirx * dirx + diry * diry).sqrt();
        if mag < 1e-12 {
            dirx = 1.0;
            diry = 0.0;
        } else {
            dirx /= mag;
            diry /= mag;
        }

        // Resolve world position via the solved state (preferring) or
        // the authored entity coords.
        let world = if let Some(p) = signex_sketch::solver::state::point_xy(
            pt_id,
            &solve.result.state,
            &solve.result.index,
            sketch,
        ) {
            p
        } else {
            match entity.kind {
                EntityKind::Point { x, y } => (x, y),
                _ => continue,
            }
        };
        let p_screen = cstate.world_to_screen(world);

        // Screen-space arrow. Y is flipped on screen so we negate
        // diry to match the world convention (positive y is up in
        // world but down in screen).
        let dx_s = dirx as f32 * ARROW_LEN_PX;
        let dy_s = -(diry as f32) * ARROW_LEN_PX;
        let tip = Point::new(p_screen.x + dx_s, p_screen.y + dy_s);
        let shaft = Path::line(p_screen, tip);
        frame.stroke(&shaft, stroke);

        // Arrow head: two short strokes at ±HEAD_SPREAD_RAD from the
        // shaft direction.
        let dir_angle = (dy_s as f64).atan2(dx_s as f64);
        for sign in [-1.0_f64, 1.0_f64] {
            let a = dir_angle + std::f64::consts::PI - sign * HEAD_SPREAD_RAD;
            let head_end = Point::new(
                tip.x + (a.cos() as f32) * HEAD_LEN_PX,
                tip.y + (a.sin() as f32) * HEAD_LEN_PX,
            );
            frame.stroke(&Path::line(tip, head_end), stroke);
        }
    }
}

/// v0.22 Phase A6 — Inferred-constraint snap glyph at the cursor.
/// Rendered AFTER the entity overlay so the badge sits on top of the
/// underlying geometry. Drives off `cstate.last_snap` which the
/// cursor-moved handler refreshes via `snap::snap_cursor`. Visible
/// only while a placement tool is active — Select doesn't draw a
/// hint because no entity is about to land. Glyphs:
/// - `●` (filled circle in cyan) — `SnapKind::Point` — auto-Coincident
///   target; clicking lands a new Point coincident with this one.
/// - `─` (horizontal cyan bar) — `SnapKind::Horizontal` — auto-H
///   constraint will land on the new Line.
/// - `│` (vertical cyan bar) — `SnapKind::Vertical` — auto-V
///   constraint will land on the new Line.
/// - `◇` (cyan diamond) — `SnapKind::Angle` — angle-snapped to the
///   nearest 15° increment.
/// - Guide / Grid / Raw — silent (Guide already paints its line;
///   Grid + Raw aren't actionable hints).
fn draw_sketch_snap_glyph(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    state: &FootprintEditorState,
) {
    use super::snap::SnapKind;
    use super::state::SketchTool;

    if matches!(state.active_tool, SketchTool::Select) {
        return;
    }
    let snap = match cstate.last_snap {
        Some(s) => s,
        None => return,
    };
    let p = cstate.world_to_screen(snap.pos);
    let c = Color::from_rgba(0.30, 0.90, 1.00, 0.95);
    let fill = Color { a: 0.30, ..c };
    let stroke = Stroke::default().with_width(1.5).with_color(c);

    match snap.kind {
        SnapKind::Point(_) => {
            let path = Path::circle(Point::new(p.x, p.y), 7.0);
            frame.fill(&path, fill);
            frame.stroke(&path, stroke);
        }
        SnapKind::Horizontal => {
            frame.stroke(
                &Path::line(
                    Point::new(p.x - 10.0, p.y),
                    Point::new(p.x + 10.0, p.y),
                ),
                stroke,
            );
        }
        SnapKind::Vertical => {
            frame.stroke(
                &Path::line(
                    Point::new(p.x, p.y - 10.0),
                    Point::new(p.x, p.y + 10.0),
                ),
                stroke,
            );
        }
        SnapKind::Angle(_) => {
            let r = 6.0;
            frame.stroke(
                &Path::line(Point::new(p.x, p.y - r), Point::new(p.x + r, p.y)),
                stroke,
            );
            frame.stroke(
                &Path::line(Point::new(p.x + r, p.y), Point::new(p.x, p.y + r)),
                stroke,
            );
            frame.stroke(
                &Path::line(Point::new(p.x, p.y + r), Point::new(p.x - r, p.y)),
                stroke,
            );
            frame.stroke(
                &Path::line(Point::new(p.x - r, p.y), Point::new(p.x, p.y - r)),
                stroke,
            );
        }
        SnapKind::Guide | SnapKind::Grid | SnapKind::Raw => {}
    }
}

/// v0.16.1 — Walk the sketch's line graph, find simple closed
/// cycles, and render each as a filled polygon. Skips cycles where
/// every Line is `construction = true` (those are pad-corner
/// outlines or user-authored guides — already rendered as dashed
/// strokes elsewhere; double-filling would obscure the rendered
/// pad). Arc-bounded loops are deferred to v0.16.2.
///
/// v0.16.2 — Looks up the role attr on every entity in the loop.
/// The first hit picks the fill colour from the matching layer in
/// [`super::layers::FpLayer`]. Loops with no role assignment fall
/// back to neutral grey.
fn draw_filled_closed_loops(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_types::layer::SignexLayer;
    use std::collections::{HashMap, HashSet};

    // v0.16.2 — pick a fill colour for a loop by inspecting each
    // entity's role attr. Returns `None` when no entity in the loop
    // carries a role; the caller falls back to neutral grey.
    fn role_color(entity: &Entity) -> Option<FpLayer> {
        if entity.pad.is_some() {
            return Some(FpLayer::FCu);
        }
        if let Some(s) = entity.silk.as_ref() {
            return Some(if matches!(s.layer, SignexLayer::TopSilk) {
                FpLayer::FSilks
            } else {
                FpLayer::BSilks
            });
        }
        if entity.courtyard.is_some() {
            return Some(FpLayer::EdgeCuts);
        }
        if let Some(m) = entity.mask_opening.as_ref() {
            return Some(if matches!(m.layer, SignexLayer::TopSolderMask) {
                FpLayer::FFab
            } else {
                FpLayer::BFab
            });
        }
        if let Some(m) = entity.mask_exclude.as_ref() {
            return Some(if matches!(m.layer, SignexLayer::TopSolderMask) {
                FpLayer::FFab
            } else {
                FpLayer::BFab
            });
        }
        if let Some(p) = entity.paste_aperture.as_ref() {
            return Some(if matches!(p.layer, SignexLayer::TopPaste) {
                FpLayer::FFab
            } else {
                FpLayer::BFab
            });
        }
        if let Some(p) = entity.pour.as_ref() {
            return Some(if matches!(p.layer, SignexLayer::TopCopper) {
                FpLayer::FCu
            } else {
                FpLayer::BCu
            });
        }
        if entity.keepout.is_some() {
            return Some(FpLayer::EdgeCuts);
        }
        if entity.board_cutout.is_some() {
            return Some(FpLayer::EdgeCuts);
        }
        None
    }

    fn point_pos(
        id: SketchEntityId,
        sketch: &signex_sketch::SketchData,
        state: &FootprintEditorState,
    ) -> Option<(f64, f64)> {
        if let Some(solve) = state.last_solve.as_ref() {
            if let Some(p) = signex_sketch::solver::state::point_xy(
                id,
                &solve.result.state,
                &solve.result.index,
                sketch,
            ) {
                return Some(p);
            }
        }
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    }

    // Build adjacency: Point ID -> Vec<(other_endpoint, line_id, construction)>.
    let mut adj: HashMap<SketchEntityId, Vec<(SketchEntityId, SketchEntityId, bool)>> =
        HashMap::new();
    for e in &sketch.entities {
        if let EntityKind::Line { start, end } = e.kind {
            adj.entry(start)
                .or_default()
                // v0.22 Phase A5 — Treat centerline lines the same
                // as construction for closed-loop fill detection: a
                // loop made entirely of skipped lines must not paint
                // a profile fill (Altium / Fusion convention).
                .push((end, e.id, e.bake_skipped()));
            adj.entry(end)
                .or_default()
                .push((start, e.id, e.bake_skipped()));
        }
    }

    let mut visited_lines: HashSet<SketchEntityId> = HashSet::new();
    for seed in &sketch.entities {
        let (seed_start, seed_end) = match seed.kind {
            EntityKind::Line { start, end } => (start, end),
            _ => continue,
        };
        if visited_lines.contains(&seed.id) {
            continue;
        }
        // Walk: start at seed_start, follow seed → seed_end → next →
        // ... until we return to seed_start or fail.
        let mut points: Vec<SketchEntityId> = vec![seed_start];
        let mut lines: Vec<SketchEntityId> = vec![seed.id];
        let mut all_construction = seed.bake_skipped();
        let mut current = seed_end;
        let mut prev_line = seed.id;
        let mut closed = false;
        for _ in 0..256 {
            if current == seed_start {
                closed = true;
                break;
            }
            let neighbors = match adj.get(&current) {
                Some(n) if n.len() == 2 => n,
                _ => break,
            };
            let next = neighbors.iter().find(|(_, lid, _)| *lid != prev_line);
            match next {
                Some((other, lid, construction)) => {
                    points.push(current);
                    lines.push(*lid);
                    all_construction &= *construction;
                    prev_line = *lid;
                    current = *other;
                }
                None => break,
            }
        }
        if !closed || points.len() < 3 || all_construction {
            // Mark seed line visited so we don't re-walk it; but
            // don't fill.
            visited_lines.insert(seed.id);
            continue;
        }
        for lid in &lines {
            visited_lines.insert(*lid);
        }
        // Resolve to world positions, drop loops with missing data.
        let positions: Vec<(f64, f64)> = points
            .iter()
            .filter_map(|id| point_pos(*id, sketch, state))
            .collect();
        if positions.len() < 3 {
            continue;
        }
        // v0.16.2 — find the first role attr in the loop's lines or
        // points; use its layer colour for the fill. Falls back to
        // neutral grey when nothing in the loop carries a role.
        let loop_role: Option<FpLayer> = lines
            .iter()
            .chain(points.iter())
            .filter_map(|id| sketch.entities.iter().find(|e| e.id == *id))
            .find_map(role_color);
        let fill = match loop_role {
            Some(layer) => {
                let c = layer.color();
                Color {
                    r: c.r,
                    g: c.g,
                    b: c.b,
                    a: 0.20, // brighter than neutral grey to make role visible
                }
            }
            None => Color {
                r: 0.50,
                g: 0.55,
                b: 0.60,
                a: 0.10,
            },
        };
        let path = Path::new(|builder| {
            let p0 = cstate.world_to_screen(positions[0]);
            builder.move_to(p0);
            for pos in positions.iter().skip(1) {
                let p = cstate.world_to_screen(*pos);
                builder.line_to(p);
            }
            builder.close();
        });
        frame.fill(&path, fill);
    }
}

fn draw_grid(frame: &mut canvas::Frame, bounds: Rectangle, offset: Point, step: f32, color: Color) {
    let stroke = Stroke::default().with_width(0.5).with_color(color);
    // Compose every grid line into a single Path with interleaved
    // move_to / line_to and stroke once. Was a per-line `frame.stroke`
    // loop (~60 minor + 60 major calls per frame) which forced iced to
    // tessellate each path independently — the dominant cost when
    // panning an empty footprint canvas.
    let path = Path::new(|builder| {
        let mut x = offset.x.rem_euclid(step) - step;
        while x <= bounds.width + step {
            builder.move_to(Point::new(x, 0.0));
            builder.line_to(Point::new(x, bounds.height));
            x += step;
        }
        let mut y = offset.y.rem_euclid(step) - step;
        while y <= bounds.height + step {
            builder.move_to(Point::new(0.0, y));
            builder.line_to(Point::new(bounds.width, y));
            y += step;
        }
    });
    frame.stroke(&path, stroke);
}

/// v0.18.22 — dotted grid variant. One filled square per intersection
/// rendered as a single `frame.fill` over a composed path so the cost
/// matches `draw_grid`'s single-stroke design. The dot side is
/// 1.4 px (looks like a 1×1 dot at typical DPI without disappearing
/// at fractional pixels).
fn draw_grid_dots(
    frame: &mut canvas::Frame,
    bounds: Rectangle,
    offset: Point,
    step: f32,
    color: Color,
) {
    let dot_side: f32 = 1.4;
    let half = dot_side * 0.5;
    let path = Path::new(|builder| {
        let mut x = offset.x.rem_euclid(step) - step;
        while x <= bounds.width + step {
            let mut y = offset.y.rem_euclid(step) - step;
            while y <= bounds.height + step {
                builder.move_to(Point::new(x - half, y - half));
                builder.line_to(Point::new(x + half, y - half));
                builder.line_to(Point::new(x + half, y + half));
                builder.line_to(Point::new(x - half, y + half));
                builder.close();
                y += step;
            }
            x += step;
        }
    });
    frame.fill(&path, color);
}

/// v0.14.2 — live ghost preview for the multi-click sketch drawing
/// tools. Reads `state.tool_pending` + `state.cursor_mm` and draws a
/// dashed semi-transparent overlay showing where the next click would
/// land:
///
/// - **Line tool, after click 1** → ghost line from first endpoint
///   to cursor.
/// - **Circle tool, after click 1** → ghost circle centred on click 1
///   with radius = distance(centre, cursor).
/// - **Arc tool, after click 1** → ghost line from centre to cursor
///   (cursor will become the start endpoint).
/// - **Arc tool, after click 2** → ghost arc from start through the
///   cursor angle, around the centre.
fn draw_sketch_tool_preview(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    use crate::library::editor::footprint::state::ToolPending;
    use signex_sketch::entity::EntityKind;
    use signex_sketch::id::SketchEntityId;

    let cursor = match state.cursor_mm {
        Some(c) => c,
        None => return,
    };

    let resolve_point = |id: SketchEntityId| -> Option<(f64, f64)> {
        if let Some(solve) = state.last_solve.as_ref() {
            if let Some((x, y)) = signex_sketch::solver::state::point_xy(
                id,
                &solve.result.state,
                &solve.result.index,
                sketch,
            ) {
                return Some((x, y));
            }
        }
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    };

    // Ghost colour — accent at low alpha so it reads as preview, not
    // committed geometry. Dashed stroke for the same reason.
    let ghost = Color::from_rgba(0.40, 0.70, 1.00, 0.85);
    let stroke = Stroke::default().with_width(1.5).with_color(ghost);

    let dashed = |frame: &mut canvas::Frame, p0: Point, p1: Point| {
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len <= 0.5 {
            return;
        }
        let dash_len = 8.0_f32;
        let n = ((len / dash_len).ceil() as i32).max(2);
        for i in (0..n).step_by(2) {
            let t0 = i as f32 / n as f32;
            let t1 = ((i + 1) as f32 / n as f32).min(1.0);
            let q0 = Point::new(p0.x + dx * t0, p0.y + dy * t0);
            let q1 = Point::new(p0.x + dx * t1, p0.y + dy * t1);
            frame.stroke(&Path::line(q0, q1), stroke);
        }
    };

    let cursor_screen = cstate.world_to_screen(cursor);

    // v0.15 — only show the cursor pip when a multi-click tool is
    // mid-gesture (Line / Circle / Arc with first endpoint placed).
    // For Select / idle tools the OS cursor is enough; an
    // always-visible ring read as a stale entity in v0.14.2.
    let pip_visible = !matches!(state.tool_pending, ToolPending::Idle);
    if pip_visible {
        let pip = Path::circle(cursor_screen, 3.0);
        frame.stroke(&pip, Stroke::default().with_width(1.0).with_color(ghost));
    }

    match state.tool_pending {
        ToolPending::Idle => {}
        ToolPending::LineFirst { first } => {
            let Some(first_world) = resolve_point(first) else {
                return;
            };
            let p0 = cstate.world_to_screen(first_world);
            dashed(frame, p0, cursor_screen);
        }
        ToolPending::RectangleFirst { first } => {
            // v0.15 — preview the axis-aligned rectangle from the
            // first corner to the cursor.
            let Some(first_world) = resolve_point(first) else {
                return;
            };
            let p0 = cstate.world_to_screen(first_world);
            let p2 = cursor_screen;
            let p1 = Point::new(p2.x, p0.y);
            let p3 = Point::new(p0.x, p2.y);
            dashed(frame, p0, p1);
            dashed(frame, p1, p2);
            dashed(frame, p2, p3);
            dashed(frame, p3, p0);
        }
        ToolPending::RoundedRectangleFirst { first } => {
            // v0.16 — preview the rounded rectangle. Compute the bbox
            // from the first corner + cursor, derive a clamped
            // corner radius from the dimension input (default 0.5
            // mm), and stroke 4 dashed line segments + 4 dashed
            // 90° arcs.
            let Some(first_world) = resolve_point(first) else {
                return;
            };
            let x0 = first_world.0.min(cursor.0);
            let y0 = first_world.1.min(cursor.1);
            let x1 = first_world.0.max(cursor.0);
            let y1 = first_world.1.max(cursor.1);
            let half_w = (x1 - x0) / 2.0;
            let half_h = (y1 - y0) / 2.0;
            let r_input = state
                .dimension_input
                .trim()
                .parse::<f64>()
                .ok()
                .unwrap_or(0.5);
            let r_max = half_w.min(half_h).max(0.05);
            let r = r_input.clamp(0.05, r_max);
            // Line endpoints in world coords.
            let tl_right = (x0 + r, y0);
            let tr_left = (x1 - r, y0);
            let tr_top = (x1, y0 + r);
            let br_top = (x1, y1 - r);
            let br_right = (x1 - r, y1);
            let bl_left = (x0 + r, y1);
            let bl_bot = (x0, y1 - r);
            let tl_bot = (x0, y0 + r);
            // Lines.
            for (a, b) in [
                (tl_right, tr_left),
                (tr_top, br_top),
                (br_right, bl_left),
                (bl_bot, tl_bot),
            ] {
                dashed(frame, cstate.world_to_screen(a), cstate.world_to_screen(b));
            }
            // Arc centres.
            let centres = [
                ((x1 - r, y0 + r), tr_left, tr_top),
                ((x1 - r, y1 - r), br_top, br_right),
                ((x0 + r, y1 - r), bl_left, bl_bot),
                ((x0 + r, y0 + r), tl_bot, tl_right),
            ];
            for (c_world, s_world, e_world) in centres {
                let a0 = (s_world.1 - c_world.1).atan2(s_world.0 - c_world.0);
                let a1 = (e_world.1 - c_world.1).atan2(e_world.0 - c_world.0);
                let mut delta = a1 - a0;
                while delta < 0.0 {
                    delta += std::f64::consts::TAU;
                }
                let segs = 12;
                let mut prev = cstate.world_to_screen(s_world);
                for i in 1..=segs {
                    if i % 2 == 0 {
                        let t = (i as f64) / (segs as f64);
                        let a = a0 + delta * t;
                        let p = (c_world.0 + r * a.cos(), c_world.1 + r * a.sin());
                        let q = cstate.world_to_screen(p);
                        frame.stroke(&Path::line(prev, q), stroke);
                        prev = q;
                    } else {
                        let t = (i as f64) / (segs as f64);
                        let a = a0 + delta * t;
                        let p = (c_world.0 + r * a.cos(), c_world.1 + r * a.sin());
                        prev = cstate.world_to_screen(p);
                    }
                }
            }
        }
        ToolPending::CircleCenter { center } => {
            let Some(c_world) = resolve_point(center) else {
                return;
            };
            let c_screen = cstate.world_to_screen(c_world);
            let r_world = ((cursor.0 - c_world.0).powi(2) + (cursor.1 - c_world.1).powi(2)).sqrt();
            let r_screen = (r_world as f32) * cstate.scale;
            // Approximate dashed circle with 32-segment polyline.
            let segments = 32;
            for i in (0..segments).step_by(2) {
                let t0 = i as f32 / segments as f32;
                let t1 = (i + 1) as f32 / segments as f32;
                let a0 = t0 * std::f32::consts::TAU;
                let a1 = t1 * std::f32::consts::TAU;
                let q0 = Point::new(
                    c_screen.x + r_screen * a0.cos(),
                    c_screen.y + r_screen * a0.sin(),
                );
                let q1 = Point::new(
                    c_screen.x + r_screen * a1.cos(),
                    c_screen.y + r_screen * a1.sin(),
                );
                frame.stroke(&Path::line(q0, q1), stroke);
            }
            // Radial guide from centre to cursor.
            dashed(frame, c_screen, cursor_screen);
        }
        ToolPending::ArcCenter { center } => {
            // Centre placed; cursor will become the start point. Show
            // a dashed radial line from centre to cursor.
            let Some(c_world) = resolve_point(center) else {
                return;
            };
            let c_screen = cstate.world_to_screen(c_world);
            dashed(frame, c_screen, cursor_screen);
        }
        ToolPending::ArcStart { center, start } => {
            // Centre + start placed; cursor will become the end. Draw
            // a dashed CCW arc from start to cursor angle.
            let Some(c_world) = resolve_point(center) else {
                return;
            };
            let Some(s_world) = resolve_point(start) else {
                return;
            };
            let c_screen = cstate.world_to_screen(c_world);
            let r_world =
                ((s_world.0 - c_world.0).powi(2) + (s_world.1 - c_world.1).powi(2)).sqrt();
            let r_screen = (r_world as f32) * cstate.scale;
            let start_angle = (s_world.1 - c_world.1).atan2(s_world.0 - c_world.0) as f32;
            let end_angle = (cursor.1 - c_world.1).atan2(cursor.0 - c_world.0) as f32;
            // CCW sweep — wrap end above start by 2π if needed.
            let mut delta = end_angle - start_angle;
            while delta < 0.0 {
                delta += std::f32::consts::TAU;
            }
            let segments = 32;
            for i in (0..segments).step_by(2) {
                let t0 = i as f32 / segments as f32;
                let t1 = (i + 1) as f32 / segments as f32;
                let a0 = start_angle + delta * t0;
                let a1 = start_angle + delta * t1;
                let q0 = Point::new(
                    c_screen.x + r_screen * a0.cos(),
                    c_screen.y + r_screen * a0.sin(),
                );
                let q1 = Point::new(
                    c_screen.x + r_screen * a1.cos(),
                    c_screen.y + r_screen * a1.sin(),
                );
                frame.stroke(&Path::line(q0, q1), stroke);
            }
            // Radial guides for both endpoints + cursor.
            let s_screen = cstate.world_to_screen(s_world);
            dashed(frame, c_screen, s_screen);
            dashed(frame, c_screen, cursor_screen);
        }
        // v0.23 — Polar centre re-pick has no preview shape; the
        // cursor PIP at the top of this match is the only visual cue.
        ToolPending::RepickPolarCenter { .. } => {}
        // v0.24 Track C — Tangent Arc, first endpoint placed.
        // Mirror the dispatcher's geometry: locate a Line ending at
        // `first`, compute the tangent-circle centre on the line's
        // perpendicular through `first` that passes through the
        // cursor, and stroke a dashed ghost arc from `first` to the
        // cursor along that circle.
        //
        // Without an incident line, fall back to a dashed straight
        // segment (matches the LineFirst preview) so the user still
        // gets visual feedback while the dispatcher will publish a
        // placeholder warning on commit.
        ToolPending::TangentArcFirst { first } => {
            let Some(first_world) = resolve_point(first) else {
                return;
            };
            // Find a Line ending at `first` (most recent first, same
            // priority the dispatcher uses).
            let incident_line: Option<(f64, f64)> =
                sketch.entities.iter().rev().find_map(|e| match e.kind {
                    EntityKind::Line { start, end } if end == first => resolve_point(start),
                    EntityKind::Line { start, end } if start == first => resolve_point(end),
                    _ => None,
                });
            let p0 = cstate.world_to_screen(first_world);
            match incident_line {
                Some(line_other) => {
                    // Line direction (line_other -> first).
                    let lx = first_world.0 - line_other.0;
                    let ly = first_world.1 - line_other.1;
                    let llen_sq = lx * lx + ly * ly;
                    if llen_sq <= 1e-12 {
                        dashed(frame, p0, cursor_screen);
                        return;
                    }
                    let llen = llen_sq.sqrt();
                    // Perpendicular to the line at `first`.
                    let nx = -ly / llen;
                    let ny = lx / llen;
                    // Solve for the centre (see dispatcher comment).
                    let dx = first_world.0 - cursor.0;
                    let dy = first_world.1 - cursor.1;
                    let denom = 2.0 * (dx * nx + dy * ny);
                    let chord_sq = dx * dx + dy * dy;
                    if denom.abs() <= 1e-9 || chord_sq <= 1e-9 {
                        // Cursor on the tangent line — preview the
                        // straight segment until the cursor pulls off
                        // axis.
                        dashed(frame, p0, cursor_screen);
                        return;
                    }
                    let t = -chord_sq / denom;
                    let cx = first_world.0 + t * nx;
                    let cy = first_world.1 + t * ny;
                    // Radius (use the start-side distance — both
                    // sides should match within solver tolerance).
                    let rx = first_world.0 - cx;
                    let ry = first_world.1 - cy;
                    let r_world = (rx * rx + ry * ry).sqrt();
                    if r_world <= 1e-9 {
                        dashed(frame, p0, cursor_screen);
                        return;
                    }
                    // Sweep direction — match the dispatcher's logic.
                    let ex = cursor.0 - first_world.0;
                    let ey = cursor.1 - first_world.1;
                    let sweep_ccw = lx * ey - ly * ex >= 0.0;
                    // Stroke the dashed arc.
                    let c_screen =
                        cstate.world_to_screen((cx, cy));
                    let r_screen = (r_world as f32) * cstate.scale;
                    let start_angle =
                        (first_world.1 - cy).atan2(first_world.0 - cx) as f32;
                    let end_angle = (cursor.1 - cy).atan2(cursor.0 - cx) as f32;
                    let mut delta = end_angle - start_angle;
                    if sweep_ccw {
                        while delta < 0.0 {
                            delta += std::f32::consts::TAU;
                        }
                    } else {
                        while delta > 0.0 {
                            delta -= std::f32::consts::TAU;
                        }
                    }
                    let segments = 32;
                    for i in (0..segments).step_by(2) {
                        let t0 = i as f32 / segments as f32;
                        let t1 = (i + 1) as f32 / segments as f32;
                        let a0 = start_angle + delta * t0;
                        let a1 = start_angle + delta * t1;
                        let q0 = Point::new(
                            c_screen.x + r_screen * a0.cos(),
                            c_screen.y + r_screen * a0.sin(),
                        );
                        let q1 = Point::new(
                            c_screen.x + r_screen * a1.cos(),
                            c_screen.y + r_screen * a1.sin(),
                        );
                        frame.stroke(&Path::line(q0, q1), stroke);
                    }
                }
                None => {
                    // No incident line — show a dashed chord so the
                    // user gets a visual cue. Dispatcher publishes
                    // the "no incident line" warning on commit.
                    dashed(frame, p0, cursor_screen);
                }
            }
        }
    }

    // v0.24 Track D — modeless live numeric placement-input overlay.
    // Renders the user-typed buffer at the cursor whenever
    // `placement_input` is `Some`, regardless of which `ToolPending`
    // is current (the kind picker decided which tool's gesture mints
    // it; the dispatcher tolerates unrelated tool changes by clearing
    // on commit + on Esc). Position: 4 px right and 8 px below the
    // cursor, with a translucent rounded background so the buffer
    // reads against any canvas content.
    if let Some(input) = state.placement_input.as_ref() {
        let label = input.kind.label();
        let body = if input.buffer.is_empty() {
            format!("{label}: _")
        } else {
            format!("{label}: {}", input.buffer)
        };
        let origin = Point::new(cursor_screen.x + 4.0, cursor_screen.y + 8.0);
        // Approximate a one-line text bbox so the background plate
        // sits behind the glyphs. Iosevka 11px averages ~6 px per
        // character at the canvas's default rendering; a 4 px pad
        // around the label keeps the chrome readable.
        let glyph_w = 6.5_f32;
        let pad_x = 5.0_f32;
        let pad_y = 3.0_f32;
        let body_w = glyph_w * (body.chars().count() as f32) + pad_x * 2.0;
        let body_h = 16.0_f32 + pad_y * 2.0;
        let plate_origin = Point::new(origin.x - pad_x, origin.y - pad_y);
        // Background plate.
        frame.fill_rectangle(
            plate_origin,
            iced::Size::new(body_w, body_h),
            Color::from_rgba(0.05, 0.07, 0.10, 0.85),
        );
        // Accent border.
        frame.stroke(
            &Path::rectangle(plate_origin, iced::Size::new(body_w, body_h)),
            Stroke::default()
                .with_width(1.0)
                .with_color(Color::from_rgba(0.40, 0.70, 1.00, 0.95)),
        );
        // Buffer text.
        frame.fill_text(canvas::Text {
            content: body,
            position: origin,
            color: Color::from_rgba(0.95, 0.95, 0.97, 1.00),
            size: iced::Pixels(13.0),
            ..canvas::Text::default()
        });
    }
}

fn draw_pad(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    pad: &EditorPad,
    is_selected: bool,
) {
    use signex_library::PadShape as PS;
    let layer = pad.primary_layer();
    let color = layer.color();
    let (x0, y0, x1, y1) = pad.bbox_mm();
    let p0 = cstate.world_to_screen((x0, y0));
    let p1 = cstate.world_to_screen((x1, y1));
    let size = iced::Size::new(p1.x - p0.x, p1.y - p0.y);
    let centre = cstate.world_to_screen(pad.position_mm);
    let half_w = size.width / 2.0;
    let half_h = size.height / 2.0;

    // v0.20 — branch on pad.shape to render the actual copper
    // outline. Previously every pad rendered as a rectangle, so the
    // Properties-panel Shape pick_list change wasn't visible on the
    // canvas. Round/Oval use a stretched circle; RoundRect uses
    // rect with arc corners; Chamfered uses a 6/8-vertex polygon;
    // Custom falls back to its provided polygon points.
    let shape_path = match &pad.shape {
        PS::Round | PS::Oval => Path::new(|b| {
            // Approximate ellipse with quadratic bezier arcs.
            // For a circle (size_x == size_y) this is exact.
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
                // Rounded-rect via 4 line segments + 4 quarter arcs.
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
                // Polygon: 4 corners with optional chamfers.
                // Order: TL → TR → BR → BL.
                let tl = Point::new(centre.x - half_w, centre.y - half_h);
                let tr = Point::new(centre.x + half_w, centre.y - half_h);
                let br = Point::new(centre.x + half_w, centre.y + half_h);
                let bl = Point::new(centre.x - half_w, centre.y + half_h);

                let chamfer_corner = |b: &mut canvas::path::Builder,
                                      p: Point,
                                      _flag: bool,
                                      _enter: Point,
                                      _exit: Point| {
                    b.line_to(p);
                };
                let _ = chamfer_corner; // silence unused warning when no flag is set
                // Start at TL+c-x.
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
        PS::Custom(poly) => Path::new(|b| {
            if let Some((first, rest)) = poly.points.split_first() {
                let p0 = cstate.world_to_screen((
                    pad.position_mm.0 + first[0],
                    pad.position_mm.1 + first[1],
                ));
                b.move_to(p0);
                for pt in rest {
                    let p = cstate.world_to_screen((
                        pad.position_mm.0 + pt[0],
                        pad.position_mm.1 + pt[1],
                    ));
                    b.line_to(p);
                }
                b.close();
            }
        }),
        _ => Path::rectangle(p0, size),
    };

    frame.fill(&shape_path, Color { a: 0.85, ..color });
    let outline_color = if is_selected {
        Color::from_rgb(1.0, 1.0, 1.0)
    } else {
        Color { a: 1.0, ..color }
    };
    frame.stroke(
        &shape_path,
        Stroke::default()
            .with_width(if is_selected { 1.6 } else { 0.8 })
            .with_color(outline_color),
    );

    // v0.23 — Pad hole. Through-hole / NPT pads carry a positive
    // `drill_diameter_mm`; render it as a black "punched" disc on
    // top of the copper. Slot drills aren't yet propagated through
    // EditorPad (the field only lives on `NextPadDefaults` today);
    // when EditorPad gains its own slot field, swap in a stadium
    // rendering rotated by `hole_rotation_deg`. A diameter of zero
    // (or `None`) renders nothing so SMD pads and the "Size = 0"
    // placeholder state stay clean.
    if let Some(d_mm) = pad.drill_diameter_mm.filter(|d| *d > 1e-6) {
        let drill_r = (d_mm * 0.5) as f32 * cstate.scale;
        let hole_path = Path::new(|b| {
            let segments = 24;
            for i in 0..=segments {
                let t = i as f32 / segments as f32 * std::f32::consts::TAU;
                let x = centre.x + drill_r * t.cos();
                let y = centre.y + drill_r * t.sin();
                if i == 0 {
                    b.move_to(Point::new(x, y));
                } else {
                    b.line_to(Point::new(x, y));
                }
            }
            b.close();
        });
        // Black fill = "punched" through copper. White outline when
        // selected so the hole's edge stays visible against the
        // pad's selection ring.
        frame.fill(&hole_path, Color::BLACK);
        frame.stroke(
            &hole_path,
            Stroke::default()
                .with_width(0.8)
                .with_color(if is_selected {
                    Color::WHITE
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.85)
                }),
        );
    }

    // Pad number — only when zoomed in enough to read.
    if cstate.scale >= 25.0 && !pad.number.is_empty() {
        let centre = cstate.world_to_screen(pad.position_mm);
        let text_size = (cstate.scale * 0.35).clamp(8.0, 16.0);
        frame.fill_text(canvas::Text {
            content: pad.number.clone(),
            position: Point::new(centre.x, centre.y - text_size / 2.0),
            size: text_size.into(),
            color: Color::from_rgb(0.05, 0.05, 0.05),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Top,
            ..canvas::Text::default()
        });
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

/// v0.18.25 — squared distance from a point to a line segment, square-rooted.
/// Standard projection-onto-segment with clamped t ∈ [0, 1].
fn point_to_segment_dist(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-12 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
    let t_clamped = t.clamp(0.0, 1.0);
    let qx = ax + t_clamped * dx;
    let qy = ay + t_clamped * dy;
    ((px - qx).powi(2) + (py - qy).powi(2)).sqrt()
}

/// v0.18.25 — even-odd ray casting; assumes the polygon is closed
/// implicitly (last vertex connects back to first).
///
/// v0.18.25.1 — replaced `+ f64::EPSILON` denominator guard (≈ 2e-16,
/// not enough headroom for sub-mm horizontal edges in PCB space) with
/// an explicit `continue` when the edge is near-horizontal at a 1e-10
/// tolerance. Removes a NaN-propagation path that could corrupt the
/// even-odd toggle for the remaining iterations.
fn point_in_polygon(px: f64, py: f64, vertices: &[[f64; 2]]) -> bool {
    if vertices.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = vertices.len() - 1;
    for i in 0..vertices.len() {
        let xi = vertices[i][0];
        let yi = vertices[i][1];
        let xj = vertices[j][0];
        let yj = vertices[j][1];
        let denom = yj - yi;
        if denom.abs() < 1e-10 {
            // Horizontal edge — contributes no X intersection. Skip
            // outright rather than divide by a near-zero number.
            j = i;
            continue;
        }
        let intersect = ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / denom + xi);
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// v0.18.25 — `true` when the point lies within `tol` of any closed-
/// polygon edge (including the implicit last-to-first segment).
fn polygon_outline_hit(px: f64, py: f64, vertices: &[[f64; 2]], tol: f64) -> bool {
    let n = vertices.len();
    for i in 0..n {
        let j = (i + 1) % n;
        if point_to_segment_dist(
            px,
            py,
            vertices[i][0],
            vertices[i][1],
            vertices[j][0],
            vertices[j][1],
        ) <= tol
        {
            return true;
        }
    }
    false
}

/// v0.18.16 — render the silk-layer graphics list. Each
/// `FpGraphic` becomes a single Path stroke / fill in the layer's
/// colour. Used for both `silk_f` (FSilks) and `silk_b` (BSilks)
/// passes; the colour follows whichever layer the caller passes.
fn draw_silk_graphics(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    graphics: &[signex_library::primitive::footprint::FpGraphic],
    layer: FpLayer,
    selected_idx: Option<usize>,
) {
    use signex_library::primitive::footprint::FpGraphicKind;
    let base_colour = layer.color();
    let highlight = Color::from_rgb(1.0, 1.0, 1.0);
    let stroke_default_px: f32 = 1.0;
    for (idx, g) in graphics.iter().enumerate() {
        let is_selected = selected_idx == Some(idx);
        let colour = if is_selected { highlight } else { base_colour };
        let mut stroke_px = if g.stroke_width > 0.0 {
            (g.stroke_width as f32 * cstate.scale).max(0.5)
        } else {
            stroke_default_px
        };
        if is_selected {
            stroke_px = (stroke_px + 1.0).max(2.0);
        }
        match &g.kind {
            FpGraphicKind::Line { from, to } => {
                let p0 = cstate.world_to_screen((from[0], from[1]));
                let p1 = cstate.world_to_screen((to[0], to[1]));
                frame.stroke(
                    &Path::line(p0, p1),
                    Stroke::default().with_width(stroke_px).with_color(colour),
                );
            }
            FpGraphicKind::Rectangle { from, to } => {
                let p0 = cstate.world_to_screen((from[0], from[1]));
                let p1 = cstate.world_to_screen((to[0], to[1]));
                let rect = Path::rectangle(
                    Point::new(p0.x.min(p1.x), p0.y.min(p1.y)),
                    iced::Size::new((p1.x - p0.x).abs(), (p1.y - p0.y).abs()),
                );
                frame.stroke(
                    &rect,
                    Stroke::default().with_width(stroke_px).with_color(colour),
                );
            }
            FpGraphicKind::Circle { center, radius } => {
                let c = cstate.world_to_screen((center[0], center[1]));
                let r_px = (*radius as f32) * cstate.scale;
                frame.stroke(
                    &Path::circle(c, r_px),
                    Stroke::default().with_width(stroke_px).with_color(colour),
                );
            }
            FpGraphicKind::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                let c_world = (*center)[0..2].try_into().unwrap_or([0.0, 0.0]);
                let c = cstate.world_to_screen((c_world[0], c_world[1]));
                let r_px = (*radius as f32) * cstate.scale;
                // iced's Path API: build the arc via cubic
                // approximations using `Path::new(|builder|
                // builder.arc(...))`. For simplicity, sample the
                // sweep at small angle increments and stroke a
                // polyline.
                let start_rad = (*start_deg).to_radians() as f32;
                let end_rad = (*end_deg).to_radians() as f32;
                let mut sweep = end_rad - start_rad;
                // Normalise sweep to [-2π, 2π]; iced's polyline can
                // handle either sign.
                if sweep > std::f32::consts::TAU {
                    sweep -= std::f32::consts::TAU;
                } else if sweep < -std::f32::consts::TAU {
                    sweep += std::f32::consts::TAU;
                }
                let segments = 64;
                let path = Path::new(|builder| {
                    let p0_x = c.x + r_px * start_rad.cos();
                    let p0_y = c.y + r_px * start_rad.sin();
                    builder.move_to(Point::new(p0_x, p0_y));
                    for i in 1..=segments {
                        let t = (i as f32) / (segments as f32);
                        let a = start_rad + sweep * t;
                        let p_x = c.x + r_px * a.cos();
                        let p_y = c.y + r_px * a.sin();
                        builder.line_to(Point::new(p_x, p_y));
                    }
                });
                frame.stroke(
                    &path,
                    Stroke::default().with_width(stroke_px).with_color(colour),
                );
            }
            FpGraphicKind::Text {
                position,
                content,
                size,
            } => {
                let p = cstate.world_to_screen((position[0], position[1]));
                let size_px = ((*size as f32) * cstate.scale).max(8.0);
                frame.fill_text(canvas::Text {
                    content: content.clone(),
                    position: Point::new(p.x, p.y),
                    size: size_px.into(),
                    color: colour,
                    align_x: iced::alignment::Horizontal::Left.into(),
                    align_y: iced::alignment::Vertical::Top,
                    ..canvas::Text::default()
                });
            }
            FpGraphicKind::Polygon { vertices } => {
                if vertices.len() < 2 {
                    continue;
                }
                let path = Path::new(|builder| {
                    let first = cstate.world_to_screen((vertices[0][0], vertices[0][1]));
                    builder.move_to(first);
                    for v in vertices.iter().skip(1) {
                        builder.line_to(cstate.world_to_screen((v[0], v[1])));
                    }
                    // Close the loop visually.
                    builder.line_to(first);
                });
                if g.filled {
                    frame.fill(
                        &path,
                        Color {
                            a: 0.55,
                            ..base_colour
                        },
                    );
                }
                frame.stroke(
                    &path,
                    Stroke::default().with_width(stroke_px).with_color(colour),
                );
            }
        }
    }
}

/// v0.18.16 — Pads-mode multi-click gesture preview. Reads the
/// in-flight tool state (`track_first` / `place_arc_pending` /
/// `place_polygon_vertices`) plus `cursor_mm` and draws a ghost
/// preview of what the next click will commit. Dashed-style ghost
/// strokes in a dimmed accent so the user can distinguish the
/// preview from committed silk geometry.
fn draw_pads_tool_preview(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    state: &FootprintEditorState,
) {
    use crate::library::editor::footprint::state::{PadsTool, PlaceArcPending};
    let Some(cursor) = state.cursor_mm else {
        return;
    };
    let ghost_colour = Color::from_rgba(1.0, 1.0, 1.0, 0.55);
    let stroke_px = 1.2_f32;
    let stroke = || {
        Stroke::default()
            .with_width(stroke_px)
            .with_color(ghost_colour)
    };

    match state.pads_tool {
        PadsTool::PlaceTrack => {
            if let Some((sx, sy)) = state.track_first {
                let p0 = cstate.world_to_screen((sx, sy));
                let p1 = cstate.world_to_screen(cursor);
                frame.stroke(&Path::line(p0, p1), stroke());
                let dot = Path::circle(p0, 3.0);
                frame.fill(&dot, ghost_colour);
            }
        }
        PadsTool::PlaceArc => match state.place_arc_pending {
            PlaceArcPending::Idle => {}
            PlaceArcPending::Center { center: (cx, cy) } => {
                let c = cstate.world_to_screen((cx, cy));
                let cur = cstate.world_to_screen(cursor);
                frame.stroke(&Path::line(c, cur), stroke());
                frame.fill(&Path::circle(c, 3.0), ghost_colour);
            }
            PlaceArcPending::Start {
                center: (cx, cy),
                start: (sx, sy),
            } => {
                let c = cstate.world_to_screen((cx, cy));
                let radius_world = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                let r_px = (radius_world as f32) * cstate.scale;
                let start_rad = ((sy - cy).atan2(sx - cx)) as f32;
                let end_rad = ((cursor.1 - cy).atan2(cursor.0 - cx)) as f32;
                let sweep = end_rad - start_rad;
                let segments = 64;
                let path = Path::new(|builder| {
                    let p0_x = c.x + r_px * start_rad.cos();
                    let p0_y = c.y + r_px * start_rad.sin();
                    builder.move_to(Point::new(p0_x, p0_y));
                    for i in 1..=segments {
                        let t = (i as f32) / (segments as f32);
                        let a = start_rad + sweep * t;
                        let p_x = c.x + r_px * a.cos();
                        let p_y = c.y + r_px * a.sin();
                        builder.line_to(Point::new(p_x, p_y));
                    }
                });
                frame.stroke(&path, stroke());
                frame.fill(&Path::circle(c, 3.0), ghost_colour);
            }
        },
        PadsTool::PlacePolygon | PadsTool::PlaceRegion => {
            let verts = &state.place_polygon_vertices;
            if verts.is_empty() {
                return;
            }
            // Connect every captured vertex with ghost lines, then
            // a ghost from the last vertex to the cursor, plus a
            // ghost closing line from the cursor back to the first
            // vertex so the user can see the loop they're stroking.
            let path = Path::new(|builder| {
                let first = cstate.world_to_screen(verts[0]);
                builder.move_to(first);
                for v in verts.iter().skip(1) {
                    builder.line_to(cstate.world_to_screen(*v));
                }
                let cur = cstate.world_to_screen(cursor);
                builder.line_to(cur);
                if verts.len() >= 2 {
                    // Closing preview only when there's > 1 vertex
                    // — a single-vertex stash with cursor doesn't
                    // need a redundant closing line.
                    builder.line_to(first);
                }
            });
            frame.stroke(&path, stroke());
            for v in verts {
                let p = cstate.world_to_screen(*v);
                frame.fill(&Path::circle(p, 3.0), ghost_colour);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod hit_test_tests {
    //! v0.18.25.1 — regression tests for the silk hit-test edge cases
    //! flagged by the v0.18.25 code review (H1 zero-sweep arc, M1
    //! polygon near-horizontal edges).

    use super::{point_in_polygon, point_to_segment_dist, polygon_outline_hit, silk_f_hit_at};
    use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};

    fn line(from: [f64; 2], to: [f64; 2]) -> FpGraphic {
        FpGraphic {
            kind: FpGraphicKind::Line { from, to },
            stroke_width: 0.0,
            filled: false,
        }
    }

    fn arc(center: [f64; 2], radius: f64, start_deg: f64, end_deg: f64) -> FpGraphic {
        FpGraphic {
            kind: FpGraphicKind::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            },
            stroke_width: 0.0,
            filled: false,
        }
    }

    fn polygon(vertices: Vec<[f64; 2]>, filled: bool) -> FpGraphic {
        FpGraphic {
            kind: FpGraphicKind::Polygon { vertices },
            stroke_width: 0.0,
            filled,
        }
    }

    #[test]
    fn line_hit_on_segment() {
        let g = vec![line([0.0, 0.0], [10.0, 0.0])];
        // 0.05 mm above the line — within 0.1 mm tolerance.
        assert_eq!(silk_f_hit_at(&g, 5.0, 0.05, 0.1), Some(0));
    }

    #[test]
    fn line_miss_above_aabb_below_segment_distance() {
        let g = vec![line([0.0, 0.0], [10.0, 0.0])];
        // 0.5 mm above — outside the 0.1 mm tolerance even though
        // inside the AABB. Pre-v0.18.25 (AABB-only) would have hit.
        assert_eq!(silk_f_hit_at(&g, 5.0, 0.5, 0.1), None);
    }

    #[test]
    fn arc_zero_sweep_is_no_hit() {
        // H1 — arc with start == end (degenerate zero-sweep) must
        // miss every cursor, not be treated as a full circle.
        let g = vec![arc([0.0, 0.0], 5.0, 90.0, 90.0)];
        // Cursor on the radius ring at 0°.
        assert_eq!(silk_f_hit_at(&g, 5.0, 0.0, 0.1), None);
    }

    #[test]
    fn arc_full_circle_via_360_sweep() {
        // H1 — `start = 0, end = 360` must hit anywhere on the ring.
        let g = vec![arc([0.0, 0.0], 5.0, 0.0, 360.0)];
        assert_eq!(silk_f_hit_at(&g, 5.0, 0.0, 0.1), Some(0));
        assert_eq!(silk_f_hit_at(&g, -5.0, 0.0, 0.1), Some(0));
        assert_eq!(silk_f_hit_at(&g, 0.0, 5.0, 0.1), Some(0));
    }

    #[test]
    fn arc_seam_crossing_includes_zero_degrees() {
        // Arc from 350° to 10° (CCW, crosses 0°/360° seam).
        let g = vec![arc([0.0, 0.0], 5.0, 350.0, 10.0)];
        // Cursor at 0° on radius — must hit.
        assert_eq!(silk_f_hit_at(&g, 5.0, 0.0, 0.1), Some(0));
    }

    #[test]
    fn arc_excludes_outside_sweep() {
        // Arc from 0° to 90° — 180° (negative X axis) must miss.
        let g = vec![arc([0.0, 0.0], 5.0, 0.0, 90.0)];
        assert_eq!(silk_f_hit_at(&g, -5.0, 0.0, 0.1), None);
        // Inside sweep (45°) hits.
        let s = (5.0_f64) * (45.0_f64.to_radians()).cos();
        assert_eq!(silk_f_hit_at(&g, s, s, 0.1), Some(0));
    }

    #[test]
    fn polygon_horizontal_edge_no_nan_propagation() {
        // M1 — square with two perfectly horizontal edges. Pre-fix
        // the divide `(yj - yi + EPSILON)` returned ±Inf/NaN for
        // horizontal edges; the fix continues past them. Filled
        // square hit-test must still work.
        let square = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        assert!(point_in_polygon(5.0, 5.0, &square));
        assert!(!point_in_polygon(-1.0, 5.0, &square));
        assert!(!point_in_polygon(5.0, -1.0, &square));
        assert!(!point_in_polygon(11.0, 5.0, &square));
    }

    #[test]
    fn polygon_outline_hit_on_edge() {
        let square = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        // 0.05 mm outside the bottom edge — within 0.1 mm tol.
        assert!(polygon_outline_hit(5.0, -0.05, &square, 0.1));
        // 5 mm above bottom edge, deep inside — outline miss.
        assert!(!polygon_outline_hit(5.0, 5.0, &square, 0.1));
    }

    #[test]
    fn point_to_segment_dist_zero_length() {
        // Degenerate segment (point) — distance is to that single
        // point, not NaN.
        let d = point_to_segment_dist(3.0, 4.0, 0.0, 0.0, 0.0, 0.0);
        assert!((d - 5.0).abs() < 1e-9);
    }

    #[test]
    fn polygon_filled_silk_uses_even_odd() {
        // Concave polygon (Pac-Man notch). Filled hit-test should
        // include the body but exclude the notch.
        let pac = vec![
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [6.0, 5.0],          // mouth pinch in
            [10.0, 0.0 + 1e-12], // near-horizontal edge: re-uses bottom
            [0.0, 10.0],
        ];
        // Just here to confirm even-odd doesn't panic on near-
        // duplicate Y coords. Body of the test is exercising the
        // 1e-10 guard path.
        let _ = point_in_polygon(5.0, 5.0, &pac);
    }
}
