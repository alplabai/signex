//! Polygon offset (Minkowski-style outward / inward expansion).
//!
//! Given a closed polygon and a signed offset distance `d`:
//! - `d > 0` grows the polygon outward (used for soldermask
//!   expansion, courtyard buffer).
//! - `d < 0` shrinks it inward (used for pad keepout zones).
//!
//! Two corner styles are supported:
//! - [`CornerStyle::Round`] — replaces each convex corner with an
//!   arc of radius `|d|`. The arc is sampled into segments at a
//!   user-configurable `arc_segments` count so the output stays a
//!   plain polygon.
//! - [`CornerStyle::Miter`] — extends the offset edges until they
//!   meet, with a `miter_limit` that falls back to a bevel when
//!   the join would explode (sharp interior angles).
//!
//! Inward offsets that exceed the polygon's smallest local radius
//! produce self-intersections. The implementation does NOT clean
//! those up — callers needing a polygon-boolean cleanup pass must
//! use the Phase 2 boolean module (queued).

use std::f64::consts::TAU;

use super::predicates::{orient2d, signed_area, Sign};
use super::Point2;

/// How offset corners are joined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CornerStyle {
    /// Replace each convex corner with a circular arc, sampled at
    /// `arc_segments` line segments.
    Round { arc_segments: u32 },
    /// Extend the offset edges; bevel when the miter would exceed
    /// `miter_limit * |offset|`.
    Miter { miter_limit: f64 },
}

impl Default for CornerStyle {
    fn default() -> Self {
        CornerStyle::Round { arc_segments: 8 }
    }
}

