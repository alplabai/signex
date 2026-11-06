//! The sheet's backdrop, paper and grid: one drawing shared by the schematic
//! editor and the Components panel's Symbol preview, so the grid-over-paper
//! order (#1993) holds in both.

use iced::advanced::graphics::geometry::Renderer as GeometryRenderer;
use iced::widget::canvas;
use iced::{Color, Point, Rectangle, Size};

use super::{Camera, grid};
use crate::render_config::GridStyle;

/// How the paper and its grid look.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PaperLook {
    pub backdrop: Color,
    pub paper: Color,
    /// Paper border and grid colour.
    pub grid: Color,
    pub grid_visible: bool,
    /// Visible grid pitch, mm.
    pub grid_mm: f32,
    pub grid_style: GridStyle,
    pub device_px_per_logical_px: f32,
}

/// Paint, in order: backdrop, paper, paper border, grid. The grid comes after
/// the paper fill or the opaque paper would cover it.
pub(crate) fn paint_paper_and_grid<R: GeometryRenderer>(
    frame: &mut canvas::Frame<R>,
    camera: &Camera,
    bounds: Rectangle,
    paper_mm: (f32, f32),
    look: &PaperLook,
) {
    frame.fill_rectangle(Point::ORIGIN, bounds.size(), look.backdrop);

    let paper_tl = camera.world_to_screen(Point::new(0.0, 0.0), bounds);
    let paper_br = camera.world_to_screen(Point::new(paper_mm.0, paper_mm.1), bounds);
    let paper_w = paper_br.x - paper_tl.x;
    let paper_h = paper_br.y - paper_tl.y;
    if paper_w > 0.0 && paper_h > 0.0 {
        frame.fill_rectangle(paper_tl, Size::new(paper_w, paper_h), look.paper);
        let border = canvas::Path::rectangle(paper_tl, Size::new(paper_w, paper_h));
        frame.stroke(
            &border,
            canvas::Stroke::default()
                .with_color(look.grid)
                .with_width(1.0),
        );
    }

    if look.grid_visible {
        let _probe = crate::frame_probe::span("canvas.background.grid");
        grid::draw_grid(
            frame,
            camera,
            look.grid_mm,
            bounds,
            look.grid,
            paper_mm.0,
            paper_mm.1,
            look.grid_style,
            look.device_px_per_logical_px,
        );
    }
}
