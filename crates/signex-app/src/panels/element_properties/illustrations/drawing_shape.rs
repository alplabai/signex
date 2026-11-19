//! Line-art illustration of a drawing shape (#48). A rectangle shows its
//! W / H dimensions, a Border callout, its fill (None / Outline / Solid) and
//! a sample wire drawn behind or above it as Placement says. An arc shows its
//! centre mark, a Radius dimension, Start / End callouts at its two ends (no
//! angle arc) and a Width callout. A line, a circle and a polygon show their
//! own geometry. Data only; `property_illustration` draws it.

use iced::{Point, Radians, Rectangle};
use signex_i18n::{I18n, ids, t};
use signex_types::schematic::{DrawingPlacement, FillType};
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Illustration, Ink, Leader, Paint, Part, Primitive, Style, TextLabel,
    TextRole, Tint, Weight,
};

/// Field ids the drawing rows and the illustration share.
pub(in crate::panels) const DRAW_SIZE: FieldId = FieldId::new("draw.size");
pub(in crate::panels) const DRAW_BORDER: FieldId = FieldId::new("draw.border");
pub(in crate::panels) const DRAW_FILL: FieldId = FieldId::new("draw.fill");
pub(in crate::panels) const DRAW_PLACEMENT: FieldId = FieldId::new("draw.placement");
pub(in crate::panels) const DRAW_CENTER: FieldId = FieldId::new("draw.center");
pub(in crate::panels) const DRAW_RADIUS: FieldId = FieldId::new("draw.radius");
pub(in crate::panels) const DRAW_ANGLES: FieldId = FieldId::new("draw.angles");
pub(in crate::panels) const DRAW_WIDTH: FieldId = FieldId::new("draw.width");
pub(in crate::panels) const DRAW_ENDS: FieldId = FieldId::new("draw.ends");

const VIEW_W: f32 = 264.0;
const VIEW_H: f32 = 140.0;
const CENTER: Point = Point::new(VIEW_W / 2.0, VIEW_H / 2.0 + 4.0);
/// The largest extent a shape is drawn at.
const SHAPE_MAX_W: f32 = 140.0;
const SHAPE_MAX_H: f32 = 80.0;

/// What the illustration of one drawing kind needs, read from its rows.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::panels) enum ShapeFacts {
    Line {
        start: (f64, f64),
        end: (f64, f64),
    },
    Rect {
        size: (f64, f64),
        fill: FillType,
        placement: DrawingPlacement,
    },
    Circle {
        fill: FillType,
    },
    /// Angles in degrees, counter-clockwise from the +X axis.
    Arc {
        start_deg: f64,
        end_deg: f64,
    },
    Polygon {
        points: Vec<(f64, f64)>,
        fill: FillType,
    },
}

fn label(at: Point, text: signex_i18n::Localized, anchor: Anchor) -> TextLabel {
    TextLabel {
        at,
        text: text.into(),
        anchor,
        role: TextRole::Caption,
    }
}

/// The edge style of a shape filled as `fill`: None is an outline only,
/// Outline fills with the border ink, Solid with the panel ground.
pub(in crate::panels) fn fill_style(fill: FillType) -> Style {
    let edge = Style::outline(Ink::Secondary, Weight::Heavy);
    match fill {
        FillType::None => edge,
        FillType::Outline => edge.with_fill(Paint::Ink(Ink::Secondary), Tint::Body),
        FillType::Background => edge.with_fill(Paint::Ink(Ink::Panel), Tint::Solid),
    }
}

/// `size` scaled to fit the drawing, aspect kept.
fn fitted(size: (f64, f64)) -> (f32, f32) {
    let (w, h) = (size.0.abs().max(1e-6) as f32, size.1.abs().max(1e-6) as f32);
    let scale = (SHAPE_MAX_W / w).min(SHAPE_MAX_H / h);
    (w * scale, h * scale)
}

/// A sample wire across the shape, drawn before it (behind) or after it
/// (above) as `placement` says.
fn sample_wire(frame: Rectangle) -> Part {
    let y = frame.y + frame.height * 0.6;
    Part::new(
        Primitive::Band {
            points: vec![
                Point::new(frame.x - 30.0, y),
                Point::new(frame.x + frame.width + 30.0, y),
            ],
            width: 3.0,
            cap: signex_widgets::property_illustration::Cap::Flat,
        },
        Style::outline(Ink::Tertiary, Weight::Thin)
            .with_fill(Paint::Ink(Ink::Tertiary), Tint::Strong),
    )
    .linked(DRAW_PLACEMENT)
}

