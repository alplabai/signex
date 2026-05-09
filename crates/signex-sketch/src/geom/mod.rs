//! 2D computational-geometry primitives for the sketch crate.
//!
//! Submodules:
//! - [`predicates`] — orientation + signed area predicates with
//!   epsilon-aware sign returns.
//! - [`segment`] — segment×segment, segment×circle, segment×arc
//!   intersection. Drives the snap "Intersection" priority and
//!   the constraint solver's residual computation for tangency /
//!   intersection constraints.
//! - [`hull`] — convex hull, O(n log n) on the input size.
//! - [`triangulate`] — polygon triangulation for the sketch
//!   overlay's filled-loop renderer.
//!
//! Public entry point: re-exports below.

pub mod aabb_index;
pub mod boolean;
pub mod boolean_general;
pub mod curves;
pub mod halfedge;
pub mod hull;
pub mod offset;
pub mod polylabel;
pub mod predicates;
pub mod segment;
pub mod triangulate;

pub use aabb_index::{Aabb, AabbIndex};
pub use boolean::intersect_convex_clip;
pub use boolean_general::{polygon_op, BoolOp};
pub use curves::{arc_arc_intersections, arc_circle_intersections, circle_circle_intersections};
pub use hull::convex_hull;
pub use offset::{offset_polygon, CornerStyle};
pub use polylabel::pole_of_inaccessibility;
pub use predicates::{orient2d, signed_area, Sign};
pub use segment::{
    segment_arc_intersections, segment_circle_intersections, segment_segment_intersection,
    Arc2, Circle2, Segment2, SegmentIntersection,
};
pub use triangulate::ear_clip;

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
