//! The selected graphic's illustration. A rectangle shows its Width,
//! Height, Start, End, Stroke and Fill callouts; the other shapes show the
//! shape with the callouts of their own fields. A shape without a fill
//! draws none, and its Fill callout drops out.

use iced::{Point, Radians, Rectangle, Size};
use signex_i18n::{I18n, UserText, ids, t};
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, Ink, Leader, Paint, Part, Primitive, Style, TextLabel, TextRole,
    Tint,
};

use super::{Fit, field, frame};
use crate::panels::symbol_context::{GraphicKindSummary, GraphicSummary, PartShape};

/// Room around the shape for the callouts, in view units.
const MARGIN: f32 = 34.0;
/// Distance from the shape to a dimension line.
const CALLOUT_GAP: f32 = 14.0;

/// The illustration of `g`; `shapes` are the part's shapes, where a
/// polygon finds its vertices.
pub(in crate::panels::symbol_editor_properties) fn graphic(
    g: &GraphicSummary,
    shapes: &[PartShape],
    i18n: &I18n,
) -> Option<Illustration> {
    let filled = g.fill.is_some();
    let body = |primitive: Primitive| {
        Part::new(
            primitive,
            Style::filled(Paint::Ink(Ink::Secondary), Tint::Light),
        )
        .requires(field::G_FILLED)
        .linked(field::G_FILL)
    };
    let edge = |primitive: Primitive| {
        Part::new(primitive, Style::edge()).linked_all([field::G_STROKE, field::G_SIZE])
    };
    let base = frame().feature(field::G_FILLED, filled);
    let ill = match &g.kind {
        GraphicKindSummary::Rectangle { from, to } => {
            let fit = Fit::of(&[*from, *to], MARGIN, false)?;
            let (a, b) = (fit.point(*from), fit.point(*to));
            let rect = Rectangle::new(
                Point::new(a.x.min(b.x), a.y.min(b.y)),
                Size::new((a.x - b.x).abs(), (a.y - b.y).abs()),
            );
            let (left, right, top, bottom) =
                (rect.x, rect.x + rect.width, rect.y, rect.y + rect.height);
            base.part(body(Primitive::Rect { rect }))
                .part(edge(Primitive::Rect { rect }))
                .part(
                    Part::callout(
                        Callout::new(
                            Point::new(left, top - CALLOUT_GAP),
                            Point::new(right, top - CALLOUT_GAP),
                        )
                        .label(t!(i18n, ids::EDITORS_SYMPROPS_CALLOUT_WIDTH))
                        .extension(
                            Point::new(left, top),
                            Point::new(left, top - CALLOUT_GAP - 3.0),
                        )
                        .extension(
                            Point::new(right, top),
                            Point::new(right, top - CALLOUT_GAP - 3.0),
                        ),
                    )
                    .linked(field::G_SIZE),
                )
                .part(
                    Part::callout(
                        Callout::new(
                            Point::new(right + CALLOUT_GAP, top),
                            Point::new(right + CALLOUT_GAP, bottom),
                        )
                        .label(t!(i18n, ids::EDITORS_SYMPROPS_CALLOUT_HEIGHT))
                        .extension(
                            Point::new(right, top),
                            Point::new(right + CALLOUT_GAP + 3.0, top),
                        )
                        .extension(
                            Point::new(right, bottom),
                            Point::new(right + CALLOUT_GAP + 3.0, bottom),
                        ),
                    )
                    .linked(field::G_SIZE),
                )
                .part(corner_leader(
                    a,
                    ids::EDITORS_SYMPROPS_CALLOUT_START,
                    field::G_START,
                    i18n,
                ))
                .part(corner_leader(
                    b,
                    ids::EDITORS_SYMPROPS_CALLOUT_END,
                    field::G_END,
                    i18n,
                ))
                .part(stroke_leader(Point::new(left, (top + bottom) / 2.0), i18n))
                .part(fill_leader(
                    Point::new((left + right) / 2.0, (top + bottom) / 2.0),
                    i18n,
                ))
        }
        GraphicKindSummary::Line { from, to } => {
            let fit = Fit::of(&[*from, *to], MARGIN, false)?;
            let (a, b) = (fit.point(*from), fit.point(*to));
            base.part(edge(Primitive::Line { from: a, to: b }))
                .part(corner_leader(
                    a,
                    ids::EDITORS_SYMPROPS_CALLOUT_START,
                    field::G_START,
                    i18n,
                ))
                .part(corner_leader(
                    b,
                    ids::EDITORS_SYMPROPS_CALLOUT_END,
                    field::G_END,
                    i18n,
                ))
        }
        GraphicKindSummary::Circle { center, radius } => {
            let fit = round_fit(*center, *radius)?;
            let (c, r) = (fit.point(*center), fit.length(*radius));
            base.part(body(Primitive::Circle {
                center: c,
                radius: r,
            }))
            .part(edge(Primitive::Circle {
                center: c,
                radius: r,
            }))
            .part(radius_callout(c, r, i18n))
            .part(fill_leader(Point::new(c.x - r / 2.0, c.y + r / 2.0), i18n))
            .part(center_label(c))
        }
        GraphicKindSummary::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            let fit = round_fit(*center, *radius)?;
            let (c, r) = (fit.point(*center), fit.length(*radius));
            let (start, end) = fit.arc(*start_deg, *end_deg);
            base.part(edge(Primitive::Arc {
                center: c,
                radius: r,
                start: Radians(start),
                end: Radians(end),
            }))
            .part(radius_callout(c, r, i18n))
            .part(center_label(c))
        }
        GraphicKindSummary::Polygon { .. } => {
            let vertices = shapes.iter().find_map(|s| match s {
                PartShape::Polygon { idx, vertices, .. } if *idx == g.idx => Some(vertices),
                _ => None,
            })?;
            let fit = Fit::of(vertices, MARGIN, false)?;
            let points: Vec<Point> = vertices.iter().map(|v| fit.point(*v)).collect();
            let first = *points.first()?;
            base.part(body(Primitive::Polyline {
                points: points.clone(),
                is_closed: true,
            }))
            .part(edge(Primitive::Polyline {
                points,
                is_closed: true,
            }))
            .part(stroke_leader(first, i18n))
        }
        GraphicKindSummary::Bezier { points } => bezier_parts(base, points, i18n)?,
        GraphicKindSummary::Pie {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            let fit = round_fit(*center, *radius)?;
            let (c, r) = (fit.point(*center), fit.length(*radius));
            let ring: Vec<Point> = signex_library::SymbolGraphicKind::Pie {
                center: *center,
                radius: *radius,
                start_deg: *start_deg,
                end_deg: *end_deg,
            }
            .flattened_outline()
            .into_iter()
            .map(|v| fit.point(v))
            .collect();
            base.part(body(Primitive::Polyline {
                points: ring.clone(),
                is_closed: true,
            }))
            .part(edge(Primitive::Polyline {
                points: ring,
                is_closed: true,
            }))
            .part(radius_callout(c, r, i18n))
            .part(center_label(c))
        }
        GraphicKindSummary::Text { content, .. } => base
            .part(
                Part::label(
                    TextLabel {
                        at: Point::new(super::VIEW_W / 2.0, super::VIEW_H / 2.0),
                        text: UserText::new(content.as_str()).into(),
                        anchor: Anchor::Middle,
                        role: TextRole::Mono,
                    },
                    Ink::Secondary,
                )
                .linked_all([field::G_CONTENT, field::G_TEXT_SIZE]),
            )
            .part(
                Part::leader(Leader {
                    at: Point::new(super::VIEW_W / 2.0, super::VIEW_H / 2.0 + 8.0),
                    label_at: Point::new(super::VIEW_W / 2.0 - 40.0, super::VIEW_H - 16.0),
                    label: t!(i18n, ids::EDITORS_SYMPROPS_X_Y).into(),
                    anchor: Anchor::End,
                })
                .linked(field::G_POSITION),
            ),
    };
    Some(ill)
}

