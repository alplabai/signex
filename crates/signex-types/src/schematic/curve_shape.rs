//! The curve and shape primitives shared by the schematic drawings and the
//! symbol body (#1267, #2838): ellipse, elliptical arc, round rectangle and
//! pie. One geometry for drawing, bounds and export, so the canvas, SVG, PDF
//! and the symbol editor agree on the shape. A cubic Bezier is
//! [`super::SchBezier`].
//!
//! Angles are geometric, in degrees: the direction from the centre to the
//! point, counter-clockwise (increasing) in model coordinates. An arc or pie
//! sweeps from `start_deg` to `end_deg` in that direction, wrapping through
//! a full turn when `end_deg < start_deg`.

use std::f64::consts::{FRAC_PI_2, TAU};

use serde::{Deserialize, Serialize};

use super::Point;
use super::drawing_bezier::points_bounds;

/// Segments a whole ellipse is flattened to.
pub const ELLIPSE_SEGMENTS: usize = 64;
/// Segments an elliptical arc is flattened to.
pub const ELLIPSE_ARC_SEGMENTS: usize = 48;
/// Segments a pie's arc edge is flattened to.
pub const PIE_ARC_SEGMENTS: usize = 32;
/// Segments each round-rectangle corner is flattened to.
pub const CORNER_SEGMENTS: usize = 8;
/// Below this a radius or a side has no visible size; such a shape is not
/// placed.
pub const MIN_CURVE_SIZE_MM: f64 = 0.01;
/// Two outline points this close are one point.
const SAME_POINT_MM: f64 = 1e-9;
/// How far a mapped axis may stray and still count as a plain move.
const TRANSLATION_TOLERANCE_MM: f64 = 1e-9;

/// One curve or shape primitive.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CurveShape {
    /// An axis-aligned ellipse; closed.
    Ellipse {
        center: Point,
        radius_x: f64,
        radius_y: f64,
    },
    /// An open arc of an axis-aligned ellipse.
    EllipticalArc {
        center: Point,
        radius_x: f64,
        radius_y: f64,
        start_deg: f64,
        end_deg: f64,
    },
    /// A rectangle with one corner radius, at most half its short side;
    /// closed. `from` is the minimum corner.
    RoundRect {
        from: Point,
        to: Point,
        corner_radius: f64,
    },
    /// A closed wedge: a circular arc joined to `center` by two edges.
    Pie {
        center: Point,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },
}

impl CurveShape {
    /// Corner radius a new round rectangle takes (DECISIONS 2026-11-22).
    pub const DEFAULT_CORNER_RADIUS_MM: f64 = 1.27;

