//! Pure geometry shared by drawing and hit testing, in screen space.

use iced::{Point, Rectangle, Size, Vector};

use super::metrics::Metrics;
use super::primitive::{Anchor, Callout, TextRole};

/// Uniform fit of the view box into the widget bounds, centred.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    pub scale: f32,
    pub offset: Vector,
}

impl Fit {
    pub fn new(view: Size, bounds: Size) -> Self {
        let scale = (bounds.width / view.width)
            .min(bounds.height / view.height)
            .max(0.0);
        let offset = Vector::new(
            (bounds.width - view.width * scale) / 2.0,
            (bounds.height - view.height * scale) / 2.0,
        );
        Self { scale, offset }
    }

    pub fn point(&self, p: Point) -> Point {
        Point::new(
            p.x * self.scale + self.offset.x,
            p.y * self.scale + self.offset.y,
        )
    }

    pub fn length(&self, length: f32) -> f32 {
        length * self.scale
    }

    pub fn rect(&self, rect: Rectangle) -> Rectangle {
        let top_left = self.point(rect.position());
        Rectangle::new(
            top_left,
            Size::new(self.length(rect.width), self.length(rect.height)),
        )
    }
}

/// Vertical placement of a label against its point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Baseline {
    /// The label sits above the point.
    Above,
    /// The label is centred on the point.
    Centre,
}

/// Where a label is drawn, in screen space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelPlacement {
    pub at: Point,
    pub anchor: Anchor,
    pub baseline: Baseline,
}

impl LabelPlacement {
    /// The box the label covers, for hover.
    pub fn bounds(&self, text: &str, role: TextRole, metrics: &Metrics) -> Rectangle {
        let width = metrics.text_width(text, role);
        let height = metrics.text(role);
        let x = match self.anchor {
            Anchor::Start => self.at.x,
            Anchor::Middle => self.at.x - width / 2.0,
            Anchor::End => self.at.x - width,
        };
        let y = match self.baseline {
            Baseline::Above => self.at.y - height,
            Baseline::Centre => self.at.y - height / 2.0,
        };
        Rectangle::new(Point::new(x, y), Size::new(width, height))
    }
}

/// A filled arrow head: tip and the two base corners.
pub type Head = [Point; 3];

/// The screen geometry of a dimension callout.
#[derive(Debug, Clone, PartialEq)]
pub struct CalloutGeometry {
    pub line: (Point, Point),
    pub heads: [Head; 2],
    pub label: Option<LabelPlacement>,
}

fn unit(from: Point, to: Point) -> Vector {
    let d = to - from;
    let length = d.x.hypot(d.y);
    if length <= f32::EPSILON {
        Vector::new(1.0, 0.0)
    } else {
        Vector::new(d.x / length, d.y / length)
    }
}

/// An arrow head with its tip at `tip`, pointing along `dir`.
pub fn head(tip: Point, dir: Vector, metrics: &Metrics) -> Head {
    let (length, half) = (metrics.arrow_length(), metrics.arrow_half_width());
    let base = Point::new(tip.x - dir.x * length, tip.y - dir.y * length);
    [
        tip,
        Point::new(base.x - dir.y * half, base.y + dir.x * half),
        Point::new(base.x + dir.y * half, base.y - dir.x * half),
    ]
}

/// Lays out `callout` on screen: heads point out on a long span and in,
/// from outside, on a short one.
pub fn callout_geometry(callout: &Callout, fit: &Fit, metrics: &Metrics) -> CalloutGeometry {
    let (a, b) = (fit.point(callout.from), fit.point(callout.to));
    let u = unit(a, b);
    let span = (b - a).x.hypot((b - a).y);
    let (line, heads) = if span < metrics.short_span() {
        let over = metrics.short_overhang();
        (
            (
                Point::new(a.x - u.x * over, a.y - u.y * over),
                Point::new(b.x + u.x * over, b.y + u.y * over),
            ),
            [
                head(a, u, metrics),
                head(b, Vector::new(-u.x, -u.y), metrics),
            ],
        )
    } else {
        (
            (a, b),
            [
                head(a, Vector::new(-u.x, -u.y), metrics),
                head(b, u, metrics),
            ],
        )
    };
    let label = callout.label.as_ref().map(|_| match callout.label_at {
        Some((at, anchor)) => LabelPlacement {
            at: fit.point(at),
            anchor,
            baseline: Baseline::Above,
        },
        None => beside_middle(a, b, u, metrics),
    });
    CalloutGeometry { line, heads, label }
}

