//! Which of a zone's holes can be drawn as cut-outs (#897).
//!
//! `Zone.holes` documents its invariant: each hole is a ring of at least 3
//! vertices, lies strictly inside the outline, and is disjoint from every
//! other hole. The parser does not enforce it. The GPU fill and the CPU
//! even-odd fill only agree on holes that keep it, so both render paths draw
//! only [`Zone::usable_holes`], and the app reports the rest when a board is
//! opened.

use crate::pcb::{Point, Zone};

impl Zone {
    /// Indices into [`Zone::holes`] of the holes that keep the invariant,
    /// in order. A hole is dropped when it has fewer than 3 vertices, a
    /// non-finite coordinate, any part on or outside the outline, or any
    /// contact with a hole already kept (crossing, touching, or nested).
    pub fn usable_holes(&self) -> Vec<usize> {
        if self.outline.len() < 3 || !ring_is_finite(&self.outline) {
            return Vec::new();
        }
        let mut kept: Vec<usize> = Vec::new();
        for (index, hole) in self.holes.iter().enumerate() {
            let usable = hole.len() >= 3
                && ring_is_finite(hole)
                && strictly_inside(hole, &self.outline)
                && kept
                    .iter()
                    .all(|&other| rings_disjoint(hole, &self.holes[other]));
            if usable {
                kept.push(index);
            }
        }
        kept
    }
}

fn ring_is_finite(ring: &[Point]) -> bool {
    ring.iter().all(|p| p.x.is_finite() && p.y.is_finite())
}

/// Every vertex of `inner` is strictly inside `outer`, and no edge of one
/// touches or crosses an edge of the other.
fn strictly_inside(inner: &[Point], outer: &[Point]) -> bool {
    inner
        .iter()
        .all(|&p| point_in_ring(p, outer) && !on_ring(p, outer))
        && !rings_touch(inner, outer)
}

/// Neither ring touches, crosses, or contains the other.
fn rings_disjoint(a: &[Point], b: &[Point]) -> bool {
    !rings_touch(a, b) && !point_in_ring(a[0], b) && !point_in_ring(b[0], a)
}

fn edges(ring: &[Point]) -> impl Iterator<Item = (Point, Point)> + '_ {
    (0..ring.len()).map(move |i| (ring[i], ring[(i + 1) % ring.len()]))
}

fn rings_touch(a: &[Point], b: &[Point]) -> bool {
    edges(a).any(|(p, q)| edges(b).any(|(r, s)| segments_meet(p, q, r, s)))
}

fn on_ring(p: Point, ring: &[Point]) -> bool {
    edges(ring).any(|(a, b)| orient(a, b, p) == 0.0 && within_box(a, b, p))
}

/// Even-odd ray cast; points on an edge count as either, so callers that
/// need "strictly" also test [`on_ring`].
fn point_in_ring(p: Point, ring: &[Point]) -> bool {
    let mut inside = false;
    for (a, b) in edges(ring) {
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
    }

    #[test]
    fn malformed_and_non_finite_holes_are_dropped() {
        let mut nan = rect(1.0, 1.0, 2.0, 2.0);
        nan[1].x = f64::NAN;
        let z = zone(vec![rect(1.0, 1.0, 2.0, 2.0)[..2].to_vec(), nan]);
        assert!(z.usable_holes().is_empty());
    }
}
