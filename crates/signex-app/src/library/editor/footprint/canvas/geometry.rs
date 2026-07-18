//! Pure geometry helpers used by the canvas hit-test + draw passes.
//! All free functions, all `pub(super)` so the surrounding canvas/
//! module can reach them.

use iced::Point;

/// v0.18.25 — squared distance from a screen-space point to a line
/// segment. Standard projection-onto-segment with clamped t ∈ [0, 1].
/// Used by `sketch_hit_other` to score nearest-Line candidates.
pub(super) fn screen_dist_to_segment_sq(p: Point, a: Point, b: Point) -> f32 {
    let abx = b.x - a.x;
    let aby = b.y - a.y;
    let len_sq = abx * abx + aby * aby;
    if len_sq < 1e-6 {
        let dx = p.x - a.x;
        let dy = p.y - a.y;
        return dx * dx + dy * dy;
    }
    let t = ((p.x - a.x) * abx + (p.y - a.y) * aby) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let qx = a.x + abx * t;
    let qy = a.y + aby * t;
    let dx = p.x - qx;
    let dy = p.y - qy;
    dx * dx + dy * dy
}

/// Distance (world-mm) from a point to a line segment — a thin adapter
/// over [`signex_sketch::geom::point_to_segment_distance`].
pub(super) fn point_to_segment_dist(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    signex_sketch::geom::point_to_segment_distance([px, py], [ax, ay], [bx, by])
}

/// Even-odd point-in-polygon test (implicitly-closed vertex ring) — a thin
/// adapter over [`signex_sketch::geom::point_in_polygon`].
pub(super) fn point_in_polygon(px: f64, py: f64, vertices: &[[f64; 2]]) -> bool {
    let polygon: Vec<signex_sketch::geom::Point2> = vertices.iter().map(|&v| v.into()).collect();
    signex_sketch::geom::point_in_polygon([px, py], &polygon)
}

/// v0.18.25 — `true` when the point lies within `tol` of any closed-
/// polygon edge (including the implicit last-to-first segment).
pub(super) fn polygon_outline_hit(px: f64, py: f64, vertices: &[[f64; 2]], tol: f64) -> bool {
    let n = vertices.len();
    for i in 0..n {
        let j = (i + 1) % n;
        if point_to_segment_dist(
            px,
            py,
            vertices[i][0],
            vertices[i][1],
            vertices[j][0],
            vertices[j][1],
        ) <= tol
        {
            return true;
        }
    }
    false
}
