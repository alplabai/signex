//! Per-tool left-press gesture arms — the sequence of "try to handle
//! this click" guards that the primary-press classifier walks in
//! order (lasso, touching-line, round-pad handle, sketch point / line
//! grabs, closed-loop select, pad grab, silk select).
//!
//! Each `try_*` returns `Some(action)` when it claims the click and
//! `None` to fall through to the next handler — reproducing the
//! original top-to-bottom `if … { return … }` order byte-for-byte.

use iced::widget::canvas;
use iced::Point;

use crate::library::editor::footprint::state::{EditorMode, PadsTool, SketchTool};
use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage};

use super::super::draw_sketch::{find_closed_loops, ClosedLoop};
use super::super::geometry::point_in_polygon;
use super::super::hit_test::sketch_snap;
use super::super::silk_f_hit_at;
use super::super::{DragState, FootprintCanvas, FootprintCanvasState};

impl FootprintCanvas<'_> {
    /// v0.27 — Lasso Select intercept. While the lasso tool is armed
    /// (set from the active-bar Selection Mode dropdown), each
    /// left-click adds a vertex to the in-flight polygon. Right-click
    /// commits / Esc cancels are handled in their own arms.
    pub(in crate::library::editor::footprint::canvas) fn try_lasso_add_vertex(
        &self,
        cstate: &FootprintCanvasState,
        cursor_pos: Point,
    ) -> Option<canvas::Action<LibraryMessage>> {
        if self.state.lasso_mode_active {
            let world = cstate.screen_to_world(cursor_pos);
            return Some(
                canvas::Action::publish(LibraryMessage::EditorEvent {
                    library_path: self.address.library_path.clone(),
                    table: self.address.table.clone(),
                    row_id: self.address.row_id,
                    msg: EditorMsg::Footprint(FootprintEditorMsg::LassoAddVertex {
                        x_mm: world.0,
                        y_mm: world.1,
                    }),
                })
                .and_capture(),
            );
        }
        None
    }

    /// v0.27 — Touching Line intercept. First click stashes the start
    /// point; second click commits by publishing
    /// FootprintTouchingLineCommit, the dispatcher walks pads +
    /// selects everything the segment intersects.
    pub(in crate::library::editor::footprint::canvas) fn try_touching_line_click(
        &self,
        cstate: &FootprintCanvasState,
        cursor_pos: Point,
    ) -> Option<canvas::Action<LibraryMessage>> {
        if self.state.touching_line_active {
            let world = cstate.screen_to_world(cursor_pos);
            let msg = if self.state.touching_line_first.is_none() {
                EditorMsg::Footprint(FootprintEditorMsg::TouchingLineFirst {
                    x_mm: world.0,
                    y_mm: world.1,
                })
            } else {
                EditorMsg::Footprint(FootprintEditorMsg::TouchingLineCommit {
                    x_mm: world.0,
                    y_mm: world.1,
                })
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
        None
    }

    /// v0.27 — Sketch mode + Select tool: hit-test the east-edge cyan
    /// diameter handle of any Round pad before the generic Point-snap.
    /// The handle is drawn at (pad.position + (pad.size_x/2, 0)) with
    /// a 4 px radius, so allow a 6 px hit slop.
    pub(in crate::library::editor::footprint::canvas) fn try_round_handle_grab(
        &self,
        cstate: &mut FootprintCanvasState,
        cursor_pos: Point,
        world: (f64, f64),
    ) -> Option<canvas::Action<LibraryMessage>> {
        if matches!(self.state.mode, EditorMode::Sketch) && self.state.active_tool == SketchTool::Select
        {
            const HANDLE_HIT_RADIUS_PX: f32 = 6.0;
            for (idx, pad) in self.state.pads.iter().enumerate() {
                if !matches!(pad.shape, signex_library::PadShape::Round) {
                    continue;
                }
                if pad.sketch_entity_id.is_none() {
                    continue;
                }
                let handle_world = (pad.position_mm.0 + pad.size_mm.0 / 2.0, pad.position_mm.1);
                let handle_screen = cstate.world_to_screen(handle_world);
                let dx = cursor_pos.x - handle_screen.x;
                let dy = cursor_pos.y - handle_screen.y;
                if dx * dx + dy * dy <= HANDLE_HIT_RADIUS_PX * HANDLE_HIT_RADIUS_PX {
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
        None
    }

    /// v0.16 — Sketch mode + Select tool click within snap radius of a
    /// sketch `Point` starts a Point-drag gesture and publishes a
    /// select so the inspector + DOF overlay highlight immediately.
    pub(in crate::library::editor::footprint::canvas) fn try_sketch_point_grab(
        &self,
        cstate: &mut FootprintCanvasState,
        cursor_pos: Point,
        raw_world: (f64, f64),
        world: (f64, f64),
    ) -> Option<canvas::Action<LibraryMessage>> {
        if matches!(self.state.mode, EditorMode::Sketch)
            && self.state.active_tool == SketchTool::Select
            && let Some(point_id) = sketch_snap(self.sketch, cstate, raw_world)
        {
            // Defensive: clear any stale rubber-band anchor so it
            // can't render alongside the Point drag.
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
            return Some(
                canvas::Action::publish(LibraryMessage::EditorEvent {
                    library_path: self.address.library_path.clone(),
                    table: self.address.table.clone(),
                    row_id: self.address.row_id,
                    msg: EditorMsg::Footprint(FootprintEditorMsg::SketchSelect {
                        id: Some(point_id),
                        shift: false,
                    }),
                })
                .and_capture(),
            );
        }
        None
    }

    /// v0.27 — Fusion-style Line drag. In Sketch mode + Select tool, a
    /// click within ~10 px of a Line's stroke (but missing the snap
    /// radius for both endpoints) starts a Line-drag gesture: the
    /// dispatcher translates BOTH endpoints by the per-tick delta in
    /// one solver pass so an edge of a closed shape can be pushed
    /// without having to grab a corner.
    pub(in crate::library::editor::footprint::canvas) fn try_sketch_line_grab(
        &self,
        cstate: &mut FootprintCanvasState,
        cursor_pos: Point,
        world: (f64, f64),
    ) -> Option<canvas::Action<LibraryMessage>> {
        if matches!(self.state.mode, EditorMode::Sketch)
            && self.state.active_tool == SketchTool::Select
            && let Some(sketch_ref) = self.sketch
        {
            const LINE_HIT_TOL_PX: f32 = 10.0;
            let tol_mm = (LINE_HIT_TOL_PX / cstate.scale.max(1.0)) as f64;
            let mut best_line: Option<(f64, signex_sketch::id::SketchEntityId)> = None;
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
                    if d2 <= tol_mm * tol_mm && best_line.as_ref().is_none_or(|(b2, _)| d2 < *b2) {
                        best_line = Some((d2, ent.id));
                    }
                }
            }
            if let Some((_, line_id)) = best_line {
                // Make sure no stale rubber-band anchor is left over
                // from a prior gesture.
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
                        msg: EditorMsg::Footprint(FootprintEditorMsg::SketchSelect {
                            id: Some(line_id),
                            shift: false,
                        }),
                    })
                    .and_capture(),
                );
            }
        }
        None
    }

    /// v0.27 — Fusion-style "click the fill, select the closed shape."
    /// Only in Sketch mode + Select tool, and only when the
    /// Point-snap path missed. Walks the same closed-loop adjacency
    /// the fill renderer uses and dispatches a SelectMany carrying
    /// every Line + Point in the loop.
    pub(in crate::library::editor::footprint::canvas) fn try_closed_loop_select(
        &self,
        world: (f64, f64),
    ) -> Option<canvas::Action<LibraryMessage>> {
        if matches!(self.state.mode, EditorMode::Sketch)
            && self.state.active_tool == SketchTool::Select
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
                let mut ids: Vec<signex_sketch::id::SketchEntityId> = lp.lines.clone();
                ids.extend(lp.points.iter().copied());
                return Some(
                    canvas::Action::publish(LibraryMessage::EditorEvent {
                        library_path: self.address.library_path.clone(),
                        table: self.address.table.clone(),
                        row_id: self.address.row_id,
                        msg: EditorMsg::Footprint(FootprintEditorMsg::SketchSelectMany(ids)),
                    })
                    .and_capture(),
                );
            }
        }
        None
    }

    /// v0.21/v0.23/v0.27 — Pad hit (Pads mode + pads filter on). Starts
    /// a pad drag and publishes the selection, honouring Ctrl/Cmd
    /// toggle + Shift extend modifiers.
    pub(in crate::library::editor::footprint::canvas) fn try_pad_grab(
        &self,
        cstate: &mut FootprintCanvasState,
        cursor_pos: Point,
        world: (f64, f64),
    ) -> Option<canvas::Action<LibraryMessage>> {
        if matches!(self.state.mode, EditorMode::Normal) && self.state.selection_filter.pads {
            if let Some(pad_idx) = self.state.pad_at(world.0, world.1) {
                let pad = &self.state.pads[pad_idx];
                // v0.27 — defensively clear any stale rubber-band
                // anchor from a prior gesture so the pad drag doesn't
                // render alongside a phantom selection box.
                cstate.box_select_anchor_screen = None;
                cstate.box_select_current_screen = None;
                cstate.drag = Some(DragState {
                    pad_idx,
                    sketch_point: None,
                    sketch_line: None,
                    grab_offset_mm: (world.0 - pad.position_mm.0, world.1 - pad.position_mm.1),
                    last_world: world,
                    press_screen: cursor_pos,
                    moved: false,
                });
                // v0.27 — Altium-parity modifier handling on the
                // pad-hit branch. Ctrl/Cmd toggles the pad in/out of
                // the multi-select set; Shift extends without removal;
                // bare click replaces the selection.
                let cmd = cstate.current_modifiers.command();
                let shift = cstate.current_modifiers.shift();
                let select_msg = if cmd || shift {
                    let mut current: Vec<usize> = self.state.selected_pad.into_iter().collect();
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
                    EditorMsg::Footprint(FootprintEditorMsg::SelectPads(current))
                } else {
                    EditorMsg::Footprint(FootprintEditorMsg::SelectPad(Some(pad_idx)))
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
        None
    }

    /// v0.18.18/v0.21 — Silk-front graphic hit, filter-gated per kind.
    /// Maps each FpGraphicKind to its matching `selection_filter.*`
    /// bit so the user can disable Tracks / Arcs / Texts / Regions /
    /// Fills independently.
    pub(in crate::library::editor::footprint::canvas) fn try_silk_select(
        &self,
        cstate: &FootprintCanvasState,
        world: (f64, f64),
    ) -> Option<canvas::Action<LibraryMessage>> {
        if matches!(self.state.mode, EditorMode::Normal) && self.state.pads_tool == PadsTool::Select {
            let tolerance = 4.0_f64 / (cstate.scale.max(1.0) as f64);
            if let Some(silk_idx) = silk_f_hit_at(self.silk_f, world.0, world.1, tolerance) {
                use signex_library::primitive::footprint::FpGraphicKind;
                let g = &self.silk_f[silk_idx];
                let allowed = match &g.kind {
                    FpGraphicKind::Line { .. } => self.state.selection_filter.tracks,
                    FpGraphicKind::Arc { .. } | FpGraphicKind::Circle { .. } => {
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
                    FpGraphicKind::Text { .. } => self.state.selection_filter.texts,
                };
                if allowed {
                    return Some(
                        canvas::Action::publish(LibraryMessage::EditorEvent {
                            library_path: self.address.library_path.clone(),
                            table: self.address.table.clone(),
                            row_id: self.address.row_id,
                            msg: EditorMsg::Footprint(FootprintEditorMsg::SelectSilkF(Some(
                                silk_idx,
                            ))),
                        })
                        .and_capture(),
                    );
                }
            }
        }
        None
    }
}
