//! Which of a zone's holes can be drawn as cut-outs (#897).
//!
//! `Zone.holes` documents its invariant: each hole is a simple ring of at
//! least 3 vertices, lies strictly inside the outline, and is disjoint from
//! every other hole. The parser does not enforce it. The GPU fill and the CPU
//! even-odd fill only agree on holes that keep it, so both render paths draw
//! only [`Zone::usable_holes`], and the app reports [`Zone::hole_problems`]
//! when a board is opened or reloaded.
//!
//! Each ring's bounding box is computed once and checked before any edge
//! test, so holes far apart (the common case: a grid of via clearances) cost
//! a box comparison, not an edge-by-edge scan.

use crate::pcb::{Point, Zone};

/// Why a hole is not drawn as a cut-out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoleProblem {
    /// Fewer than 3 vertices.
    Malformed,
    /// A coordinate is NaN or infinite.
    NotFinite,
    /// Two of its own edges cross or touch.
    SelfIntersecting,
    /// Some part lies on or outside the zone outline.
    NotInsideOutline,
    /// It touches, crosses or nests with the hole at this index, which
    /// comes earlier in `holes` and was kept.
    OverlapsHole(usize),
}

impl HoleProblem {
    pub fn describe(self) -> String {
        match self {
            Self::Malformed => "has fewer than 3 vertices".into(),
            Self::NotFinite => "has a non-finite coordinate".into(),
            Self::SelfIntersecting => "crosses itself".into(),
            Self::NotInsideOutline => "is not strictly inside the outline".into(),
            Self::OverlapsHole(other) => format!("touches or overlaps hole {other}"),
        }
    }
}

impl Zone {
    /// Indices into [`Zone::holes`] of the holes that keep the invariant,
    /// in order. Holes are checked in file order and an overlap drops the
    /// later one, so one large bad hole listed first suppresses every valid
    /// hole it overlaps.
    pub fn usable_holes(&self) -> Vec<usize> {
        self.classify_holes().0
    }

    /// Every hole that is not drawn as a cut-out, with the reason.
    pub fn hole_problems(&self) -> Vec<(usize, HoleProblem)> {
        self.classify_holes().1
    }

    fn classify_holes(&self) -> (Vec<usize>, Vec<(usize, HoleProblem)>) {
        if self.outline.len() < 3 || !ring_is_finite(&self.outline) {
            let all = (0..self.holes.len())
                .map(|i| (i, HoleProblem::NotInsideOutline))
                .collect();
            return (Vec::new(), all);
        }
        let outline = Ring::new(&self.outline);
        let mut kept: Vec<(usize, Ring<'_>)> = Vec::new();
        let mut problems = Vec::new();
        for (index, hole) in self.holes.iter().enumerate() {
            match check_hole(hole, &outline, &kept) {
                Ok(ring) => kept.push((index, ring)),
                Err(problem) => problems.push((index, problem)),
            }
        }
        (kept.into_iter().map(|(i, _)| i).collect(), problems)
    }
}

fn check_hole<'a>(
    hole: &'a [Point],
    outline: &Ring<'_>,
    kept: &[(usize, Ring<'_>)],
) -> Result<Ring<'a>, HoleProblem> {
    if hole.len() < 3 {
        return Err(HoleProblem::Malformed);
    }
    if !ring_is_finite(hole) {
        return Err(HoleProblem::NotFinite);
    }
    let ring = Ring::new(hole);
    if ring.self_intersects() {
        return Err(HoleProblem::SelfIntersecting);
    }
    if !ring.strictly_inside(outline) {
        return Err(HoleProblem::NotInsideOutline);
    }
    if let Some((other, _)) = kept.iter().find(|(_, k)| !ring.disjoint_from(k)) {
        return Err(HoleProblem::OverlapsHole(*other));
    }
    Ok(ring)
}

fn ring_is_finite(ring: &[Point]) -> bool {
    ring.iter().all(|p| p.x.is_finite() && p.y.is_finite())
}

/// A ring with its bounding box.
struct Ring<'a> {
    points: &'a [Point],
    min: Point,
    max: Point,
}

impl<'a> Ring<'a> {
    fn new(points: &'a [Point]) -> Self {
        let mut min = points[0];
        let mut max = points[0];
        for p in points {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
        Self { points, min, max }
    }

    /// Bounding boxes overlap or touch.
    fn boxes_meet(&self, other: &Ring<'_>) -> bool {
        self.min.x <= other.max.x
            && other.min.x <= self.max.x
            && self.min.y <= other.max.y
            && other.min.y <= self.max.y
    }

    fn edges(&self) -> impl Iterator<Item = (Point, Point)> + '_ {
        let n = self.points.len();
        (0..n).map(move |i| (self.points[i], self.points[(i + 1) % n]))
    }

    /// Two edges that are not neighbours in the ring share a point.
    fn self_intersects(&self) -> bool {
        let n = self.points.len();
        let edge = |i: usize| (self.points[i], self.points[(i + 1) % n]);
        (0..n).any(|i| {
            ((i + 2)..n).filter(|&j| !(i == 0 && j == n - 1)).any(|j| {
                let ((p, q), (r, s)) = (edge(i), edge(j));
                segments_meet(p, q, r, s)
            })
        })
    }

    fn touches(&self, other: &Ring<'_>) -> bool {
        self.boxes_meet(other)
            && self
                .edges()
                .any(|(p, q)| other.edges().any(|(r, s)| segments_meet(p, q, r, s)))
    }

