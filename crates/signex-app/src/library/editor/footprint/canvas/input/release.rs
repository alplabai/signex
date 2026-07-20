//! Left-release commit arms — box-select rubber-band commit, Place
//! Text Frame commit, the not-moved click-add tool dispatch (sketch
//! click / place pad / via / hole / string / track / arc / polygon)
//! and empty-canvas selection clear.
//!
//! Split out of `tools.rs` to keep each input file under the
//! file-size cap. Behaviour is byte-identical to the original
//! `Program::update` left-release branch — same order, same
//! conditions, same `Action::publish` sites.

use iced::mouse;
use iced::widget::canvas;
use iced::{Point, Rectangle};

use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage};

use super::super::hit_test::{sketch_hit_other, sketch_snap};
use super::super::{DragState, FootprintCanvas, FootprintCanvasState};

impl FootprintCanvas<'_> {
    /// The `pad_idx == usize::MAX` left-release branch: a press that
    /// started on empty canvas. Either it moved (Text Frame commit /
    /// rubber-band select) or it was a click (sketch click-add / place
    /// tool / selection clear).
    pub(in crate::library::editor::footprint::canvas) fn released_empty_or_tool(
        &self,
        cstate: &mut FootprintCanvasState,
        drag: &DragState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<LibraryMessage>> {
        if drag.moved {
            return self.released_moved_empty(cstate, drag, bounds, cursor);
        }
        // v0.13.1 Phase 6.3 — Sketch mode redirects empty-canvas
        // click-add to the Place Point sketch-tool path. Normal mode
        // keeps the existing FootprintAddPad behaviour.
        if matches!(
            self.state.mode,
            crate::library::editor::footprint::state::EditorMode::Sketch
        ) {
            return self.released_sketch_click(cstate, drag);
        }
        self.released_place_tools(cstate, drag)
    }

    /// Moved empty-canvas release — Place Text Frame commit, else the
    /// rubber-band box-select commit.
    fn released_moved_empty(
        &self,
        cstate: &mut FootprintCanvasState,
        drag: &DragState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<LibraryMessage>> {
        // v0.14 — Place Text Frame press-drag-release (item ③).
        // `grab_offset_mm` is the press-time world anchor; resolve the
        // release-time world position from the live cursor and commit
        // the min-corner + abs-delta box as a single message. A drag
        // that collapses to ~0 in either axis is a cancelled gesture,
        // not a degenerate frame.
        if self.state.pads_tool == crate::library::editor::footprint::state::PadsTool::PlaceTextFrame
        {
            let anchor = drag.grab_offset_mm;
            let release_world = cursor
                .position_in(bounds)
                .map(|p| cstate.screen_to_world(p))
                .unwrap_or(anchor);
            let x_mm = anchor.0.min(release_world.0);
            let y_mm = anchor.1.min(release_world.1);
            let w_mm = (release_world.0 - anchor.0).abs();
            let h_mm = (release_world.1 - anchor.1).abs();
            self.cache.clear();
            if w_mm > 1e-3 && h_mm > 1e-3 {
                return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                    library_path: self.address.library_path.clone(),
                    table: self.address.table.clone(),
                    row_id: self.address.row_id,
                    msg: EditorMsg::Footprint(FootprintEditorMsg::AddTextFrame {
                        x_mm,
                        y_mm,
                        w_mm,
                        h_mm,
                    }),
                }));
            }
            return None;
        }
        // v0.26-I / v0.27 — empty-canvas drag with motion armed a
        // rubber-band on press; commit it now.
        if let (Some(a), Some(c)) = (
            cstate.box_select_anchor_screen.take(),
            cstate.box_select_current_screen.take(),
        ) {
            return self.released_box_select(cstate, a, c);
        }
        None
    }

    /// Rubber-band commit — derive the world-space rectangle from the
    /// press/current screen anchors, then pick sketch entities (Sketch
    /// mode) or pads (otherwise).
    fn released_box_select(
        &self,
        cstate: &FootprintCanvasState,
        a: Point,
        c: Point,
    ) -> Option<canvas::Action<LibraryMessage>> {
        use crate::library::editor::footprint::state::EditorMode;
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
        // v0.27 — Sketch-mode rubber-band: pick every entity whose
        // bbox is inside the rect.
        if matches!(self.state.mode, EditorMode::Sketch) {
            if let Some(sketch) = self.sketch {
                return self.box_select_sketch(sketch, x0, y0, x1, y1);
            }
        }
        self.box_select_pads(cstate, x0, y0, x1, y1)
    }

    /// Sketch-mode rubber-band picker — bbox per entity kind, pick
    /// every entity fully inside the rectangle.
    fn box_select_sketch(
        &self,
        sketch: &signex_sketch::SketchData,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    ) -> Option<canvas::Action<LibraryMessage>> {
        use signex_sketch::entity::EntityKind;
        let resolve = |id: signex_sketch::id::SketchEntityId| -> Option<(f64, f64)> {
            sketch
                .entities
                .iter()
                .find(|e| e.id == id)
                .and_then(|e| match e.kind {
                    EntityKind::Point { x, y } => Some((x, y)),
                    _ => None,
                })
        };
        let bbox_of = |e: &signex_sketch::entity::Entity| -> Option<(f64, f64, f64, f64)> {
            match e.kind {
                EntityKind::Point { x, y } => Some((x, y, x, y)),
                EntityKind::Line { start, end } => {
                    let s = resolve(start)?;
                    let f = resolve(end)?;
                    Some((s.0.min(f.0), s.1.min(f.1), s.0.max(f.0), s.1.max(f.1)))
                }
                EntityKind::Circle { center, radius } => {
                    let c = resolve(center)?;
                    Some((c.0 - radius, c.1 - radius, c.0 + radius, c.1 + radius))
                }
                EntityKind::Arc { center, start, .. } => {
                    let c = resolve(center)?;
                    let s = resolve(start)?;
                    let r = ((s.0 - c.0).powi(2) + (s.1 - c.1).powi(2)).sqrt();
                    Some((c.0 - r, c.1 - r, c.0 + r, c.1 + r))
                }
            }
        };
        let mut hits: Vec<signex_sketch::id::SketchEntityId> = Vec::new();
        for e in &sketch.entities {
            let Some((bx0, by0, bx1, by1)) = bbox_of(e) else {
                continue;
            };
            let fully_inside = bx0 >= x0 && bx1 <= x1 && by0 >= y0 && by1 <= y1;
            if fully_inside {
                hits.push(e.id);
            }
        }
        self.cache.clear();
        Some(canvas::Action::publish(LibraryMessage::EditorEvent {
            library_path: self.address.library_path.clone(),
            table: self.address.table.clone(),
            row_id: self.address.row_id,
            msg: EditorMsg::Footprint(FootprintEditorMsg::SketchSelectMany(hits)),
        }))
    }

    /// Pads-mode rubber-band picker — honour the active-bar Selection
    /// mode (Inside / Touching / Outside) and combine with the current
    /// selection under Ctrl/Shift.
    fn box_select_pads(
        &self,
        cstate: &FootprintCanvasState,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    ) -> Option<canvas::Action<LibraryMessage>> {
        use crate::library::editor::footprint::state::FpSelectionMode;
        // v0.27 — honour the active-bar Selection picker. Inside
        // (default): pad bbox fully inside rect. Touching: pad bbox
        // intersects rect. Outside: pad bbox fully outside.
        let mode = self.state.selection_mode_2d;
        // v0.27 — Altium-parity rubber-band multi-select. Collect
        // EVERY pad matching the active mode rather than breaking at
        // the first hit.
        let mut hits: Vec<usize> = Vec::new();
        for (idx, pad) in self.state.pads.iter().enumerate() {
            let (px0, py0, px1, py1) = pad.bbox_mm();
            let fully_inside = px0 >= x0 && px1 <= x1 && py0 >= y0 && py1 <= y1;
            let fully_outside = px1 < x0 || px0 > x1 || py1 < y0 || py0 > y1;
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
        // v0.27 — Ctrl/Shift modifier on release combines the
        // rubber-band hits with the existing selection. Ctrl toggles
        // each hit in/out; Shift unions; no modifier replaces.
        let cmd = cstate.current_modifiers.command();
        let shift = cstate.current_modifiers.shift();
        let combined: Vec<usize> = if cmd || shift {
            let mut acc: Vec<usize> = self.state.selected_pad.into_iter().collect();
            acc.extend(self.state.selected_pads_extra.iter().copied());
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
        Some(canvas::Action::publish(LibraryMessage::EditorEvent {
            library_path: self.address.library_path.clone(),
            table: self.address.table.clone(),
            row_id: self.address.row_id,
            msg: EditorMsg::Footprint(FootprintEditorMsg::SelectPads(combined)),
        }))
    }

    /// Sketch-mode empty-canvas click-add — route to the active sketch
    /// tool (Select / Point / Line / … multi-click gestures), honouring
    /// TAB placement-pause.
    fn released_sketch_click(
        &self,
        cstate: &FootprintCanvasState,
        drag: &DragState,
    ) -> Option<canvas::Action<LibraryMessage>> {
        use crate::library::editor::footprint::state::SketchTool;
        let click_world = drag.grab_offset_mm;
        let snap_id = sketch_snap(self.sketch, cstate, click_world);
        // v0.13.3 — also try to hit-test Lines / Arcs / Circles (not
        // just snap-to-Point) for the Select tool.
        let select_id = snap_id.or_else(|| sketch_hit_other(self.sketch, cstate, click_world));
        // v0.16.1 — TAB pause/resume also applies to sketch placement
        // tools. While paused, suppress click-publish for non-Select
        // tools. Select-tool clicks still resolve.
        if self.state.placement_paused && self.state.active_tool != SketchTool::Select {
            return None;
        }
        let msg = match self.state.active_tool {
            SketchTool::Select => EditorMsg::Footprint(FootprintEditorMsg::SketchSelect {
                id: select_id,
                shift: false,
            }),
            // #361 — Drag Track End arms its endpoint grab on PRESS
            // (`try_drag_track_end_grab`); a release reaching here means
            // the press missed every line, so an un-moved empty click is
            // a no-op — it never places geometry.
            SketchTool::DragTrackEnd => return None,
            SketchTool::Point => EditorMsg::Footprint(FootprintEditorMsg::SketchPlacePoint {
                x_mm: click_world.0,
                y_mm: click_world.1,
            }),
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
            | SketchTool::Trim
            // #372 — Break Track routes its single click through the
            // same SketchToolClick path as Trim; the dispatcher's edit
            // arm hit-tests the Line and hands off to `split_line`.
            | SketchTool::BreakTrack => EditorMsg::Footprint(FootprintEditorMsg::SketchToolClick {
                x_mm: click_world.0,
                y_mm: click_world.1,
                snap_id,
            }),
        };
        Some(canvas::Action::publish(LibraryMessage::EditorEvent {
            library_path: self.address.library_path.clone(),
            table: self.address.table.clone(),
            row_id: self.address.row_id,
            msg,
        }))
    }

    /// Pads-mode empty-canvas click — place pad / via / hole / string /
    /// track / arc / polygon / region (gated on `placement_paused`) or,
    /// for the Select tool, clear the current selection.
    fn released_place_tools(
        &self,
        cstate: &mut FootprintCanvasState,
        drag: &DragState,
    ) -> Option<canvas::Action<LibraryMessage>> {
        use crate::library::editor::footprint::state::PadsTool;
        // v0.15 / v0.16.1 — gate empty-click pad-add on
        // PadsTool::PlacePad and `placement_paused`.
        if self.state.pads_tool == PadsTool::PlacePad && !self.state.placement_paused {
            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                library_path: self.address.library_path.clone(),
                table: self.address.table.clone(),
                row_id: self.address.row_id,
                msg: EditorMsg::Footprint(FootprintEditorMsg::AddPad {
                    x_mm: drag.grab_offset_mm.0,
                    y_mm: drag.grab_offset_mm.1,
                }),
            }));
        }
        // v0.27 — PlaceVia drops a canonical via via a dedicated
        // dispatcher path.
        if self.state.pads_tool == PadsTool::PlaceVia && !self.state.placement_paused {
            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                library_path: self.address.library_path.clone(),
                table: self.address.table.clone(),
                row_id: self.address.row_id,
                msg: EditorMsg::Footprint(FootprintEditorMsg::AddVia {
                    x_mm: drag.grab_offset_mm.0,
                    y_mm: drag.grab_offset_mm.1,
                }),
            }));
        }
        // v0.18.12 — Place Hole drops a non-plated through hole.
        if self.state.pads_tool == PadsTool::PlaceHole && !self.state.placement_paused {
            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                library_path: self.address.library_path.clone(),
                table: self.address.table.clone(),
                row_id: self.address.row_id,
                msg: EditorMsg::Footprint(FootprintEditorMsg::AddHole {
                    x_mm: drag.grab_offset_mm.0,
                    y_mm: drag.grab_offset_mm.1,
                }),
            }));
        }
        // v0.18.15 — Place String drops a silk-layer text label.
        if self.state.pads_tool == PadsTool::PlaceString && !self.state.placement_paused {
            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                library_path: self.address.library_path.clone(),
                table: self.address.table.clone(),
                row_id: self.address.row_id,
                msg: EditorMsg::Footprint(FootprintEditorMsg::AddText {
                    x_mm: drag.grab_offset_mm.0,
                    y_mm: drag.grab_offset_mm.1,
                }),
            }));
        }
        // v0.18.15.1 — Place Track is a 2-click gesture; the
        // dispatcher uses `state.track_first` to decide start vs
        // commit.
        if self.state.pads_tool == PadsTool::PlaceTrack && !self.state.placement_paused {
            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                library_path: self.address.library_path.clone(),
                table: self.address.table.clone(),
                row_id: self.address.row_id,
                msg: EditorMsg::Footprint(FootprintEditorMsg::TrackClick {
                    x_mm: drag.grab_offset_mm.0,
                    y_mm: drag.grab_offset_mm.1,
                }),
            }));
        }
        // v0.18.15.3 — Place Arc is a 3-click gesture (centre / radius
        // / sweep); the dispatcher reads `state.place_arc_pending`.
        if self.state.pads_tool == PadsTool::PlaceArc && !self.state.placement_paused {
            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                library_path: self.address.library_path.clone(),
                table: self.address.table.clone(),
                row_id: self.address.row_id,
                msg: EditorMsg::Footprint(FootprintEditorMsg::ArcClick {
                    x_mm: drag.grab_offset_mm.0,
                    y_mm: drag.grab_offset_mm.1,
                }),
            }));
        }
        // v0.18.15.4 / v0.18.17 — Place Polygon and Place Region share
        // the same gesture; the dispatcher reads `pads_tool` at commit
        // time to decide `filled`.
        if (self.state.pads_tool == PadsTool::PlacePolygon
            || self.state.pads_tool == PadsTool::PlaceRegion)
            && !self.state.placement_paused
        {
            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                library_path: self.address.library_path.clone(),
                table: self.address.table.clone(),
                row_id: self.address.row_id,
                msg: EditorMsg::Footprint(FootprintEditorMsg::PolygonClick {
                    x_mm: drag.grab_offset_mm.0,
                    y_mm: drag.grab_offset_mm.1,
                }),
            }));
        }
        // v0.20 — Select tool: empty-area left-click clears the
        // current selection. Only fires for the Select tool.
        if self.state.pads_tool == PadsTool::Select {
            // Clean the box-select arming we set on press — an
            // un-moved click stays a click, not a rubber band.
            cstate.box_select_anchor_screen = None;
            cstate.box_select_current_screen = None;
            return Some(canvas::Action::publish(LibraryMessage::EditorEvent {
                library_path: self.address.library_path.clone(),
                table: self.address.table.clone(),
                row_id: self.address.row_id,
                msg: EditorMsg::Footprint(FootprintEditorMsg::SelectPad(None)),
            }));
        }
        None
    }
}