/// A Bezier: the curve, its control arms, and a leader on each point.
fn bezier_parts(base: Illustration, points: &[[f64; 2]; 4], i18n: &I18n) -> Option<Illustration> {
    let curve = signex_library::SymbolGraphicKind::Bezier { points: *points }.flattened_outline();
    let fit = Fit::of(&curve, MARGIN, false)?;
    let [start, c1, c2, end] = points.map(|p| fit.point(p));
    let arm = |from: Point, to: Point| Part::new(Primitive::Line { from, to }, Style::dashed());
    Some(
        base.part(arm(start, c1))
            .part(arm(end, c2))
            .part(
                Part::new(
                    Primitive::Polyline {
                        points: curve.iter().map(|v| fit.point(*v)).collect(),
                        is_closed: false,
                    },
                    Style::edge(),
                )
                .linked(field::G_STROKE),
            )
            .part(corner_leader(
                start,
                ids::EDITORS_SYMPROPS_CALLOUT_START,
                field::G_START,
                i18n,
            ))
            .part(corner_leader(
                c1,
                ids::PROPERTIES_BEZIER_C1,
                field::G_CONTROL_1,
                i18n,
            ))
            .part(corner_leader(
                c2,
                ids::PROPERTIES_BEZIER_C2,
                field::G_CONTROL_2,
                i18n,
            ))
            .part(corner_leader(
                end,
                ids::EDITORS_SYMPROPS_CALLOUT_END,
                field::G_END,
                i18n,
            )),
    )
}

