//! "Part on the sheet" (mockup row 42): the placed part as line art above General. The
//! symbol is drawn as a body with its pins, pin numbers, net stubs,
//! designator and value; mirror is drawn, rotation is not. DNP draws a cross
//! and Locked a lock mark. Designator, Value and Position are callouts linked
//! to their fields.

use iced::{Point, Radians, Rectangle};
use signex_i18n::UserText;
use signex_widgets::property_illustration::{
    Anchor, FieldId, Illustration, Ink, Leader, Paint, Part, Primitive, Style, Text, TextLabel,
    TextRole, Tint, Weight,
};

use super::super::pins::pin_rows;

pub(in crate::panels) const DESIGNATOR: FieldId = FieldId::new("designator");
pub(in crate::panels) const VALUE: FieldId = FieldId::new("value");
pub(in crate::panels) const POSITION: FieldId = FieldId::new("position");
pub(in crate::panels) const MIRROR_X: FieldId = FieldId::new("mirror-x");
pub(in crate::panels) const MIRROR_Y: FieldId = FieldId::new("mirror-y");
pub(in crate::panels) const LOCKED: FieldId = FieldId::new("locked");
pub(in crate::panels) const DNP: FieldId = FieldId::new("dnp");

/// View box of the drawing, the size of the approved mockup drawing.
const VIEW_W: f32 = 264.0;
const VIEW_H: f32 = 140.0;
/// Where the part's origin sits.
const CX: f32 = 132.0;
const CY: f32 = 60.0;
const BODY_HALF_W: f32 = 22.0;
/// Pin tip, net stub end and net label, from the origin.
const PIN_TIP: f32 = 46.0;
const STUB_END: f32 = 62.0;
const NET_AT: f32 = 66.0;
const PIN_PITCH: f32 = 14.0;
/// Most pins drawn on one side; a larger part shows its first pins.
const MAX_PINS_PER_SIDE: usize = 3;

/// One pin as the drawing shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::panels) struct SketchPin {
    pub number: String,
    pub net: Option<String>,
}

/// What the drawing shows of a placed part, read from its Properties rows.
/// It has no rotation: rotation is not drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::panels) struct PartSketch {
    pub designator: String,
    /// The value as the sheet shows it: an expression's result.
    pub value: String,
    pub pins: Vec<SketchPin>,
    pub mirror_x: bool,
    pub mirror_y: bool,
    pub locked: bool,
    pub dnp: bool,
}

impl PartSketch {
    /// The sketch of the part `info` describes.
    pub(in crate::panels) fn from_info(info: &[(String, String)]) -> Self {
        let get = |key: &str| {
            info.iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
                .unwrap_or_default()
        };
        let mirrored = |axis: &str| info.iter().any(|(k, v)| k == "Mirror" && v == axis);
        let shows = get("Value Shows");
        let value = if shows.is_empty() {
            get("Value")
        } else {
            shows
        };
        Self {
            designator: get("Reference").to_string(),
            value: value.to_string(),
            pins: pin_rows(info)
                .into_iter()
                .map(|row| SketchPin {
                    number: row.designator,
                    net: row.net,
                })
                .collect(),
            mirror_x: mirrored("X"),
            mirror_y: mirrored("Y"),
            locked: get("Locked") == "Yes",
            dnp: get("DNP") == "Yes",
        }
    }

    /// The pins drawn on the left and on the right, top to bottom.
    fn sides(&self) -> (&[SketchPin], &[SketchPin]) {
        let left = self.pins.len().div_ceil(2);
        let (l, r) = self.pins.split_at(left);
        (
            &l[..l.len().min(MAX_PINS_PER_SIDE)],
            &r[..r.len().min(MAX_PINS_PER_SIDE)],
        )
    }

    fn body_half_h(&self) -> f32 {
        let (l, r) = self.sides();
        (l.len().max(r.len()).max(1) as f32 * PIN_PITCH) / 2.0
    }
}

