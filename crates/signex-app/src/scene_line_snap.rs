//! Pixel snapping for axis-aligned hairlines in the CPU scene replay.
//!
//! The canvas rasteriser has no anti-aliasing, so a 1.9 px stroke covers one
//! or two pixel rows depending on where its centre falls, and a horizontal and
//! a vertical segment of one wire differ in thickness (#1654). Snapping the
//! width to whole pixels and the centre line to the pixel grid makes the
//! thickness independent of orientation and position. The grid is the
//! device-pixel grid, `device_px_per_logical_px` cells per logical pixel
//! (#1858); dashed strokes snap the same way. Runs after the stroke floor.

use iced::Point;

/// Strokes at or above this width (device px) keep their exact value: rounding a wide
/// stroke by half a pixel is not visible, and it must not distort bus weights.
const SNAP_BELOW_PX: f32 = 4.0;

/// Endpoints below this distance apart are treated as the same coordinate.
const AXIS_EPS: f32 = 1e-3;

/// An axis-aligned segment snapped to whole device pixels, or the input
/// unchanged when it is diagonal or already wide. Points and width are in
/// logical pixels.
#[must_use]
pub(crate) fn snap_hairline(
    p0: Point,
    p1: Point,
    width: f32,
    device_px_per_logical_px: f32,
) -> (Point, Point, f32) {
    let dpr = if device_px_per_logical_px.is_finite() && device_px_per_logical_px > 0.0 {
        device_px_per_logical_px
    } else {
        1.0
    };
    if dpr == 1.0 {
        return snap_device(p0, p1, width);
    }
    let to_device = |p: Point| Point::new(p.x * dpr, p.y * dpr);
    let to_logical = |p: Point| Point::new(p.x / dpr, p.y / dpr);
    let (q0, q1, w) = snap_device(to_device(p0), to_device(p1), width * dpr);
    (to_logical(q0), to_logical(q1), w / dpr)
}

/// [`snap_hairline`] on the device-pixel grid.
fn snap_device(p0: Point, p1: Point, width: f32) -> (Point, Point, f32) {
    if width >= SNAP_BELOW_PX || !width.is_finite() {
        return (p0, p1, width);
    }
    let snapped_width = width.round().max(1.0);
    if (p0.y - p1.y).abs() < AXIS_EPS {
        let y = snap_centre(p0.y, snapped_width);
        // The ends snap like a centre line, so a vertical leg that meets this
        // one at an elbow is centred on exactly the x this one ends at.
        let (x0, x1) = (
            snap_centre(p0.x, snapped_width),
            snap_centre(p1.x, snapped_width),
        );
        return (Point::new(x0, y), Point::new(x1, y), snapped_width);
    }
    if (p0.x - p1.x).abs() < AXIS_EPS {
        let x = snap_centre(p0.x, snapped_width);
        let (y0, y1) = (
            snap_centre(p0.y, snapped_width),
            snap_centre(p1.y, snapped_width),
        );
        return (Point::new(x, y0), Point::new(x, y1), snapped_width);
    }
    (p0, p1, width)
}

/// The centre line that makes a `width`-px stroke cover exactly `width` whole
/// pixel rows: a pixel middle for an odd width, a pixel edge for an even one.
fn snap_centre(centre: f32, width: f32) -> f32 {
    if (width as u32).is_multiple_of(2) {
        centre.round()
    } else {
        centre.floor() + 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thickness_px(centre: f32, width: f32) -> usize {
        // Pixel rows whose middle lies inside the stroke (no anti-aliasing).
        let (lo, hi) = (centre - width / 2.0, centre + width / 2.0);
        (0..1000)
            .filter(|row| {
                let mid = *row as f32 + 0.5;
                mid >= lo && mid < hi
            })
            .count()
    }

    #[test]
    fn horizontal_and_vertical_hairlines_get_the_same_thickness_at_any_phase() {
        for phase in 0..20 {
            let offset = 295.0 + phase as f32 * 0.05;
            let (h0, h1, hw) = snap_hairline(
                Point::new(10.0, offset),
                Point::new(200.0, offset),
                1.9,
                1.0,
            );
            let (v0, v1, vw) = snap_hairline(
                Point::new(offset, 10.0),
                Point::new(offset, 200.0),
                1.9,
                1.0,
            );
            assert_eq!(h0.y, h1.y);
            assert_eq!(v0.x, v1.x);
            assert_eq!(hw, vw);
            assert_eq!(thickness_px(h0.y, hw), 2, "horizontal at phase {phase}");
            assert_eq!(thickness_px(v0.x, vw), 2, "vertical at phase {phase}");
        }
    }

    /// At an elbow the horizontal leg ends where the vertical leg is centred,
    /// so the round caps meet with no notch or overhang.
    #[test]
    fn the_legs_of_an_elbow_meet_on_the_same_pixel_grid_point() {
        for phase in 0..20 {
            let corner = Point::new(50.0 + phase as f32 * 0.05, 20.0 + phase as f32 * 0.07);
            let (_, h1, hw) = snap_hairline(Point::new(10.3, corner.y), corner, 1.9, 1.0);
            let (v0, _, vw) = snap_hairline(corner, Point::new(corner.x, 80.2), 1.9, 1.0);
            assert_eq!(hw, vw);
            assert_eq!(h1, v0, "the corner at phase {phase}");
        }
    }

    #[test]
    fn a_sub_pixel_width_becomes_one_whole_pixel() {
        let (a, _, w) = snap_hairline(Point::new(0.0, 5.3), Point::new(9.0, 5.3), 0.6, 1.0);
        assert_eq!(w, 1.0);
        assert_eq!(thickness_px(a.y, w), 1);
    }

    /// At 2x the grid is half a logical pixel: a 0.3 px floored hairline is
    /// one device pixel, centred on a device-pixel middle.
    #[test]
    fn the_snap_grid_is_device_pixels() {
        let (a, b, w) = snap_hairline(Point::new(0.0, 5.3), Point::new(9.0, 5.3), 0.3, 2.0);
        assert_eq!(w, 0.5);
        assert_eq!(a.y, b.y);
        assert_eq!(thickness_px(a.y * 2.0, w * 2.0), 1);
        let (_, _, w) = snap_hairline(Point::new(0.0, 5.3), Point::new(9.0, 5.3), 1.9, 2.0);
        assert_eq!(w, 2.0, "3.8 device px rounds to 4");
        let (_, _, w) = snap_hairline(Point::new(0.0, 5.3), Point::new(9.0, 5.3), 2.5, 2.0);
        assert_eq!(w, 2.5, "5 device px is wide enough to keep");
    }

    #[test]
    fn diagonals_and_wide_strokes_are_left_alone() {
        let (a, b) = (Point::new(0.0, 0.0), Point::new(10.0, 7.3));
        assert_eq!(snap_hairline(a, b, 1.9, 1.0), (a, b, 1.9));
        let (c, d) = (Point::new(0.0, 5.3), Point::new(9.0, 5.3));
        assert_eq!(snap_hairline(c, d, 6.4, 1.0), (c, d, 6.4));
    }
}