    /// The stable name of the kind: the serde tag, and what a command's
    /// answer and a refusal call it.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Ellipse { .. } => "ellipse",
            Self::EllipticalArc { .. } => "elliptical_arc",
            Self::RoundRect { .. } => "round_rect",
            Self::Pie { .. } => "pie",
        }
    }

    /// Whether the outline is a closed ring, so it can take a fill.
    pub fn is_closed(&self) -> bool {
        !matches!(self, Self::EllipticalArc { .. })
    }

    /// A round rectangle on the corners `a` and `b`, with the default
    /// radius clamped to the shape.
    pub fn round_rect(a: Point, b: Point) -> Self {
        let from = Point::new(a.x.min(b.x), a.y.min(b.y));
        let to = Point::new(a.x.max(b.x), a.y.max(b.y));
        Self::RoundRect {
            from,
            to,
            corner_radius: Self::DEFAULT_CORNER_RADIUS_MM.min(max_corner_radius(from, to)),
        }
    }

    /// Hold a round rectangle's radius to half its short side, after
    /// anything that resized it. Other kinds are untouched.
    pub fn clamp_corner_radius(&mut self) {
        if let Self::RoundRect {
            from,
            to,
            corner_radius,
        } = self
        {
            *corner_radius = corner_radius.clamp(0.0, max_corner_radius(*from, *to));
        }
    }

    /// Every stored number is finite.
    pub fn is_finite(&self) -> bool {
        let numbers: Vec<f64> = match *self {
            Self::Ellipse {
                center,
                radius_x,
                radius_y,
            } => vec![center.x, center.y, radius_x, radius_y],
            Self::EllipticalArc {
                center,
                radius_x,
                radius_y,
                start_deg,
                end_deg,
            } => vec![center.x, center.y, radius_x, radius_y, start_deg, end_deg],
            Self::RoundRect {
                from,
                to,
                corner_radius,
            } => vec![from.x, from.y, to.x, to.y, corner_radius],
            Self::Pie {
                center,
                radius,
                start_deg,
                end_deg,
            } => vec![center.x, center.y, radius, start_deg, end_deg],
        };
        numbers.iter().all(|n| n.is_finite())
    }

    /// Too small to see, sweeping nothing, or not finite: not placed.
    pub fn is_degenerate(&self) -> bool {
        let small = |v: f64| v.is_nan() || v < MIN_CURVE_SIZE_MM;
        let no_sweep = |start: f64, end: f64| (end - start).rem_euclid(360.0) == 0.0;
        !self.is_finite()
            || match *self {
                Self::Ellipse {
                    radius_x, radius_y, ..
                } => small(radius_x) || small(radius_y),
                Self::EllipticalArc {
                    radius_x,
                    radius_y,
                    start_deg,
                    end_deg,
                    ..
                } => small(radius_x) || small(radius_y) || no_sweep(start_deg, end_deg),
                Self::RoundRect { from, to, .. } => {
                    small((to.x - from.x).abs()) || small((to.y - from.y).abs())
                }
                Self::Pie {
                    radius,
                    start_deg,
                    end_deg,
                    ..
                } => small(radius) || no_sweep(start_deg, end_deg),
            }
    }

    /// The points that place the shape: the centre, or a round rectangle's
    /// two corners. Moving them moves the shape; radii and angles are not
    /// points.
    pub fn anchor_points(&self) -> Vec<Point> {
        match self {
            Self::Ellipse { center, .. }
            | Self::EllipticalArc { center, .. }
            | Self::Pie { center, .. } => vec![*center],
            Self::RoundRect { from, to, .. } => vec![*from, *to],
        }
    }

    /// [`Self::anchor_points`], to move them.
    pub fn anchor_points_mut(&mut self) -> Vec<&mut Point> {
        match self {
            Self::Ellipse { center, .. }
            | Self::EllipticalArc { center, .. }
            | Self::Pie { center, .. } => vec![center],
            Self::RoundRect { from, to, .. } => vec![from, to],
        }
    }

    /// Map the shape through `f` (move, a quarter-turn rotation, mirror).
    /// The shape stays axis-aligned: a radius goes to the axis its image
    /// lies along, a mirror swaps an arc's ends so it still sweeps
    /// counter-clockwise, and a plain move keeps the angles exactly.
    pub fn map_points(&mut self, f: impl Fn(Point) -> Point) {
        match self {
            Self::RoundRect { from, to, .. } => {
                let (a, b) = (f(*from), f(*to));
                *from = Point::new(a.x.min(b.x), a.y.min(b.y));
                *to = Point::new(a.x.max(b.x), a.y.max(b.y));
            }
            Self::Ellipse {
                center,
                radius_x,
                radius_y,
            } => {
                let mapped = MappedAxes::of(&f, *center, *radius_x, *radius_y);
                (*center, *radius_x, *radius_y) = (mapped.center, mapped.radius_x, mapped.radius_y);
            }
            Self::EllipticalArc {
                center,
                radius_x,
                radius_y,
                start_deg,
                end_deg,
            } => {
                let mapped = MappedAxes::of(&f, *center, *radius_x, *radius_y);
                if !mapped.is_translation {
                    let ends = [*start_deg, *end_deg]
                        .map(|deg| f(point_on_ellipse(*center, *radius_x, *radius_y, deg)));
                    (*start_deg, *end_deg) = mapped.angles(ends);
                }
                (*center, *radius_x, *radius_y) = (mapped.center, mapped.radius_x, mapped.radius_y);
            }
            Self::Pie {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                let mapped = MappedAxes::of(&f, *center, *radius, *radius);
                if !mapped.is_translation {
                    let ends = [*start_deg, *end_deg]
                        .map(|deg| f(point_on_ellipse(*center, *radius, *radius, deg)));
                    (*start_deg, *end_deg) = mapped.angles(ends);
                }
                *center = mapped.center;
            }
        }
    }

    /// The outline: an open polyline for an elliptical arc, an implicitly
    /// closed ring (no repeated first point) for the others; a pie's ring
    /// starts at its centre.
    pub fn flatten(&self) -> Vec<Point> {
        match *self {
            Self::Ellipse {
                center,
                radius_x,
                radius_y,
            } => (0..ELLIPSE_SEGMENTS)
                .map(|i| {
                    let t = TAU * i as f64 / ELLIPSE_SEGMENTS as f64;
                    Point::new(center.x + radius_x * t.cos(), center.y + radius_y * t.sin())
                })
                .collect(),
            Self::EllipticalArc {
                center,
                radius_x,
                radius_y,
                start_deg,
                end_deg,
            } => elliptical_outline(center, radius_x, radius_y, start_deg, end_deg),
            Self::RoundRect {
                from,
                to,
                corner_radius,
            } => round_rect_outline(from, to, corner_radius),
            Self::Pie {
                center,
                radius,
                start_deg,
                end_deg,
            } => pie_outline(center, radius, start_deg, end_deg),
        }
    }

    /// The smallest box holding the outline.
    pub fn bounds(&self) -> (Point, Point) {
        points_bounds(&self.flatten())
    }
}

