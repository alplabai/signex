//! A marquee dragged on a view turned on screen (#2316). The box runs along
//! the screen, so on the board it is a rectangle turned the other way: four
//! corners, not two. At no turn it is [`marquee_select`] exactly.
//!
//! The screen maps a board vector `v` to `R(view_deg) v`, y down, positive
//! clockwise; the screen's x axis is the board direction
//! `(cos, -sin)` and its y axis `(sin, cos)`.

use crate::index::{BoardIndex, ItemRef, RectSelect};
use crate::selection::{marquee_mode, marquee_select};
use crate::shape::Shape;
use crate::shape_overlap::Core;
use crate::units::PointNm;

/// The screen's x and y axes as board unit vectors, for a view turned
/// `view_deg` clockwise.
fn screen_axes(view_deg: f64) -> ([f64; 2], [f64; 2]) {
    let (sin, cos) = view_deg.to_radians().sin_cos();
    ([cos, -sin], [sin, cos])
}

/// How far `from`→`to` runs along the screen's x and y axes, in nm.
fn screen_spans(from: PointNm, to: PointNm, view_deg: f64) -> (f64, f64) {
    let (u, v) = screen_axes(view_deg);
    let d = [(to.x - from.x) as f64, (to.y - from.y) as f64];
    (d[0] * u[0] + d[1] * u[1], d[0] * v[0] + d[1] * v[1])
}

/// The marquee's mode: left to right *on screen* selects what lies inside.
pub fn turned_marquee_mode(from: PointNm, to: PointNm, view_deg: f64) -> RectSelect {
    if view_deg == 0.0 {
        return marquee_mode(from, to);
    }
    if screen_spans(from, to, view_deg).0 > 0.0 {
        RectSelect::Inside
    } else {
        RectSelect::Touching
    }
}

/// The four board corners of a marquee from `from` to `to` drawn on a view
/// turned `view_deg` clockwise, in order around the box.
pub fn turned_marquee_corners(from: PointNm, to: PointNm, view_deg: f64) -> [PointNm; 4] {
    let (u, v) = screen_axes(view_deg);
    let (du, dv) = screen_spans(from, to, view_deg);
    let at = |s: [f64; 2], k: f64| {
        PointNm::new(
            from.x + (s[0] * k).round() as i64,
            from.y + (s[1] * k).round() as i64,
        )
    };
    [from, at(u, du), to, at(v, dv)]
}

/// The items a marquee from `from` to `to` selects on a view turned
/// `view_deg` clockwise.
pub fn turned_marquee_select(
    index: &BoardIndex,
    from: PointNm,
    to: PointNm,
    view_deg: f64,
) -> Vec<ItemRef> {
    if view_deg == 0.0 {
        return marquee_select(index, from, to);
    }
    let corners = turned_marquee_corners(from, to, view_deg);
    index.hit_polygon(&corners, turned_marquee_mode(from, to, view_deg))
}

impl Shape {
    /// Whether the shape lies wholly inside the convex polygon `poly`
    /// (either winding; its border counts as inside). A polygon with no
    /// area holds nothing.
    pub fn is_inside_convex(&self, poly: &[PointNm]) -> bool {
        let n = poly.len();
        let doubled_area: i128 = (0..n).map(|i| poly[i].cross(poly[(i + 1) % n])).sum();
        if n < 3 || doubled_area == 0 {
            return false;
        }
        let sign = if doubled_area > 0 { 1.0 } else { -1.0 };
        let (points, radius) = match self.core() {
            Core::Stadium { a, b, radius } => (vec![a, b], radius.max(0)),
            Core::Polygon(points, radius) => (points, radius.max(0)),
        };
        (0..n).all(|i| {
            let (c, d) = (poly[i], poly[(i + 1) % n]);
            let edge = d.minus(c);
            let length = (edge.dot(edge) as f64).sqrt();
            length == 0.0
                || points
                    .iter()
                    .all(|p| sign * edge.cross(p.minus(c)) as f64 >= radius as f64 * length)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i64, y: i64) -> PointNm {
        PointNm::new(x, y)
    }

    #[test]
    fn a_quarter_turned_box_has_the_board_corners_of_the_screen_box() {
        // Turned 90 degrees: screen x runs along board -y, screen y along +x.
        let corners = turned_marquee_corners(p(0, 0), p(10, -20), 90.0);
        assert_eq!(corners, [p(0, 0), p(0, -20), p(10, -20), p(10, 0)]);
    }

    #[test]
    fn left_to_right_is_judged_on_screen() {
        // At 180 degrees a drag to the left on the board is to the right on
        // screen.
        assert_eq!(
            turned_marquee_mode(p(10, 0), p(0, 0), 180.0),
            RectSelect::Inside
        );
        assert_eq!(
            turned_marquee_mode(p(0, 0), p(10, 0), 180.0),
            RectSelect::Touching
        );
    }

    #[test]
    fn a_disc_is_inside_only_with_its_whole_radius() {
        let square = [p(0, 0), p(100, 0), p(100, 100), p(0, 100)];
        let disc = |x, r| Shape::Disc {
            center: p(x, 50),
            radius: r,
        };
        assert!(disc(50, 50).is_inside_convex(&square));
        assert!(!disc(50, 51).is_inside_convex(&square));
        assert!(!disc(95, 10).is_inside_convex(&square));
        // Either winding.
        let reversed: Vec<PointNm> = square.iter().rev().copied().collect();
        assert!(disc(50, 50).is_inside_convex(&reversed));
        assert!(!disc(50, 10).is_inside_convex(&[p(0, 0), p(1, 1), p(2, 2)]));
    }
}
