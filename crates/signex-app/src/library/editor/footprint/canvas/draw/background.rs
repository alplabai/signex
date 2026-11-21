//! Backdrop layers: background fill + fine/coarse grid, guide lines and
//! the origin crosshair. Drawn first (bottom of the z-stack). The fill,
//! grid and origin are free functions so the Components-panel preview
//! paints the same backdrop as the editor.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Rectangle};

use super::super::{FootprintCanvas, FootprintCanvasState};
use super::graphite::{board_look, dot_step_px, look_from_bg};
use super::grid::{draw_grid, draw_grid_dots};
use crate::library::editor::footprint::state::{
    EditorMode, GridDisplay as Gd, GuideAxis, SnapOptions,
};
use crate::pcb_canvas::{layer_style, stroke_origin_marker};

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
    /// Sketch mode paints the board ground on every theme (decision 7,
    /// Caner 2026-11-20).
    pub sketch: bool,
    pub snap: &'a SnapOptions,
    /// The model point the grid steps from; `None` is (0, 0).
    pub grid_origin: Option<(f64, f64)>,
}

/// The canvas pixel the grid's lattice is laid from.
fn grid_screen_origin(cam: &FootprintCanvasState, grid_origin: Option<(f64, f64)>) -> Point {
    grid_origin.map_or(cam.offset, |at| cam.world_to_screen(at))
}

/// Background fill + fine/coarse grid.
pub(crate) fn draw_backdrop(
    frame: &mut canvas::Frame,
    cam: &FootprintCanvasState,
    bounds: Rectangle,
    look: BackdropLook<'_>,
) {
    let origin = grid_screen_origin(cam, look.grid_origin);
    if let Some(g) = look_from_bg(look.bg).or_else(|| look.sketch.then(board_look)) {
        draw_graphite_backdrop(frame, cam, bounds, origin, g, look.snap);
        return;
    }
    let grid = look.grid;
    frame.fill_rectangle(Point::ORIGIN, bounds.size(), look.bg);

    let fine_step = (look.snap.grid_step_mm as f32) * cam.scale;
    let coarse_step = fine_step * look.snap.coarse_multiplier.max(1) as f32;
    let (fine_alpha, coarse_alpha) = (0.10, 0.30);
    // Fine pass only when each cell is at least 6 px wide.
    if fine_step >= 6.0 {
        let color = Color {
            a: fine_alpha,
            ..grid
        };
        let style = look.snap.fine_grid_display;
        paint_grid(frame, bounds, origin, fine_step, color, style);
    }
    if coarse_step >= 8.0 {
        let color = Color {
            a: coarse_alpha,
            ..grid
        };
        let style = look.snap.coarse_grid_display;
        paint_grid(frame, bounds, origin, coarse_step, color, style);
    }
}

/// Graphite ground: one dot grid on the paper / charcoal fill, hidden when
/// the user hides both grids.
fn draw_graphite_backdrop(
    frame: &mut canvas::Frame,
    cam: &FootprintCanvasState,
    bounds: Rectangle,
    origin: Point,
    look: super::graphite::GraphiteLook,
    snap: &SnapOptions,
) {
    frame.fill_rectangle(Point::ORIGIN, bounds.size(), look.bg);
    if matches!(snap.fine_grid_display, Gd::Hidden)
        && matches!(snap.coarse_grid_display, Gd::Hidden)
    {
        return;
    }
    if let Some(step) = dot_step_px((snap.grid_step_mm as f32) * cam.scale) {
        draw_grid_dots(frame, bounds, origin, step, look.dot);
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

/// Origin crosshair: yellow on the black pads canvas.
pub(crate) fn draw_origin(frame: &mut canvas::Frame, cam: &FootprintCanvasState) {
    draw_origin_in(frame, cam, Color::from_rgba(1.0, 0.95, 0.30, 0.85));
}

/// Origin crosshair in an explicit colour.
fn draw_origin_in(frame: &mut canvas::Frame, cam: &FootprintCanvasState, color: Color) {
    let origin = cam.world_to_screen((0.0, 0.0));
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
            grid_origin: self.state.grid_origin_mm(),
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
        // A board sketch marks the board's origin, as the PCB view draws it.
        if self.state.board_backdrop.is_some() {
            if let Some(at) = self.state.origin_marker_mm() {
                stroke_origin_marker(
                    frame,
                    cstate.world_to_screen(at),
                    layer_style::origin_marker_colour(),
                );
            }
            return;
        }
        let sketch = matches!(self.state.mode, EditorMode::Sketch);
        if let Some(g) = look_from_bg(self.bg_color).or_else(|| sketch.then(board_look)) {
            draw_origin_in(
                frame,
                cstate,
                Color {
                    a: 0.85,
                    ..g.origin
                },
            );
            return;
        }
        draw_origin(frame, cstate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lattice's line 0 is laid at this pixel: a board's origin, or the
    /// camera offset (model zero) for a footprint.
    #[test]
    fn the_grid_is_laid_from_the_board_origin_on_a_board() {
        let cam = FootprintCanvasState::with_view(12.0, Point::new(40.0, 25.0));
        let origin = (5.3, 30.7);
        assert_eq!(
            grid_screen_origin(&cam, Some(origin)),
            cam.world_to_screen(origin)
        );
        assert_eq!(grid_screen_origin(&cam, None), cam.offset);
    }
}
