//! A wire or a bus: the segment at its width preset, the Start dot, the End
//! mark and the derived length. Callouts stay off the stroke: Start / End
//! under the ends of a horizontal wire and left of a vertical bus, the
//! length label beyond its dimension line, Width on a short leader below.

use iced::Point;
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::schematic::WireWidthPreset;
use signex_widgets::property_illustration::{
    Anchor, Callout, Cap, Illustration, Ink, Leader, Paint, Part, Primitive, Style, Text,
    TextLabel, TextRole, Tint,
};

use super::{HEIGHT_EM, fields};

/// Whether the run is drawn as a horizontal wire or a vertical bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunShape {
    Wire,
    Bus,
}

const VIEW_W: f32 = 264.0;
const VIEW_H: f32 = 120.0;
/// Radius of the Start dot and the End ring, view units.
const MARK_R: f32 = 3.0;

/// The drawn stroke of a width preset, view units.
pub(super) fn band_width(preset: WireWidthPreset) -> f32 {
    match preset {
        WireWidthPreset::Smallest => 1.5,
        WireWidthPreset::Small => 3.0,
        WireWidthPreset::Medium => 5.0,
        WireWidthPreset::Large => 7.0,
    }
}

/// The run as drawn: `length` is the derived length as the Length row shows
/// it (value and unit).
pub fn run(shape: RunShape, width: WireWidthPreset, length: String, i18n: &I18n) -> Illustration {
    let (start, end) = match shape {
        RunShape::Wire => (Point::new(48.0, 56.0), Point::new(216.0, 56.0)),
        RunShape::Bus => (Point::new(132.0, 20.0), Point::new(132.0, 96.0)),
    };
    let segment = Part::new(
        Primitive::Band {
            points: vec![start, end],
            width: band_width(width),
            cap: Cap::Round,
        },
        Style::edge().with_fill(Paint::Ink(Ink::Secondary), Tint::Body),
    )
    .linked(fields::WIDTH);
    let start_dot = Part::new(
        Primitive::Circle {
            center: start,
            radius: MARK_R,
        },
        Style::filled(Paint::Ink(Ink::Secondary), Tint::Solid),
    )
    .linked(fields::START);
    let end_ring = Part::new(
        Primitive::Circle {
            center: end,
            radius: MARK_R,
        },
        Style::edge(),
    )
    .linked(fields::END);
    let name = |at: Point, anchor: Anchor, text: Text| {
        Part::label(
            TextLabel {
                at,
                text,
                anchor,
                role: TextRole::Caption,
            },
            Ink::Faint,
        )
    };
    let start_text: Text = t!(i18n, ids::PROPERTIES_CALLOUT_START).into();
    let end_text: Text = t!(i18n, ids::PANELS_PLACED_END).into();
    let width_text: Text = t!(i18n, ids::PROPERTIES_WIRE_WIDTH).into();
    let (names, length_callout, width_leader) = match shape {
        RunShape::Wire => (
            [
                name(Point::new(start.x, 76.0), Anchor::Middle, start_text),
                name(Point::new(end.x, 76.0), Anchor::Middle, end_text),
            ],
            Callout::new(Point::new(start.x, 24.0), Point::new(end.x, 24.0))
                .label(UserText::new(length))
                .label_at(Point::new(132.0, 14.0), Anchor::Middle)
                .extension(Point::new(start.x, 48.0), Point::new(start.x, 20.0))
                .extension(Point::new(end.x, 48.0), Point::new(end.x, 20.0)),
            Leader {
                at: Point::new(132.0, start.y),
                label_at: Point::new(150.0, 100.0),
                label: width_text,
                anchor: Anchor::Start,
            },
        ),
        RunShape::Bus => (
            [
                name(Point::new(112.0, start.y), Anchor::End, start_text),
                name(Point::new(112.0, end.y), Anchor::End, end_text),
            ],
            Callout::new(Point::new(184.0, start.y), Point::new(184.0, end.y))
                .label(UserText::new(length))
                .label_at(Point::new(192.0, 58.0), Anchor::Start)
                .extension(Point::new(140.0, start.y), Point::new(188.0, start.y))
                .extension(Point::new(140.0, end.y), Point::new(188.0, end.y)),
            Leader {
                at: Point::new(start.x, 76.0),
                label_at: Point::new(150.0, 110.0),
                label: width_text,
                anchor: Anchor::Start,
            },
        ),
    };
    let [start_name, end_name] = names;
    Illustration::new(VIEW_W, VIEW_H)
        .height_em(HEIGHT_EM)
        .part(segment)
        .part(start_dot)
        .part(end_ring)
        .part(start_name.linked(fields::START))
        .part(end_name.linked(fields::END))
        .part(Part::callout(length_callout).linked(fields::LENGTH))
        .part(Part::leader(width_leader).linked(fields::WIDTH))
}
