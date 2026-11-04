//! Point-to-segment distance, the one copy the editors share.

/// Squared distance from `p` to the segment `[a, b]`, using the clamped
/// projection of `p` onto the segment. A zero-length segment falls back to
/// the distance to `a`.
#[must_use]
pub fn point_to_segment_distance_sq(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len_sq = dx * dx + dy * dy;
    if len_sq <= f64::EPSILON {
        return (p.0 - a.0).powi(2) + (p.1 - a.1).powi(2);
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len_sq).clamp(0.0, 1.0);
    (p.0 - (a.0 + t * dx)).powi(2) + (p.1 - (a.1 + t * dy)).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_is_to_the_nearest_point_of_the_segment() {
        let d = |p| point_to_segment_distance_sq(p, (0.0, 0.0), (10.0, 0.0)).sqrt();
        assert_eq!(d((5.0, 3.0)), 3.0);
        assert_eq!(d((-4.0, 3.0)), 5.0);
        assert_eq!(d((13.0, 4.0)), 5.0);
        assert_eq!(
            point_to_segment_distance_sq((3.0, 4.0), (0.0, 0.0), (0.0, 0.0)),
            25.0
        );
    }
}
