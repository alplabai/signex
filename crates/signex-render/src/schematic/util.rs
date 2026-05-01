//! Tiny shared helpers used by every primitive renderer — overlap
//! tests, finite-coordinate guards, theme colour conversions.
//!
//! Kept in one file so changing the culling predicate (e.g. expanding
//! the visible bounds by a stroke margin) only edits one place.

use signex_types::schematic::Aabb;

/// Two AABBs overlap iff neither lies strictly to one side of the
/// other. Touching boxes are treated as overlapping (the renderer
/// would otherwise drop the stroke pixel that sits exactly on the
/// viewport edge).
#[inline]
pub(crate) fn aabbs_overlap(a: &Aabb, b: &Aabb) -> bool {
    a.min_x <= b.max_x && a.max_x >= b.min_x && a.min_y <= b.max_y && a.max_y >= b.min_y
}

/// Guard against NaN/Inf coordinates from a malformed transform —
/// used at the screen-space boundary so iced never receives a degenerate
/// point.
#[inline]
pub(crate) fn point_finite(p: iced::Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

/// Convert a theme colour to iced. Single re-export so primitives
/// don't all `use crate::colors::to_iced`.
#[inline]
pub(crate) fn iced_color(c: &signex_types::theme::Color) -> iced::Color {
    crate::colors::to_iced(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touching_aabbs_overlap() {
        let a = Aabb::new(0.0, 0.0, 1.0, 1.0);
        let b = Aabb::new(1.0, 0.0, 2.0, 1.0);
        assert!(aabbs_overlap(&a, &b));
    }

    #[test]
    fn separated_aabbs_do_not_overlap() {
        let a = Aabb::new(0.0, 0.0, 1.0, 1.0);
        let b = Aabb::new(2.0, 2.0, 3.0, 3.0);
        assert!(!aabbs_overlap(&a, &b));
    }

    #[test]
    fn finite_filter_rejects_nan_and_inf() {
        assert!(point_finite(iced::Point::new(0.0, 0.0)));
        assert!(!point_finite(iced::Point::new(f32::NAN, 0.0)));
        assert!(!point_finite(iced::Point::new(0.0, f32::INFINITY)));
    }
}
