//! Placement-tool cursor ghosts — the PlacePad pre-placement preview
//! (shape-aware outline + drill) and the PlaceVia disc. Drawn above
//! the footprint content, below the sketch overlays. Extracted
//! verbatim from `Program::draw`.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};

use super::super::{FootprintCanvas, FootprintCanvasState};

impl FootprintCanvas<'_> {
    /// v0.16.1 — Pads-mode placement ghost: a shape-aware outline at
    /// the cursor showing where the next pad will land (hidden while
    /// `placement_paused`). Reflects `next_pad_defaults`.
    pub(in crate::library::editor::footprint::canvas) fn draw_place_pad_ghost(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        use crate::library::editor::footprint::state::{EditorMode, PadsTool};
        if matches!(self.state.mode, EditorMode::Normal)
            && self.state.pads_tool == PadsTool::PlacePad
            && !self.state.placement_paused
            && let Some((cx, cy)) = self.state.cursor_mm
        {
            use signex_library::PadShape as PS;
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

            // v0.26 — drill hole on the ghost so the user sees the THT
            // punch BEFORE clicking.
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
    }

    /// v0.27 — PlaceVia ghost preview: a translucent green disc with a
    /// black drilled hole, off hardcoded via geometry (Round 0.6 mm
    /// copper / 0.3 mm drill).
    pub(in crate::library::editor::footprint::canvas) fn draw_place_via_ghost(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        use crate::library::editor::footprint::state::{EditorMode, PadsTool};
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
    }
}
