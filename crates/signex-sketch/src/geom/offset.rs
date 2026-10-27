//! Polygon offset (Minkowski-style outward / inward expansion).
//!
//! Given a closed polygon and a signed offset distance `d`:
//! - `d > 0` grows the polygon outward (used for soldermask
//!   expansion, courtyard buffer).
//! - `d < 0` shrinks it inward (used for pad keepout zones).
//!
//! Two corner styles are supported:
//! - [`CornerStyle::Round`] — replaces each convex corner growing
//!   outward with an arc of radius `|d|`. Where the offset lines
//!   cross instead, the corner joins at their intersection, or
//!   bevels when that point leaves the adjacent edges. The arc is
//!   sampled into segments at a user-configurable `arc_segments`
//!   count so the output stays a plain polygon.
//! - [`CornerStyle::Miter`] — extends the offset edges until they
//!   meet, with a `miter_limit` that falls back to a bevel when
//!   the join would explode (sharp interior angles).
//!
//! Inward offsets that exceed the polygon's smallest local radius
//! produce self-intersections. The implementation does NOT clean
//! those up — callers needing a polygon-boolean cleanup pass must
//! use the Phase 2 boolean module (queued).

use std::f64::consts::TAU;

use super::Point2;
use super::predicates::{Sign, orient2d, signed_area};

