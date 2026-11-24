//! A global or hierarchical port: the port shape on a wire stub with its
//! name inside, and the Port Name, Font Size and Position callouts.

use iced::Point;
use signex_i18n::{I18n, UserText, ids, t};
use signex_widgets::property_illustration::{
    Anchor, Illustration, Ink, Leader, Paint, Part, Primitive, Style, Text, TextLabel, TextRole,
    Tint, Weight,
};

use super::{HEIGHT_EM, fields};

const VIEW_W: f32 = 264.0;
const VIEW_H: f32 = 120.0;
/// Centre line of the port and its wire.
const MID_Y: f32 = 64.0;
/// Half the port's height.
const HALF_H: f32 = 12.0;
/// The point where the wire meets the port.
const TIP_X: f32 = 72.0;
/// Rough width of one character of the name, view units.
const CHAR_W: f32 = 7.0;

/// The port named `name`.
pub fn port(name: &str, i18n: &I18n) -> Illustration {
    let width = (CHAR_W * name.chars().count().max(1) as f32 + 24.0).min(150.0);
    let body_x = TIP_X + HALF_H;
    let far_x = body_x + width;
    let outline = Part::new(
        Primitive::Polyline {
            points: vec![
                Point::new(TIP_X, MID_Y),
                Point::new(body_x, MID_Y - HALF_H),
                Point::new(far_x, MID_Y - HALF_H),
                Point::new(far_x, MID_Y + HALF_H),
                Point::new(body_x, MID_Y + HALF_H),
            ],
            is_closed: true,
        },
        Style::edge().with_fill(Paint::Ink(Ink::Secondary), Tint::Light),
    );
    let wire = Part::new(
        Primitive::Line {
            from: Point::new(16.0, MID_Y),
            to: Point::new(TIP_X, MID_Y),
        },
        Style::outline(Ink::Secondary, Weight::Regular),
    );
    let middle = (body_x + far_x) / 2.0;
    let label = Part::label(
        TextLabel {
            at: Point::new(middle, MID_Y),
            text: UserText::new(name.to_string()).into(),
            anchor: Anchor::Middle,
            role: TextRole::Mono,
        },
        Ink::Secondary,
    )
    .linked_all([fields::PORT_NAME, fields::FONT_SIZE]);
    let anchor_dot = Part::new(
        Primitive::Circle {
            center: Point::new(TIP_X, MID_Y),
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
    Illustration::new(VIEW_W, VIEW_H)
        .height_em(HEIGHT_EM)
        .part(wire)
        .part(outline)
        .part(label)
        .part(anchor_dot)
        .part(
            leader(
                Point::new(middle, MID_Y - HALF_H),
                Point::new(middle, 22.0),
                t!(i18n, ids::PROPERTIES_PORT_NAME).into(),
                Anchor::Middle,
            )
            .linked(fields::PORT_NAME),
        )
        .part(
            leader(
                Point::new(far_x, MID_Y + HALF_H),
                Point::new(far_x + 12.0, 100.0),
                t!(i18n, ids::PROPERTIES_FONT_SIZE).into(),
                Anchor::Start,
            )
            .linked(fields::FONT_SIZE),
        )
        .part(
            leader(
                Point::new(TIP_X, MID_Y),
                Point::new(TIP_X, 100.0),
                t!(i18n, ids::PROPERTIES_POSITION).into(),
                Anchor::Middle,
            )
            .linked(fields::POSITION),
        )
}