/// The largest corner radius a rectangle takes: half its short side.
pub fn max_corner_radius(from: Point, to: Point) -> f64 {
    (to.x - from.x).abs().min((to.y - from.y).abs()) * 0.5
}

/// Where the ray at the geometric angle `deg` from `center` meets the
/// ellipse.
pub fn point_on_ellipse(center: Point, radius_x: f64, radius_y: f64, deg: f64) -> Point {
    let t = ellipse_parameter(radius_x, radius_y, deg);
    Point::new(center.x + radius_x * t.cos(), center.y + radius_y * t.sin())
}

/// The geometric angle (degrees, `[0, 360)`) of `p` seen from `center`.
pub fn angle_deg(center: Point, p: Point) -> f64 {
    let deg = (p.y - center.y).atan2(p.x - center.x).to_degrees();
    deg.rem_euclid(360.0) + 0.0
}

/// The ellipse parameter `t` (point `(rx cos t, ry sin t)`) of the geometric
/// angle `deg`, so sampling in `t` stays even on a narrow ellipse.
fn ellipse_parameter(radius_x: f64, radius_y: f64, deg: f64) -> f64 {
    let (sin, cos) = deg.to_radians().sin_cos();
    (radius_x * sin).atan2(radius_y * cos)
}

/// An axis-aligned ellipse's centre and radii after a map.
struct MappedAxes {
    center: Point,
    radius_x: f64,
    radius_y: f64,
    /// The map flips orientation (a mirror).
    is_mirror: bool,
    /// The map only moves the shape.
    is_translation: bool,
}

impl MappedAxes {
    fn of(f: &impl Fn(Point) -> Point, center: Point, radius_x: f64, radius_y: f64) -> Self {
        let c = f(center);
        let along = |dx: f64, dy: f64| {
            let p = f(Point::new(center.x + dx, center.y + dy));
            (p.x - c.x, p.y - c.y)
        };
        let (ex, ey) = (along(radius_x, 0.0), along(0.0, radius_y));
        let near = |(x, y): (f64, f64), (wx, wy): (f64, f64)| {
            (x - wx).abs() <= TRANSLATION_TOLERANCE_MM && (y - wy).abs() <= TRANSLATION_TOLERANCE_MM
        };
        let is_translation = near(ex, (radius_x, 0.0)) && near(ey, (0.0, radius_y));
        if is_translation {
            return Self {
                center: c,
                radius_x,
                radius_y,
                is_mirror: false,
                is_translation,
            };
        }
        let (lx, ly) = (ex.0.hypot(ex.1), ey.0.hypot(ey.1));
        let (radius_x, radius_y) = if ex.0.abs() >= ex.1.abs() {
            (lx, ly)
        } else {
            (ly, lx)
        };
        Self {
            center: c,
            radius_x,
            radius_y,
            is_mirror: ex.0 * ey.1 - ex.1 * ey.0 < 0.0,
            is_translation,
        }
    }

    /// The `[start, end]` angles of an arc whose mapped end points are
    /// `ends`; swapped by a mirror so the arc still sweeps
    /// counter-clockwise over the same points.
    fn angles(&self, [start, end]: [Point; 2]) -> (f64, f64) {
        let (start, end) = (angle_deg(self.center, start), angle_deg(self.center, end));
        if self.is_mirror {
            (end, start)
        } else {
            (start, end)
        }
    }
}

/// The arc counter-clockwise from the geometric `start_deg` to `end_deg`.
fn elliptical_outline(
    center: Point,
    radius_x: f64,
    radius_y: f64,
    start_deg: f64,
    end_deg: f64,
) -> Vec<Point> {
    let t0 = ellipse_parameter(radius_x, radius_y, start_deg);
    let t1 = ellipse_parameter(radius_x, radius_y, end_deg);
    let sweep = (t1 - t0).rem_euclid(TAU);
    (0..=ELLIPSE_ARC_SEGMENTS)
        .map(|i| {
            let t = t0 + sweep * i as f64 / ELLIPSE_ARC_SEGMENTS as f64;
            Point::new(center.x + radius_x * t.cos(), center.y + radius_y * t.sin())
        })
        .collect()
}

