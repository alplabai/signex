//! A rectangle's Width and Height, read from and written to its corners.
//! The Properties panel's Width / Height pair stays in step with End X / Y
//! because both are the same `to` coordinate.

/// A rectangle's width (or height) from its two corner coordinates.
pub fn rect_extent(from: f64, to: f64) -> f64 {
    (to - from).abs()
}

/// The `to` coordinate that gives a rectangle `extent` from `from`, on the
/// side `to` is on now (the positive side for a zero-size rectangle). A
/// negative extent is read as zero.
pub fn rect_end_for_extent(from: f64, to: f64, extent: f64) -> f64 {
    let side = if to < from { -1.0 } else { 1.0 };
    from + side * extent.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extent_is_the_distance_between_the_corners() {
        assert_eq!(rect_extent(-2.0, 3.0), 5.0);
        assert_eq!(rect_extent(3.0, -2.0), 5.0);
    }

    #[test]
    fn a_new_extent_keeps_the_side_the_end_corner_is_on() {
        assert_eq!(rect_end_for_extent(1.0, 4.0, 6.0), 7.0);
        assert_eq!(rect_end_for_extent(1.0, -4.0, 6.0), -5.0);
        assert_eq!(rect_end_for_extent(1.0, 1.0, 2.0), 3.0);
    }

    #[test]
    fn a_negative_extent_collapses_the_side() {
        assert_eq!(rect_end_for_extent(1.0, 4.0, -3.0), 1.0);
    }

    #[test]
    fn writing_the_read_extent_leaves_the_corner_alone() {
        let (from, to) = (-1.5, -7.25);
        assert_eq!(rect_end_for_extent(from, to, rect_extent(from, to)), to);
    }
}
