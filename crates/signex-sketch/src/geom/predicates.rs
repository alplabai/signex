//! Geometric predicates with epsilon-aware sign returns.
//!
//! These wrap the textbook formulae for orientation and signed area
//! with a tolerance-aware sign so callers don't have to repeat the
//! same `det.abs() < EPS` boilerplate everywhere. PCB sketch
//! tolerances live at the mm scale where IEEE-754 f64 has ~12
//! decimal digits of headroom; the relative + absolute bound check
//! below is enough without a multi-precision fallback.

use super::Point2;

/// Discrete sign of a predicate. `Zero` means "within tolerance of
/// the boundary" — callers branch on the three cases the same way
/// they would on the `Ordering` of a comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    /// Predicate evaluates positive (CCW for orientation, > 0 for area).
    Positive,
    /// Predicate evaluates negative (CW for orientation, < 0 for area).
    Negative,
    /// Predicate falls within the absolute tolerance band — the three
    /// inputs are colinear (orientation) or the polygon is degenerate
    /// (signed area).
    Zero,
}

impl Sign {
    pub fn from_signed(value: f64, tol: f64) -> Self {
        if value.abs() <= tol {
            Sign::Zero
        } else if value > 0.0 {
            Sign::Positive
        } else {
            Sign::Negative
        }
    }
}

/// Default absolute tolerance for predicate sign decisions. 1e-9 mm
/// = 1 fm at sketch scale — well below any physical PCB feature
/// and well above f64 noise on typical operands.
pub const DEFAULT_TOL: f64 = 1.0e-9;

/// Orientation of triangle `(a, b, c)`. Positive = CCW (left turn at
/// b), Negative = CW (right turn), Zero = colinear within tolerance.
///
/// Determinant form:
/// ```text
///   det = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
/// ```
/// equals twice the signed area of the triangle. Positive when the
/// three points wind CCW, negative for CW, zero when colinear.
///
/// The error bound for f64 evaluation of this 2x2 determinant
/// scales with the magnitude of the products; we apply a relative
/// bound plus an absolute floor at `DEFAULT_TOL` for robustness
/// when both terms are tiny.
pub fn orient2d(a: Point2, b: Point2, c: Point2) -> Sign {
    let l = (b.x - a.x) * (c.y - a.y);
    let r = (b.y - a.y) * (c.x - a.x);
    let det = l - r;
    let mag = l.abs().max(r.abs());
    let rel_tol = mag * 4.0 * f64::EPSILON;
    let tol = rel_tol.max(DEFAULT_TOL);
    Sign::from_signed(det, tol)
}

/// Twice the signed area of a polygon defined by `points` (treated
/// as a closed ring — last vertex connects back to first). The
/// shoelace formula. Positive = CCW winding, Negative = CW, Zero
/// = degenerate. Returns `0.0` for fewer than three vertices.
pub fn signed_area(points: &[Point2]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut acc = 0.0_f64;
    for i in 0..points.len() {
        let j = (i + 1) % points.len();
        acc += points[i].x * points[j].y;
        acc -= points[j].x * points[i].y;
    }
    acc / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    #[test]
    fn ccw_triangle_is_positive() {
        // Standard CCW triangle: (0,0) -> (1,0) -> (0,1) is left-turn.
        assert_eq!(
            orient2d(p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0)),
            Sign::Positive
        );
    }

    #[test]
    fn cw_triangle_is_negative() {
        assert_eq!(
            orient2d(p(0.0, 0.0), p(0.0, 1.0), p(1.0, 0.0)),
            Sign::Negative
        );
    }

    #[test]
    fn colinear_points_are_zero() {
        assert_eq!(orient2d(p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)), Sign::Zero);
        assert_eq!(orient2d(p(1.0, 1.0), p(2.0, 2.0), p(3.0, 3.0)), Sign::Zero);
    }

    #[test]
    fn near_colinear_within_tolerance_zero() {
        // Sub-femtometre offset — well below DEFAULT_TOL.
        assert_eq!(
            orient2d(p(0.0, 0.0), p(1.0, 0.0), p(2.0, 1.0e-12)),
            Sign::Zero
        );
    }

    #[test]
    fn signed_area_unit_square_ccw_is_one() {
        let sq = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let area = signed_area(&sq);
        assert!((area - 1.0).abs() < 1e-12);
    }

    #[test]
    fn signed_area_unit_square_cw_is_negative_one() {
        let sq = [p(0.0, 0.0), p(0.0, 1.0), p(1.0, 1.0), p(1.0, 0.0)];
        let area = signed_area(&sq);
        assert!((area + 1.0).abs() < 1e-12);
    }

    #[test]
    fn signed_area_degenerate_returns_zero() {
        assert_eq!(signed_area(&[]), 0.0);
        assert_eq!(signed_area(&[p(0.0, 0.0)]), 0.0);
        assert_eq!(signed_area(&[p(0.0, 0.0), p(1.0, 1.0)]), 0.0);
    }
}