/// Offset the closed polygon by signed distance `d`. Returns a new
/// polygon walking the same winding direction as the input.
///
/// The algorithm:
///   1. For each edge `(p[i], p[i+1])`, compute the unit-perpendicular
///      `n_i` (CCW polygon → outward normal points right of edge
///      direction; the sign is normalised internally so positive
///      `d` always grows outward regardless of input winding).
///   2. Each edge's offset is `(p[i] + d * n_i, p[i+1] + d * n_i)`.
///   3. Adjacent offset edges are joined per `style`:
///      - `Round` — emit the corner vertex `p[i+1]`'s offset point
///        for the incoming edge, then sample an arc on the convex
///        side, then emit the offset point for the outgoing edge.
///      - `Miter` — intersect the two offset lines; clamp the join
///        distance against `miter_limit`.
///   4. Concave corners always bevel (intersecting offset lines
///      land inside the polygon, which the Round / Miter logic
///      degenerates into a clean two-edge join).
///
/// Degenerate inputs (< 3 vertices, zero-area, or self-intersecting)
/// return an empty Vec.
pub fn offset_polygon(polygon: &[Point2], d: f64, style: CornerStyle) -> Vec<Point2> {
    if polygon.len() < 3 {
        return Vec::new();
    }
    let area = signed_area(polygon);
    if area.abs() <= 1e-12 {
        return Vec::new();
    }
    let ccw = area > 0.0;
    // For CW input, flip the sign so positive `d` still grows
    // outward. The output winding matches the input.
    let signed_d = if ccw { d } else { -d };
    let arc_segments = match style {
        CornerStyle::Round { arc_segments } => arc_segments.max(1),
        CornerStyle::Miter { .. } => 0,
    };

    let n = polygon.len();
    let mut out: Vec<Point2> = Vec::with_capacity(n + (arc_segments as usize) * n);

    for i in 0..n {
        let prev = polygon[(i + n - 1) % n];
        let curr = polygon[i];
        let next = polygon[(i + 1) % n];

        // Edge directions normalised.
        let (e_in_dx, e_in_dy) = (curr.x - prev.x, curr.y - prev.y);
        let (e_out_dx, e_out_dy) = (next.x - curr.x, next.y - curr.y);
        let l_in = (e_in_dx * e_in_dx + e_in_dy * e_in_dy).sqrt().max(1e-12);
        let l_out = (e_out_dx * e_out_dx + e_out_dy * e_out_dy).sqrt().max(1e-12);
        let in_dir = (e_in_dx / l_in, e_in_dy / l_in);
        let out_dir = (e_out_dx / l_out, e_out_dy / l_out);

        // Outward normal (rotate edge direction 90° CW for CCW
        // polygons) is `(dy, -dx)` for each unit edge direction.
        let n_in = (in_dir.1, -in_dir.0);
        let n_out = (out_dir.1, -out_dir.0);

        // The corner type — convex (left turn for CCW) or concave.
        let corner_sign = orient2d(prev, curr, next);

        // Endpoint of the inbound edge offset, and start of the
        // outbound edge offset, both at world `curr` shifted by
        // their respective offset normals.
        let in_off = Point2::new(curr.x + signed_d * n_in.0, curr.y + signed_d * n_in.1);
        let out_off = Point2::new(curr.x + signed_d * n_out.0, curr.y + signed_d * n_out.1);

        // Concave corners always bevel — emit both endpoints. The
        // overlap between offset edges lands inside the polygon and
        // the boolean cleanup pass (Phase 2) handles it. For now we
        // accept the local self-intersection rather than miter
        // through it (which would produce a spurious vertex outside
        // the polygon body).
        let convex = match corner_sign {
            Sign::Positive => ccw,
            Sign::Negative => !ccw,
            Sign::Zero => false,
        };

        if !convex || arc_segments == 0 {
            // Bevel / miter join.
            match style {
                CornerStyle::Miter { miter_limit } if convex => {
                    // Intersect offset lines. Each line is
                    // `p[i] + d*n_in + t*in_dir` and
                    // `p[i] + d*n_out + s*out_dir`. Solve for the
                    // join point.
                    let denom = in_dir.0 * out_dir.1 - in_dir.1 * out_dir.0;
                    if denom.abs() <= 1e-12 {
                        out.push(in_off);
                        out.push(out_off);
                    } else {
                        let dx = out_off.x - in_off.x;
                        let dy = out_off.y - in_off.y;
                        let t = (dx * out_dir.1 - dy * out_dir.0) / denom;
                        let join = Point2::new(
                            in_off.x + t * in_dir.0,
                            in_off.y + t * in_dir.1,
                        );
                        let bulge = ((join.x - curr.x).powi(2) + (join.y - curr.y).powi(2))
                            .sqrt();
                        if bulge <= miter_limit * signed_d.abs().max(1e-9) {
                            out.push(join);
                        } else {
                            out.push(in_off);
                            out.push(out_off);
                        }
                    }
                }
                _ => {
                    out.push(in_off);
                    if (in_off.x - out_off.x).abs() > 1e-12
                        || (in_off.y - out_off.y).abs() > 1e-12
                    {
                        out.push(out_off);
                    }
                }
            }
        } else {
            // Convex round corner — sample an arc from in_off to
            // out_off centred at `curr`. The two offset normals
            // already give us the start and end angles; we walk
            // a fixed number of equal angular steps between them.
            out.push(in_off);
            let theta_a = (in_off.y - curr.y).atan2(in_off.x - curr.x);
            let theta_b = (out_off.y - curr.y).atan2(out_off.x - curr.x);
            // Walk the SHORT-way sweep on the convex side. For CCW
            // polygons the short-way is positive; for CW it's
            // negative. Normalise to [-π, π] so we always cross
            // the smaller arc.
            let mut sweep = theta_b - theta_a;
            if sweep > std::f64::consts::PI {
                sweep -= TAU;
            } else if sweep < -std::f64::consts::PI {
                sweep += TAU;
            }
            let r = signed_d.abs();
            for k in 1..arc_segments {
                let t = k as f64 / arc_segments as f64;
                let theta = theta_a + sweep * t;
                out.push(Point2::new(
                    curr.x + r * theta.cos(),
                    curr.y + r * theta.sin(),
                ));
            }
            out.push(out_off);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn close(a: Point2, b: Point2, tol: f64) -> bool {
        (a.x - b.x).abs() < tol && (a.y - b.y).abs() < tol
    }

    #[test]
    fn empty_polygon_returns_empty() {
        assert!(offset_polygon(&[], 1.0, CornerStyle::default()).is_empty());
        assert!(offset_polygon(&[p(0.0, 0.0)], 1.0, CornerStyle::default()).is_empty());
        assert!(offset_polygon(&[p(0.0, 0.0), p(1.0, 0.0)], 1.0, CornerStyle::default()).is_empty());
    }

    #[test]
    fn square_outward_miter_grows_corners() {
        // Unit square CCW. Miter offset by 1 should grow it to a
        // 3x3 square centred on the original.
        let sq = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let out = offset_polygon(&sq, 1.0, CornerStyle::Miter { miter_limit: 4.0 });
        assert_eq!(out.len(), 4);
        // Verify corners landed at the expected world positions.
        let expected = [p(-1.0, -1.0), p(2.0, -1.0), p(2.0, 2.0), p(-1.0, 2.0)];
        for e in expected {
            assert!(
                out.iter().any(|o| close(*o, e, 1e-9)),
                "missing expected corner {:?} in {out:?}",
                e
            );
        }
    }

    #[test]
    fn square_round_offset_emits_arcs() {
        let sq = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let out = offset_polygon(&sq, 0.5, CornerStyle::Round { arc_segments: 4 });
        // Per corner: in_off + (arc_segments - 1) interior arc
        // samples + out_off = 1 + arc_segments points. The
        // out_off of corner i and in_off of corner i+1 are at
        // different world positions (offset edge endpoints), so
        // there is no sharing between corners.
        // Total = 4 corners × (1 + 4) = 20 vertices.
        assert_eq!(out.len(), 20);
        // Every point must lie at distance 0.5 from the nearest
        // corner of the original square.
        let corners = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        for v in &out {
            let min_d = corners
                .iter()
                .map(|c| ((v.x - c.x).powi(2) + (v.y - c.y).powi(2)).sqrt())
                .fold(f64::INFINITY, f64::min);
            assert!(
                (min_d - 0.5).abs() < 1e-6,
                "vertex {v:?} should be 0.5 from the nearest corner; got {min_d}"
            );
        }
    }

    #[test]
    fn cw_input_grows_outward_too() {
        // Same square in CW winding. Positive `d` should still
        // grow it outward.
        let sq = vec![p(0.0, 0.0), p(0.0, 1.0), p(1.0, 1.0), p(1.0, 0.0)];
        let out = offset_polygon(&sq, 1.0, CornerStyle::Miter { miter_limit: 4.0 });
        assert_eq!(out.len(), 4);
        // Output should still be the 3x3 square at -1 / +2, just CW.
        let expected = [p(-1.0, -1.0), p(2.0, -1.0), p(2.0, 2.0), p(-1.0, 2.0)];
        for e in expected {
            assert!(
                out.iter().any(|o| close(*o, e, 1e-9)),
                "missing expected corner {:?} in {out:?}",
                e
            );
        }
    }

    #[test]
    fn negative_offset_shrinks() {
        // Unit square shrunk by 0.25 should be a 0.5x0.5 square
        // centred at (0.5, 0.5).
        let sq = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let out = offset_polygon(&sq, -0.25, CornerStyle::Miter { miter_limit: 4.0 });
        assert_eq!(out.len(), 4);
        let expected = [p(0.25, 0.25), p(0.75, 0.25), p(0.75, 0.75), p(0.25, 0.75)];
        for e in expected {
            assert!(
                out.iter().any(|o| close(*o, e, 1e-9)),
                "missing expected corner {:?} in {out:?}",
                e
            );
        }
    }
}
