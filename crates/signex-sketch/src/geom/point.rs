//! Point / segment / polygon predicates shared across the editor surfaces.
//!
//! These were reimplemented (with drifting signatures) in the footprint,
//! schematic, and symbol presentation code; consolidating them here keeps
//! one correct, tested implementation in the domain and lets the surfaces
//! call it through their local point representations.

use super::Point2;

/// Even-odd ray-cast point-in-polygon test. The polygon is implicitly
/// closed (its last vertex connects back to the first). Near-horizontal
/// edges (`|dy| < 1e-10`) contribute no crossing — this matches the
/// standard even-odd rule for horizontal edges and removes a
/// NaN-propagation path that could otherwise corrupt the toggle for the
/// remaining edges.
pub fn point_in_polygon(p: impl Into<Point2>, polygon: &[Point2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let p = p.into();
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let vi = polygon[i];
        let vj = polygon[j];
        let denom = vj.y - vi.y;
        if denom.abs() < 1e-10 {
            // Horizontal edge — contributes no X intersection.
            j = i;
            continue;
        }
        let intersects =
            ((vi.y > p.y) != (vj.y > p.y)) && (p.x < (vj.x - vi.x) * (p.y - vi.y) / denom + vi.x);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Squared distance from `p` to the segment `[a, b]`, using the clamped
/// projection of `p` onto the segment. Degenerate (zero-length) segments
/// fall back to the point-to-`a` distance.
pub fn point_to_segment_distance_sq(
    p: impl Into<Point2>,
    a: impl Into<Point2>,
    b: impl Into<Point2>,
) -> f64 {
    let (p, a, b) = (p.into(), a.into(), b.into());
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= f64::EPSILON {
        return a.distance_sq(p);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0);
    let cx = a.x + t * dx;
    let cy = a.y + t * dy;
    let ddx = p.x - cx;
    let ddy = p.y - cy;
    ddx * ddx + ddy * ddy
}

/// Euclidean distance from `p` to the segment `[a, b]`.
pub fn point_to_segment_distance(
    p: impl Into<Point2>,
    a: impl Into<Point2>,
    b: impl Into<Point2>,
) -> f64 {
    point_to_segment_distance_sq(p, a, b).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_and_outside_a_square() {
        let square = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]].map(Point2::from);
        assert!(point_in_polygon([5.0, 5.0], &square));
        assert!(!point_in_polygon([-1.0, 5.0], &square));
        assert!(!point_in_polygon([5.0, -1.0], &square));
        assert!(!point_in_polygon([11.0, 5.0], &square));
    }

    #[test]
    fn degenerate_polygon_is_never_inside() {
        let two = [Point2::new(0.0, 0.0), Point2::new(1.0, 1.0)];
        assert!(!point_in_polygon([0.5, 0.5], &two));
    }

    #[test]
    fn distance_to_segment() {
        // perpendicular foot inside the segment
        assert!(
            (point_to_segment_distance([3.0, 4.0], [0.0, 0.0], [10.0, 0.0]) - 4.0).abs() < 1e-9
        );
        // beyond an endpoint clamps to the endpoint
        assert!(
            (point_to_segment_distance([-3.0, 4.0], [0.0, 0.0], [10.0, 0.0]) - 5.0).abs() < 1e-9
        );
        // degenerate segment → distance to the point
        assert!((point_to_segment_distance([3.0, 4.0], [0.0, 0.0], [0.0, 0.0]) - 5.0).abs() < 1e-9);
    }
}
