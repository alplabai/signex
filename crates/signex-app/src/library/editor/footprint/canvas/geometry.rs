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

/// v0.18.25 — distance from a world-mm point to a line segment.
/// Returns Euclidean distance (square-rooted) so callers can compare
/// directly with a tolerance-mm.
pub(super) fn point_to_segment_dist(
    px: f64,
    py: f64,
    ax: f64,
    ay: f64,
    bx: f64,
    by: f64,
) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-12 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
    let t_clamped = t.clamp(0.0, 1.0);
    let qx = ax + t_clamped * dx;
    let qy = ay + t_clamped * dy;
    ((px - qx).powi(2) + (py - qy).powi(2)).sqrt()
}

/// v0.18.25 — even-odd ray casting; assumes the polygon is closed
/// implicitly (last vertex connects back to first).
///
/// v0.18.25.1 — replaced `+ f64::EPSILON` denominator guard (≈ 2e-16,
/// not enough headroom for sub-mm horizontal edges in PCB space) with
/// an explicit `continue` when the edge is near-horizontal at a 1e-10
/// tolerance. Removes a NaN-propagation path that could corrupt the
/// even-odd toggle for the remaining iterations.
pub(super) fn point_in_polygon(px: f64, py: f64, vertices: &[[f64; 2]]) -> bool {
    if vertices.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = vertices.len() - 1;
    for i in 0..vertices.len() {
        let xi = vertices[i][0];
        let yi = vertices[i][1];
        let xj = vertices[j][0];
        let yj = vertices[j][1];
        let denom = yj - yi;
        if denom.abs() < 1e-10 {
            // Horizontal edge — contributes no X intersection.
            j = i;
            continue;
        }
        let intersect = ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / denom + xi);
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
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
