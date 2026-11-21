//! Pure drawing geometry, in screen space.

use iced::{Point, Rectangle, Size, Vector};

use super::metrics::Metrics;
use super::primitive::{Anchor, Callout};

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
