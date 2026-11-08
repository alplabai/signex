//! Axis-aligned bounding boxes in integer nm.

use crate::units::PointNm;

/// A closed axis-aligned rectangle; `min <= max` on both axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RectNm {
    pub min: PointNm,
    pub max: PointNm,
}

impl RectNm {
    /// The rectangle spanned by two corners, in any order.
    pub fn new(a: PointNm, b: PointNm) -> Self {
        RectNm {
            min: PointNm::new(a.x.min(b.x), a.y.min(b.y)),
            max: PointNm::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    pub fn point(p: PointNm) -> Self {
        RectNm { min: p, max: p }
    }

    /// A rectangle centred on `center`; negative half sizes count as 0.
    pub fn around(center: PointNm, half_w: i64, half_h: i64) -> Self {
        RectNm::point(center).inflate_xy(half_w, half_h)
    }

    /// The bounds of `points`, or `None` for none.
    pub fn bounding(points: impl IntoIterator<Item = PointNm>) -> Option<Self> {
        points
            .into_iter()
            .map(RectNm::point)
            .reduce(|acc, r| acc.union(&r))
    }

    /// Grow every side by `d` (a negative `d` counts as 0).
    pub fn inflate(&self, d: i64) -> Self {
        self.inflate_xy(d, d)
    }

    fn inflate_xy(&self, dx: i64, dy: i64) -> Self {
        let (dx, dy) = (dx.max(0), dy.max(0));
        RectNm {
            min: PointNm::new(self.min.x - dx, self.min.y - dy),
            max: PointNm::new(self.max.x + dx, self.max.y + dy),
        }
    }

    pub fn union(&self, other: &RectNm) -> Self {
        RectNm {
            min: PointNm::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: PointNm::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    pub fn width(&self) -> i64 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> i64 {
        self.max.y - self.min.y
    }

    /// Whether `p` lies inside or on the border.
    pub fn contains_point(&self, p: PointNm) -> bool {
        (self.min.x..=self.max.x).contains(&p.x) && (self.min.y..=self.max.y).contains(&p.y)
    }

    /// Whether `other` lies wholly inside, borders included.
    pub fn contains_rect(&self, other: &RectNm) -> bool {
        self.contains_point(other.min) && self.contains_point(other.max)
    }

    /// Whether the two share at least one point; touching borders count.
    pub fn intersects(&self, other: &RectNm) -> bool {
        self.min.x <= other.max.x
            && other.min.x <= self.max.x
            && self.min.y <= other.max.y
            && other.min.y <= self.max.y
    }

    /// The point of the rectangle nearest to `p`.
    pub fn clamp(&self, p: PointNm) -> PointNm {
        PointNm::new(
            p.x.clamp(self.min.x, self.max.x),
            p.y.clamp(self.min.y, self.max.y),
        )
    }

    /// Corners counter-clockwise from `min`.
    pub fn corners(&self) -> [PointNm; 4] {
        [
            self.min,
            PointNm::new(self.max.x, self.min.y),
            self.max,
            PointNm::new(self.min.x, self.max.y),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x0: i64, y0: i64, x1: i64, y1: i64) -> RectNm {
        RectNm::new(PointNm::new(x0, y0), PointNm::new(x1, y1))
    }

    #[test]
    fn new_orders_the_corners() {
        assert_eq!(r(5, 9, 1, 2), r(1, 2, 5, 9));
        assert_eq!(r(1, 2, 5, 9).width(), 4);
        assert_eq!(r(1, 2, 5, 9).height(), 7);
    }

    #[test]
    fn bounding_covers_every_point() {
        let pts = [PointNm::new(3, -1), PointNm::new(-2, 4), PointNm::new(0, 0)];
        assert_eq!(RectNm::bounding(pts), Some(r(-2, -1, 3, 4)));
        assert_eq!(RectNm::bounding([]), None);
    }

    #[test]
    fn borders_count_for_contains_and_intersects() {
        let a = r(0, 0, 10, 10);
        assert!(a.contains_point(PointNm::new(10, 0)));
        assert!(!a.contains_point(PointNm::new(11, 0)));
        assert!(a.intersects(&r(10, 10, 20, 20)));
        assert!(!a.intersects(&r(11, 0, 20, 20)));
        assert!(a.contains_rect(&r(0, 0, 10, 10)));
        assert!(!a.contains_rect(&r(0, 0, 10, 11)));
    }

    #[test]
    fn inflate_ignores_negative_growth() {
        assert_eq!(r(0, 0, 2, 2).inflate(3), r(-3, -3, 5, 5));
        assert_eq!(r(0, 0, 2, 2).inflate(-3), r(0, 0, 2, 2));
        assert_eq!(RectNm::around(PointNm::new(1, 1), 2, 3), r(-1, -2, 3, 4));
    }

    #[test]
    fn clamp_projects_onto_the_rect() {
        let a = r(0, 0, 10, 10);
        assert_eq!(a.clamp(PointNm::new(-5, 20)), PointNm::new(0, 10));
        assert_eq!(a.clamp(PointNm::new(4, 6)), PointNm::new(4, 6));
    }
}
