//! Background grid renderers — line-grid + dot-grid variants.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Rectangle};

/// Render a grid of straight lines stroked at the given step. All
/// lines compose into a single Path so iced tessellates once per
/// frame — the per-line `frame.stroke` loop was the dominant cost
/// when panning an empty footprint canvas.
pub(super) fn draw_grid(
    frame: &mut canvas::Frame,
    bounds: Rectangle,
    offset: Point,
    step: f32,
    color: Color,
) {
    let stroke = Stroke::default().with_width(0.5).with_color(color);
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
pub(super) fn draw_grid_dots(
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
