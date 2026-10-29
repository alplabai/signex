//! Pixel snapping for axis-aligned hairlines in the CPU scene replay.
//!
//! The canvas rasteriser has no anti-aliasing, so a 1.9 px stroke covers one
//! or two pixel rows depending on where its centre falls. Two segments of the
//! same wire, one horizontal and one vertical, then differ in thickness (1 px
//! vs 2 px) and the thinner one reads as a different colour (#1654). Snapping
//! the width to whole pixels and the centre line to the pixel grid makes the
//! thickness independent of orientation and position.

use iced::Point;

/// Strokes at or above this width keep their exact value: rounding a wide
/// stroke by half a pixel is not visible, and it must not distort bus weights.
const SNAP_BELOW_PX: f32 = 4.0;

/// Endpoints below this distance apart are treated as the same coordinate.
const AXIS_EPS: f32 = 1e-3;

/// A solid axis-aligned segment snapped to whole device pixels, or the input
/// unchanged when it is diagonal or already wide.
#[must_use]
pub(crate) fn snap_hairline(p0: Point, p1: Point, width: f32) -> (Point, Point, f32) {
    if width >= SNAP_BELOW_PX || !width.is_finite() {
        return (p0, p1, width);
    }
    let snapped_width = width.round().max(1.0);
    if (p0.y - p1.y).abs() < AXIS_EPS {
        let y = snap_centre(p0.y, snapped_width);
        return (Point::new(p0.x, y), Point::new(p1.x, y), snapped_width);
    }
    if (p0.x - p1.x).abs() < AXIS_EPS {
        let x = snap_centre(p0.x, snapped_width);
        return (Point::new(x, p0.y), Point::new(x, p1.y), snapped_width);
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
            let (h0, h1, hw) =
                snap_hairline(Point::new(10.0, offset), Point::new(200.0, offset), 1.9);
            let (v0, v1, vw) =
                snap_hairline(Point::new(offset, 10.0), Point::new(offset, 200.0), 1.9);
            assert_eq!(h0.y, h1.y);
            assert_eq!(v0.x, v1.x);
            assert_eq!(hw, vw);
            assert_eq!(thickness_px(h0.y, hw), 2, "horizontal at phase {phase}");
            assert_eq!(thickness_px(v0.x, vw), 2, "vertical at phase {phase}");
        }
    }

    #[test]
    fn a_sub_pixel_width_becomes_one_whole_pixel() {
        let (a, _, w) = snap_hairline(Point::new(0.0, 5.3), Point::new(9.0, 5.3), 0.6);
        assert_eq!(w, 1.0);
        assert_eq!(thickness_px(a.y, w), 1);
    }

    #[test]
    fn diagonals_and_wide_strokes_are_left_alone() {
        let (a, b) = (Point::new(0.0, 0.0), Point::new(10.0, 7.3));
        assert_eq!(snap_hairline(a, b, 1.9), (a, b, 1.9));
        let (c, d) = (Point::new(0.0, 5.3), Point::new(9.0, 5.3));
        assert_eq!(snap_hairline(c, d, 6.4), (c, d, 6.4));
    }
}
