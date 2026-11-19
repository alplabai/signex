//! A cubic Bezier annotation curve on a schematic sheet (#1267).
//!
//! Annotation only: it never joins a net and ERC never reads it. Drawing,
//! hit test and export all flatten it with [`SchBezier::flatten`], so the
//! canvas, SVG and PDF agree on the shape.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Point, StrokeColor};
use crate::net_class::NetLineStyle;

/// Segments a Bezier is flattened to for drawing, hit test and export.
pub const BEZIER_FLATTEN_SEGMENTS: usize = 32;

/// Arrowhead length at a drawing's line end (mm).
pub const LINE_END_ARROW_LENGTH_MM: f64 = 1.27;

/// Arrowhead half width at a drawing's line end (mm).
pub const LINE_END_ARROW_HALF_WIDTH_MM: f64 = 0.45;

/// Which ends of an open curve carry an arrowhead.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineEnds {
    #[default]
    None,
    Start,
    End,
    Both,
}

impl LineEnds {
    pub const ALL: [Self; 4] = [Self::None, Self::Start, Self::End, Self::Both];

    /// `skip_serializing_if` predicate: no arrowheads is not written.
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn at_start(self) -> bool {
        matches!(self, Self::Start | Self::Both)
    }

    pub fn at_end(self) -> bool {
        matches!(self, Self::End | Self::Both)
    }
}

/// A cubic Bezier from `start` to `end`, bent by the control points `c1`
/// (pulls the start) and `c2` (pulls the end).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchBezier {
    pub uuid: Uuid,
    pub start: Point,
    pub c1: Point,
    pub c2: Point,
    pub end: Point,
    /// Stroke width in mm; `0` draws the sheet's default drawing width.
    #[serde(default)]
    pub width: f64,
    /// `None` draws in the sheet's drawing colour.
    #[serde(default)]
    pub stroke_color: Option<StrokeColor>,
    #[serde(default, skip_serializing_if = "is_solid")]
    pub line_style: NetLineStyle,
    #[serde(default, skip_serializing_if = "LineEnds::is_none")]
    pub ends: LineEnds,
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
}

fn is_solid(style: &NetLineStyle) -> bool {
    matches!(style, NetLineStyle::Solid)
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl SchBezier {
    /// A solid curve in the sheet's drawing colour, default width, no
    /// arrowheads.
    pub fn new(uuid: Uuid, start: Point, c1: Point, c2: Point, end: Point) -> Self {
        Self {
            uuid,
            start,
            c1,
            c2,
            end,
            width: 0.0,
            stroke_color: None,
            line_style: NetLineStyle::Solid,
            ends: LineEnds::None,
            locked: false,
        }
    }

    /// Start, C1, C2, end, in that order.
    pub fn points(&self) -> [Point; 4] {
        [self.start, self.c1, self.c2, self.end]
    }

    /// The four points, to move them.
    pub fn points_mut(&mut self) -> [&mut Point; 4] {
        [&mut self.start, &mut self.c1, &mut self.c2, &mut self.end]
    }

    /// Map every point through `f` (move, rotate, mirror).
    pub fn map_points(&mut self, f: impl Fn(Point) -> Point) {
        for p in self.points_mut() {
            *p = f(*p);
        }
    }

    /// The curve at parameter `t` in `[0, 1]`.
    pub fn point_at(&self, t: f64) -> Point {
        let u = 1.0 - t;
        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        Point::new(
            a * self.start.x + b * self.c1.x + c * self.c2.x + d * self.end.x,
            a * self.start.y + b * self.c1.y + c * self.c2.y + d * self.end.y,
        )
    }

    /// The curve as [`BEZIER_FLATTEN_SEGMENTS`] straight segments
    /// (`BEZIER_FLATTEN_SEGMENTS + 1` points, both ends exact).
    pub fn flatten(&self) -> Vec<Point> {
        let n = BEZIER_FLATTEN_SEGMENTS;
        let mut out: Vec<Point> = (0..n).map(|i| self.point_at(i as f64 / n as f64)).collect();
        out.push(self.end);
        out
    }

    /// The smallest box holding the curve (its flattened points).
    pub fn bounds(&self) -> (Point, Point) {
        points_bounds(&self.flatten())
    }

    /// Distance from `p` to the curve (to its flattened segments).
    pub fn distance_to(&self, p: Point) -> f64 {
        self.flatten()
            .windows(2)
            .map(|w| segment_distance(p, w[0], w[1]))
            .fold(f64::INFINITY, f64::min)
    }

    /// The arrowheads this curve's [`LineEnds`] asks for, each as
    /// `[tip, wing, wing]`. A head points along the curve's end tangent,
    /// taken from its control point (or the far end when the two coincide).
    pub fn arrow_heads(&self) -> Vec<[Point; 3]> {
        let mut heads = Vec::new();
        if self.ends.at_start() {
            let from = first_distinct(self.start, &[self.c1, self.c2, self.end]);
            heads.extend(from.map(|from| arrow_head(from, self.start)));
        }
        if self.ends.at_end() {
            let from = first_distinct(self.end, &[self.c2, self.c1, self.start]);
            heads.extend(from.map(|from| arrow_head(from, self.end)));
        }
        heads
    }
}

fn first_distinct(at: Point, candidates: &[Point]) -> Option<Point> {
    candidates
        .iter()
        .copied()
        .find(|c| (c.x - at.x).hypot(c.y - at.y) > 1e-9)
}

/// A filled arrowhead `[tip, wing, wing]` with its tip at `tip`, pointing
/// away from `from`.
pub fn arrow_head(from: Point, tip: Point) -> [Point; 3] {
    let (dx, dy) = (tip.x - from.x, tip.y - from.y);
    let len = dx.hypot(dy).max(1e-12);
    let (ux, uy) = (dx / len, dy / len);
    let base = Point::new(
        tip.x - ux * LINE_END_ARROW_LENGTH_MM,
        tip.y - uy * LINE_END_ARROW_LENGTH_MM,
    );
    let (nx, ny) = (
        -uy * LINE_END_ARROW_HALF_WIDTH_MM,
        ux * LINE_END_ARROW_HALF_WIDTH_MM,
    );
    [
        tip,
        Point::new(base.x + nx, base.y + ny),
        Point::new(base.x - nx, base.y - ny),
    ]
}

/// Smallest `(min, max)` box over `points`; the origin for none.
pub fn points_bounds(points: &[Point]) -> (Point, Point) {
    let Some(first) = points.first() else {
        return (Point::ZERO, Point::ZERO);
    };
    points.iter().fold((*first, *first), |(lo, hi), p| {
        (
            Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    })
}

/// Distance from `p` to the segment `a`-`b`.
pub fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= 1e-18 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
}
