//! Bounding-box spatial index for 2D primitives.
//!
//! A flat AABB array with linear-scan queries that uses early
//! rejection on the bounding box. Backs the snap + hit-test
//! pipelines once entity counts climb past the point where O(n)
//! scans become noticeable (~hundreds).
//!
//! The structure is intentionally simple — no kd-tree, no
//! R-tree — because:
//!   1. Sketch-mode entity counts are typically <500. The
//!      constant factor of a tree is bigger than the win at this
//!      scale.
//!   2. Inserts / rebuilds happen on every solve+bake. A tree
//!      that pays for fast queries via slow inserts trades the
//!      wrong direction.
//!   3. The flat-array form is trivial to verify and benchmark.
//!
//! When sketches start carrying tens of thousands of primitives
//! (PCB-level use of this module), drop a kd-tree behind the same
//! `AabbIndex` API and the call sites won't change.

use super::Point2;

/// Axis-aligned bounding box in plane-local mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Point2,
    pub max: Point2,
}

impl Aabb {
    pub fn new(min: Point2, max: Point2) -> Self {
        Self { min, max }
    }

    /// Build an Aabb covering all `points`. Returns `None` for an
    /// empty slice.
    pub fn from_points(points: &[Point2]) -> Option<Self> {
        let first = points.first().copied()?;
        let mut min = first;
        let mut max = first;
        for &p in &points[1..] {
            if p.x < min.x {
                min.x = p.x;
            }
            if p.y < min.y {
                min.y = p.y;
            }
            if p.x > max.x {
                max.x = p.x;
            }
            if p.y > max.y {
                max.y = p.y;
            }
        }
        Some(Self { min, max })
    }

    pub fn contains(&self, p: Point2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    /// `true` when this box overlaps `other` — inclusive on the
    /// shared boundary.
    pub fn overlaps(&self, other: Aabb) -> bool {
        !(self.max.x < other.min.x
            || other.max.x < self.min.x
            || self.max.y < other.min.y
            || other.max.y < self.min.y)
    }

    /// Expand the box by `pad` in every direction. Used to query
    /// "anything within `pad` of this point" via point-vs-box.
    pub fn expanded(&self, pad: f64) -> Self {
        Self {
            min: Point2::new(self.min.x - pad, self.min.y - pad),
            max: Point2::new(self.max.x + pad, self.max.y + pad),
        }
    }
}

/// Flat-array AABB index. The user inserts `(item, bbox)` pairs;
/// the index rebuilds after each batch of inserts. Queries return
/// the items whose bbox intersects the query region.
#[derive(Debug, Default, Clone)]
pub struct AabbIndex<T> {
    items: Vec<(T, Aabb)>,
}

impl<T: Clone> AabbIndex<T> {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            items: Vec::with_capacity(cap),
        }
    }

    pub fn insert(&mut self, item: T, bbox: Aabb) {
        self.items.push((item, bbox));
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Items whose bbox contains `p`. Returns clones so callers
    /// can use the items after the index goes out of scope.
    pub fn query_point(&self, p: Point2) -> Vec<T> {
        self.items
            .iter()
            .filter(|(_, bb)| bb.contains(p))
            .map(|(item, _)| item.clone())
            .collect()
    }

    /// Items whose bbox overlaps `region`. Iterator-style so
    /// callers that just want to walk hits don't pay the Vec
    /// allocation.
    pub fn query_region<'a>(&'a self, region: Aabb) -> impl Iterator<Item = &'a T> + 'a {
        self.items.iter().filter_map(move |(item, bb)| {
            if bb.overlaps(region) {
                Some(item)
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    #[test]
    fn aabb_from_points_empty() {
        assert_eq!(Aabb::from_points(&[]), None);
    }

    #[test]
    fn aabb_from_points_finds_extents() {
        let pts = [p(1.0, 2.0), p(-1.0, 5.0), p(3.0, 0.0), p(2.0, -2.0)];
        let bb = Aabb::from_points(&pts).unwrap();
        assert_eq!(bb.min, p(-1.0, -2.0));
        assert_eq!(bb.max, p(3.0, 5.0));
    }

    #[test]
    fn aabb_contains_inclusive() {
        let bb = Aabb::new(p(0.0, 0.0), p(1.0, 1.0));
        assert!(bb.contains(p(0.5, 0.5)));
        assert!(bb.contains(p(0.0, 0.0)));
        assert!(bb.contains(p(1.0, 1.0)));
        assert!(!bb.contains(p(1.5, 0.5)));
    }

    #[test]
    fn aabb_overlaps_touching_is_true() {
        let a = Aabb::new(p(0.0, 0.0), p(1.0, 1.0));
        let b = Aabb::new(p(1.0, 0.0), p(2.0, 1.0));
        assert!(a.overlaps(b));
    }

    #[test]
    fn aabb_overlaps_disjoint_is_false() {
        let a = Aabb::new(p(0.0, 0.0), p(1.0, 1.0));
        let b = Aabb::new(p(2.0, 0.0), p(3.0, 1.0));
        assert!(!a.overlaps(b));
    }

    #[test]
    fn index_query_point_finds_overlapping() {
        let mut idx: AabbIndex<usize> = AabbIndex::new();
        idx.insert(0, Aabb::new(p(0.0, 0.0), p(1.0, 1.0)));
        idx.insert(1, Aabb::new(p(0.5, 0.5), p(2.0, 2.0)));
        idx.insert(2, Aabb::new(p(3.0, 3.0), p(4.0, 4.0)));
        let hits = idx.query_point(p(0.75, 0.75));
        assert_eq!(hits.len(), 2);
        assert!(hits.contains(&0));
        assert!(hits.contains(&1));
    }

    #[test]
    fn index_query_region_iterator() {
        let mut idx: AabbIndex<usize> = AabbIndex::new();
        idx.insert(0, Aabb::new(p(0.0, 0.0), p(1.0, 1.0)));
        idx.insert(1, Aabb::new(p(2.0, 2.0), p(3.0, 3.0)));
        idx.insert(2, Aabb::new(p(0.5, 0.5), p(2.5, 2.5)));
        let region = Aabb::new(p(1.5, 1.5), p(3.5, 3.5));
        let hits: Vec<usize> = idx.query_region(region).copied().collect();
        assert_eq!(hits.len(), 2);
        assert!(hits.contains(&1));
        assert!(hits.contains(&2));
    }

    #[test]
    fn expanded_grows_in_all_directions() {
        let a = Aabb::new(p(0.0, 0.0), p(1.0, 1.0));
        let e = a.expanded(0.5);
        assert_eq!(e.min, p(-0.5, -0.5));
        assert_eq!(e.max, p(1.5, 1.5));
    }
}
