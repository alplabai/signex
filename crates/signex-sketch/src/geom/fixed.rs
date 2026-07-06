//! Fixed-point arithmetic for deterministic geometry.
//!
//! `f64` results aren't bit-stable across machines because the
//! order of operations and the FPU rounding mode can shift the
//! last few bits. For tests / persistence / cross-machine
//! reproducibility we want geometry that returns IDENTICAL
//! integer coordinates from identical inputs.
//!
//! `FixPoint2` carries `(i64, i64)` coordinates that map to mm
//! through a power-of-two `SCALE`. Default `SCALE = 1024` gives
//! ≈ 0.001 mm precision (≈ 1 µm) over a ±1 km world — way more
//! headroom than any practical PCB.
//!
//! The `to_f64` / `from_f64` helpers round-trip with deterministic
//! truncation. Geometry on `FixPoint2` is integer arithmetic
//! end-to-end, so two runs with the same inputs produce
//! byte-identical outputs.

use super::Point2;

/// Fixed-point scale — number of integer units per millimetre.
/// Power-of-two so float-to-fixed conversion stays exact for
/// representable mm values.
pub const SCALE: i64 = 1024;
const SCALE_F: f64 = SCALE as f64;

/// Fixed-point 2D coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FixPoint2 {
    pub x: i64,
    pub y: i64,
}

impl FixPoint2 {
    pub const fn new(x: i64, y: i64) -> Self {
        Self { x, y }
    }

    /// Convert from world-mm with rounding.
    pub fn from_f64(p: Point2) -> Self {
        Self {
            x: (p.x * SCALE_F).round() as i64,
            y: (p.y * SCALE_F).round() as i64,
        }
    }

    /// Convert back to world-mm.
    pub fn to_f64(self) -> Point2 {
        Point2::new((self.x as f64) / SCALE_F, (self.y as f64) / SCALE_F)
    }
}

/// Fixed-point variant of `signed_area`. Returns `2 * signed_area`
/// in fixed-point units (the doubled form keeps it integer).
pub fn signed_area_2x(points: &[FixPoint2]) -> i128 {
    if points.len() < 3 {
        return 0;
    }
    let mut acc: i128 = 0;
    for i in 0..points.len() {
        let j = (i + 1) % points.len();
        acc += (points[i].x as i128) * (points[j].y as i128);
        acc -= (points[j].x as i128) * (points[i].y as i128);
    }
    acc
}

/// Fixed-point orient2d: sign of the determinant. Returns +1 / -1
/// / 0 — exact arithmetic, no eps tolerance because integers.
pub fn orient2d(a: FixPoint2, b: FixPoint2, c: FixPoint2) -> i32 {
    let det: i128 = (b.x as i128 - a.x as i128) * (c.y as i128 - a.y as i128)
        - (b.y as i128 - a.y as i128) * (c.x as i128 - a.x as i128);
    if det > 0 {
        1
    } else if det < 0 {
        -1
    } else {
        0
    }
}

/// Convert a slice of f64 points to fixed-point.
pub fn polygon_from_f64(polygon: &[Point2]) -> Vec<FixPoint2> {
    polygon.iter().copied().map(FixPoint2::from_f64).collect()
}

/// Convert a slice of fixed-point points back to f64.
pub fn polygon_to_f64(polygon: &[FixPoint2]) -> Vec<Point2> {
    polygon.iter().copied().map(FixPoint2::to_f64).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    #[test]
    fn round_trip_preserves_value_at_scale_resolution() {
        // Values that are an integer multiple of 1/SCALE round
        // trip exactly.
        let p_in = p(1.5, -2.25);
        let fixed = FixPoint2::from_f64(p_in);
        let p_out = fixed.to_f64();
        assert!((p_out.x - p_in.x).abs() < 1e-9);
        assert!((p_out.y - p_in.y).abs() < 1e-9);
    }

    #[test]
    fn signed_area_unit_square_is_2_times_one() {
        let sq = [
            FixPoint2::from_f64(p(0.0, 0.0)),
            FixPoint2::from_f64(p(1.0, 0.0)),
            FixPoint2::from_f64(p(1.0, 1.0)),
            FixPoint2::from_f64(p(0.0, 1.0)),
        ];
        // Signed area of unit square in world mm = 1; doubled in
        // fixed-point = 2 * SCALE * SCALE.
        let area2x = signed_area_2x(&sq);
        let expected = 2_i128 * (SCALE as i128) * (SCALE as i128);
        assert_eq!(area2x, expected);
    }

    #[test]
    fn orient2d_ccw_triangle_positive() {
        let a = FixPoint2::from_f64(p(0.0, 0.0));
        let b = FixPoint2::from_f64(p(1.0, 0.0));
        let c = FixPoint2::from_f64(p(0.0, 1.0));
        assert_eq!(orient2d(a, b, c), 1);
        assert_eq!(orient2d(a, c, b), -1);
    }

    #[test]
    fn orient2d_colinear_zero() {
        let a = FixPoint2::from_f64(p(0.0, 0.0));
        let b = FixPoint2::from_f64(p(1.0, 0.0));
        let c = FixPoint2::from_f64(p(2.0, 0.0));
        assert_eq!(orient2d(a, b, c), 0);
    }

    #[test]
    fn round_trip_polygon() {
        let pts = vec![p(0.0, 0.0), p(1.5, 2.25), p(-3.5, 4.0)];
        let fixed = polygon_from_f64(&pts);
        let back = polygon_to_f64(&fixed);
        for (a, b) in pts.iter().zip(back.iter()) {
            assert!((a.x - b.x).abs() < 1e-9);
            assert!((a.y - b.y).abs() < 1e-9);
        }
    }
}
