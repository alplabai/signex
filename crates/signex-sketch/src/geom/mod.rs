//! 2D computational-geometry primitives for the sketch crate.
//!
//! Submodules:
//! - [`predicates`] — orientation + signed area predicates with
//!   epsilon-aware sign returns.
//! - [`segment`] — segment×segment, segment×circle, segment×arc
//!   intersection. Drives the snap "Intersection" priority and
//!   the constraint solver's residual computation for tangency /
//!   intersection constraints.
//! - [`triangulate`] — polygon triangulation for the sketch
//!   overlay's filled-loop renderer.
//!
//! Public entry point: re-exports below.

pub mod aabb_index;
pub mod boolean_general;
pub mod curves;
mod hole_ring;
mod holes;
#[cfg(test)]
mod holes_reference_tests;
#[cfg(test)]
mod holes_touching_tests;
pub mod offset;
pub mod point;
mod point_grid;
pub mod predicates;
pub mod segment;
mod tolerant_clip;
pub mod triangulate;

pub use aabb_index::{Aabb, AabbIndex};
pub use boolean_general::{BoolOp, PolygonOpError, polygon_op};
pub use curves::{arc_arc_intersections, arc_circle_intersections, circle_circle_intersections};
pub use offset::{CornerStyle, offset_polygon};
pub use point::{point_in_polygon, point_to_segment_distance, point_to_segment_distance_sq};
pub use predicates::{Sign, orient2d, signed_area};
pub use segment::{
    Arc2, Circle2, Segment2, SegmentIntersection, segment_arc_intersections,
    segment_circle_intersections, segment_segment_intersection,
};
pub use triangulate::{ear_clip, ear_clip_with_holes};

/// Plain-old 2D point in plane-local mm. The crate's `EntityKind::Point`
/// uses bare `(x, y): f64` fields; this struct lets the geom helpers
/// take an idiomatic struct argument while staying convertible from
/// tuples and arrays so existing call sites can pass either.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance_sq(self, other: Point2) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    pub fn distance(self, other: Point2) -> f64 {
        self.distance_sq(other).sqrt()
    }
}

impl From<(f64, f64)> for Point2 {
    fn from((x, y): (f64, f64)) -> Self {
        Self { x, y }
    }
}

impl From<Point2> for (f64, f64) {
    fn from(p: Point2) -> Self {
        (p.x, p.y)
    }
}

impl From<[f64; 2]> for Point2 {
    fn from([x, y]: [f64; 2]) -> Self {
        Self { x, y }
    }
}

impl From<Point2> for [f64; 2] {
    fn from(p: Point2) -> Self {
        [p.x, p.y]
    }
}