/// The catalog text the drawing needs, rendered by the caller.
pub(in crate::panels) struct PartLabels {
    pub locked: Text,
    pub dnp: Text,
    /// The corner note of a mirrored part; `None` when not mirrored.
    pub mirror: Option<Text>,
    /// "Position X, Y"; `None` when the position is unknown.
    pub position: Option<Text>,
}

fn label(at: Point, text: impl Into<Text>, anchor: Anchor, role: TextRole) -> TextLabel {
    TextLabel {
        at,
        text: text.into(),
        anchor,
        role,
    }
}

fn user(text: &str) -> Text {
    UserText::new(text.to_string()).into()
}

/// The drawing of `sketch`.
pub(in crate::panels) fn part_illustration(
    sketch: &PartSketch,
    labels: PartLabels,
) -> Illustration {
    let sx = if sketch.mirror_x { -1.0 } else { 1.0 };
    let sy = if sketch.mirror_y { -1.0 } else { 1.0 };
    let half_h = sketch.body_half_h();
    let body = Rectangle {
        x: CX - BODY_HALF_W,
        y: CY - half_h,
        width: BODY_HALF_W * 2.0,
        height: half_h * 2.0,
    };
    let mut ill = Illustration::new(VIEW_W, VIEW_H).part(
        Part::new(
            Primitive::Rect { rect: body },
            Style::edge().with_fill(Paint::Ink(Ink::Secondary), Tint::Light),
        )
        .linked_all([MIRROR_X, MIRROR_Y]),
    );
    let (left, right) = sketch.sides();
    for (side, pins) in [(-1.0_f32, left), (1.0, right)] {
        let first = -(pins.len() as f32 - 1.0) * PIN_PITCH / 2.0;
        for (i, pin) in pins.iter().enumerate() {
            let dir = side * sx;
            let y = CY + (first + i as f32 * PIN_PITCH) * sy;
            ill = ill.parts(pin_parts(pin, dir, y, sy));
        }
    }
    ill = ill
        .part(
            Part::label(
                label(
                    Point::new(CX, CY - half_h - 13.0),
                    user(&sketch.designator),
                    Anchor::Middle,
                    TextRole::Mono,
                ),
                Ink::Secondary,
            )
            .linked(DESIGNATOR),
        )
        .part(
            Part::label(
                label(
                    Point::new(CX, CY + half_h + 13.0),
                    user(&sketch.value),
                    Anchor::Middle,
                    TextRole::Mono,
                ),
                Ink::Secondary,
            )
            .linked(VALUE),
        );
    if sketch.dnp {
        ill = ill.parts(dnp_parts(half_h, labels.dnp));
    }
    if sketch.locked {
        ill = ill.parts(lock_parts(half_h, labels.locked));
    }
    if let Some(note) = labels.mirror {
        let fields = match (sketch.mirror_x, sketch.mirror_y) {
            (true, false) => vec![MIRROR_X],
            (false, true) => vec![MIRROR_Y],
            _ => vec![MIRROR_X, MIRROR_Y],
        };
        ill = ill.part(
            Part::label(
                label(
                    Point::new(VIEW_W - 8.0, 12.0),
                    note,
                    Anchor::End,
                    TextRole::Note,
                ),
                Ink::Faint,
            )
            .linked_all(fields),
        );
    }
    ill.parts(position_parts(labels.position))
}

