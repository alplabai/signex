//! The selected pin's illustration: the pin at its orientation against a
//! piece of the body, its number above the line and its name inside the
//! body, the edge symbols (clock wedge, active-low dot), the inside and
//! outside symbols and a glyph for the electrical type. No rotation arc.
//! Callouts: Length and X / Y at the connection end.

use iced::{Point, Rectangle, Size, Vector};
use signex_i18n::{I18n, UserText, ids, t};
use signex_library::{PinOrientation, PinSymbolKind};
use signex_types::schematic::PinDirection;
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, Ink, Leader, Paint, Part, Primitive, Style, TextLabel, TextRole,
    Tint,
};

use super::{field, frame};
use crate::library::editor::pin_labels::{
    direction_by_name, orientation_by_name, symbol_kind_message,
};
use crate::panels::symbol_context::SymbolPinSummary;

/// Where the pin sits for one orientation, in view units: its connection
/// end, the unit direction toward the body, the unit normal text is set
/// off along, and the drawn pin length.
struct Layout {
    tip: Point,
    dir: Vector,
    normal: Vector,
    length: f32,
    /// How deep and how wide the drawn piece of body is.
    body_depth: f32,
    body_half: f32,
    horizontal: bool,
}

impl Layout {
    fn of(orientation: PinOrientation) -> Self {
        let horizontal = |tip_x: f32, dir_x: f32| Self {
            tip: Point::new(tip_x, 76.0),
            dir: Vector::new(dir_x, 0.0),
            normal: Vector::new(0.0, -1.0),
            length: 106.0,
            body_depth: 98.0,
            body_half: 50.0,
            horizontal: true,
        };
        let vertical = |tip_y: f32, dir_y: f32| Self {
            tip: Point::new(100.0, tip_y),
            dir: Vector::new(0.0, dir_y),
            normal: Vector::new(1.0, 0.0),
            length: 68.0,
            body_depth: 50.0,
            body_half: 80.0,
            horizontal: false,
        };
        match orientation {
            PinOrientation::Left => horizontal(220.0, -1.0),
            PinOrientation::Up => vertical(126.0, -1.0),
            PinOrientation::Down => vertical(14.0, 1.0),
            _ => horizontal(44.0, 1.0),
        }
    }

    /// `from` moved `u` along the pin and `v` along the normal.
    fn at(&self, from: Point, u: f32, v: f32) -> Point {
        from + self.dir * u + self.normal * v
    }

    fn body_end(&self) -> Point {
        self.at(self.tip, self.length, 0.0)
    }

    /// The anchor of a label set past `self.body_end()` into the body.
    fn inward_anchor(&self) -> Anchor {
        match (self.horizontal, self.dir.x > 0.0) {
            (true, true) => Anchor::Start,
            (true, false) => Anchor::End,
            (false, _) => Anchor::Middle,
        }
    }

    /// The anchor of a label set before the tip, away from the body.
    fn outward_anchor(&self) -> Anchor {
        match (self.horizontal, self.dir.x > 0.0) {
            (true, true) => Anchor::End,
            (true, false) => Anchor::Start,
            (false, _) => Anchor::Middle,
        }
    }
}

const NUMBER_OFFSET: f32 = 6.0;
const NAME_INSET: f32 = 6.0;
const CALLOUT_OFFSET: f32 = 24.0;
const GLYPH_AT: f32 = 20.0;
const GLYPH: f32 = 5.0;
const DOT_RADIUS: f32 = 4.0;

pub(in crate::panels::symbol_editor_properties) fn pin(
    pin: &SymbolPinSummary,
    i18n: &I18n,
) -> Illustration {
    let layout = Layout::of(orientation_by_name(&pin.orientation).unwrap_or_default());
    let tip = layout.tip;
    let body_end = layout.body_end();
    let details = &pin.details;

    let corner_a = layout.at(body_end, 0.0, -layout.body_half);
    let corner_b = layout.at(body_end, layout.body_depth, layout.body_half);
    let body = Rectangle::new(
        Point::new(corner_a.x.min(corner_b.x), corner_a.y.min(corner_b.y)),
        Size::new(
            (corner_a.x - corner_b.x).abs(),
            (corner_a.y - corner_b.y).abs(),
        ),
    );
    let line_style = if details.hidden {
        Style::dashed()
    } else {
        Style::edge()
    };
    let mut ill = frame()
        .feature(field::PIN_NAME_SHOWN, details.name_visible)
        .feature(field::PIN_NUMBER_SHOWN, details.designator_visible)
        .part(Part::new(
            Primitive::Rect { rect: body },
            Style::edge().with_fill(Paint::Ink(Ink::Secondary), Tint::Wash),
        ))
        .part(
            Part::new(
                Primitive::Line {
                    from: tip,
                    to: body_end,
                },
                line_style,
            )
            .linked_all([field::PIN_LEN, field::PIN_ORIENT, field::PIN_XY]),
        );

    let mid = Point::new((tip.x + body_end.x) / 2.0, (tip.y + body_end.y) / 2.0);
    let number_anchor = if layout.horizontal {
        Anchor::Middle
    } else {
        Anchor::Start
    };
    ill = ill
        .part(
            label(
                layout.at(mid, 0.0, NUMBER_OFFSET),
                UserText::new(pin.number.as_str()).into(),
                number_anchor,
                TextRole::Mono,
                Ink::Secondary,
            )
            .linked(field::PIN_DN)
            .requires(field::PIN_NUMBER_SHOWN),
        )
        .part(
            label(
                layout.at(body_end, NAME_INSET, 0.0),
                UserText::new(pin.name.as_str()).into(),
                layout.inward_anchor(),
                TextRole::Caption,
                Ink::Secondary,
            )
            .linked(field::PIN_DN)
            .requires(field::PIN_NAME_SHOWN),
        );

    // Callouts: the length along the pin, X / Y at the connection end.
    let from = layout.at(tip, 0.0, -CALLOUT_OFFSET);
    let to = layout.at(body_end, 0.0, -CALLOUT_OFFSET);
    ill = ill
        .part(
            Part::callout(
                Callout::new(from, to)
                    .label(t!(i18n, ids::EDITORS_SYMPROPS_LENGTH))
                    .extension(
                        layout.at(tip, 0.0, -3.0),
                        layout.at(tip, 0.0, -CALLOUT_OFFSET - 3.0),
                    )
                    .extension(
                        layout.at(body_end, 0.0, -3.0),
                        layout.at(body_end, 0.0, -CALLOUT_OFFSET - 3.0),
                    ),
            )
            .linked(field::PIN_LEN),
        )
        .part(
            Part::leader(Leader {
                at: tip,
                label_at: layout.at(tip, -10.0, 30.0),
                label: t!(i18n, ids::EDITORS_SYMPROPS_X_Y).into(),
                anchor: layout.outward_anchor(),
            })
            .linked(field::PIN_XY),
        );

    if let Some(glyph) =
        direction_by_name(&pin.electrical).and_then(|d| electrical_glyph(d, &layout))
    {
        ill = ill.parts(glyph);
    }
    ill.parts(edge_symbols(pin, &layout, i18n))
}

