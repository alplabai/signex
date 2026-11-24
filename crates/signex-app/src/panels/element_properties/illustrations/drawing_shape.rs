//! Line-art illustration of a drawing shape (mockup row 48). A rectangle shows its
//! W / H dimensions, a Border and Fill callout and a Position leader from its
//! corner, each with the value its field holds, its fill (None / Outline /
//! Solid) and a sample wire drawn behind or above it as Placement says, named
//! left of the box. An arc shows its
//! centre mark, a Radius dimension, Start / End callouts at its two ends (no
//! angle arc) and a Width callout. A line, a circle and a polygon show their
//! own geometry. Data only; `property_illustration` draws it.

use iced::{Point, Radians, Rectangle};
use signex_i18n::{I18n, Translatable, ids, t};
use signex_types::schematic::{DrawingPlacement, FillType};
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Illustration, Ink, Leader, Paint, Part, Primitive, Style, TextLabel,
    TextRole, Tint, Weight,
};

/// Field ids the drawing rows and the illustration share.
pub(in crate::panels) const DRAW_POSITION: FieldId = FieldId::new("draw.position");
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
/// The rectangle is smaller and right of centre: the column left of it
/// holds the Placement name, the band above the Position and W callouts,
/// the strip right of it the H callout and the band under it the Border
/// callout, each with its value.
const RECT_MAX: (f32, f32) = (72.0, 56.0);
const RECT_CENTER: Point = Point::new(136.0, 74.0);
/// Free space between the box and the Placement name left of it.
const PLACEMENT_GAP: f32 = 6.0;
/// Radius of the Position dot on the box's corner.
const CORNER_DOT_R: f32 = 2.2;

/// A length as its field shows it.
pub(in crate::panels) fn value_text(mm: f64) -> String {
    format!("{mm:.3}")
}