/// The default label spot: above a mostly horizontal span, right of a
/// mostly vertical one.
fn beside_middle(a: Point, b: Point, u: Vector, metrics: &Metrics) -> LabelPlacement {
    let middle = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let gap = metrics.label_gap();
    if u.x.abs() >= u.y.abs() {
        LabelPlacement {
            at: Point::new(middle.x, middle.y - gap),
            anchor: Anchor::Middle,
            baseline: Baseline::Above,
        }
    } else {
        LabelPlacement {
            at: Point::new(middle.x + gap, middle.y),
            anchor: Anchor::Start,
            baseline: Baseline::Centre,
        }
    }
}

/// The label spot of a leader: just past the end of its line.
pub fn leader_label(end: Point, anchor: Anchor, metrics: &Metrics) -> LabelPlacement {
    let gap = metrics.label_gap() / 2.0;
    let x = match anchor {
        Anchor::Start => end.x + gap,
        Anchor::Middle => end.x,
        Anchor::End => end.x - gap,
    };
    LabelPlacement {
        at: Point::new(x, end.y),
        anchor,
        baseline: Baseline::Centre,
    }
}

/// Distance from `p` to the segment `a`-`b`.
pub fn distance_to_segment(p: Point, a: Point, b: Point) -> f32 {
    let ab = b - a;
    let ap = p - a;
    let length_sq = ab.x * ab.x + ab.y * ab.y;
    let t = if length_sq <= f32::EPSILON {
        0.0
    } else {
        ((ap.x * ab.x + ap.y * ab.y) / length_sq).clamp(0.0, 1.0)
    };
    let closest = Point::new(a.x + ab.x * t, a.y + ab.y * t);
    p.distance(closest)
}

/// Distance from `p` to an open or closed polyline.
pub fn distance_to_polyline(p: Point, points: &[Point], is_closed: bool) -> f32 {
    let mut best = match points {
        [] => return f32::INFINITY,
        [only] => p.distance(*only),
        _ => f32::INFINITY,
    };
    for pair in points.windows(2) {
        best = best.min(distance_to_segment(p, pair[0], pair[1]));
    }
    if is_closed && points.len() > 2 {
        best = best.min(distance_to_segment(p, points[points.len() - 1], points[0]));
    }
    best
}

/// Even-odd point-in-polygon test.
pub fn contains_point(points: &[Point], p: Point) -> bool {
    let mut inside = false;
    let mut j = points.len().wrapping_sub(1);
    for (i, pi) in points.iter().enumerate() {
        let pj = points[j];
        if (pi.y > p.y) != (pj.y > p.y) && p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y) + pi.x
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// The four corners of `rect`, clockwise from the top left.
pub fn corners(rect: Rectangle) -> [Point; 4] {
    [
        Point::new(rect.x, rect.y),
        Point::new(rect.x + rect.width, rect.y),
        Point::new(rect.x + rect.width, rect.y + rect.height),
        Point::new(rect.x, rect.y + rect.height),
    ]
}

/// Whether `angle` lies on the clockwise sweep from `start` to `end`.
pub fn angle_in_sweep(angle: f32, start: f32, end: f32) -> bool {
    use std::f32::consts::TAU;
    let sweep = (end - start).rem_euclid(TAU);
    let offset = (angle - start).rem_euclid(TAU);
    if sweep <= f32::EPSILON {
        // A zero sweep with distinct ends is a full turn.
        return (end - start).abs() > f32::EPSILON;
    }
    offset <= sweep
}
