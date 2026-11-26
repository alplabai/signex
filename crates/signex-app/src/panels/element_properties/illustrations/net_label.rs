//! A net label on its wire: the name at its justification, the anchor dot,
//! and the Net Name, Font Size and Position callouts.

use iced::Point;
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::schematic::HAlign;
use signex_widgets::property_illustration::{
    Anchor, Illustration, Ink, Leader, Paint, Part, Primitive, Style, Text, TextLabel, TextRole,
    Tint, Weight,
};

use super::{HEIGHT_EM, fields};

const VIEW_W: f32 = 264.0;
const VIEW_H: f32 = 120.0;
/// The wire the label sits on, view units.
const WIRE_Y: f32 = 72.0;
/// Baseline row of the name, just above the wire.
const NAME_Y: f32 = 62.0;
/// Rough width of one character of the name, view units.
const CHAR_W: f32 = 7.0;
/// Gap between the anchor and the name.
const GAP: f32 = 4.0;

/// Where the anchor sits on the wire and how the name hangs off it.
pub(super) fn anchor_of(justify: HAlign) -> (f32, Anchor) {
    match justify {
        HAlign::Left => (96.0, Anchor::Start),
        HAlign::Center => (132.0, Anchor::Middle),
        HAlign::Right => (168.0, Anchor::End),
    }
}

/// The net label `name`, justified `justify`.
pub fn net_label(name: &str, justify: HAlign, i18n: &I18n) -> Illustration {
    let (anchor_x, anchor) = anchor_of(justify);
    let width = CHAR_W * name.chars().count().max(1) as f32;
    let (left, text_x) = match anchor {
        Anchor::Start => (anchor_x + GAP, anchor_x + GAP),
        Anchor::Middle => (anchor_x - width / 2.0, anchor_x),
        Anchor::End => (anchor_x - GAP - width, anchor_x - GAP),
    };
    let right = left + width;
    let wire = Part::new(
        Primitive::Line {
            from: Point::new(16.0, WIRE_Y),
            to: Point::new(248.0, WIRE_Y),
        },
        Style::outline(Ink::Secondary, Weight::Regular),
    );
    let label = Part::label(
        TextLabel {
            at: Point::new(text_x, NAME_Y),
            text: UserText::new(name.to_string()).into(),
            anchor,
            role: TextRole::Mono,
        },
        Ink::Secondary,
    )
    .linked_all([fields::NET_NAME, fields::FONT_SIZE]);
    let anchor_dot = Part::new(
        Primitive::Circle {
            center: Point::new(anchor_x, WIRE_Y),
            radius: 2.5,
        },
        Style::filled(Paint::Ink(Ink::Secondary), Tint::Solid),
    )
    .linked(fields::POSITION);
    let leader = |at: Point, label_at: Point, text: Text, anchor: Anchor| {
        Part::leader(Leader {
            at,
            label_at,
            label: text,
            anchor,
        })
    };
    let middle = (left + right) / 2.0;
    Illustration::new(VIEW_W, VIEW_H)
        .height_em(HEIGHT_EM)
        .part(wire)
        .part(label)
        .part(anchor_dot)
        .part(
            leader(
                Point::new(middle, NAME_Y - 8.0),
                Point::new(middle, 24.0),
                t!(i18n, ids::PROPERTIES_NET_NAME).into(),
                Anchor::Middle,
            )
            .linked(fields::NET_NAME),
        )
        .part(
            leader(
                Point::new(right + 2.0, NAME_Y),
                Point::new(right + 28.0, 40.0),
                t!(i18n, ids::PROPERTIES_FONT_SIZE).into(),
                Anchor::Start,
            )
            .linked(fields::FONT_SIZE),
        )
        .part(
            leader(
                Point::new(anchor_x, WIRE_Y),
                Point::new(anchor_x, 104.0),
                t!(i18n, ids::PROPERTIES_POSITION).into(),
                Anchor::Middle,
            )
            .linked(fields::POSITION),
        )
}
