//! Polygon simplification — removes duplicate vertices, merges
//! colinear edges, and snaps near-equal coordinates to a grid.
//! Used as a pre-processing pass before the boolean operations
//! to dodge the degenerate cases (vertex-on-edge, colinear
//! shared boundaries, near-duplicate vertices) that trip the
//! basic Greiner-Hormann variant.
//!
//! For a future full robust-boolean implementation these helpers
//! become inner steps of the boolean itself; for the current
//! Greiner-Hormann they're a "make my inputs cleaner" entry point.

use super::Point2;
use super::Sign;
use super::predicates::orient2d;

/// Multi-contour polygon: one outer ring plus zero or more holes.
/// Outer winds CCW; holes wind CW by convention. Useful as a
/// type to express "polygon with holes" even before the full
/// hole-aware boolean implementation lands.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MultiContour {
    pub outer: Vec<Point2>,
    pub holes: Vec<Vec<Point2>>,
}

impl MultiContour {
    pub fn new(outer: Vec<Point2>) -> Self {
        Self {
            outer,
            holes: Vec::new(),
        }
    }

    pub fn with_holes(outer: Vec<Point2>, holes: Vec<Vec<Point2>>) -> Self {
        Self { outer, holes }
    }
}

/// Drop adjacent duplicate vertices (within `eps`). The closing
/// vertex of a closed ring isn't stored in the input convention
/// (last != first), so the wrap-around comparison runs separately.
pub fn dedup(polygon: &[Point2], eps: f64) -> Vec<Point2> {
    let mut out: Vec<Point2> = Vec::with_capacity(polygon.len());
    for &p in polygon {
        if let Some(last) = out.last() {
            if (p.x - last.x).abs() <= eps && (p.y - last.y).abs() <= eps {
                continue;
            }
        }
        out.push(p);
    }
    if out.len() >= 2 {
        let first = out[0];
        let last = *out.last().unwrap();
        if (first.x - last.x).abs() <= eps && (first.y - last.y).abs() <= eps {
            out.pop();
        }
    }
    out
}

/// Drop the middle vertex from any colinear consecutive triple
/// (a, b, c) where b lies on segment a→c within `eps`. After this
/// pass the polygon has only "essential" corners — no spurious
/// midpoints on a straight edge.
pub fn merge_colinear(polygon: &[Point2]) -> Vec<Point2> {
    if polygon.len() < 3 {
        return polygon.to_vec();
    }
    let mut out: Vec<Point2> = Vec::with_capacity(polygon.len());
    let n = polygon.len();
    for i in 0..n {
        let prev = polygon[(i + n - 1) % n];
        let curr = polygon[i];
        let next = polygon[(i + 1) % n];
        if matches!(orient2d(prev, curr, next), Sign::Zero) {
            continue;
        }
        out.push(curr);
    }
    out
}

/// Snap each coordinate to the nearest multiple of `step`. Kills
/// near-duplicate vertices that would otherwise fail dedup at a
/// looser eps. Use a step matching the user-facing precision —
/// too loose snaps two distinct features together; too tight is
/// a no-op.
pub fn snap_to_grid(polygon: &[Point2], step: f64) -> Vec<Point2> {
    if step <= 0.0 {
        return polygon.to_vec();
    }
    polygon
        .iter()
        .map(|p| Point2::new((p.x / step).round() * step, (p.y / step).round() * step))
        .collect()
}

/// One-call pipeline: snap to grid, dedup adjacent duplicates,
/// merge colinear runs. Pre-processes a polygon for the boolean
/// operations.
pub fn simplify_polygon(polygon: &[Point2], snap_step: f64, dedup_eps: f64) -> Vec<Point2> {
    let snapped = snap_to_grid(polygon, snap_step);
    let deduped = dedup(&snapped, dedup_eps);
    merge_colinear(&deduped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    #[test]
    fn dedup_removes_adjacent_duplicates() {
        let pts = vec![
            p(0.0, 0.0),
            p(0.0, 0.0),
            p(1.0, 0.0),
            p(1.0, 0.0),
            p(1.0, 1.0),
        ];
        let out = dedup(&pts, 1e-9);
        assert_eq!(out, vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)]);
    }

    #[test]
    fn dedup_removes_wrap_around_duplicate() {
        let pts = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 0.0)];
        let out = dedup(&pts, 1e-9);
        assert_eq!(out, vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)]);
    }

    #[test]
    fn merge_colinear_drops_midpoints() {
        // Square with extra midpoint on the bottom edge.
        let pts = vec![
            p(0.0, 0.0),
            p(0.5, 0.0),
            p(1.0, 0.0),
            p(1.0, 1.0),
            p(0.0, 1.0),
        ];
        let out = merge_colinear(&pts);
        assert_eq!(out.len(), 4);
        assert!(!out.contains(&p(0.5, 0.0)));
    }

    #[test]
    fn snap_to_grid_rounds_to_step() {
        let pts = vec![p(0.001, 0.0), p(1.001, 0.001), p(1.0, 1.0001)];
        let out = snap_to_grid(&pts, 0.01);
        assert_eq!(out, vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)]);
    }

    #[test]
    fn simplify_pipeline_combines_all_three() {
        let pts = vec![
            p(0.0, 0.0),
            p(0.0001, 0.0), // near-duplicate after snap
            p(0.5, 0.0),    // colinear after dedup
            p(1.0, 0.0),
            p(1.0, 1.0),
            p(0.0, 1.0),
        ];
        let out = simplify_polygon(&pts, 0.01, 1e-9);
        assert_eq!(out.len(), 4);
        assert!(!out.contains(&p(0.5, 0.0)));
        assert!(!out.contains(&p(0.0001, 0.0)));
    }
}