fn rect_parts(
    size: (f64, f64),
    fill: FillType,
    placement: DrawingPlacement,
    i18n: &I18n,
) -> Vec<Part> {
    let (w, h) = fitted(size);
    let frame = Rectangle {
        x: CENTER.x - w / 2.0,
        y: CENTER.y - h / 2.0,
        width: w,
        height: h,
    };
    let (left, top, right, bottom) = (frame.x, frame.y, frame.x + w, frame.y + h);
    let shape = Part::new(Primitive::Rect { rect: frame }, fill_style(fill))
        .linked_all([DRAW_BORDER, DRAW_FILL]);
    let mut parts = Vec::new();
    if placement == DrawingPlacement::Behind {
        parts.push(shape);
        parts.push(sample_wire(frame));
    } else {
        parts.push(sample_wire(frame));
        parts.push(shape);
    }
    let placement_text = match placement {
        DrawingPlacement::Behind => ids::PANELS_DRAWING_PLACEMENT_BEHIND,
        DrawingPlacement::Above => ids::PANELS_DRAWING_PLACEMENT_ABOVE,
    };
    // Left of the box, outside its outline.
    parts.push(
        Part::label(
            label(
                Point::new(left - 34.0, top + h * 0.6 - 6.0),
                t!(i18n, placement_text),
                Anchor::End,
            ),
            Ink::Faint,
        )
        .linked(DRAW_PLACEMENT),
    );
    parts.push(
        Part::callout(
            Callout::new(Point::new(left, top - 12.0), Point::new(right, top - 12.0))
                .label(t!(i18n, ids::PROPERTIES_AXIS_W))
                .label_at(Point::new(CENTER.x, top - 17.0), Anchor::Middle)
                .extension(Point::new(left, top - 2.0), Point::new(left, top - 15.0))
                .extension(Point::new(right, top - 2.0), Point::new(right, top - 15.0)),
        )
        .linked(DRAW_SIZE),
    );
    parts.push(
        Part::callout(
            Callout::new(
                Point::new(right + 12.0, top),
                Point::new(right + 12.0, bottom),
            )
            .label(t!(i18n, ids::PROPERTIES_AXIS_H))
            .label_at(Point::new(right + 17.0, CENTER.y), Anchor::Start)
            .extension(Point::new(right + 2.0, top), Point::new(right + 15.0, top))
            .extension(
                Point::new(right + 2.0, bottom),
                Point::new(right + 15.0, bottom),
            ),
        )
        .linked(DRAW_SIZE),
    );
    parts.push(
        Part::leader(Leader {
            at: Point::new(right - w * 0.2, bottom),
            label_at: Point::new(right + 6.0, bottom + 14.0),
            label: t!(i18n, ids::PANELS_DRAWING_BORDER).into(),
            anchor: Anchor::Start,
        })
        .linked(DRAW_BORDER),
    );
    parts
}

/// The screen point at `deg` degrees (counter-clockwise, y down) on a circle.
fn on_circle(center: Point, radius: f32, deg: f64) -> Point {
    let a = deg.to_radians() as f32;
    Point::new(center.x + radius * a.cos(), center.y - radius * a.sin())
}

/// The sweep of an arc from `start` to `end` degrees counter-clockwise, as
/// the clockwise-on-screen `(from, to)` radians the drawing takes.
pub(in crate::panels) fn arc_sweep(start_deg: f64, end_deg: f64) -> (f32, f32) {
    let mut end = end_deg;
    while end <= start_deg {
        end += 360.0;
    }
    ((-end).to_radians() as f32, (-start_deg).to_radians() as f32)
}