/// A pin's lead, tip dot, number, net stub and net name. `dir` is the side
/// the pin points to (-1 left, 1 right) after the mirror.
fn pin_parts(pin: &SketchPin, dir: f32, y: f32, sy: f32) -> Vec<Part> {
    let at = |dx: f32| Point::new(CX + dx * dir, y);
    let mut parts = vec![
        Part::new(
            Primitive::Line {
                from: at(BODY_HALF_W),
                to: at(PIN_TIP),
            },
            Style::edge(),
        ),
        Part::new(
            Primitive::Circle {
                center: at(PIN_TIP),
                radius: 1.6,
            },
            Style::filled(Paint::Ink(Ink::Secondary), Tint::Solid),
        ),
        Part::label(
            label(
                Point::new(CX + 34.0 * dir, y - 8.0 * sy),
                user(&pin.number),
                Anchor::Middle,
                TextRole::Note,
            ),
            Ink::Faint,
        ),
    ];
    if let Some(net) = &pin.net {
        let anchor = if dir < 0.0 {
            Anchor::End
        } else {
            Anchor::Start
        };
        parts.push(Part::new(
            Primitive::Line {
                from: at(PIN_TIP),
                to: at(STUB_END),
            },
            Style::outline(Ink::Faint, Weight::Thin),
        ));
        parts.push(Part::label(
            label(at(NET_AT), user(net), anchor, TextRole::Note),
            Ink::Faint,
        ));
    }
    parts
}

/// The not-fitted cross over the body and its leader.
fn dnp_parts(half_h: f32, text: Text) -> Vec<Part> {
    let (dx, dy) = (18.0, half_h.min(12.0));
    let cross = Style::outline(Ink::Error, Weight::Heavy);
    vec![
        Part::new(
            Primitive::Line {
                from: Point::new(CX - dx, CY - dy),
                to: Point::new(CX + dx, CY + dy),
            },
            cross,
        )
        .linked(DNP),
        Part::new(
            Primitive::Line {
                from: Point::new(CX - dx, CY + dy),
                to: Point::new(CX + dx, CY - dy),
            },
            cross,
        )
        .linked(DNP),
        Part::leader(Leader {
            at: Point::new(CX + 14.0, CY + dy - 2.0),
            label_at: Point::new(CX + 52.0, CY + half_h + 27.0),
            label: text,
            anchor: Anchor::Start,
        })
        .linked(DNP),
    ]
}

/// A small padlock above the body's top left corner, and its label.
fn lock_parts(half_h: f32, text: Text) -> Vec<Part> {
    let top = CY - half_h - 23.0;
    let x = CX - 40.0;
    let thin = Style::outline(Ink::Secondary, Weight::Thin);
    vec![
        Part::new(
            Primitive::Rect {
                rect: Rectangle {
                    x,
                    y: top,
                    width: 8.0,
                    height: 6.0,
                },
            },
            thin,
        )
        .linked(LOCKED),
        Part::new(
            Primitive::Arc {
                center: Point::new(x + 4.0, top),
                radius: 2.5,
                start: Radians(std::f32::consts::PI),
                end: Radians(std::f32::consts::TAU),
            },
            thin,
        )
        .linked(LOCKED),
        Part::label(
            label(
                Point::new(x + 10.0, top + 3.0),
                text,
                Anchor::Start,
                TextRole::Note,
            ),
            Ink::Faint,
        )
        .linked(LOCKED),
    ]
}

