//! Pour Cutout and Slice Pour (#2216): the shapes a polygon pour's outline
//! becomes when an area is cut out of it or a line slices it. Integer nm on
//! the region operations; the board commands that write the shapes are
//! `signex_engine::pcb::pour_edit`.

use signex_types::pcb::{Point, Zone};

use crate::area_touch::shape_meets_area;
use crate::rect::RectNm;
use crate::region::{Polygon, difference, intersection};
use crate::shape::Shape;
use crate::units::PointNm;

/// One area in board mm: an outline and the holes cut out of it.
#[derive(Debug, Clone, PartialEq)]
pub struct AreaShape {
    pub outline: Vec<Point>,
    pub holes: Vec<Vec<Point>>,
}

impl AreaShape {
    fn of(polygon: &Polygon) -> Self {
        let ring = |r: &[PointNm]| r.iter().map(|p| p.to_mm()).collect::<Vec<_>>();
        AreaShape {
            outline: ring(&polygon.outer),
            holes: polygon.holes.iter().map(|h| ring(h)).collect(),
        }
    }
}

/// What is left of `zone`'s area once `cutout` is taken out of it, largest
/// first: one shape with a new hole or notch, several when the cutout cuts
/// it in pieces, none when it covers the whole area. `None` when the cutout
/// does not overlap the area (nothing changes), or a ring has fewer than
/// three points or a point the geometry cannot hold.
pub fn cut_area(zone: &Zone, cutout: &[Point]) -> Option<Vec<AreaShape>> {
    let area = whole_area(zone)?;
    let cut = Polygon::new(ring(cutout).filter(|r| r.len() >= 3)?, Vec::new());
    let (area, cut) = (std::slice::from_ref(&area), std::slice::from_ref(&cut));
    let overlap: i128 = intersection(area, cut)
        .iter()
        .map(Polygon::twice_area)
        .sum();
    if overlap == 0 {
        return None;
    }
    Some(largest_first(difference(area, cut)))
}

/// `zone`'s area sliced along the line through `a` and `b`, largest piece
/// first. `None` when `a` and `b` are one point, the segment between them
/// misses the area, or the line leaves the area in one piece.
pub fn slice_area(zone: &Zone, a: Point, b: Point) -> Option<Vec<AreaShape>> {
    let area = whole_area(zone)?;
    let a = PointNm::from_mm(a, "slice.start").ok()?;
    let b = PointNm::from_mm(b, "slice.end").ok()?;
    if a == b || !shape_meets_area(&Shape::Capsule { a, b, radius: 0 }, &area) {
        return None;
    }
    let (left, right) = half_planes(a, b, &area.bounds()?);
    let area = std::slice::from_ref(&area);
    let mut pieces = intersection(area, &[left]);
    pieces.extend(intersection(area, &[right]));
    let shapes = largest_first(pieces);
    (shapes.len() >= 2).then_some(shapes)
}

/// The zone's outline less every hole of three or more points. Unlike the
/// fill's [`crate::zone_area`], a hole that breaks the hole rules is kept,
/// so an edit does not drop it from the saved shape.
fn whole_area(zone: &Zone) -> Option<Polygon> {
    let outer = ring(&zone.outline).filter(|r| r.len() >= 3)?;
    let holes = zone
        .holes
        .iter()
        .filter(|h| h.len() >= 3)
        .map(|h| ring(h))
        .collect::<Option<Vec<_>>>()?;
    Some(Polygon::new(outer, holes))
}

fn ring(points: &[Point]) -> Option<Vec<PointNm>> {
    points
        .iter()
        .map(|&p| PointNm::from_mm(p, "pour edit"))
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

/// The two sides of the line through `a` and `b`, each a rectangle reaching
/// past every corner of `bounds`. Both share the two points on the line, so
/// the pieces they cut meet exactly along it.
fn half_planes(a: PointNm, b: PointNm, bounds: &RectNm) -> (Polygon, Polygon) {
    let (dx, dy) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
    let length = dx.hypot(dy);
    let (ux, uy) = (dx / length, dy / length);
    let (vx, vy) = (-uy, ux);
    let reach = bounds
        .corners()
        .iter()
        .map(|c| ((c.x - a.x) as f64).hypot((c.y - a.y) as f64))
        .fold(0.0_f64, f64::max)
        + 1.0e6;
    let at = |along: f64, across: f64| {
        PointNm::new(
            (a.x as f64 + ux * along + vx * across).round() as i64,
            (a.y as f64 + uy * along + vy * across).round() as i64,
        )
    };
    let (p, q) = (at(-reach, 0.0), at(reach, 0.0));
    let left = vec![p, q, at(reach, reach), at(-reach, reach)];
    let right = vec![p, at(-reach, -reach), at(reach, -reach), q];
    (
        Polygon::new(left, Vec::new()),
        Polygon::new(right, Vec::new()),
    )
}

/// `pieces` as shapes, the largest first; equal areas in the order of their
/// first outline point, so the result does not depend on the engine's.
fn largest_first(mut pieces: Vec<Polygon>) -> Vec<AreaShape> {
    pieces.retain(|p| p.outer.len() >= 3 && p.twice_area() > 0);
    pieces.sort_by(|p, q| {
        q.twice_area()
            .cmp(&p.twice_area())
            .then_with(|| p.outer.iter().min().cmp(&q.outer.iter().min()))
    });
    pieces.iter().map(AreaShape::of).collect()
}

#[cfg(test)]
#[path = "pour_edit_tests.rs"]
mod tests;
