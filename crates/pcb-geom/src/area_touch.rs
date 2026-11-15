//! Whether copper meets an area with holes (#2216): the join test the
//! ratsnest runs against a zone's copper, a fill piece or a solid zone.
//!
//! An area's holes are open: copper wholly inside a hole, touching none of
//! its edges, does not meet the area. A hole's edge is the area's copper,
//! so touching it counts. Integer nm, like the tests it builds on.

use crate::rect::RectNm;
use crate::region::Polygon;
use crate::shape::{Shape, segments_intersect};
use crate::shape_overlap::{point_in_polygon, polygons_meet};
use crate::units::PointNm;

/// Whether `shape` shares a point with `area`.
pub(crate) fn shape_meets_area(shape: &Shape, area: &Polygon) -> bool {
    if !shape.overlaps_polygon(&area.outer) {
        return false;
    }
    let bounds = shape.aabb();
    !area
        .holes
        .iter()
        .any(|hole| shape_inside_hole(shape, &bounds, hole))
}

/// Whether two areas share a point.
pub(crate) fn areas_meet(a: &Polygon, b: &Polygon) -> bool {
    polygons_meet(&a.outer, &b.outer)
        && !b.holes.iter().any(|hole| ring_inside_hole(&a.outer, hole))
        && !a.holes.iter().any(|hole| ring_inside_hole(&b.outer, hole))
}

/// Whether `shape` (bounded by `bounds`) lies wholly inside the open hole.
fn shape_inside_hole(shape: &Shape, bounds: &RectNm, hole: &[PointNm]) -> bool {
    if hole.len() < 3
        || !RectNm::bounding(hole.iter().copied()).is_some_and(|r| r.contains_rect(bounds))
    {
        return false;
    }
    let edge_touched = (0..hole.len()).any(|i| {
        shape.overlaps(&Shape::Capsule {
            a: hole[i],
            b: hole[(i + 1) % hole.len()],
            radius: 0,
        })
    });
    !edge_touched && shape.overlaps_polygon(hole)
}

/// Whether the ring `ring` lies wholly inside the open hole: no edge of it
/// meets an edge of the hole and one of its corners is inside.
fn ring_inside_hole(ring: &[PointNm], hole: &[PointNm]) -> bool {
    let (Some(&first), Some(within)) = (ring.first(), RectNm::bounding(ring.iter().copied()))
    else {
        return false;
    };
    if hole.len() < 3
        || !RectNm::bounding(hole.iter().copied()).is_some_and(|r| r.contains_rect(&within))
    {
        return false;
    }
    let edge = |p: &[PointNm], i: usize| (p[i], p[(i + 1) % p.len()]);
    let crossed = (0..ring.len()).any(|i| {
        let (a, b) = edge(ring, i);
        (0..hole.len()).any(|j| {
            let (c, d) = edge(hole, j);
            segments_intersect(a, b, c, d)
        })
    });
    !crossed && point_in_polygon(hole, first)
}

#[cfg(test)]
#[path = "area_touch_tests.rs"]
mod tests;
