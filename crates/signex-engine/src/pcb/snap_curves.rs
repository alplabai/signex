//! The curves of board objects snapping runs along (#2879): straight edges
//! and arcs, the point of one nearest the pointer, and where two cross
//! (line-line, line-arc, arc-arc).

use std::f64::consts::TAU;

use pcb_geom::{GraphicShape, PointNm, nm_to_mm};
use signex_types::pcb::Point;
use signex_types::pcb_arc::ArcSweep;

/// Slack on the ends of a segment or an arc when a crossing is tested.
const END_SLACK: f64 = 1e-9;

/// One straight edge or arc of a board object, in mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Curve {
    Segment(Point, Point),
    /// An arc; a circle sweeps a whole turn.
    Arc(ArcSweep),
}

impl Curve {
    /// A whole circle; `None` for a radius not above zero.
    pub(super) fn circle(center: Point, radius: f64) -> Option<Curve> {
        (radius.is_finite() && radius > 0.0).then_some(Curve::Arc(ArcSweep {
            center,
            radius,
            start_angle: 0.0,
            sweep: TAU,
        }))
    }

    /// The point of this curve nearest `p`.
    pub(super) fn nearest(&self, p: Point) -> Point {
        match *self {
            Curve::Segment(a, b) => nearest_on_segment(a, b, p),
            Curve::Arc(arc) => nearest_on_sweep(&arc, p),
        }
    }
}

/// The edges of the closed ring `points`.
pub(super) fn ring_edges(points: &[Point]) -> impl Iterator<Item = Curve> + '_ {
    let n = points.len();
    let count = if n >= 2 { n } else { 0 };
    (0..count).map(move |i| Curve::Segment(points[i], points[(i + 1) % n]))
}

/// The curves a placed drawing is stroked along; none for a text.
pub(super) fn graphic_curves(shape: &GraphicShape) -> Vec<Curve> {
    match shape {
        GraphicShape::Line { a, b } => vec![Curve::Segment(a.to_mm(), b.to_mm())],
        GraphicShape::Arc {
            center,
            radius,
            start_deg,
            sweep_deg,
        } => vec![Curve::Arc(ArcSweep {
            center: center.to_mm(),
            radius: nm_to_mm(*radius),
            start_angle: start_deg.to_radians(),
            sweep: sweep_deg.to_radians(),
        })],
        GraphicShape::Circle { center, radius } => Curve::circle(center.to_mm(), nm_to_mm(*radius))
            .into_iter()
            .collect(),
        GraphicShape::Polygon { points } => {
            let ring: Vec<Point> = points.iter().copied().map(PointNm::to_mm).collect();
            ring_edges(&ring).collect()
        }
        GraphicShape::Text(_) => Vec::new(),
    }
}

/// The point of segment `a`–`b` nearest `p`.
pub(super) fn nearest_on_segment(a: Point, b: Point, p: Point) -> Point {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len_sq = dx * dx + dy * dy;
    if len_sq <= 0.0 {
        return a;
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0);
    Point::new(a.x + t * dx, a.y + t * dy)
}

/// Whether the direction `angle` (radians) about the arc's centre falls on
/// `arc`.
fn on_sweep(arc: &ArcSweep, angle: f64) -> bool {
    if arc.sweep.abs() >= TAU {
        return true;
    }
    let turn = (angle - arc.start_angle).rem_euclid(TAU);
    if arc.sweep >= 0.0 {
        turn <= arc.sweep + END_SLACK || turn >= TAU - END_SLACK
    } else {
        turn <= END_SLACK || turn - TAU >= arc.sweep - END_SLACK
    }
}

/// The point of `arc` nearest `p`: on the curve when `p` faces it, else the
/// nearer end.
pub(super) fn nearest_on_sweep(arc: &ArcSweep, p: Point) -> Point {
    let angle = (p.y - arc.center.y).atan2(p.x - arc.center.x);
    if on_sweep(arc, angle) {
        return arc.at(angle);
    }
    let (start, end) = (arc.at(arc.start_angle), arc.at(arc.start_angle + arc.sweep));
    let distance = |q: Point| (q.x - p.x).powi(2) + (q.y - p.y).powi(2);
    if distance(start) <= distance(end) {
        start
    } else {
        end
    }
}