/// The ring counter-clockwise from the top-right corner's arc. A radius past
/// half the short side (only a hand-edited file holds one) draws clamped.
/// Repeated points, as where two corners meet at the full radius, are
/// dropped.
fn round_rect_outline(from: Point, to: Point, radius: f64) -> Vec<Point> {
    let r = radius.clamp(0.0, max_corner_radius(from, to));
    let (x0, x1) = (from.x.min(to.x), from.x.max(to.x));
    let (y0, y1) = (from.y.min(to.y), from.y.max(to.y));
    let corners = [
        (x1 - r, y1 - r),
        (x0 + r, y1 - r),
        (x0 + r, y0 + r),
        (x1 - r, y0 + r),
    ];
    let mut ring: Vec<Point> = Vec::with_capacity(4 * (CORNER_SEGMENTS + 1));
    for (quarter, (cx, cy)) in corners.into_iter().enumerate() {
        for i in 0..=CORNER_SEGMENTS {
            let a = FRAC_PI_2 * (quarter as f64 + i as f64 / CORNER_SEGMENTS as f64);
            let p = Point::new(cx + r * a.cos(), cy + r * a.sin());
            if ring.last().is_none_or(|&q| !same_point(p, q)) {
                ring.push(p);
            }
        }
    }
    if ring.len() > 1 && same_point(ring[0], ring[ring.len() - 1]) {
        ring.pop();
    }
    ring
}

fn same_point(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() <= SAME_POINT_MM && (a.y - b.y).abs() <= SAME_POINT_MM
}

/// `center`, then the arc from `start_deg` counter-clockwise to `end_deg`.
fn pie_outline(center: Point, radius: f64, start_deg: f64, end_deg: f64) -> Vec<Point> {
    let sweep = (end_deg - start_deg).rem_euclid(360.0);
    let mut ring = Vec::with_capacity(PIE_ARC_SEGMENTS + 2);
    ring.push(center);
    ring.extend((0..=PIE_ARC_SEGMENTS).map(|i| {
        let rad = (start_deg + sweep * i as f64 / PIE_ARC_SEGMENTS as f64).to_radians();
        Point::new(center.x + radius * rad.cos(), center.y + radius * rad.sin())
    }));
    ring
}

/// The schematic tools that draw a [`CurveShape`] click by click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveTool {
    /// The centre, then a corner of the bounding box: both radii.
    Ellipse,
    /// Five clicks, as in the symbol editor: centre, X radius, Y radius,
    /// start, end (DECISIONS 2026-11-22).
    EllipticalArc,
    /// Two opposite corners, as the Rectangle.
    RoundRect,
}

impl CurveTool {
    /// How many clicks place the shape.
    pub const fn clicks(self) -> usize {
        match self {
            Self::Ellipse | Self::RoundRect => 2,
            Self::EllipticalArc => 5,
        }
    }

    /// The shape `clicks` draw with `cursor` standing in for every click
    /// still missing; `None` before the first. An elliptical arc shows its
    /// whole ellipse until the start click.
    pub fn shape(self, clicks: &[Point], cursor: Point) -> Option<CurveShape> {
        let center = *clicks.first()?;
        let at = |i: usize| clicks.get(i).copied().unwrap_or(cursor);
        Some(match self {
            Self::Ellipse => {
                let corner = at(1);
                CurveShape::Ellipse {
                    center,
                    radius_x: (corner.x - center.x).abs(),
                    radius_y: (corner.y - center.y).abs(),
                }
            }
            Self::RoundRect => CurveShape::round_rect(center, at(1)),
            Self::EllipticalArc => {
                let radius_x = (at(1).x - center.x).abs();
                let radius_y = if clicks.len() >= 2 {
                    (at(2).y - center.y).abs()
                } else {
                    radius_x
                };
                if clicks.len() < 4 {
                    return Some(CurveShape::Ellipse {
                        center,
                        radius_x,
                        radius_y,
                    });
                }
                CurveShape::EllipticalArc {
                    center,
                    radius_x,
                    radius_y,
                    start_deg: angle_deg(center, at(3)),
                    end_deg: angle_deg(center, at(4)),
                }
            }
        })
    }

    /// The shape to place once every click is in; `None` while one is
    /// missing or when the shape has no visible size.
    pub fn placed(self, clicks: &[Point]) -> Option<CurveShape> {
        let last = *clicks.get(self.clicks() - 1)?;
        self.shape(&clicks[..self.clicks()], last)
            .filter(|shape| !shape.is_degenerate())
    }
}
