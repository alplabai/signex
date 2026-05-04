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
                    return Some(canvas::Action::capture());
                }
                if *button == mouse::Button::Left
                    && let Some(cursor_pos) = cursor.position_in(bounds)
                {
                    let raw_world = cstate.screen_to_world(cursor_pos);
                    // v0.16.1 — apply snap (Point / H / V / Angle /
                    // Grid) only in Sketch mode. Pads-mode clicks
                    // stay raw (literal pads aren't grid-locked).
                    use crate::library::editor::footprint::state::{
                        EditorMode as _EM, SketchTool as _ST,
                    };
                    let snap_in_effect = matches!(self.state.mode, _EM::Sketch);
                    let world = if snap_in_effect {
                        let point_hit = sketch_snap(self.sketch, cstate, raw_world);
                        let result =
                            snap::snap_cursor(raw_world, self.sketch, self.state, point_hit);
                        cstate.last_snap = Some(result);
                        result.pos
                    } else {
                        cstate.last_snap = None;
                        raw_world
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
                        // Emit a select message so the model
                        // highlights the pad on press.
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
                    return Some(canvas::Action::capture());
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                if matches!(button, mouse::Button::Right | mouse::Button::Middle) {
                    cstate.panning = false;
                    cstate.last_pan_pos = None;
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
                            let snap_id = sketch_snap(
                                self.sketch,
                                cstate,
                                click_world,
                            );
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
                                | SketchTool::Arc => {
                                    EditorMsg::FootprintSketchToolClick {
                                        x_mm: click_world.0,
                                        y_mm: click_world.1,
                                        snap_id,
                                    }
                                }
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
                        // Select tool: empty click does nothing
                        // (selection-clear is handled by the model
                        // via the existing FootprintSelectPad(None)
                        // path on actual canvas-click events that
                        // miss every pad).
                        return None;
                    }
                    if drag.moved {
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
                    cstate.offset.x += cursor_pos.x - last.x;
                    cstate.offset.y += cursor_pos.y - last.y;
                    cstate.last_pan_pos = Some(cursor_pos);
                    self.cache.clear();
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
                // v0.16.1 — Sketch-mode snap (Point / H / V / Angle /
                // Grid). The canvas-local `last_snap` drives the
                // badge in `draw_sketch_tool_preview`. Pads-mode is
                // not snapped.
                use crate::library::editor::footprint::state::EditorMode as _EM;
                let world = if matches!(self.state.mode, _EM::Sketch) {
                    let point_hit = sketch_snap(self.sketch, cstate, raw_world);
                    let result =
                        snap::snap_cursor(raw_world, self.sketch, self.state, point_hit);
                    cstate.last_snap = Some(result);
                    result.pos
                } else {
                    cstate.last_snap = None;
                    raw_world
                };
                if let Some(drag) = cstate.drag.as_mut() {
                    let dx = (cursor_pos.x - drag.press_screen.x).abs();
                    let dy = (cursor_pos.y - drag.press_screen.y).abs();
                    if !drag.moved && dx.max(dy) >= DRAG_THRESHOLD_PX {
                        drag.moved = true;
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
                use crate::library::editor::footprint::state::{
                    EditorMode, PadsTool, ToolPending,
                };
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

            // 1mm minor grid + 5mm major. Scale makes minor grid
            // disappear when zoomed out.
            let minor_step = 1.0_f32 * cstate.scale;
            let major_step = 5.0_f32 * cstate.scale;
            if minor_step >= 6.0 {
                draw_grid(
                    frame,
                    bounds,
                    cstate.offset,
                    minor_step,
                    Color {
                        a: 0.10,
                        ..self.grid_color
                    },
                );
            }
            if major_step >= 8.0 {
                draw_grid(
                    frame,
                    bounds,
                    cstate.offset,
                    major_step,
                    Color {
                        a: 0.30,
                        ..self.grid_color
                    },
                );
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

            // Silk/fab graphics live on the Footprint primitive
            // (`silk_f` / `silk_b` / `fab_f` / `fab_b`). Pads-only
            // rendering for now — the graphics pipeline reads
            // directly from the primitive once the standalone
            // `.snxfpt` editor grows that surface.

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
                let half = 0.5_f32 * cstate.scale; // 1 mm pad
                let centre = cstate.world_to_screen((cx, cy));
                let p0 = Point::new(centre.x - half, centre.y - half);
                let size = iced::Size::new(half * 2.0, half * 2.0);
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
                let rect = Path::rectangle(p0, size);
                frame.fill(&rect, ghost_fill);
                frame.stroke(
                    &rect,
                    Stroke::default().with_width(1.0).with_color(ghost_stroke),
                );
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
                draw_sketch_tool_preview(frame, cstate, sketch, self.state);
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
        sketch.entities.iter().find(|e| e.id == id).and_then(|e| {
            match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            }
        })
    };
    let line_endpoints_local = |id: SketchEntityId| -> Option<(SketchEntityId, SketchEntityId)> {
        sketch.entities.iter().find(|e| e.id == id).and_then(|e| {
            match e.kind {
                EntityKind::Line { start, end } => Some((start, end)),
                _ => None,
            }
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
        let colour = if over_set.contains(&c.id) {
            Color::from_rgba(1.0, 0.20, 0.20, 1.00)
        } else {
            Color::from_rgba(0.85, 0.85, 0.85, 0.85)
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
        sketch.entities.iter().find(|e| e.id == id).and_then(|e| match e.kind {
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
                let r = if entity.construction { 2.5 } else { 4.0 };
                let path = Path::circle(Point::new(p.x, p.y), r);
                let col = dof_colour(entity.id);
                frame.fill(&path, col);
                frame.stroke(
                    &path,
                    Stroke::default()
                        .with_width(1.0)
                        .with_color(Color {
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
                let col = dof_colour(start);
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
        sketch.entities.iter().find(|e| e.id == id).and_then(|e| {
            match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            }
        })
    }

    // Build adjacency: Point ID -> Vec<(other_endpoint, line_id, construction)>.
    let mut adj: HashMap<SketchEntityId, Vec<(SketchEntityId, SketchEntityId, bool)>> =
        HashMap::new();
    for e in &sketch.entities {
        if let EntityKind::Line { start, end } = e.kind {
            adj.entry(start).or_default().push((end, e.id, e.construction));
            adj.entry(end).or_default().push((start, e.id, e.construction));
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
        let mut all_construction = seed.construction;
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
            let r_world =
                ((cursor.0 - c_world.0).powi(2) + (cursor.1 - c_world.1).powi(2)).sqrt();
            let r_screen = (r_world as f32) * cstate.scale;
            // Approximate dashed circle with 32-segment polyline.
            let segments = 32;
            for i in (0..segments).step_by(2) {
                let t0 = i as f32 / segments as f32;
                let t1 = (i + 1) as f32 / segments as f32;
                let a0 = t0 * std::f32::consts::TAU;
                let a1 = t1 * std::f32::consts::TAU;
                let q0 = Point::new(c_screen.x + r_screen * a0.cos(), c_screen.y + r_screen * a0.sin());
                let q1 = Point::new(c_screen.x + r_screen * a1.cos(), c_screen.y + r_screen * a1.sin());
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
                let q0 = Point::new(c_screen.x + r_screen * a0.cos(), c_screen.y + r_screen * a0.sin());
                let q1 = Point::new(c_screen.x + r_screen * a1.cos(), c_screen.y + r_screen * a1.sin());
                frame.stroke(&Path::line(q0, q1), stroke);
            }
            // Radial guides for both endpoints + cursor.
            let s_screen = cstate.world_to_screen(s_world);
            dashed(frame, c_screen, s_screen);
            dashed(frame, c_screen, cursor_screen);
        }
    }
}

fn draw_pad(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    pad: &EditorPad,
    is_selected: bool,
) {
    let layer = pad.primary_layer();
    let color = layer.color();
    let (x0, y0, x1, y1) = pad.bbox_mm();
    let p0 = cstate.world_to_screen((x0, y0));
    let p1 = cstate.world_to_screen((x1, y1));
    let size = iced::Size::new(p1.x - p0.x, p1.y - p0.y);
    let rect = Path::rectangle(p0, size);
    frame.fill(&rect, Color { a: 0.85, ..color });
    let outline_color = if is_selected {
        Color::from_rgb(1.0, 1.0, 1.0)
    } else {
        Color { a: 1.0, ..color }
    };
    frame.stroke(
        &rect,
        Stroke::default()
            .with_width(if is_selected { 1.6 } else { 0.8 })
            .with_color(outline_color),
    );

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