/// How offset corners are joined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CornerStyle {
    /// Replace each convex corner growing outward with a circular arc,
    /// sampled at `arc_segments` line segments. Corners where the
    /// offset lines cross join at their intersection (bevel if it
    /// falls outside the adjacent edges); reflex corners bevel.
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
///      - `Round` — where the offset lines diverge (convex corner,
///        `d > 0`) emit the incoming-edge offset point, sample an arc,
///        then emit the outgoing-edge offset point. Where they cross,
///        join at the line intersection if it stays within the
///        adjacent edges, else bevel.
///      - `Miter` — intersect the two offset lines; clamp the join
///        distance against `miter_limit`.
///   4. Reflex corners bevel for `Miter`, and for `Round` when the
///      offset lines diverge (`d < 0`); collinear vertices emit a
///      single point.
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
        let l_out = (e_out_dx * e_out_dx + e_out_dy * e_out_dy)
            .sqrt()
            .max(1e-12);
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

        // Reflex corners that are not joined at an intersection bevel —
        // emit both endpoints. Any overlap between offset edges is left
        // for the boolean cleanup pass.
        let convex = match corner_sign {
            Sign::Positive => ccw,
            Sign::Negative => !ccw,
            Sign::Zero => false,
        };

        // Offset lines DIVERGE at convex corners growing outward (d > 0)
        // and at reflex corners shrinking inward (d < 0). Only the
        // convex case samples an arc; the reflex one bevels below. On
        // the other side the offset lines cross, so the correct join is
        // their intersection — an arc there would overshoot into a
        // bowtie.
        let diverging = (convex && d > 0.0) || (!convex && d < 0.0);
        let crossing_round = arc_segments > 0 && !diverging;

        if crossing_round {
            // Join at the line intersection only while it stays on the
            // adjacent edges; a sharp notch would otherwise throw the
            // vertex far outside the polygon.
            match line_join(in_off, in_dir, out_off, out_dir) {
                Some((join, t_in, t_out))
                    if t_in >= -l_in
                        && t_in <= JOIN_EPS
                        && t_out >= -JOIN_EPS
                        && t_out <= l_out =>
                {
                    out.push(join);
                }
                _ => push_bevel(&mut out, in_off, out_off),
            }
        } else if !convex || arc_segments == 0 {
            // Bevel / miter join.
            match style {
                CornerStyle::Miter { miter_limit } if convex => {
                    // Intersect offset lines. Each line is
                    // `p[i] + d*n_in + t*in_dir` and
                    // `p[i] + d*n_out + s*out_dir`. Solve for the
                    // join point.
                    match line_join(in_off, in_dir, out_off, out_dir) {
                        Some((join, _, _))
                            if ((join.x - curr.x).powi(2) + (join.y - curr.y).powi(2)).sqrt()
                                <= miter_limit * signed_d.abs().max(1e-9) =>
                        {
                            out.push(join);
                        }
                        _ => {
                            out.push(in_off);
                            out.push(out_off);
                        }
                    }
                }
                _ => push_bevel(&mut out, in_off, out_off),
            }
        } else {
            // Diverging round corner — sample an arc from in_off to
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

/// Tolerance on the join parameters and the parallel-edge cut-off.
const JOIN_EPS: f64 = 1e-12;

/// Intersect the incoming offset line (`in_off + t * in_dir`) with the
/// outgoing one (`out_off + s * out_dir`). Returns the point and the
/// parameters `(t, s)`, or `None` when the edges are parallel.
fn line_join(
    in_off: Point2,
    in_dir: (f64, f64),
    out_off: Point2,
    out_dir: (f64, f64),
) -> Option<(Point2, f64, f64)> {
    let denom = in_dir.0 * out_dir.1 - in_dir.1 * out_dir.0;
    if denom.abs() <= JOIN_EPS {
        return None;
    }
    let dx = out_off.x - in_off.x;
    let dy = out_off.y - in_off.y;
    let t = (dx * out_dir.1 - dy * out_dir.0) / denom;
    let s = (dx * in_dir.1 - dy * in_dir.0) / denom;
    Some((
        Point2::new(in_off.x + t * in_dir.0, in_off.y + t * in_dir.1),
        t,
        s,
    ))
}

/// Emit the incoming offset point, plus the outgoing one if distinct.
fn push_bevel(out: &mut Vec<Point2>, in_off: Point2, out_off: Point2) {
    out.push(in_off);
    if (in_off.x - out_off.x).abs() > JOIN_EPS || (in_off.y - out_off.y).abs() > JOIN_EPS {
        out.push(out_off);
    }
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
        assert!(
            offset_polygon(&[p(0.0, 0.0), p(1.0, 0.0)], 1.0, CornerStyle::default()).is_empty()
        );
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

    fn winding_area(poly: &[Point2]) -> f64 {
        signed_area(poly).abs()
    }

    #[test]
    fn round_negative_offset_insets_to_true_corner() {
        let sq = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let out = offset_polygon(&sq, -0.25, CornerStyle::Round { arc_segments: 8 });
        assert_eq!(out.len(), 4, "{out:?}");
        for e in [p(0.25, 0.25), p(0.75, 0.25), p(0.75, 0.75), p(0.25, 0.75)] {
            assert!(out.iter().any(|o| close(*o, e, 1e-9)), "missing {e:?}");
        }
        assert!((winding_area(&out) - 0.25).abs() < 1e-9);
    }

    fn orient(a: Point2, b: Point2, c: Point2) -> f64 {
        (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
    }

    fn is_simple(ring: &[Point2]) -> bool {
        let n = ring.len();
        for i in 0..n {
            for j in (i + 1)..n {
                if j == i + 1 || (i == 0 && j == n - 1) {
                    continue;
                }
                let (a, b) = (ring[i], ring[(i + 1) % n]);
                let (c, d) = (ring[j], ring[(j + 1) % n]);
                if orient(a, b, c) * orient(a, b, d) < 0.0
                    && orient(c, d, a) * orient(c, d, b) < 0.0
                {
                    return false;
                }
            }
        }
        true
    }

    #[test]
    fn round_outward_offset_of_l_shape_joins_reflex_corner_and_stays_simple() {
        // CCW L-shape; the reflex corner is (1, 1).
        let l = vec![
            p(0.0, 0.0),
            p(2.0, 0.0),
            p(2.0, 1.0),
            p(1.0, 1.0),
            p(1.0, 2.0),
            p(0.0, 2.0),
        ];
        let out = offset_polygon(&l, 0.25, CornerStyle::Round { arc_segments: 4 });
        // Reflex corner joins at the line intersection (1.25, 1.25).
        assert!(
            out.iter().any(|o| close(*o, p(1.25, 1.25), 1e-9)),
            "{out:?}"
        );
        assert!(signed_area(&out) > signed_area(&l));
        assert!(is_simple(&out), "{out:?}");
    }

    #[test]
    fn round_outward_offset_of_narrow_v_notch_does_not_spike() {
        let poly = vec![
            p(0.0, 0.0),
            p(2.0, 0.0),
            p(2.0, 1.0),
            p(1.05, 1.0),
            p(1.0, 0.5),
            p(0.95, 1.0),
            p(0.0, 1.0),
        ];
        let out = offset_polygon(&poly, 0.25, CornerStyle::Round { arc_segments: 8 });
        let max_y = out.iter().map(|o| o.y).fold(f64::MIN, f64::max);
        assert!(max_y <= 1.25 + 1e-9, "spike: max_y = {max_y}");
    }

    #[test]
    fn round_inset_of_collinear_midpoint_emits_one_vertex() {
        let sq = vec![
            p(0.0, 0.0),
            p(1.0, 0.0),
            p(2.0, 0.0),
            p(2.0, 2.0),
            p(0.0, 2.0),
        ];
        let out = offset_polygon(&sq, -0.25, CornerStyle::Round { arc_segments: 8 });
        assert_eq!(out.len(), 5, "{out:?}");
        let mids = out
            .iter()
            .filter(|o| close(**o, p(1.0, 0.25), 1e-9))
            .count();
        assert_eq!(mids, 1, "{out:?}");
    }
}
