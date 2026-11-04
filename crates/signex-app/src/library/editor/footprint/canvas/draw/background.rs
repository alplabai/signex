//! Backdrop layers: background fill + fine/coarse grid, guide lines and
//! the origin crosshair. Drawn first (bottom of the z-stack). The fill,
//! grid and origin are free functions so the Components-panel preview
//! paints the same backdrop as the editor.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Rectangle};

use super::super::{FootprintCanvas, FootprintCanvasState};
use super::grid::{draw_grid, draw_grid_dots};
use crate::library::editor::footprint::state::{
    EditorMode, GridDisplay as Gd, GuideAxis, SnapOptions,
};

/// Footprint canvas background: Altium-PCB black regardless of app theme.
pub(crate) const BACKDROP_BG: Color = Color::BLACK;
/// Footprint canvas grid colour (dark navy).
pub(crate) const BACKDROP_GRID: Color = Color {
    r: 0.32,
    g: 0.36,
    b: 0.55,
    a: 1.0,
};

/// What the backdrop needs: colours, sketch-mode look and grid options.
#[derive(Clone, Copy)]
pub(crate) struct BackdropLook<'a> {
    pub bg: Color,
    pub grid: Color,
    /// Sketch mode flips to a white canvas + single mid-grey grid.
    pub sketch: bool,
    pub snap: &'a SnapOptions,
}

/// Background fill + fine/coarse grid.
pub(crate) fn draw_backdrop(
    frame: &mut canvas::Frame,
    cam: &FootprintCanvasState,
    bounds: Rectangle,
    look: BackdropLook<'_>,
) {
    let bg = if look.sketch { Color::WHITE } else { look.bg };
    let grid = if look.sketch {
        Color::from_rgba(0.55, 0.55, 0.55, 1.0)
    } else {
        look.grid
    };
    frame.fill_rectangle(Point::ORIGIN, bounds.size(), bg);

    let fine_step = (look.snap.grid_step_mm as f32) * cam.scale;
    let coarse_step = fine_step * look.snap.coarse_multiplier.max(1) as f32;
    let (fine_alpha, coarse_alpha) = if look.sketch {
        (0.50, 0.55)
    } else {
        (0.10, 0.30)
    };
    // Fine pass only when each cell is at least 6 px wide.
    if fine_step >= 6.0 {
        let color = Color {
            a: fine_alpha,
            ..grid
        };
        let style = look.snap.fine_grid_display;
        paint_grid(frame, bounds, cam.offset, fine_step, color, style);
    }
    // Sketch mode shows only the fine grid.
    if !look.sketch && coarse_step >= 8.0 {
        let color = Color {
            a: coarse_alpha,
            ..grid
        };
        let style = look.snap.coarse_grid_display;
        paint_grid(frame, bounds, cam.offset, coarse_step, color, style);
    }
}

fn paint_grid(
    frame: &mut canvas::Frame,
    bounds: Rectangle,
    offset: Point,
    step: f32,
    color: Color,
    style: Gd,
) {
    match style {
        Gd::Lines => draw_grid(frame, bounds, offset, step, color),
        Gd::Dots => draw_grid_dots(frame, bounds, offset, step, color),
        Gd::Hidden => {}
    }
}

/// Origin crosshair: yellow on the dark canvas, slate on the sketch canvas.
pub(crate) fn draw_origin(frame: &mut canvas::Frame, cam: &FootprintCanvasState, sketch: bool) {
    let origin = cam.world_to_screen((0.0, 0.0));
    let color = if sketch {
        Color::from_rgba(0.20, 0.25, 0.32, 0.85)
    } else {
        Color::from_rgba(1.0, 0.95, 0.30, 0.85)
    };
    let arms = [
        (
            Point::new(origin.x - 8.0, origin.y),
            Point::new(origin.x + 8.0, origin.y),
        ),
        (
            Point::new(origin.x, origin.y - 8.0),
            Point::new(origin.x, origin.y + 8.0),
        ),
    ];
    for (a, b) in arms {
        frame.stroke(
            &Path::line(a, b),
            Stroke::default().with_width(1.5).with_color(color),
        );
    }
}

impl FootprintCanvas<'_> {
    /// Background fill + grid for the editor's current mode and options.
    pub(in crate::library::editor::footprint::canvas) fn draw_background_and_grid(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
        bounds: Rectangle,
    ) {
        let look = BackdropLook {
            bg: self.bg_color,
            grid: self.grid_color,
            sketch: matches!(self.state.mode, EditorMode::Sketch),
            snap: &self.state.snap_options,
        };
        draw_backdrop(frame, cstate, bounds, look);
    }

    /// Altium-style guide lines: each enabled guide is a full-bleed
    /// dashed cyan line at its world coordinate.
    pub(in crate::library::editor::footprint::canvas) fn draw_guides(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
        bounds: Rectangle,
    ) {
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

    /// Origin crosshair for the editor's current mode.
    pub(in crate::library::editor::footprint::canvas) fn draw_origin_crosshair(
        &self,
        frame: &mut canvas::Frame,
        cstate: &FootprintCanvasState,
    ) {
        draw_origin(frame, cstate, matches!(self.state.mode, EditorMode::Sketch));
    }
}
