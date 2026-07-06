//! Arc-aware polygon offset. Accepts a closed polyline whose
//! segments are either straight Lines or circular Arcs, and
//! returns the offset polyline whose arc segments stay arcs (no
//! sampling-to-polyline-then-offset roundtrip that would lose
//! precision and emit excess vertices).
//!
//! Each segment offsets independently:
//! - Straight segment `(a, b)` shifts by `d` along its outward
//!   normal.
//! - Arc `(centre, radius, start_angle, end_angle, ccw)` becomes
//!   a concentric arc with `radius + d` (or `radius − d` for an
//!   inward offset on a CCW arc); same centre, same angular
//!   range. For a CW arc the sign flips.
//!
//! Convex breaks between consecutive offset segments are bridged
//! with a round-corner arc of radius `|d|`, matching the standard
//! Minkowski offset semantics. Concave breaks bevel — the local
//! self-intersection that follows is left for the boolean cleanup
//! pass to resolve.

use std::f64::consts::TAU;

use super::Point2;

/// One element of an arc-aware closed polyline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PolyElement {
    /// Straight segment from `a` to `b`.
    Line { a: Point2, b: Point2 },
    /// Circular arc — `centre` + `radius` + `start_rad`/`end_rad`
    /// + `sweep_ccw`. The arc's start point is at
    /// `(centre.x + radius·cos(start_rad), centre.y + radius·sin(start_rad))`
    /// and similar for the end. Adjacent elements must connect at
    /// these endpoints.
    Arc {
        centre: Point2,
        radius: f64,
        start_rad: f64,
        end_rad: f64,
        sweep_ccw: bool,
    },
}

impl PolyElement {
    fn start(&self) -> Point2 {
        match *self {
            PolyElement::Line { a, .. } => a,
            PolyElement::Arc {
                centre,
                radius,
                start_rad,
                ..
            } => Point2::new(
                centre.x + radius * start_rad.cos(),
                centre.y + radius * start_rad.sin(),
            ),
        }
    }

    fn end(&self) -> Point2 {
        match *self {
            PolyElement::Line { b, .. } => b,
            PolyElement::Arc {
                centre,
                radius,
                end_rad,
                ..
            } => Point2::new(
                centre.x + radius * end_rad.cos(),
                centre.y + radius * end_rad.sin(),
            ),
        }
    }

    /// Outward unit-perpendicular direction at the START of the
    /// element. For a Line, this is the perpendicular to the
    /// segment direction. For an Arc, it's the radial direction
    /// (outward from centre = positive offset direction for CCW).
    fn start_outward_normal(&self, polygon_ccw: bool) -> (f64, f64) {
        match *self {
            PolyElement::Line { a, b } => unit_perp_outward(a, b, polygon_ccw),
            PolyElement::Arc {
                centre,
                radius,
                start_rad,
                sweep_ccw,
                ..
            } => {
                // For a CCW arc on a CCW polygon, outward = radial.
                // CW arc on CCW polygon → outward = inward radial.
                // Same logic for CW polygon flips.
                let radial = (start_rad.cos(), start_rad.sin());
                let outward = if sweep_ccw == polygon_ccw {
                    radial
                } else {
                    (-radial.0, -radial.1)
                };
                let _ = (centre, radius);
                outward
            }
        }
    }

    fn end_outward_normal(&self, polygon_ccw: bool) -> (f64, f64) {
        match *self {
            PolyElement::Line { a, b } => unit_perp_outward(a, b, polygon_ccw),
            PolyElement::Arc {
                end_rad,
                sweep_ccw,
                ..
            } => {
                let radial = (end_rad.cos(), end_rad.sin());
                if sweep_ccw == polygon_ccw {
                    radial
                } else {
                    (-radial.0, -radial.1)
                }
            }
        }
    }
}

/// Unit perpendicular pointing OUT of a CCW polygon (right of edge
/// direction). Flipped for CW.
fn unit_perp_outward(a: Point2, b: Point2, ccw: bool) -> (f64, f64) {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).sqrt().max(1e-12);
    let perp = (dy / len, -dx / len);
    if ccw {
        perp
    } else {
        (-perp.0, -perp.1)
    }
}