/// The origin cross and the Position callout.
fn position_parts(position: Option<Text>) -> Vec<Part> {
    let cross = Style::outline(Ink::Secondary, Weight::Thin);
    let mut parts = vec![
        Part::new(
            Primitive::Line {
                from: Point::new(CX - 4.0, CY),
                to: Point::new(CX + 4.0, CY),
            },
            cross,
        )
        .linked(POSITION),
        Part::new(
            Primitive::Line {
                from: Point::new(CX, CY - 4.0),
                to: Point::new(CX, CY + 4.0),
            },
            cross,
        )
        .linked(POSITION),
    ];
    if let Some(text) = position {
        parts.push(
            Part::leader(Leader {
                at: Point::new(CX, CY + 4.0),
                label_at: Point::new(CX - 40.0, VIEW_H - 28.0),
                label: text,
                anchor: Anchor::Middle,
            })
            .linked(POSITION),
        );
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> Vec<(String, String)> {
        [
            ("Type", "Symbol"),
            ("Reference", "R12"),
            ("Value", "=RFB_TOP"),
            ("Value Shows", "10k"),
            ("Rotation", "0"),
            ("Locked", "No"),
            ("DNP", "No"),
            ("Pin: 1", "~\tPassive\tNo"),
            ("Pin: 2", "~\tPassive\tNo"),
            ("Pin Net: 1", "/VFB"),
            ("Pin Net: 2", "+3V3"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
    }

    fn labels() -> PartLabels {
        PartLabels {
            locked: user("Locked"),
            dnp: user("DNP"),
            mirror: None,
            position: None,
        }
    }

    fn linked(ill: &Illustration, field: &FieldId) -> usize {
        ill.visible_parts()
            .filter(|p| p.is_linked_to(field))
            .count()
    }

    /// The x of pin 1's tip dot.
    fn pin_one_tip_x(ill: &Illustration) -> f32 {
        ill.visible_parts()
            .find_map(|p| match p.primitive {
                Primitive::Circle { center, .. } => Some(center.x),
                _ => None,
            })
            .expect("a tip dot")
    }

    #[test]
    fn the_sketch_reads_the_rows_and_shows_the_evaluated_value() {
        let sketch = PartSketch::from_info(&info());
        assert_eq!(sketch.designator, "R12");
        assert_eq!(sketch.value, "10k");
        assert_eq!(sketch.pins.len(), 2);
        assert_eq!(sketch.pins[0].net.as_deref(), Some("/VFB"));
        assert!(!sketch.dnp && !sketch.locked && !sketch.mirror_x);
    }

    #[test]
    fn the_dnp_cross_is_drawn_only_on_a_dnp_part() {
        let mut rows = info();
        assert_eq!(
            linked(
                &part_illustration(&PartSketch::from_info(&rows), labels()),
                &DNP
            ),
            0
        );
        rows.retain(|(k, _)| k != "DNP");
        rows.push(("DNP".into(), "Yes".into()));
        let ill = part_illustration(&PartSketch::from_info(&rows), labels());
        // Two strokes of the cross and the leader.
        assert_eq!(linked(&ill, &DNP), 3);
    }

    #[test]
    fn the_lock_mark_is_drawn_only_on_a_locked_part() {
        let mut rows = info();
        assert_eq!(
            linked(
                &part_illustration(&PartSketch::from_info(&rows), labels()),
                &LOCKED
            ),
            0
        );
        rows.retain(|(k, _)| k != "Locked");
        rows.push(("Locked".into(), "Yes".into()));
        let ill = part_illustration(&PartSketch::from_info(&rows), labels());
        assert!(linked(&ill, &LOCKED) > 0);
    }

    #[test]
    fn mirror_x_puts_pin_one_on_the_other_side() {
        let plain = pin_one_tip_x(&part_illustration(
            &PartSketch::from_info(&info()),
            labels(),
        ));
        let mut rows = info();
        rows.push(("Mirror".into(), "X".into()));
        let sketch = PartSketch::from_info(&rows);
        assert!(sketch.mirror_x);
        let mirrored = pin_one_tip_x(&part_illustration(&sketch, labels()));
        assert!(
            plain < CX && mirrored > CX,
            "plain {plain}, mirrored {mirrored}"
        );
        assert!((plain - CX + (mirrored - CX)).abs() < 1e-3);
    }

    #[test]
    fn rotation_is_not_drawn() {
        let mut rotated = info();
        rotated.retain(|(k, _)| k != "Rotation");
        rotated.push(("Rotation".into(), "90".into()));
        assert_eq!(
            PartSketch::from_info(&info()),
            PartSketch::from_info(&rotated)
        );
    }

    #[test]
    fn designator_value_and_position_are_callouts_of_their_fields() {
        let mut l = labels();
        l.position = Some(user("Position 1, 2"));
        let ill = part_illustration(&PartSketch::from_info(&info()), l);
        assert_eq!(linked(&ill, &DESIGNATOR), 1);
        assert_eq!(linked(&ill, &VALUE), 1);
        assert_eq!(linked(&ill, &POSITION), 3);
    }
}