/// What the illustration of one drawing kind needs, read from its rows.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::panels) enum ShapeFacts {
    Line {
        start: (f64, f64),
        end: (f64, f64),
    },
    /// Position (the top-left corner), size and border in mm.
    Rect {
        position: (f64, f64),
        size: (f64, f64),
        border: f64,
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

/// `size` scaled to fit within `max`, aspect kept.
fn fitted(size: (f64, f64), max: (f32, f32)) -> (f32, f32) {
    let (w, h) = (size.0.abs().max(1e-6) as f32, size.1.abs().max(1e-6) as f32);
    let scale = (max.0 / w).min(max.1 / h);
    (w * scale, h * scale)
}

/// The height the sample wire crosses `frame` at.
fn wire_y(frame: Rectangle) -> f32 {
    frame.y + frame.height * 0.6
}

/// A sample wire across the shape, drawn before it (behind) or after it
/// (above) as `placement` says.
fn sample_wire(frame: Rectangle) -> Part {
    let y = wire_y(frame);
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

/// The rectangle's box in the drawing: `size` fitted into [`RECT_MAX`]
/// round [`RECT_CENTER`].
fn rect_frame(size: (f64, f64)) -> Rectangle {
    let (w, h) = fitted(size, RECT_MAX);
    Rectangle {
        x: RECT_CENTER.x - w / 2.0,
        y: RECT_CENTER.y - h / 2.0,
        width: w,
        height: h,
    }
}

/// The Placement name, on the sample wire left of the box and clear of
/// its outline.
fn placement_name(frame: Rectangle, placement: DrawingPlacement, i18n: &I18n) -> Part {
    let name = match placement {
        DrawingPlacement::Behind => ids::PANELS_DRAWING_PLACEMENT_BEHIND,
        DrawingPlacement::Above => ids::PANELS_DRAWING_PLACEMENT_ABOVE,
    };
    let at = Point::new(frame.x - PLACEMENT_GAP, wire_y(frame) - 8.0);
    Part::label(label(at, t!(i18n, name), Anchor::End), Ink::Faint).linked(DRAW_PLACEMENT)
}

/// The W and H dimensions of `frame`, each with the size its field holds.
fn size_callouts(frame: Rectangle, size: (f64, f64), i18n: &I18n) -> [Part; 2] {
    let (left, top) = (frame.x, frame.y);
    let (right, bottom) = (left + frame.width, top + frame.height);
    let w = t!(i18n, ids::PROPERTIES_ILLUSTRATION_PAPER_WIDTH, "size" => value_text(size.0));
    let h = t!(i18n, ids::PROPERTIES_ILLUSTRATION_PAPER_HEIGHT, "size" => value_text(size.1));
    let across = Callout::new(Point::new(left, top - 10.0), Point::new(right, top - 10.0))
        .label(w)
        .label_at(Point::new(RECT_CENTER.x, top - 15.0), Anchor::Middle)
        .extension(Point::new(left, top - 2.0), Point::new(left, top - 13.0))
        .extension(Point::new(right, top - 2.0), Point::new(right, top - 13.0));
    let down = Callout::new(
        Point::new(right + 10.0, top),
        Point::new(right + 10.0, bottom),
    )
    .label(h)
    .label_at(Point::new(right + 15.0, RECT_CENTER.y), Anchor::Start)
    .extension(Point::new(right + 2.0, top), Point::new(right + 13.0, top))
    .extension(
        Point::new(right + 2.0, bottom),
        Point::new(right + 13.0, bottom),
    );
    [
        Part::callout(across).linked(DRAW_SIZE),
        Part::callout(down).linked(DRAW_SIZE),
    ]
}

/// The Border and Fill leader under the box and the Position dot and
/// leader on its top-left corner, with the values their fields hold.
fn rect_leaders(
    frame: Rectangle,
    position: (f64, f64),
    border: f64,
    fill: FillType,
    i18n: &I18n,
) -> [Part; 3] {
    let (left, top, bottom) = (frame.x, frame.y, frame.y + frame.height);
    let fill_name = Translatable::new(match fill {
        FillType::None => ids::PANELS_SHAPE_FILL_NONE,
        FillType::Outline => ids::PANELS_SHAPE_FILL_OUTLINE,
        FillType::Background => ids::PROPERTIES_FILL_SOLID,
    });
    let foot = left + frame.width * 0.25;
    let border_fill = Leader {
        at: Point::new(foot, bottom),
        label_at: Point::new(foot - 10.0, bottom + 16.0),
        label: t!(
            i18n,
            ids::PROPERTIES_DRAWING_ILLUSTRATION_BORDER,
            "width" => value_text(border),
            "fill" => fill_name
        )
        .into(),
        anchor: Anchor::Middle,
    };
    let corner = Point::new(left, top);
    let position = Leader {
        at: corner,
        label_at: Point::new(left - 8.0, top - 32.0),
        label: t!(
            i18n,
            ids::PROPERTIES_PART_ILLUSTRATION_POSITION,
            "x" => value_text(position.0),
            "y" => value_text(position.1)
        )
        .into(),
        anchor: Anchor::Start,
    };
    let dot = Part::new(
        Primitive::Circle {
            center: corner,
            radius: CORNER_DOT_R,
        },
        Style::filled(Paint::Ink(Ink::Secondary), Tint::Solid),
    );
    [
        Part::leader(border_fill).linked_all([DRAW_BORDER, DRAW_FILL]),
        dot.linked(DRAW_POSITION),
        Part::leader(position).linked(DRAW_POSITION),
    ]
}

fn rect_parts(
    (position, size, border): ((f64, f64), (f64, f64), f64),
    fill: FillType,
    placement: DrawingPlacement,
    i18n: &I18n,
) -> Vec<Part> {
    let frame = rect_frame(size);
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
    parts.push(placement_name(frame, placement, i18n));
    parts.extend(size_callouts(frame, size, i18n));
    parts.extend(rect_leaders(frame, position, border, fill, i18n));
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
    let (w, h) = fitted(
        (
            (end.0 - start.0).abs().max(1e-3),
            (end.1 - start.1).abs().max(1e-3),
        ),
        (SHAPE_MAX_W, SHAPE_MAX_H),
    );
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
    let (w, h) = fitted((max_x - min_x, max_y - min_y), (SHAPE_MAX_W, SHAPE_MAX_H));
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
            position,
            size,
            border,
            fill,
            placement,
        } => rect_parts((*position, *size, *border), *fill, *placement, i18n),
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
                position: (0.0, 0.0),
                size: (20.0, 10.0),
                border: 0.0,
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

    fn rectangle(placement: DrawingPlacement) -> Illustration {
        shape_illustration(
            &ShapeFacts::Rect {
                position: (38.1, 25.4),
                size: (76.2, 50.8),
                border: 0.254,
                fill: FillType::Background,
                placement,
            },
            &I18n::en_us(),
        )
    }

    /// Every annotation text of `drawing`, isolation marks dropped.
    fn annotations(drawing: &Illustration) -> Vec<String> {
        drawing
            .visible_parts()
            .filter_map(|p| match &p.primitive {
                Primitive::Callout(c) => c.label.as_ref().map(|l| l.as_str().to_owned()),
                Primitive::Leader(leader) => Some(leader.label.as_str().to_owned()),
                Primitive::Label(label) => Some(label.text.as_str().to_owned()),
                _ => None,
            })
            .map(|t| t.replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    /// Mockup row 48: the rectangle's callouts carry the values its fields
    /// hold: W, H, Border with the Fill, and the Position.
    #[test]
    fn the_rectangle_callouts_carry_the_field_values() {
        let shown = annotations(&rectangle(DrawingPlacement::Behind));
        for want in [
            "W 76.200 mm",
            "H 50.800 mm",
            "Border 0.254 mm \u{b7} Fill Solid",
            "Position 38.100, 25.400",
            "Behind circuit",
        ] {
            assert!(shown.iter().any(|t| t == want), "{want:?} not in {shown:?}");
        }
    }

    /// Mockup row 48: the Placement name sits left of the box, off its
    /// outline, with room for the whole name at the panel's caption size
    /// (the label fitter's character estimate).
    #[test]
    fn the_placement_name_stays_off_the_box() {
        let caption = crate::styles::TypeScale::DEFAULT.caption;
        for placement in [DrawingPlacement::Behind, DrawingPlacement::Above] {
            let drawing = rectangle(placement);
            let frame = drawing
                .visible_parts()
                .find_map(|p| match p.primitive {
                    Primitive::Rect { rect } => Some(rect),
                    _ => None,
                })
                .expect("the box");
            let name = drawing
                .visible_parts()
                .find_map(|p| match &p.primitive {
                    Primitive::Label(label) if p.is_linked_to(&DRAW_PLACEMENT) => Some(label),
                    _ => None,
                })
                .expect("the Placement name");
            assert_eq!(name.anchor, Anchor::End);
            let width = name.text.as_str().chars().count() as f32 * 0.58 * caption;
            let at = name.at;
            assert!(at.x < frame.x, "{placement:?}: {at:?} {frame:?}");
            assert!(at.x - width >= 0.0, "{placement:?}: {width} at {at:?}");
        }
    }

    /// Mockup row 48: a dot marks the Position corner, linked to the
    /// Position row.
    #[test]
    fn the_position_corner_is_marked() {
        let drawing = rectangle(DrawingPlacement::Behind);
        let frame = drawing
            .visible_parts()
            .find_map(|p| match p.primitive {
                Primitive::Rect { rect } => Some(rect),
                _ => None,
            })
            .expect("the box");
        let dot = drawing.visible_parts().find_map(|p| match p.primitive {
            Primitive::Circle { center, .. } if p.is_linked_to(&DRAW_POSITION) => Some(center),
            _ => None,
        });
        assert_eq!(dot, Some(Point::new(frame.x, frame.y)));
    }
}
