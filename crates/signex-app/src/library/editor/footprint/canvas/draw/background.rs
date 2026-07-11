//! Backdrop layers — background fill + fine/coarse grid, Altium-style
//! guide lines, and the origin crosshair. Drawn first (bottom of the
//! z-stack). Extracted verbatim from `Program::draw`.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Rectangle};

use super::super::{FootprintCanvas, FootprintCanvasState};
use super::super::draw_grid::{draw_grid, draw_grid_dots};

impl FootprintCanvas<'_> {
    /// Background fill + fine/coarse grid. Sketch mode flips to a
    /// Fusion-style white canvas + single mid-grey grid; Pads mode
    /// keeps the dark theme + 2-tier grid.
    pub(in crate::library::editor::footprint::canvas) fn draw_background_and_grid(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
        bounds: Rectangle,
    ) {
        use crate::library::editor::footprint::state::EditorMode;
        let in_sketch = matches!(self.state.mode, EditorMode::Sketch);
        let bg = if in_sketch { Color::WHITE } else { self.bg_color };
        // v0.27 — Fusion-style sketch grid: darker base (0.55 grey) so
        // it reads cleanly against the white canvas.
        let grid = if in_sketch {
            Color::from_rgba(0.55, 0.55, 0.55, 1.0)
        } else {
            self.grid_color
        };
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), bg);

        // v0.18.19 — fine + coarse grid display follows the Cartesian
        // Grid Editor's per-style picker.
        use crate::library::editor::footprint::state::GridDisplay as Gd;
        let fine_step = (self.state.snap_options.grid_step_mm as f32) * cstate.scale;
        let multiplier = self.state.snap_options.coarse_multiplier.max(1) as f32;
        let coarse_step = fine_step * multiplier;
        let fine_style = self.state.snap_options.fine_grid_display;
        let coarse_style = self.state.snap_options.coarse_grid_display;
        // Fine pass only when each cell is at least 6 px wide.
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
        // v0.27 — Fusion-style sketch grid uses ONLY a single fine grid
        // (no coarse overlay); Pads mode keeps the 2-tier grid.
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
    }

    /// v0.18.20 — Altium-style guide lines. Each enabled guide is a
    /// full-bleed dashed cyan line at its world coordinate.
    pub(in crate::library::editor::footprint::canvas) fn draw_guides(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
        bounds: Rectangle,
    ) {
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
                            &Path::line(Point::new(p.x, 0.0), Point::new(p.x, bounds.height)),
                            stroke,
                        );
                    }
                }
                GuideAxis::Horizontal => {
                    let p = cstate.world_to_screen((0.0, g.position_mm));
                    if p.y >= 0.0 && p.y <= bounds.height {
                        frame.stroke(
                            &Path::line(Point::new(0.0, p.y), Point::new(bounds.width, p.y)),
                            stroke,
                        );
                    }
                }
            }
        }
    }

    /// Origin crosshair — Altium yellow on the dark Pads canvas, slate
    /// grey on the Fusion-style white sketch canvas.
    pub(in crate::library::editor::footprint::canvas) fn draw_origin_crosshair(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        use crate::library::editor::footprint::state::EditorMode;
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
    }
}