/// The fit of a circle or arc of `radius` round `center`.
fn round_fit(center: [f64; 2], radius: f64) -> Option<Fit> {
    Fit::of(
        &[
            [center[0] - radius, center[1] - radius],
            [center[0] + radius, center[1] + radius],
        ],
        MARGIN,
        false,
    )
}

fn corner_leader(at: Point, label: signex_i18n::MessageId, id: &'static str, i18n: &I18n) -> Part {
    // Set the label off away from the middle of the view.
    let dx = if at.x < super::VIEW_W / 2.0 {
        -18.0
    } else {
        18.0
    };
    let dy = if at.y < super::VIEW_H / 2.0 {
        -10.0
    } else {
        10.0
    };
    Part::leader(Leader {
        at,
        label_at: Point::new(at.x + dx, at.y + dy),
        label: t!(i18n, label).into(),
        anchor: if dx < 0.0 { Anchor::End } else { Anchor::Start },
    })
    .linked(id)
}

fn stroke_leader(at: Point, i18n: &I18n) -> Part {
    Part::leader(Leader {
        at,
        label_at: Point::new(at.x - 16.0, at.y + 18.0),
        label: t!(i18n, ids::EDITORS_SYMPROPS_CALLOUT_STROKE).into(),
        anchor: Anchor::End,
    })
    .linked(field::G_STROKE)
}

fn fill_leader(at: Point, i18n: &I18n) -> Part {
    Part::leader(Leader {
        at,
        label_at: Point::new(at.x + 12.0, at.y + 12.0),
        label: t!(i18n, ids::EDITORS_SYMPROPS_FILL).into(),
        anchor: Anchor::Start,
    })
    .linked(field::G_FILL)
    .requires(field::G_FILLED)
}

fn radius_callout(center: Point, radius: f32, i18n: &I18n) -> Part {
    Part::callout(
        Callout::new(center, Point::new(center.x + radius, center.y))
            .label(t!(i18n, ids::EDITORS_SYMPROPS_RADIUS)),
    )
    .linked(field::G_RADIUS)
}

fn center_label(center: Point) -> Part {
    Part::new(
        Primitive::Circle {
            center,
            radius: 1.5,
        },
        Style::filled(Paint::Ink(Ink::Secondary), Tint::Solid),
    )
    .linked(field::G_CENTER)
}
