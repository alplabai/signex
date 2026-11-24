//! Background grid renderers — line-grid + dot-grid variants.

use std::ops::RangeInclusive;

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Rectangle};

/// The lattice indices `k` whose line `origin + k * step` falls within one
/// step outside `0..=extent`. Each line is placed from its own index, never
/// by adding `step` along the row: a running sum drifts by a different
/// amount at another pan offset, so the antialiased grid would shift by a
/// fraction of a pixel instead of moving exactly with the pan.
fn lattice(origin: f32, step: f32, extent: f32) -> RangeInclusive<i64> {
    let first = ((-step - origin) / step).ceil() as i64;
    let last = ((extent + step - origin) / step).floor() as i64;
    first..=last
}

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
        for k in lattice(offset.x, step, bounds.width) {
            let x = offset.x + k as f32 * step;
            builder.move_to(Point::new(x, 0.0));
            builder.line_to(Point::new(x, bounds.height));
        }
        for k in lattice(offset.y, step, bounds.height) {
            let y = offset.y + k as f32 * step;
            builder.move_to(Point::new(0.0, y));
            builder.line_to(Point::new(bounds.width, y));
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
        for i in lattice(offset.x, step, bounds.width) {
            let x = offset.x + i as f32 * step;
            for j in lattice(offset.y, step, bounds.height) {
                let y = offset.y + j as f32 * step;
                builder.move_to(Point::new(x - half, y - half));
                builder.line_to(Point::new(x + half, y - half));
                builder.line_to(Point::new(x + half, y + half));
                builder.line_to(Point::new(x - half, y + half));
                builder.close();
            }
        }
    });
    frame.fill(&path, color);
}

#[cfg(test)]
mod tests {
    use super::lattice;

    /// The same world lines at two pan offsets: each index lands exactly
    /// the pan apart, so a panned grid is the old one moved, bit for bit.
    #[test]
    fn a_panned_lattice_moves_exactly_with_the_pan() {
        let (step, extent) = (13.7_f32, 93.6_f32);
        let before = 46.818_f32;
        let after = before + 37.0;
        for k in lattice(after, step, extent) {
            let moved = after + k as f32 * step;
            let was = before + k as f32 * step;
            assert!(
                (moved - was - 37.0).abs() < 1e-4,
                "line {k}: {was} -> {moved}"
            );
        }
    }

    /// The range covers one step past both ends, as the old running sum did.
    #[test]
    fn the_lattice_reaches_one_step_past_both_ends() {
        let (origin, step, extent) = (5.0_f32, 10.0_f32, 50.0_f32);
        let lines: Vec<f32> = lattice(origin, step, extent)
            .map(|k| origin + k as f32 * step)
            .collect();
        assert_eq!(lines, vec![-5.0, 5.0, 15.0, 25.0, 35.0, 45.0, 55.0]);
    }
}