fn arc_parts(start_deg: f64, end_deg: f64, i18n: &I18n) -> Vec<Part> {
    let radius = SHAPE_MAX_H / 2.0;
    let (from, to) = arc_sweep(start_deg, end_deg);
    let start = on_circle(CENTER, radius, start_deg);
    let end = on_circle(CENTER, radius, end_deg);
    let mark = 4.0;
    let tip = |at: Point, deg: f64| on_circle(at, 14.0, deg);
    vec![
        Part::new(
            Primitive::Arc {
                center: CENTER,
                radius,
                start: Radians(from),
                end: Radians(to),
            },
            Style::outline(Ink::Secondary, Weight::Heavy),
        )
        .linked_all([DRAW_WIDTH, DRAW_RADIUS, DRAW_ANGLES]),
        Part::new(
            Primitive::Line {
                from: Point::new(CENTER.x - mark, CENTER.y),
                to: Point::new(CENTER.x + mark, CENTER.y),
            },
            Style::outline(Ink::Tertiary, Weight::Thin),
        )
        .linked(DRAW_CENTER),
        Part::new(
            Primitive::Line {
                from: Point::new(CENTER.x, CENTER.y - mark),
                to: Point::new(CENTER.x, CENTER.y + mark),
            },
            Style::outline(Ink::Tertiary, Weight::Thin),
        )
        .linked(DRAW_CENTER),
        Part::callout(
            Callout::new(
                CENTER,
                on_circle(CENTER, radius, (start_deg + end_deg) / 2.0),
            )
            .label(t!(i18n, ids::PROPERTIES_DRAWING_RADIUS)),
        )
        .linked(DRAW_RADIUS),
        Part::leader(Leader {
            at: start,
            label_at: tip(start, start_deg),
            label: t!(i18n, ids::PROPERTIES_ANGLE_START).into(),
            anchor: Anchor::Start,
        })
        .linked(DRAW_ANGLES),
        Part::leader(Leader {
            at: end,
            label_at: tip(end, end_deg),
            label: t!(i18n, ids::PROPERTIES_ANGLE_END).into(),
            anchor: Anchor::Start,
        })
        .linked(DRAW_ANGLES),
        Part::leader(Leader {
            at: on_circle(CENTER, radius, start_deg + (end_deg - start_deg) * 0.25),
            label_at: Point::new(VIEW_W - 30.0, VIEW_H - 12.0),
            label: t!(i18n, ids::PROPERTIES_DRAWING_WIDTH).into(),
            anchor: Anchor::End,
        })
        .linked(DRAW_WIDTH),
    ]
}

fn line_parts(start: (f64, f64), end: (f64, f64), i18n: &I18n) -> Vec<Part> {
    let (w, h) = fitted((
        (end.0 - start.0).abs().max(1e-3),
        (end.1 - start.1).abs().max(1e-3),
    ));
    let sx = if end.0 >= start.0 { -1.0 } else { 1.0 };
    let sy = if end.1 >= start.1 { -1.0 } else { 1.0 };
    let a = Point::new(CENTER.x + sx * w / 2.0, CENTER.y + sy * h / 2.0);
    let b = Point::new(CENTER.x - sx * w / 2.0, CENTER.y - sy * h / 2.0);
    vec![
        Part::new(
            Primitive::Line { from: a, to: b },
            Style::outline(Ink::Secondary, Weight::Heavy),
        )
        .linked_all([DRAW_WIDTH, DRAW_ENDS]),
        Part::label(
            label(
                Point::new(a.x, a.y + 12.0),
                t!(i18n, ids::PROPERTIES_DRAWING_START),
                Anchor::Middle,
            ),
            Ink::Faint,
        )
        .linked(DRAW_ENDS),
        Part::label(
            label(
                Point::new(b.x, b.y - 6.0),
                t!(i18n, ids::PROPERTIES_DRAWING_END),
                Anchor::Middle,
            ),
            Ink::Faint,
        )
        .linked(DRAW_ENDS),
        Part::leader(Leader {
            at: CENTER,
            label_at: Point::new(CENTER.x + 40.0, CENTER.y + 30.0),
            label: t!(i18n, ids::PROPERTIES_DRAWING_WIDTH).into(),
            anchor: Anchor::Start,
        })
        .linked(DRAW_WIDTH),
    ]
}

fn circle_parts(fill: FillType, i18n: &I18n) -> Vec<Part> {
    let radius = SHAPE_MAX_H / 2.0;
    vec![
        Part::new(
            Primitive::Circle {
                center: CENTER,
                radius,
            },
            fill_style(fill),
        )
        .linked_all([DRAW_BORDER, DRAW_FILL]),
        Part::callout(
            Callout::new(CENTER, Point::new(CENTER.x + radius, CENTER.y))
                .label(t!(i18n, ids::PROPERTIES_DRAWING_RADIUS)),
        )
        .linked(DRAW_RADIUS),
        Part::leader(Leader {
            at: on_circle(CENTER, radius, 225.0),
            label_at: Point::new(CENTER.x - radius - 10.0, CENTER.y + radius + 6.0),
            label: t!(i18n, ids::PANELS_DRAWING_BORDER).into(),
            anchor: Anchor::End,
        })
        .linked(DRAW_BORDER),
    ]
}

