//! Copper outlines in integer nm, with their bounds and hit predicates.
//!
//! Distances and orientations are exact `i128` arithmetic on nm values.
//! Two steps take an `f64` and round back to the nearest nm: turning a
//! point into a rotated pad's frame, and the foot of a perpendicular on a
//! track. The error is under 1 nm, below what the file stores.

use crate::rect::RectNm;
use crate::transform::{rotate, sin_cos_deg};
use crate::units::{GeomError, MAX_ABS_NM, PointNm};

/// One item's copper on the board.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// A round pad or a via.
    Disc { center: PointNm, radius: i64 },
    /// A track: every point within `radius` of the segment `a`–`b`.
    Capsule { a: PointNm, b: PointNm, radius: i64 },
    /// A rectangle turned `rotation_deg` counter-clockwise about its centre.
    Rect {
        center: PointNm,
        half_w: i64,
        half_h: i64,
        rotation_deg: f64,
    },
    /// A rectangle with quarter-circle corners of `radius` (at most the
    /// shorter half side), turned `rotation_deg` counter-clockwise about
    /// its centre.
    RoundRect {
        center: PointNm,
        half_w: i64,
        half_h: i64,
        radius: i64,
        rotation_deg: f64,
    },
}

impl Shape {
    /// Refuse a shape with a coordinate or length beyond [`MAX_ABS_NM`], the
    /// bound the `i128` arithmetic relies on.
    pub fn check_limits(&self) -> Result<(), GeomError> {
        let ok = |v: i64| v.unsigned_abs() <= MAX_ABS_NM.unsigned_abs();
        let point = |p: PointNm| ok(p.x) && ok(p.y);
        let within = match *self {
            Shape::Disc { center, radius } => point(center) && ok(radius),
            Shape::Capsule { a, b, radius } => point(a) && point(b) && ok(radius),
            Shape::Rect {
                center,
                half_w,
                half_h,
                ..
            }
            | Shape::RoundRect {
                center,
                half_w,
                half_h,
                ..
            } => point(center) && ok(half_w) && ok(half_h),
        };
        if within {
            Ok(())
        } else {
            Err(GeomError::Unrepresentable("shape"))
        }
    }

    /// The smallest axis-aligned rectangle holding the shape (rounded out).
    pub fn aabb(&self) -> RectNm {
        match *self {
            Shape::Disc { center, radius } => RectNm::around(center, radius, radius),
            Shape::Capsule { a, b, radius } => RectNm::new(a, b).inflate(radius),
            Shape::Rect { .. } => {
                let [c0, c1, c2, c3] = self.box_corners();
                RectNm::new(c0, c1).union(&RectNm::new(c2, c3))
            }
            // The inset rectangle grown by the corner radius is the exact
            // extent, however the pad is turned.
            Shape::RoundRect {
                center,
                half_w,
                half_h,
                radius,
                rotation_deg,
            } => Shape::Rect {
                center,
                half_w: half_w - radius,
                half_h: half_h - radius,
                rotation_deg,
            }
            .aabb()
            .inflate(radius),
        }
    }

    /// Whether `p` lies on the shape or within `tolerance` of it (a
    /// negative tolerance counts as 0). A rectangle grows by `tolerance`
    /// on each side, corners kept square.
    pub fn contains(&self, p: PointNm, tolerance: i64) -> bool {
        let tol = tolerance.max(0);
        match *self {
            Shape::Disc { center, radius } => within(p.distance_sq(center), radius + tol),
            Shape::Capsule { a, b, radius } => {
                within(p.distance_sq(closest_on_segment(a, b, p)), radius + tol)
            }
            Shape::Rect {
                center,
                half_w,
                half_h,
                rotation_deg,
            } => {
                let local = to_frame(p, center, rotation_deg);
                local.x.abs() <= half_w + tol && local.y.abs() <= half_h + tol
            }
            Shape::RoundRect {
                center,
                half_w,
                half_h,
                radius,
                rotation_deg,
            } => {
                let local = to_frame(p, center, rotation_deg);
                // Past the straight sides in both axes the corner arc rules.
                let dx = local.x.abs() - (half_w - radius);
                let dy = local.y.abs() - (half_h - radius);
                if dx > 0 && dy > 0 {
                    within(
                        i128::from(dx) * i128::from(dx) + i128::from(dy) * i128::from(dy),
                        radius + tol,
                    )
                } else {
                    local.x.abs() <= half_w + tol && local.y.abs() <= half_h + tol
                }
            }
        }
    }