fn label(
    at: Point,
    text: signex_widgets::Label<'static>,
    anchor: Anchor,
    role: TextRole,
    ink: Ink,
) -> Part {
    Part::label(
        TextLabel {
            at,
            text,
            anchor,
            role,
        },
        ink,
    )
}

/// An arrow head on the pin line pointing `toward` the body (or away).
fn chevron(layout: &Layout, at: Point, toward: bool) -> Primitive {
    let (back, front) = if toward {
        (-GLYPH, GLYPH * 0.6)
    } else {
        (GLYPH * 0.6, -GLYPH)
    };
    Primitive::Polyline {
        points: vec![
            layout.at(at, back, GLYPH),
            layout.at(at, front, 0.0),
            layout.at(at, back, -GLYPH),
        ],
        is_closed: false,
    }
}

/// Arrows for an input, an output or a bidirectional pin, a bar for a power
/// input; nothing for the other types.
fn electrical_glyph(direction: PinDirection, layout: &Layout) -> Option<Vec<Part>> {
    let at = layout.at(layout.body_end(), -GLYPH_AT, 0.0);
    let primitives = match direction {
        PinDirection::Input => vec![chevron(layout, at, true)],
        PinDirection::Output => vec![chevron(layout, at, false)],
        PinDirection::Bidirectional => vec![
            chevron(layout, layout.at(at, GLYPH * 1.2, 0.0), true),
            chevron(layout, layout.at(at, -GLYPH * 1.2, 0.0), false),
        ],
        PinDirection::PowerInput => vec![Primitive::Line {
            from: layout.at(at, 0.0, GLYPH * 1.2),
            to: layout.at(at, 0.0, -GLYPH * 1.2),
        }],
        _ => return None,
    };
    Some(
        primitives
            .into_iter()
            .map(|p| Part::new(p, Style::edge()).linked(field::PIN_ETYPE))
            .collect(),
    )
}

/// The four symbol slots: a clock wedge inside the edge, an active-low dot
/// outside it, any other symbol named in a small note at its place.
fn edge_symbols(pin: &SymbolPinSummary, layout: &Layout, i18n: &I18n) -> Vec<Part> {
    let d = &pin.details;
    let body_end = layout.body_end();
    let note = |at: Point, kind: PinSymbolKind, anchor: Anchor, id: &'static str| {
        label(
            at,
            t!(i18n, symbol_kind_message(kind)).into(),
            anchor,
            TextRole::Note,
            Ink::Tertiary,
        )
        .linked(id)
    };
    let mut parts = Vec::new();
    match d.inside_edge_symbol {
        PinSymbolKind::None => {}
        PinSymbolKind::ClockEdge => parts.push(
            Part::new(
                Primitive::Polyline {
                    points: vec![
                        layout.at(body_end, 0.0, 6.0),
                        layout.at(body_end, 8.0, 0.0),
                        layout.at(body_end, 0.0, -6.0),
                    ],
                    is_closed: false,
                },
                Style::edge(),
            )
            .linked(field::PIN_IN_EDGE),
        ),
        kind => parts.push(note(
            layout.at(body_end, NAME_INSET, 14.0),
            kind,
            layout.inward_anchor(),
            field::PIN_IN_EDGE,
        )),
    }
    match d.outside_edge_symbol {
        PinSymbolKind::None => {}
        PinSymbolKind::Dot => parts.push(
            Part::new(
                Primitive::Circle {
                    center: layout.at(body_end, -DOT_RADIUS, 0.0),
                    radius: DOT_RADIUS,
                },
                Style::edge(),
            )
            .linked(field::PIN_OUT_EDGE),
        ),
        kind => parts.push(note(
            layout.at(body_end, -4.0, 12.0),
            kind,
            layout.outward_anchor(),
            field::PIN_OUT_EDGE,
        )),
    }
    if d.inside_symbol != PinSymbolKind::None {
        parts.push(note(
            layout.at(body_end, NAME_INSET, -14.0),
            d.inside_symbol,
            layout.inward_anchor(),
            field::PIN_INSIDE,
        ));
    }
    if d.outside_symbol != PinSymbolKind::None {
        parts.push(note(
            layout.at(layout.tip, -6.0, 0.0),
            d.outside_symbol,
            layout.outward_anchor(),
            field::PIN_OUTSIDE,
        ));
    }
    parts
}