fn polygon_parts(points: &[(f64, f64)], fill: FillType) -> Vec<Part> {
    if points.len() < 2 {
        return Vec::new();
    }
    let (min_x, max_x) = points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.0), hi.max(p.0))
        });
    let (min_y, max_y) = points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.1), hi.max(p.1))
        });
    let (w, h) = fitted((max_x - min_x, max_y - min_y));
    let span = ((max_x - min_x).max(1e-6), (max_y - min_y).max(1e-6));
    let to_view = |p: &(f64, f64)| {
        Point::new(
            CENTER.x - w / 2.0 + ((p.0 - min_x) / span.0) as f32 * w,
            CENTER.y - h / 2.0 + ((p.1 - min_y) / span.1) as f32 * h,
        )
    };
    vec![
        Part::new(
            Primitive::Polyline {
                points: points.iter().map(to_view).collect(),
                is_closed: true,
            },
            fill_style(fill),
        )
        .linked_all([DRAW_BORDER, DRAW_FILL]),
    ]
}

/// The illustration of a drawing shape.
pub(in crate::panels) fn shape_illustration(facts: &ShapeFacts, i18n: &I18n) -> Illustration {
    let parts = match facts {
        ShapeFacts::Line { start, end } => line_parts(*start, *end, i18n),
        ShapeFacts::Rect {
            size,
            fill,
            placement,
        } => rect_parts(*size, *fill, *placement, i18n),
        ShapeFacts::Circle { fill } => circle_parts(*fill, i18n),
        ShapeFacts::Arc { start_deg, end_deg } => arc_parts(*start_deg, *end_deg, i18n),
        ShapeFacts::Polygon { points, fill } => polygon_parts(points, *fill),
    };
    Illustration::new(VIEW_W, VIEW_H).parts(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_of(drawing: &Illustration, field: &FieldId, rect: bool) -> usize {
        drawing
            .visible_parts()
            .position(|p| {
                p.is_linked_to(field) && (matches!(p.primitive, Primitive::Rect { .. }) == rect)
            })
            .expect("part")
    }

    /// Behind circuit draws the shape first and the sample wire over it;
    /// Above circuit the other way round.
    #[test]
    fn placement_orders_the_sample_wire_and_the_rectangle() {
        let i18n = I18n::en_us();
        for (placement, wire_first) in [
            (DrawingPlacement::Behind, false),
            (DrawingPlacement::Above, true),
        ] {
            let facts = ShapeFacts::Rect {
                size: (20.0, 10.0),
                fill: FillType::None,
                placement,
            };
            let drawing = shape_illustration(&facts, &i18n);
            let shape = index_of(&drawing, &DRAW_FILL, true);
            let wire = drawing
                .visible_parts()
                .position(|p| matches!(p.primitive, Primitive::Band { .. }))
                .expect("wire");
            assert_eq!(wire < shape, wire_first, "{placement:?}");
        }
    }

    #[test]
    fn fill_none_has_no_fill_and_solid_is_opaque() {
        assert!(fill_style(FillType::None).fill.is_none());
        let solid = fill_style(FillType::Background).fill.expect("filled");
        assert_eq!(solid.tint, Tint::Solid);
        assert_eq!(solid.paint, Paint::Ink(Ink::Panel));
        assert!(fill_style(FillType::Outline).fill.is_some());
    }

    #[test]
    fn an_arc_sweeps_counter_clockwise_from_start_to_end() {
        let (from, to) = arc_sweep(0.0, 90.0);
        assert!((from - (-90f32).to_radians()).abs() < 1e-6);
        assert!(to.abs() < 1e-6);
        let (from, to) = arc_sweep(270.0, 90.0);
        assert!((to - from - 180f32.to_radians()).abs() < 1e-5);
    }

    /// The arc draws no angle arc: Start and End are leaders at its ends.
    #[test]
    fn the_arc_has_start_and_end_callouts_and_no_angle_sweep() {
        let i18n = I18n::en_us();
        let drawing = shape_illustration(
            &ShapeFacts::Arc {
                start_deg: 0.0,
                end_deg: 120.0,
            },
            &i18n,
        );
        let arcs = drawing
            .visible_parts()
            .filter(|p| matches!(p.primitive, Primitive::Arc { .. }))
            .count();
        assert_eq!(arcs, 1, "only the shape itself is an arc");
        let ends = drawing
            .visible_parts()
            .filter(|p| p.is_linked_to(&DRAW_ANGLES) && matches!(p.primitive, Primitive::Leader(_)))
            .count();
        assert_eq!(ends, 2);
    }
}