    /// Whether the shape shares a point with `rect`. Exact integer nm for
    /// every shape, a rounded rectangle included.
    pub fn touches_rect(&self, rect: &RectNm) -> bool {
        match *self {
            Shape::Disc { center, radius } => {
                within(rect.clamp(center).distance_sq(center), radius)
            }
            Shape::Capsule { a, b, radius } => capsule_touches_rect(a, b, radius, rect),
            Shape::Rect { .. } => polygons_overlap(&self.box_corners(), &rect.corners()),
            Shape::RoundRect { .. } => self.overlaps_polygon(&rect.corners()),
        }
    }

    /// The outline of a rectangle (4 corners) or a rounded rectangle
    /// (`steps + 1` points per corner arc), counter-clockwise; empty for a
    /// disc or a track. Drawing a pad uses [`ring`](Self::ring), which
    /// covers every shape.
    pub fn outline(&self, steps: usize) -> Vec<PointNm> {
        match *self {
            Shape::Rect { .. } => self.box_corners().to_vec(),
            Shape::RoundRect {
                center,
                half_w,
                half_h,
                radius,
                rotation_deg,
            } => {
                let (sin, cos) = sin_cos_deg(rotation_deg);
                let steps = steps.max(2);
                // Corner arcs counter-clockwise from the +x,+y corner.
                let mut pts = [(1, 1, 0.0), (-1, 1, 0.25), (-1, -1, 0.5), (1, -1, 0.75)]
                    .into_iter()
                    .flat_map(|(sx, sy, turn)| {
                        (0..=steps).map(move |i| {
                            let t = std::f64::consts::TAU * (turn + 0.25 * i as f64 / steps as f64);
                            PointNm::new(
                                sx * (half_w - radius) + (radius as f64 * t.cos()).round() as i64,
                                sy * (half_h - radius) + (radius as f64 * t.sin()).round() as i64,
                            )
                        })
                    })
                    .map(|local| rotate(local, sin, cos).plus(center))
                    .collect::<Vec<_>>();
                // Where one arc meets the next (ratio 0.5), the points repeat.
                pts.dedup();
                if pts.len() > 1 && pts.first() == pts.last() {
                    pts.pop();
                }
                pts
            }
            Shape::Disc { .. } | Shape::Capsule { .. } => Vec::new(),
        }
    }

    /// A closed counter-clockwise ring of any shape, for drawing a pad:
    /// `steps` points per quarter turn of a round part.
    pub fn ring(&self, steps: usize) -> Vec<PointNm> {
        let steps = steps.max(2);
        let arc = |end: PointNm, radius: i64, from: f64, count: usize| {
            (0..=count).map(move |i| {
                let t = from + std::f64::consts::PI * i as f64 / count as f64;
                PointNm::new(
                    end.x + (radius as f64 * t.cos()).round() as i64,
                    end.y + (radius as f64 * t.sin()).round() as i64,
                )
            })
        };
        match *self {
            Shape::Rect { .. } | Shape::RoundRect { .. } => self.outline(steps),
            Shape::Disc { center, radius } => arc(center, radius, 0.0, 2 * steps)
                .take(2 * steps)
                .chain(arc(center, radius, std::f64::consts::PI, 2 * steps).take(2 * steps))
                .collect(),
            Shape::Capsule { a, b, radius } => {
                let axis = ((b.y - a.y) as f64).atan2((b.x - a.x) as f64);
                let half = std::f64::consts::FRAC_PI_2;
                arc(b, radius, axis - half, 2 * steps)
                    .chain(arc(a, radius, axis + half, 2 * steps))
                    .collect()
            }
        }
    }

    /// The corners of a rectangle or of a rounded rectangle's frame box,
    /// counter-clockwise; for other shapes, of the bounding box.
    fn box_corners(&self) -> [PointNm; 4] {
        match *self {
            Shape::Rect {
                center,
                half_w,
                half_h,
                rotation_deg,
            }
            | Shape::RoundRect {
                center,
                half_w,
                half_h,
                rotation_deg,
                ..
            } => {
                let (sin, cos) = sin_cos_deg(rotation_deg);
                [(-1, -1), (1, -1), (1, 1), (-1, 1)].map(|(sx, sy)| {
                    rotate(PointNm::new(sx * half_w, sy * half_h), sin, cos).plus(center)
                })
            }
            Shape::Disc { .. } | Shape::Capsule { .. } => self.aabb().corners(),
        }
    }
}

