//! The placed-pin illustration (#50): the body edge, the focused pin with its
//! length, number, name, electrical-type glyph, hotspot and net, and the
//! neighbour pins drawn faint. Read-only: pin fields change in the symbol
//! editor.

use iced::Point;
use signex_i18n::UserText;
use signex_widgets::property_illustration::{
    Anchor, Callout, Cap, FieldId, Illustration, Ink, Leader, Paint, Part, Primitive, Style, Text,
    TextLabel, TextRole, Tint, Weight,
};

pub(in crate::panels) const NUMBER: FieldId = FieldId::new("pin-number");
pub(in crate::panels) const NAME: FieldId = FieldId::new("pin-name");
pub(in crate::panels) const TYPE: FieldId = FieldId::new("pin-type");
pub(in crate::panels) const NET: FieldId = FieldId::new("pin-net");
pub(in crate::panels) const LENGTH: FieldId = FieldId::new("pin-length");
pub(in crate::panels) const HIDDEN: FieldId = FieldId::new("pin-hidden");

const VIEW_W: f32 = 264.0;
const VIEW_H: f32 = 140.0;
/// The body edge and the focused pin's row.
const BODY_X: f32 = 96.0;
const PIN_Y: f32 = 66.0;
/// View units per mm of pin length.
const UNITS_PER_MM: f32 = 26.0;
/// The drawn length stays readable for very short and very long pins.
const MIN_DRAWN_MM: f32 = 1.0;
const MAX_DRAWN_MM: f32 = 3.5;
/// Rows of the faint neighbour pins, above and below the focused one.
const NEIGHBOUR_Y: [f32; 2] = [30.0, 102.0];
const NET_BAND: f32 = 40.0;

/// A neighbour pin, drawn faint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::panels) struct NeighbourPin {
    pub number: String,
    pub name: String,
}

/// What the drawing shows of the focused pin.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::panels) struct PinSketch {
    pub number: String,
    pub name: String,
    /// The engine's type label ("Output", "Power input", ...).
    pub electrical_type: String,
    pub net: Option<String>,
    pub length_mm: f32,
    pub hidden: bool,
    /// The pin before and the pin after it, when there are.
    pub neighbours: [Option<NeighbourPin>; 2],
}

/// The catalog text the drawing needs, rendered by the caller.
pub(in crate::panels) struct PinLabels {
    /// "Type · Output".
    pub type_leader: Text,
    /// "Length 2.54 mm".
    pub length: Text,
}

fn user(text: &str) -> Text {
    UserText::new(text.to_string()).into()
}

fn label(at: Point, text: Text, anchor: Anchor, role: TextRole) -> TextLabel {
    TextLabel {
        at,
        text,
        anchor,
        role,
    }
}

/// The electrical-type glyph at the body edge, as a closed outline, or
/// `None` for a type drawn without one (passive and the rest).
pub(in crate::panels) fn type_glyph(electrical_type: &str) -> Option<Vec<Point>> {
    let (x, y) = (BODY_X, PIN_Y);
    let p = Point::new;
    match electrical_type {
        "Output" | "Power output" => Some(vec![
            p(x + 2.0, y - 4.0),
            p(x + 9.0, y),
            p(x + 2.0, y + 4.0),
        ]),
        "Input" => Some(vec![
            p(x + 9.0, y - 4.0),
            p(x + 2.0, y),
            p(x + 9.0, y + 4.0),
        ]),
        "Power input" | "Ground" => Some(vec![
            p(x + 2.0, y - 3.0),
            p(x + 8.0, y - 3.0),
            p(x + 8.0, y + 3.0),
            p(x + 2.0, y + 3.0),
        ]),
        "Bidirectional" | "Tri-state" | "Open drain (low)" | "Open drain (high)" => Some(vec![
            p(x + 2.0, y),
            p(x + 6.0, y - 4.0),
            p(x + 10.0, y),
            p(x + 6.0, y + 4.0),
        ]),
        _ => None,
    }
}