fn on_arc(arc: &ArcSweep, q: Point) -> bool {
    on_sweep(arc, (q.y - arc.center.y).atan2(q.x - arc.center.x))
}

/// Where curves `a` and `b` cross.
pub(super) fn crossings(a: &Curve, b: &Curve) -> Vec<Point> {
    match (a, b) {
        (Curve::Segment(s0, s1), Curve::Segment(t0, t1)) => {
            segment_crossing((*s0, *s1), (*t0, *t1))
                .into_iter()
                .collect()
        }
        (Curve::Segment(s0, s1), Curve::Arc(arc)) | (Curve::Arc(arc), Curve::Segment(s0, s1)) => {
            segment_arc_crossings(*s0, *s1, arc)
        }
        (Curve::Arc(x), Curve::Arc(y)) => arc_arc_crossings(x, y),
    }
}

/// Where segments `s` and `t` cross; `None` when they do not or are
/// parallel.
fn segment_crossing(s: (Point, Point), t: (Point, Point)) -> Option<Point> {
    let (r, q) = (
        (s.1.x - s.0.x, s.1.y - s.0.y),
        (t.1.x - t.0.x, t.1.y - t.0.y),
    );
    let cross = r.0 * q.1 - r.1 * q.0;
    if cross.abs() < 1e-12 {
        return None;
    }
    let w = (t.0.x - s.0.x, t.0.y - s.0.y);
    let u = (w.0 * q.1 - w.1 * q.0) / cross;
    let v = (w.0 * r.1 - w.1 * r.0) / cross;
    ((0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v))
        .then(|| Point::new(s.0.x + u * r.0, s.0.y + u * r.1))
}

/// Where segment `a`–`b` crosses `arc`: the line's roots on the circle that
/// lie on both.
fn segment_arc_crossings(a: Point, b: Point, arc: &ArcSweep) -> Vec<Point> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (fx, fy) = (a.x - arc.center.x, a.y - arc.center.y);
    let qa = dx * dx + dy * dy;
    if qa <= 0.0 {
        return Vec::new();
    }
    let qb = 2.0 * (fx * dx + fy * dy);
    let qc = fx * fx + fy * fy - arc.radius * arc.radius;
    let disc = qb * qb - 4.0 * qa * qc;
    if !disc.is_finite() || disc < 0.0 {
        return Vec::new();
    }
    let root = disc.sqrt();
    let mut ts = vec![(-qb - root) / (2.0 * qa)];
    if root > 0.0 {
        ts.push((-qb + root) / (2.0 * qa));
    }
    ts.into_iter()
        .filter(|t| (-END_SLACK..=1.0 + END_SLACK).contains(t))
        .map(|t| Point::new(a.x + t * dx, a.y + t * dy))
        .filter(|q| on_arc(arc, *q))
        .collect()
}

/// Where arcs `x` and `y` cross: their circles' crossings that lie on both.
fn arc_arc_crossings(x: &ArcSweep, y: &ArcSweep) -> Vec<Point> {
    let (dx, dy) = (y.center.x - x.center.x, y.center.y - x.center.y);
    let d = dx.hypot(dy);
    if !d.is_finite() || d <= 0.0 || d > x.radius + y.radius || d < (x.radius - y.radius).abs() {
        return Vec::new();
    }
    let along = (x.radius * x.radius - y.radius * y.radius + d * d) / (2.0 * d);
    let h = (x.radius * x.radius - along * along).max(0.0).sqrt();
    let base = Point::new(x.center.x + along * dx / d, x.center.y + along * dy / d);
    let (ox, oy) = (-dy / d * h, dx / d * h);
    let mut points = vec![Point::new(base.x + ox, base.y + oy)];
    if h > 0.0 {
        points.push(Point::new(base.x - ox, base.y - oy));
    }
    points
        .into_iter()
        .filter(|q| on_arc(x, *q) && on_arc(y, *q))
        .collect()
}

#[cfg(test)]
#[path = "snap_curves_tests.rs"]
mod tests;