/// Whether a squared distance is at most `limit` (negative limits as 0).
pub(crate) fn within(distance_sq: i128, limit: i64) -> bool {
    let limit = i128::from(limit.max(0));
    distance_sq <= limit * limit
}

/// `p` in the frame of a shape centred on `center`, turned `rotation_deg`.
fn to_frame(p: PointNm, center: PointNm, rotation_deg: f64) -> PointNm {
    let (sin, cos) = sin_cos_deg(rotation_deg);
    rotate(p.minus(center), -sin, cos)
}

/// The point of segment `a`–`b` nearest to `p`.
pub(crate) fn closest_on_segment(a: PointNm, b: PointNm, p: PointNm) -> PointNm {
    let ab = b.minus(a);
    let len_sq = ab.dot(ab);
    let along = p.minus(a).dot(ab);
    if len_sq == 0 || along <= 0 {
        return a;
    }
    if along >= len_sq {
        return b;
    }
    let t = along as f64 / len_sq as f64;
    PointNm::new(
        a.x + (ab.x as f64 * t).round() as i64,
        a.y + (ab.y as f64 * t).round() as i64,
    )
}

fn capsule_touches_rect(a: PointNm, b: PointNm, radius: i64, rect: &RectNm) -> bool {
    if rect.contains_point(a) || rect.contains_point(b) {
        return true;
    }
    let corners = rect.corners();
    let crosses_edge = (0..4).any(|i| segments_intersect(a, b, corners[i], corners[(i + 1) % 4]));
    if crosses_edge {
        return true;
    }
    // Disjoint: the gap is between an endpoint and the rect, or between a
    // rect corner and the segment.
    within(rect.clamp(a).distance_sq(a), radius)
        || within(rect.clamp(b).distance_sq(b), radius)
        || corners
            .iter()
            .any(|&c| within(c.distance_sq(closest_on_segment(a, b, c)), radius))
}

/// Sign of the turn `a` → `b` → `c`: 1 left, -1 right, 0 collinear.
pub(crate) fn orientation(a: PointNm, b: PointNm, c: PointNm) -> i8 {
    match b.minus(a).cross(c.minus(a)) {
        0 => 0,
        v if v > 0 => 1,
        _ => -1,
    }
}

fn on_segment(a: PointNm, b: PointNm, p: PointNm) -> bool {
    RectNm::new(a, b).contains_point(p)
}

/// Whether closed segments `p1`–`p2` and `q1`–`q2` share a point.
pub(crate) fn segments_intersect(p1: PointNm, p2: PointNm, q1: PointNm, q2: PointNm) -> bool {
    let (d1, d2) = (orientation(q1, q2, p1), orientation(q1, q2, p2));
    let (d3, d4) = (orientation(p1, p2, q1), orientation(p1, p2, q2));
    if d1 * d2 < 0 && d3 * d4 < 0 {
        return true;
    }
    (d1 == 0 && on_segment(q1, q2, p1))
        || (d2 == 0 && on_segment(q1, q2, p2))
        || (d3 == 0 && on_segment(p1, p2, q1))
        || (d4 == 0 && on_segment(p1, p2, q2))
}

/// Separating-axis test for two convex quads; touching counts as overlap.
fn polygons_overlap(a: &[PointNm; 4], b: &[PointNm; 4]) -> bool {
    !(separated_by_edges_of(a, b) || separated_by_edges_of(b, a))
}

fn separated_by_edges_of(poly: &[PointNm; 4], other: &[PointNm; 4]) -> bool {
    (0..4).any(|i| {
        let edge = poly[(i + 1) % 4].minus(poly[i]);
        let normal = PointNm::new(-edge.y, edge.x);
        if normal == PointNm::ORIGIN {
            return false;
        }
        let project = |pts: &[PointNm; 4]| {
            let dots = pts.map(|p| p.dot(normal));
            (
                dots.iter().copied().min().unwrap_or(0),
                dots.iter().copied().max().unwrap_or(0),
            )
        };
        let (a_min, a_max) = project(poly);
        let (b_min, b_max) = project(other);
        a_max < b_min || b_max < a_min
    })
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod tests;