/// The drawing of `pin`.
pub(in crate::panels) fn pin_illustration(pin: &PinSketch, labels: PinLabels) -> Illustration {
    let len = pin.length_mm.clamp(MIN_DRAWN_MM, MAX_DRAWN_MM) * UNITS_PER_MM;
    let tip = BODY_X + len;
    let mut ill = Illustration::new(VIEW_W, VIEW_H).part(Part::new(
        Primitive::Polyline {
            points: vec![
                Point::new(24.0, 14.0),
                Point::new(BODY_X, 14.0),
                Point::new(BODY_X, 126.0),
                Point::new(24.0, 126.0),
            ],
            is_closed: false,
        },
        Style::edge().with_fill(Paint::Ink(Ink::Secondary), Tint::Wash),
    ));
    for (neighbour, y) in pin.neighbours.iter().zip(NEIGHBOUR_Y) {
        if let Some(n) = neighbour {
            ill = ill.parts(neighbour_parts(n, y, len));
        }
    }
    let lead = if pin.hidden {
        Style::dashed()
    } else {
        Style::edge()
    };
    ill = ill.part(
        Part::new(
            Primitive::Line {
                from: Point::new(BODY_X, PIN_Y),
                to: Point::new(tip, PIN_Y),
            },
            lead,
        )
        .linked_all([LENGTH, HIDDEN]),
    );
    if let Some(points) = type_glyph(&pin.electrical_type) {
        ill = ill.part(
            Part::new(
                Primitive::Polyline {
                    points,
                    is_closed: true,
                },
                Style::outline(Ink::Secondary, Weight::Thin),
            )
            .linked(TYPE),
        );
    }
    ill = ill
        .part(
            Part::leader(Leader {
                at: Point::new(BODY_X + 6.0, PIN_Y - 4.0),
                label_at: Point::new(BODY_X + 14.0, PIN_Y - 22.0),
                label: labels.type_leader,
                anchor: Anchor::Start,
            })
            .linked(TYPE),
        )
        .part(
            Part::label(
                label(
                    Point::new(BODY_X - 5.0, PIN_Y),
                    user(&pin.name),
                    Anchor::End,
                    TextRole::Mono,
                ),
                Ink::Secondary,
            )
            .linked(NAME),
        )
        .part(
            Part::label(
                label(
                    Point::new(BODY_X + len / 2.0 + 6.0, PIN_Y - 8.0),
                    user(&pin.number),
                    Anchor::Middle,
                    TextRole::Mono,
                ),
                Ink::Secondary,
            )
            .linked(NUMBER),
        )
        .part(Part::new(
            Primitive::Circle {
                center: Point::new(tip, PIN_Y),
                radius: 2.0,
            },
            Style::filled(Paint::Ink(Ink::Secondary), Tint::Solid),
        ));
    if let Some(net) = &pin.net {
        ill = ill
            .part(
                Part::new(
                    Primitive::Band {
                        points: vec![Point::new(tip, PIN_Y), Point::new(tip + NET_BAND, PIN_Y)],
                        width: 3.0,
                        cap: Cap::Round,
                    },
                    Style::edge().with_fill(Paint::Ink(Ink::Secondary), Tint::Strong),
                )
                .linked(NET),
            )
            .part(
                Part::label(
                    label(
                        Point::new(tip + NET_BAND + 4.0, PIN_Y),
                        user(net),
                        Anchor::Start,
                        TextRole::Note,
                    ),
                    Ink::Faint,
                )
                .linked(NET),
            );
    }
    ill.part(
        Part::callout(
            Callout::new(
                Point::new(BODY_X, PIN_Y + 18.0),
                Point::new(tip, PIN_Y + 18.0),
            )
            .label(labels.length)
            .label_at(Point::new(BODY_X + len / 2.0, PIN_Y + 26.0), Anchor::Middle)
            .extension(
                Point::new(BODY_X, PIN_Y + 4.0),
                Point::new(BODY_X, PIN_Y + 21.0),
            )
            .extension(Point::new(tip, PIN_Y + 4.0), Point::new(tip, PIN_Y + 21.0)),
        )
        .linked(LENGTH),
    )
}

/// A neighbour pin: its lead, name and number, all faint.
fn neighbour_parts(pin: &NeighbourPin, y: f32, len: f32) -> Vec<Part> {
    vec![
        Part::new(
            Primitive::Line {
                from: Point::new(BODY_X, y),
                to: Point::new(BODY_X + len, y),
            },
            Style::outline(Ink::Faint, Weight::Regular),
        ),
        Part::label(
            label(
                Point::new(BODY_X - 4.0, y),
                user(&pin.name),
                Anchor::End,
                TextRole::Note,
            ),
            Ink::Faint,
        ),
        Part::label(
            label(
                Point::new(BODY_X + len / 2.0, y - 6.0),
                user(&pin.number),
                Anchor::Middle,
                TextRole::Note,
            ),
            Ink::Faint,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sketch() -> PinSketch {
        PinSketch {
            number: "5".into(),
            name: "SW".into(),
            electrical_type: "Output".into(),
            net: Some("/SW".into()),
            length_mm: 2.54,
            hidden: false,
            neighbours: [
                Some(NeighbourPin {
                    number: "4".into(),
                    name: "PG".into(),
                }),
                None,
            ],
        }
    }

    fn labels() -> PinLabels {
        PinLabels {
            type_leader: user("Type · Output"),
            length: user("Length 2.54 mm"),
        }
    }

    fn linked(ill: &Illustration, field: &FieldId) -> usize {
        ill.visible_parts()
            .filter(|p| p.is_linked_to(field))
            .count()
    }

    #[test]
    fn the_focused_pin_shows_every_fact_the_rows_name() {
        let ill = pin_illustration(&sketch(), labels());
        for field in [&NUMBER, &NAME, &TYPE, &NET, &LENGTH, &HIDDEN] {
            assert!(linked(&ill, field) > 0, "{field}");
        }
    }

    #[test]
    fn an_open_pin_draws_no_net() {
        let mut pin = sketch();
        pin.net = None;
        assert_eq!(linked(&pin_illustration(&pin, labels()), &NET), 0);
    }

    #[test]
    fn a_passive_pin_has_no_type_glyph() {
        assert!(type_glyph("Passive").is_none());
        assert!(type_glyph("Output").is_some());
    }

    #[test]
    fn a_hidden_pin_is_drawn_dashed() {
        let mut pin = sketch();
        pin.hidden = true;
        let ill = pin_illustration(&pin, labels());
        let lead = ill
            .visible_parts()
            .find(|p| p.is_linked_to(&HIDDEN))
            .expect("the lead");
        assert!(lead.style.stroke.is_some_and(|s| s.is_dashed));
    }

    #[test]
    fn neighbour_pins_are_drawn_faint_and_unlinked() {
        let ill = pin_illustration(&sketch(), labels());
        let faint = ill
            .visible_parts()
            .filter(|p| p.fields.is_empty())
            .filter(|p| {
                p.style
                    .stroke
                    .is_some_and(|s| s.paint == Paint::Ink(Ink::Faint))
            })
            .count();
        assert_eq!(faint, 3, "lead, name and number of the one neighbour");
    }
}
