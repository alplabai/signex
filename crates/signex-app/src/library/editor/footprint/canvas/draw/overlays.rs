//! Interaction overlays — the Sketch reticle, the Select-tool cursor
//! mark (with the line-hover resize arrow), the Touching-Line / Lasso
//! ghosts, the rubber-band rectangle, and the sketch-entity overlay.
//! These sit at the top of the z-stack. Extracted verbatim from
//! `Program::draw`.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Radians, Vector};

use super::sketch::{
    draw_dof_direction_arrows, draw_sketch_overlay, draw_sketch_snap_glyph,
    draw_sketch_tool_preview,
};
use super::super::{FootprintCanvas, FootprintCanvasState, DRAG_THRESHOLD_PX};

impl FootprintCanvas<'_> {
    /// v0.27 — Fusion-style sketch reticle painted at the SNAP target
    /// (state.cursor_mm) while a placement tool is active. Hidden for
    /// the Select tool + while the context menu is open.
    pub(in crate::library::editor::footprint::canvas) fn draw_sketch_reticle(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
        cursor_screen: Option<Point>,
    ) {
        use crate::library::editor::footprint::state::EditorMode;
        let in_sketch = matches!(self.state.mode, EditorMode::Sketch);
        let context_menu_open = self.state.context_menu.is_some();
        let in_select_tool =
            self.state.active_tool == crate::library::editor::footprint::state::SketchTool::Select;
        if in_sketch
            && !in_select_tool
            && !context_menu_open
            && let Some((cx, cy)) = self.state.cursor_mm
            && cursor_screen.is_some()
        {
            let p = cstate.world_to_screen((cx, cy));
            let half = 7.0_f32;
            let arm = 4.5_f32;
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
    }

    /// v0.27 — Select-tool cursor mark: a dark "+" at the raw cursor,
    /// replaced by a rotated double-headed arrow when hovering a
    /// sketch Line (the resize cue at the exact line angle).
    pub(in crate::library::editor::footprint::canvas) fn draw_select_cursor_mark(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
        cursor_screen: Option<Point>,
    ) {
        use crate::library::editor::footprint::state::{EditorMode, SketchTool};
        let in_sketch = matches!(self.state.mode, EditorMode::Sketch);
        let context_menu_open = self.state.context_menu.is_some();
        let in_select_tool = self.state.active_tool == SketchTool::Select;
        if in_sketch
            && in_select_tool
            && !context_menu_open
            && let Some(p) = cursor_screen
        {
            let arm = 5.0_f32;
            let near_black = Color::from_rgba(0.10, 0.10, 0.10, 0.90);
            let stroke = Stroke::default().with_width(1.0).with_color(near_black);

            // Hit-test the cursor against sketch Lines to find the one
            // we'd resize on drag (same tolerance as the hover test).
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
                            // World uses the same Y-down mapping as
                            // screen, so atan2 in world matches the
                            // screen angle the user sees.
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
                // Perpendicular to the line is the drag axis.
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
                    // Halo behind the shaft.
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
                    // White outline pass first so a thin halo wraps
                    // each filled head.
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
    }

    /// v0.27 — Touching Line ghost — second-endpoint preview tracking
    /// the cursor after the first click.
    pub(in crate::library::editor::footprint::canvas) fn draw_touching_line_ghost(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
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
    }

    /// v0.27 — Lasso Select polygon ghost — captured vertices as cyan
    /// dots + a closed-loop outline back to the live cursor.
    pub(in crate::library::editor::footprint::canvas) fn draw_lasso_ghost(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
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
    }

    /// v0.26-I — rubber-band selection rectangle. Drawn only when both
    /// anchor + current are set and at least the drag threshold apart.
    pub(in crate::library::editor::footprint::canvas) fn draw_rubber_band(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
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
                // Altium pen: cyan-ish translucent fill + 1 px outline.
                frame.fill(&rect_path, Color::from_rgba(0.30, 0.55, 0.90, 0.18));
                frame.stroke(
                    &rect_path,
                    Stroke::default()
                        .with_width(1.0)
                        .with_color(Color::from_rgba(0.50, 0.75, 1.00, 0.95)),
                );
            }
        }
    }

    /// v0.13.1 — sketch-entity overlay (entities, DOF arrows, tool
    /// preview, snap glyph). Only in Sketch mode with a sketch present.
    pub(in crate::library::editor::footprint::canvas) fn draw_sketch_overlays(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        if matches!(
            self.state.mode,
            crate::library::editor::footprint::state::EditorMode::Sketch
        ) && let Some(sketch) = self.sketch
        {
            draw_sketch_overlay(frame, cstate, sketch, self.state);
            // v0.22 Phase E2 — DOF direction-arrow overlay.
            draw_dof_direction_arrows(frame, cstate, sketch, self.state);
            draw_sketch_tool_preview(frame, cstate, sketch, self.state);
            // v0.22 Phase A6 — Inferred-constraint snap glyph on top.
            draw_sketch_snap_glyph(frame, cstate, self.state);
        }
    }
}
