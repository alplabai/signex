//! Polygons with holes in integer nm and the boolean operations on them:
//! union, difference, intersection and offset (#2216). Zone fills are built
//! from these.
//!
//! The operations run on the `i_overlay` crate's 64-bit integer engine, so
//! a result vertex is an exact nm value and no float rounding moves it. The
//! crate choice: `docs/DECISIONS.md` (2026-11-15, #2216).

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::{Overlay, ShapeType};
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::i_float::int::angle::Angle;
use i_overlay::i_float::int::point::IntPoint;
use i_overlay::i_shape::int::shape::{IntContour, IntShape};
use i_overlay::mesh::int::arc::ArcOptions;
use i_overlay::mesh::int::outline::offset::IntOutlineOffset;
use i_overlay::mesh::int::style::{IntLineJoin, IntOutlineStyle};

use crate::rect::RectNm;
use crate::shape_overlap::point_in_polygon;
use crate::units::PointNm;

/// Points a full turn of an offset's rounded corner is cut into (a power
/// of two).
pub const OFFSET_ARC_POINTS: u32 = 64;

/// One area of copper: an outer ring and the rings cut out of it.
///
/// A result of the operations below has its outer ring counter-clockwise
/// (positive area, y up), its holes clockwise, no closing vertex repeated
/// and every hole inside the outer ring. An input may wind either way.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Polygon {
    pub outer: Vec<PointNm>,
    pub holes: Vec<Vec<PointNm>>,
}

impl Polygon {
    pub fn new(outer: Vec<PointNm>, holes: Vec<Vec<PointNm>>) -> Self {
        Polygon { outer, holes }
    }

    /// The bounds of the outer ring; `None` for an empty one.
    pub fn bounds(&self) -> Option<RectNm> {
        RectNm::bounding(self.outer.iter().copied())
    }

    /// Whether `p` lies in or on the outer ring and in none of the holes. A
    /// point on a hole's edge counts as in the hole.
    pub fn contains_point(&self, p: PointNm) -> bool {
        self.outer.len() >= 3
            && point_in_polygon(&self.outer, p)
            && !self
                .holes
                .iter()
                .any(|hole| hole.len() >= 3 && point_in_polygon(hole, p))
    }

    /// Twice the area, exact: the outer ring's less the holes'.
    pub fn twice_area(&self) -> i128 {
        twice_signed_area(&self.outer).abs()
            - self
                .holes
                .iter()
                .map(|h| twice_signed_area(h).abs())
                .sum::<i128>()
    }

    /// The shape the overlay engine reads: outer ring counter-clockwise,
    /// holes clockwise, so a non-zero fill keeps a hole open even where it
    /// overlaps another polygon of the same operand.
    fn to_shape(&self) -> IntShape<i64> {
        std::iter::once(wound(&self.outer, true))
            .chain(self.holes.iter().map(|h| wound(h, false)))
            .collect()
    }

    fn from_shape(shape: IntShape<i64>) -> Self {
        let mut rings = shape
            .into_iter()
            .map(|ring| ring.into_iter().map(|p| PointNm::new(p.x, p.y)).collect());
        let outer = rings.next().unwrap_or_default();
        Polygon {
            outer,
            holes: rings.collect(),
        }
    }
}

/// Twice the signed area of a ring (positive counter-clockwise, y up).
pub fn twice_signed_area(ring: &[PointNm]) -> i128 {
    (0..ring.len())
        .map(|i| ring[i].cross(ring[(i + 1) % ring.len()]))
        .sum()
}

/// `ring` as engine points, turned to wind counter-clockwise when `ccw`,
/// clockwise otherwise.
fn wound(ring: &[PointNm], ccw: bool) -> IntContour<i64> {
    let mut points: IntContour<i64> = ring.iter().map(|p| IntPoint::new(p.x, p.y)).collect();
    if (twice_signed_area(ring) > 0) != ccw {
        points.reverse();
    }
    points
}

/// Every point in `a` or in `b`.
pub fn union(a: &[Polygon], b: &[Polygon]) -> Vec<Polygon> {
    overlay(a, b, OverlayRule::Union)
}

/// Every point in `a` and not inside `b`.
pub fn difference(a: &[Polygon], b: &[Polygon]) -> Vec<Polygon> {
    overlay(a, b, OverlayRule::Difference)
}

/// Every point in both `a` and `b`.
pub fn intersection(a: &[Polygon], b: &[Polygon]) -> Vec<Polygon> {
    overlay(a, b, OverlayRule::Intersect)
}

fn overlay(a: &[Polygon], b: &[Polygon], rule: OverlayRule) -> Vec<Polygon> {
    let mut engine = Overlay::<i64>::new(a.len() + b.len());
    for polygon in a {
        engine.add_source(&polygon.to_shape(), ShapeType::Subject);
    }
    for polygon in b {
        engine.add_source(&polygon.to_shape(), ShapeType::Clip);
    }
    engine
        .overlay(rule, FillRule::NonZero)
        .into_iter()
        .map(Polygon::from_shape)
        .collect()
}

/// `polygons` grown by `by` nm on every side (holes shrink), or shrunk for a
/// negative `by`; overlapping results merge. Corners grown outward are
/// rounded with [`OFFSET_ARC_POINTS`] points per turn, each on the true
/// circle, so the rounded edge lies at most [`chord_sag`]`(by)` inside it.
pub fn offset(polygons: &[Polygon], by: i64) -> Vec<Polygon> {
    let shapes: Vec<IntShape<i64>> = polygons.iter().map(Polygon::to_shape).collect();
    // An `Angle` counts a full turn as 2^32.
    let arc = ArcOptions {
        max_step: Angle::from_bits(1 << (32 - OFFSET_ARC_POINTS.trailing_zeros())),
        rotation_precision: 32,
    };
    let style = IntOutlineStyle::new(by).line_join(IntLineJoin::Round(arc));
    // `outline` builds without failing (only its optional range check can
    // refuse), and every board value lies within 1 km, far inside the
    // engine's range.
    shapes
        .outline(&style)
        .unwrap_or_default()
        .into_iter()
        .map(Polygon::from_shape)
        .collect()
}

/// How far a chord of an offset's rounded corner of radius `radius` falls
/// inside the true arc, rounded up to the nm.
pub fn chord_sag(radius: i64) -> i64 {
    let half_step = std::f64::consts::PI / f64::from(OFFSET_ARC_POINTS);
    (radius.max(0) as f64 * (1.0 - half_step.cos())).ceil() as i64
}

#[cfg(test)]
#[path = "region_tests.rs"]
mod tests;