    /// Every vertex strictly inside `outer`, and no edge meets one of its.
    fn strictly_inside(&self, outer: &Ring<'_>) -> bool {
        let box_inside = self.min.x > outer.min.x
            && self.max.x < outer.max.x
            && self.min.y > outer.min.y
            && self.max.y < outer.max.y;
        box_inside && point_in_ring(self.points[0], outer.points) && !self.touches(outer)
    }

    /// Neither ring touches, crosses or contains the other.
    fn disjoint_from(&self, other: &Ring<'_>) -> bool {
        if !self.boxes_meet(other) {
            return true;
        }
        !self.touches(other)
            && !point_in_ring(self.points[0], other.points)
            && !point_in_ring(other.points[0], self.points)
    }
}

/// Even-odd ray cast. Used only once edge contact has been ruled out, so a
/// point on an edge never reaches it.
fn point_in_ring(p: Point, ring: &[Point]) -> bool {
    let n = ring.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y);
            if p.x < x {
                inside = !inside;
            }
        }
    }
    inside
}

fn orient(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn within_box(a: Point, b: Point, p: Point) -> bool {
    p.x >= a.x.min(b.x) && p.x <= a.x.max(b.x) && p.y >= a.y.min(b.y) && p.y <= a.y.max(b.y)
}

/// Segments `pq` and `rs` share at least one point (crossing or touching).
fn segments_meet(p: Point, q: Point, r: Point, s: Point) -> bool {
    let (d1, d2) = (orient(r, s, p), orient(r, s, q));
    let (d3, d4) = (orient(p, q, r), orient(p, q, s));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    (d1 == 0.0 && within_box(r, s, p))
        || (d2 == 0.0 && within_box(r, s, q))
        || (d3 == 0.0 && within_box(p, q, r))
        || (d4 == 0.0 && within_box(p, q, s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point { x: x0, y: y0 },
            Point { x: x1, y: y0 },
            Point { x: x1, y: y1 },
            Point { x: x0, y: y1 },
        ]
    }

    fn zone(holes: Vec<Vec<Point>>) -> Zone {
        Zone {
            uuid: Uuid::nil(),
            net: 0,
            net_name: String::new(),
            layer: "TopCopper".into(),
            outline: rect(0.0, 0.0, 10.0, 10.0),
            holes,
            priority: 0,
            fill_type: String::new(),
            thermal_relief: false,
            thermal_gap: 0.0,
            thermal_width: 0.0,
            clearance: 0.0,
            min_thickness: 0.0,
            unknown: Default::default(),
        }
    }

    #[test]
    fn disjoint_holes_strictly_inside_are_kept() {
        let z = zone(vec![rect(1.0, 1.0, 3.0, 3.0), rect(5.0, 5.0, 7.0, 7.0)]);
        assert_eq!(z.usable_holes(), [0, 1]);
        assert!(z.hole_problems().is_empty());
    }

    #[test]
    fn a_hole_outside_or_touching_the_outline_is_dropped() {
        let z = zone(vec![
            rect(20.0, 20.0, 22.0, 22.0),
            rect(6.0, 4.0, 10.0, 6.0),
            rect(8.0, 8.0, 12.0, 12.0),
            rect(1.0, 1.0, 2.0, 2.0),
        ]);
        assert_eq!(z.usable_holes(), [3]);
        assert!(
            z.hole_problems()
                .iter()
                .all(|(_, p)| *p == HoleProblem::NotInsideOutline)
        );
    }

    #[test]
    fn overlapping_nested_or_touching_holes_keep_only_the_first() {
        let z = zone(vec![
            rect(2.0, 2.0, 6.0, 6.0),
            rect(4.0, 4.0, 8.0, 8.0),
            rect(3.0, 3.0, 4.0, 4.0),
            rect(6.0, 2.0, 7.0, 3.0),
        ]);
        assert_eq!(z.usable_holes(), [0]);
        assert_eq!(
            z.hole_problems(),
            [
                (1, HoleProblem::OverlapsHole(0)),
                (2, HoleProblem::OverlapsHole(0)),
                (3, HoleProblem::OverlapsHole(0))
            ]
        );
    }

    #[test]
    fn malformed_and_non_finite_holes_are_dropped() {
        let mut nan = rect(1.0, 1.0, 2.0, 2.0);
        nan[1].x = f64::NAN;
        let z = zone(vec![rect(1.0, 1.0, 2.0, 2.0)[..2].to_vec(), nan]);
        assert_eq!(
            z.hole_problems(),
            [(0, HoleProblem::Malformed), (1, HoleProblem::NotFinite)]
        );
    }

    /// #901 review: a bow-tie hole has zero signed area; the GPU drew it
    /// solid while the CPU even-odd fill cut two triangles.
    #[test]
    fn a_self_intersecting_hole_is_dropped() {
        let p = |x, y| Point { x, y };
        let z = zone(vec![vec![
            p(2.0, 2.0),
            p(6.0, 6.0),
            p(6.0, 2.0),
            p(2.0, 6.0),
        ]]);
        assert_eq!(z.hole_problems(), [(0, HoleProblem::SelfIntersecting)]);
    }

    /// A grid of valid holes, the via-clearance case the bounding boxes keep
    /// cheap (#901 review), is kept whole.
    #[test]
    fn a_large_grid_of_holes_is_kept_whole() {
        let mut holes = Vec::new();
        for i in 0..32 {
            for j in 0..32 {
                let (x, y) = (1.0 + 3.0 * f64::from(i), 1.0 + 3.0 * f64::from(j));
                holes.push(rect(x, y, x + 1.0, y + 1.0));
            }
        }
        let mut z = zone(holes);
        z.outline = rect(0.0, 0.0, 100.0, 100.0);
        assert_eq!(z.usable_holes().len(), 1024);
    }
}