/// Offset an arc-aware closed polyline by signed distance `d`.
/// Positive grows outward, negative shrinks inward. Returns a
/// new sequence of `PolyElement`s.
///
/// Approximation note: convex corner-bridge arcs are emitted
/// directly as `PolyElement::Arc` so the result stays arc-aware.
/// Concave corners bevel; the resulting local self-intersection
/// must be cleaned up by `polygon_op` if a topologically clean
/// offset is needed.
///
/// `polygon_ccw` tells us the original polyline's winding so the
/// outward normal direction is unambiguous. Caller computes this
/// via the shoelace area on the polyline endpoints.
pub fn offset_arc_polyline(
    elements: &[PolyElement],
    d: f64,
    polygon_ccw: bool,
) -> Vec<PolyElement> {
    if elements.is_empty() {
        return Vec::new();
    }
    let n = elements.len();
    let mut out: Vec<PolyElement> = Vec::with_capacity(n * 2);
    for i in 0..n {
        let el = elements[i];
        let prev = elements[(i + n - 1) % n];
        let in_normal = prev.end_outward_normal(polygon_ccw);
        let out_normal = el.start_outward_normal(polygon_ccw);
        let break_dot = in_normal.0 * out_normal.0 + in_normal.1 * out_normal.1;
        if break_dot < 1.0 - 1e-9 {
            // Normals diverge → there's a corner break to bridge.
            // Convex iff cross of the two normals matches polygon
            // orientation.
            let cross = in_normal.0 * out_normal.1 - in_normal.1 * out_normal.0;
            let convex = (cross > 0.0) == polygon_ccw;
            if convex && d.abs() > 1e-12 {
                let pivot = prev.end();
                let theta_a = in_normal.1.atan2(in_normal.0);
                let theta_b = out_normal.1.atan2(out_normal.0);
                let mut sweep = theta_b - theta_a;
                if sweep > std::f64::consts::PI {
                    sweep -= TAU;
                } else if sweep < -std::f64::consts::PI {
                    sweep += TAU;
                }
                let sweep_ccw = sweep >= 0.0;
                let r = d.abs();
                out.push(PolyElement::Arc {
                    centre: pivot,
                    radius: r,
                    start_rad: theta_a,
                    end_rad: theta_b,
                    sweep_ccw,
                });
            }
        }
        out.push(offset_element(el, d, polygon_ccw));
    }
    out
}

fn offset_element(el: PolyElement, d: f64, polygon_ccw: bool) -> PolyElement {
    match el {
        PolyElement::Line { a, b } => {
            let n = unit_perp_outward(a, b, polygon_ccw);
            PolyElement::Line {
                a: Point2::new(a.x + d * n.0, a.y + d * n.1),
                b: Point2::new(b.x + d * n.0, b.y + d * n.1),
            }
        }
        PolyElement::Arc {
            centre,
            radius,
            start_rad,
            end_rad,
            sweep_ccw,
        } => {
            // For an arc winding the same direction as the polygon,
            // outward = radius grows. Opposite winding flips it.
            let signed = if sweep_ccw == polygon_ccw { d } else { -d };
            let new_r = (radius + signed).max(1e-9);
            PolyElement::Arc {
                centre,
                radius: new_r,
                start_rad,
                end_rad,
                sweep_ccw,
            }
        }
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
    fn empty_input_empty_output() {
        let out = offset_arc_polyline(&[], 1.0, true);
        assert!(out.is_empty());
    }

    #[test]
    fn line_only_polygon_offsets_each_edge() {
        // Square as 4 Lines. Outward offset by 1 gives a 3x3
        // square at -1 / +2 with arc-bridge corners.
        let sq = vec![
            PolyElement::Line {
                a: p(0.0, 0.0),
                b: p(1.0, 0.0),
            },
            PolyElement::Line {
                a: p(1.0, 0.0),
                b: p(1.0, 1.0),
            },
            PolyElement::Line {
                a: p(1.0, 1.0),
                b: p(0.0, 1.0),
            },
            PolyElement::Line {
                a: p(0.0, 1.0),
                b: p(0.0, 0.0),
            },
        ];
        let out = offset_arc_polyline(&sq, 1.0, true);
        // 4 offset Lines + 4 corner Arcs = 8 elements.
        assert_eq!(out.len(), 8);
        // Verify the 4 arc corners are at the original square's
        // vertices with radius 1.
        let mut arc_centres: Vec<Point2> = Vec::new();
        for el in &out {
            if let PolyElement::Arc {
                centre, radius, ..
            } = el
            {
                assert!((radius - 1.0).abs() < 1e-9);
                arc_centres.push(*centre);
            }
        }
        assert_eq!(arc_centres.len(), 4);
        // Corners of the unit square — order may vary so just
        // verify the set.
        let expected = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        for c in expected {
            assert!(arc_centres.iter().any(|a| close(*a, c, 1e-9)));
        }
    }

    #[test]
    fn arc_grows_radius_on_outward_offset() {
        // Single-arc circle (closed by being a full sweep).
        // Outward offset by 0.5 grows the radius from 1.0 to 1.5.
        let arc = vec![PolyElement::Arc {
            centre: p(0.0, 0.0),
            radius: 1.0,
            start_rad: 0.0,
            end_rad: TAU,
            sweep_ccw: true,
        }];
        let out = offset_arc_polyline(&arc, 0.5, true);
        assert_eq!(out.len(), 1);
        if let PolyElement::Arc { radius, .. } = out[0] {
            assert!((radius - 1.5).abs() < 1e-9);
        } else {
            panic!("expected Arc, got {:?}", out[0]);
        }
    }

    #[test]
    fn arc_shrinks_on_inward_offset() {
        let arc = vec![PolyElement::Arc {
            centre: p(0.0, 0.0),
            radius: 1.0,
            start_rad: 0.0,
            end_rad: TAU,
            sweep_ccw: true,
        }];
        let out = offset_arc_polyline(&arc, -0.25, true);
        if let PolyElement::Arc { radius, .. } = out[0] {
            assert!((radius - 0.75).abs() < 1e-9);
        }
    }
}
