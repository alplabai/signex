//! Which linked part, if any, lies under the pointer.

use iced::Point;

use super::field::FieldId;
use super::geometry::{
    Fit, angle_in_sweep, callout_geometry, contains_point, corners, distance_to_polyline,
    distance_to_segment, grown, leader_label,
};
use super::illustration::{Illustration, Part};
use super::metrics::Metrics;
use super::primitive::{Primitive, TextRole};

/// The field of the top-most linked part under `cursor` (screen space,
/// relative to the widget). Parts with no field, and highlight-only parts,
/// are not hover targets.
pub fn hit_test(
    illustration: &Illustration,
    fit: &Fit,
    metrics: &Metrics,
    cursor: Point,
) -> Option<FieldId> {
    illustration
        .visible_parts()
        .rev()
        .filter(|part| part.reports_hover())
        .find(|part| is_under(part, fit, metrics, cursor, 0.0))
        .and_then(|part| part.primary_field().cloned())
}

/// [`hit_test`] with hysteresis: `held`, the field already lit, stays lit
/// until the pointer is [`Metrics::hit_hold`] clear of its parts, so a
/// pointer on the edge between two regions cannot alternate between them.
pub fn held_hit_test(
    illustration: &Illustration,
    fit: &Fit,
    metrics: &Metrics,
    cursor: Point,
    held: Option<&FieldId>,
) -> Option<FieldId> {
    let hold = metrics.hit_hold();
    let keeps = |field: &FieldId| {
        illustration.visible_parts().any(|part| {
            part.reports_hover()
                && part.primary_field() == Some(field)
                && is_under(part, fit, metrics, cursor, hold)
        })
    };
    match held {
        Some(field) if keeps(field) => Some(field.clone()),
        _ => hit_test(illustration, fit, metrics, cursor),
    }
}

/// Whether `cursor` is on `part`, its hit region grown by `grow` pixels.
fn is_under(part: &Part, fit: &Fit, metrics: &Metrics, cursor: Point, grow: f32) -> bool {
    let tolerance = metrics.hit_tolerance() + grow;
    let is_filled = part.style.fill.is_some();
    let near_outline = |points: &[Point], is_closed: bool| {
        distance_to_polyline(cursor, points, is_closed) <= tolerance
            || (is_filled && is_closed && contains_point(points, cursor))
    };
    match &part.primitive {
        Primitive::Line { from, to } => {
            distance_to_segment(cursor, fit.point(*from), fit.point(*to)) <= tolerance
        }
        Primitive::Rect { rect } | Primitive::RoundedRect { rect, .. } => {
            near_outline(&corners(fit.rect(*rect)), true)
        }
        Primitive::Circle { center, radius } => {
            let distance = cursor.distance(fit.point(*center));
            let radius = fit.length(*radius);
            (distance - radius).abs() <= tolerance || (is_filled && distance <= radius)
        }
        Primitive::Arc {
            center,
            radius,
            start,
            end,
        } => {
            let c = fit.point(*center);
            let on_circle = (cursor.distance(c) - fit.length(*radius)).abs() <= tolerance;
            let angle = (cursor.y - c.y).atan2(cursor.x - c.x);
            on_circle && angle_in_sweep(angle, start.0, end.0)
        }
        Primitive::Polyline { points, is_closed } => {
            let screen: Vec<Point> = points.iter().map(|p| fit.point(*p)).collect();
            near_outline(&screen, *is_closed)
        }
        Primitive::Band { points, width, .. } => {
            let screen: Vec<Point> = points.iter().map(|p| fit.point(*p)).collect();
            distance_to_polyline(cursor, &screen, false) <= fit.length(*width) / 2.0 + tolerance
        }
        Primitive::Callout(callout) => {
            let geometry = callout_geometry(callout, fit, metrics);
            let on_line =
                distance_to_segment(cursor, geometry.line.0, geometry.line.1) <= tolerance;
            let on_label = match (&callout.label, geometry.label) {
                (Some(text), Some(placement)) => grown(
                    placement.bounds(text.as_str(), TextRole::Caption, metrics),
                    grow,
                )
                .contains(cursor),
                _ => false,
            };
            on_line || on_label
        }
        Primitive::Leader(leader) => {
            let (at, end) = (fit.point(leader.at), fit.point(leader.label_at));
            distance_to_segment(cursor, at, end) <= tolerance
                || grown(
                    leader_label(end, leader.anchor, metrics).bounds(
                        leader.label.as_str(),
                        TextRole::Caption,
                        metrics,
                    ),
                    grow,
                )
                .contains(cursor)
        }
        Primitive::Label(label) => grown(
            super::draw::label_placement(label, fit).bounds(
                label.text.as_str(),
                label.role,
                metrics,
            ),
            grow,
        )
        .contains(cursor),
        Primitive::HoverArea { rect } => grown(fit.rect(*rect), grow).contains(cursor),
    }
}
