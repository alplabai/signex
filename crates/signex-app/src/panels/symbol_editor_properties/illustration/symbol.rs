//! The Symbol tab's illustration: the part the editor shows, its body
//! shapes and pins with their numbers and names, Mirrored applied.

use iced::{Point, Radians, Rectangle, Size};
use signex_i18n::UserText;
use signex_widgets::property_illustration::{
    Anchor, Illustration, Ink, Paint, Part, Primitive, Style, TextLabel, TextRole, Tint,
};

use super::{Fit, field, frame};
use crate::library::editor::pin_labels::orientation_by_name;
use crate::panels::symbol_context::{PartShape, SymbolEditorPanelContext, SymbolPinSummary};

/// Room around the drawing for the pin names and numbers, in view units.
const MARGIN: f32 = 22.0;
/// How far a pin number sits from its pin line, in view units.
const NUMBER_OFFSET: f32 = 4.0;
/// How far a pin name sits inside the body from the pin's body end.
const NAME_INSET: f32 = 4.0;

/// The illustration, or `None` when the part has nothing to draw.
pub(in crate::panels::symbol_editor_properties) fn symbol(
    sym: &SymbolEditorPanelContext,
) -> Option<Illustration> {
    let drawing = &sym.part_drawing;
    let pins: Vec<&SymbolPinSummary> = drawing
        .pins
        .iter()
        .filter_map(|&i| sym.pins.get(i))
        .collect();
    let mut points: Vec<[f64; 2]> = Vec::new();
    for shape in &drawing.shapes {
        shape_points(shape, &mut points);
    }
    for pin in &pins {
        let (tip, body) = pin_ends(pin);
        points.push(tip);
        points.push(body);
    }
    let fit = Fit::of(&points, MARGIN, sym.symbol_mirrored)?;
    let mut illustration = frame();
    for shape in &drawing.shapes {
        if let Some(part) = shape_part(shape, &fit) {
            illustration = illustration.part(part.linked_all([field::SYM_PART, field::SYM_MIRROR]));
        }
    }
    for pin in pins {
        illustration = illustration.parts(pin_parts(pin, &fit));
    }
    Some(illustration)
}

/// The connection end and the body end of a pin, in symbol mm.
pub(super) fn pin_ends(pin: &SymbolPinSummary) -> ([f64; 2], [f64; 2]) {
    let unit = orientation_by_name(&pin.orientation)
        .unwrap_or_default()
        .unit_vector();
    let tip = pin.position;
    (
        tip,
        [tip[0] + unit.0 * pin.length, tip[1] + unit.1 * pin.length],
    )
}

fn shape_points(shape: &PartShape, out: &mut Vec<[f64; 2]>) {
    match shape {
        PartShape::Rectangle { from, to, .. } | PartShape::Line { from, to } => {
            out.extend([*from, *to]);
        }
        PartShape::Circle { center, radius, .. } | PartShape::Arc { center, radius, .. } => {
            out.extend([
                [center[0] - radius, center[1] - radius],
                [center[0] + radius, center[1] + radius],
            ]);
        }
        PartShape::Polygon { vertices, .. } => out.extend(vertices.iter().copied()),
        PartShape::Outline { points, .. } => out.extend(points.iter().copied()),
    }
}

fn body_style(filled: bool) -> Style {
    let edge = Style::edge();
    if filled {
        edge.with_fill(Paint::Ink(Ink::Secondary), Tint::Light)
    } else {
        edge
    }
}

fn shape_part(shape: &PartShape, fit: &Fit) -> Option<Part> {
    Some(match shape {
        PartShape::Rectangle { from, to, filled } => {
            let (a, b) = (fit.point(*from), fit.point(*to));
            let rect = Rectangle::new(
                Point::new(a.x.min(b.x), a.y.min(b.y)),
                Size::new((a.x - b.x).abs(), (a.y - b.y).abs()),
            );
            Part::new(Primitive::Rect { rect }, body_style(*filled))
        }
        PartShape::Line { from, to } => Part::new(
            Primitive::Line {
                from: fit.point(*from),
                to: fit.point(*to),
            },
            Style::edge(),
        ),
        PartShape::Circle {
            center,
            radius,
            filled,
        } => Part::new(
            Primitive::Circle {
                center: fit.point(*center),
                radius: fit.length(*radius),
            },
            body_style(*filled),
        ),
        PartShape::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            let (start, end) = fit.arc(*start_deg, *end_deg);
            Part::new(
                Primitive::Arc {
                    center: fit.point(*center),
                    radius: fit.length(*radius),
                    start: Radians(start),
                    end: Radians(end),
                },
                Style::edge(),
            )
        }
        PartShape::Polygon {
            vertices, filled, ..
        } => {
            if vertices.len() < 2 {
                return None;
            }
            Part::new(
                Primitive::Polyline {
                    points: vertices.iter().map(|v| fit.point(*v)).collect(),
                    is_closed: true,
                },
                body_style(*filled),
            )
        }
        PartShape::Outline {
            points,
            is_closed,
            filled,
        } => {
            if points.len() < 2 {
                return None;
            }
            Part::new(
                Primitive::Polyline {
                    points: points.iter().map(|v| fit.point(*v)).collect(),
                    is_closed: *is_closed,
                },
                body_style(*filled),
            )
        }
    })
}

/// A pin's line, number and name; a hidden pin is drawn dashed.
fn pin_parts(pin: &SymbolPinSummary, fit: &Fit) -> Vec<Part> {
    let (tip, body) = pin_ends(pin);
    let (a, b) = (fit.point(tip), fit.point(body));
    let style = if pin.details.hidden {
        Style::dashed()
    } else {
        Style::edge()
    };
    let mut parts = vec![Part::new(Primitive::Line { from: a, to: b }, style)];
    let horizontal = (a.y - b.y).abs() < f32::EPSILON;
    let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    if pin.details.designator_visible && !pin.number.is_empty() {
        let (at, anchor) = if horizontal {
            (Point::new(mid.x, mid.y - NUMBER_OFFSET), Anchor::Middle)
        } else {
            (Point::new(mid.x + NUMBER_OFFSET, mid.y), Anchor::Start)
        };
        parts.push(Part::label(
            TextLabel {
                at,
                text: UserText::new(pin.number.as_str()).into(),
                anchor,
                role: TextRole::Note,
            },
            Ink::Tertiary,
        ));
    }
    if pin.details.name_visible && !pin.name.is_empty() {
        // Inside the body: past the body end, away from the tip.
        let dir = (sign(b.x - a.x), sign(b.y - a.y));
        let at = Point::new(b.x + dir.0 * NAME_INSET, b.y + dir.1 * NAME_INSET);
        let anchor = match dir.0 {
            d if d > 0.0 => Anchor::Start,
            d if d < 0.0 => Anchor::End,
            _ => Anchor::Middle,
        };
        parts.push(Part::label(
            TextLabel {
                at,
                text: UserText::new(pin.name.as_str()).into(),
                anchor,
                role: TextRole::Caption,
            },
            Ink::Secondary,
        ));
    }
    parts
}

/// -1, 0 or 1 by the sign of `v`; unlike `f32::signum`, zero stays zero.
fn sign(v: f32) -> f32 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}
