//! Line-art illustrations of the next object (#2532): the net label at its
//! justification on a wire, and the wire from Start to End at its width
//! preset. Callouts carry the names of the fields; rotation is not drawn.

use iced::Point;
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::schematic::{HAlign, VAlign, WireWidthPreset};
use signex_widgets::property_illustration::{
    Anchor, Callout, Cap, FieldId, Illustration, Ink, Leader, Paint, Part, Primitive, Style,
    TextLabel, TextRole, Tint,
};

/// The drawing box, in view units.
const VIEW_W: f32 = 264.0;
const VIEW_H: f32 = 140.0;
/// Height of the illustration, in body text lines.
const HEIGHT_EM: f32 = 9.0;
/// Half the height a label takes on the drawing, in view units.
const LABEL_HALF_HEIGHT: f32 = 7.0;
/// Radius of the point marks (the label's point, a wire's ends).
const MARK_RADIUS: f32 = 3.0;

pub(super) const FIELD_NET_NAME: FieldId = FieldId::new("net_name");
pub(super) const FIELD_FONT_SIZE: FieldId = FieldId::new("font_size");
pub(super) const FIELD_POSITION: FieldId = FieldId::new("position");
pub(super) const FIELD_START: FieldId = FieldId::new("start");
pub(super) const FIELD_END: FieldId = FieldId::new("end");
pub(super) const FIELD_LENGTH: FieldId = FieldId::new("length");
pub(super) const FIELD_WIDTH: FieldId = FieldId::new("width");

/// A free text label on the drawing.
fn label(at: Point, text: impl Into<signex_widgets::Label<'static>>, anchor: Anchor) -> TextLabel {
    TextLabel {
        at,
        text: text.into(),
        anchor,
        role: TextRole::Mono,
    }
}

/// A filled point mark linked to `field`.
fn mark(center: Point, field: FieldId) -> Part {
    Part::new(
        Primitive::Circle {
            center,
            radius: MARK_RADIUS,
        },
        Style::filled(Paint::Ink(Ink::Accent), Tint::Solid),
    )
    .linked(field)
}

/// Where the label text sits for the anchor (`h`, `v`) at `at`: the text
/// runs away from its anchor, above the wire for a bottom anchor.
fn label_text_at(at: Point, (h, v): (HAlign, VAlign)) -> (Point, Anchor) {
    let gap = MARK_RADIUS + 1.0;
    let anchor = match h {
        HAlign::Left => Anchor::Start,
        HAlign::Center => Anchor::Middle,
        HAlign::Right => Anchor::End,
    };
    let dx = match h {
        HAlign::Left => gap,
        HAlign::Center => 0.0,
        HAlign::Right => -gap,
    };
    let dy = match v {
        VAlign::Bottom => -(LABEL_HALF_HEIGHT + gap),
        VAlign::Center => 0.0,
        VAlign::Top => LABEL_HALF_HEIGHT + gap,
    };
    (Point::new(at.x + dx, at.y + dy), anchor)
}

/// The net label to place: the wire it lands on, its point (Position), its
/// text at the justification (Net Name) and its height (Font Size).
pub(super) fn net_label(i18n: &I18n, name: &str, anchor: (HAlign, VAlign)) -> Illustration {
    let wire_y = VIEW_H * 0.6;
    let at = Point::new(VIEW_W / 2.0, wire_y);
    let (text_at, text_anchor) = label_text_at(at, anchor);
    let wire = Part::new(
        Primitive::Line {
            from: Point::new(VIEW_W * 0.08, wire_y),
            to: Point::new(VIEW_W * 0.92, wire_y),
        },
        Style::edge(),
    );
    let text = Part::label(
        label(text_at, UserText::new(name.to_owned()), text_anchor),
        Ink::Secondary,
    )
    .linked_all([FIELD_NET_NAME, FIELD_FONT_SIZE]);
    let size_x = VIEW_W * 0.9;
    let font_size = Part::callout(
        Callout::new(
            Point::new(size_x, text_at.y - LABEL_HALF_HEIGHT),
            Point::new(size_x, text_at.y + LABEL_HALF_HEIGHT),
        )
        .label(t!(i18n, ids::PROPERTIES_FONT_SIZE))
        .label_at(
            Point::new(size_x, text_at.y - LABEL_HALF_HEIGHT - 4.0),
            Anchor::End,
        ),
    )
    .linked(FIELD_FONT_SIZE);
    let position = Part::leader(Leader {
        at,
        label_at: Point::new(at.x, VIEW_H * 0.9),
        label: t!(i18n, ids::PROPERTIES_POSITION).into(),
        anchor: Anchor::Middle,
    })
    .linked(FIELD_POSITION);
    Illustration::new(VIEW_W, VIEW_H)
        .height_em(HEIGHT_EM)
        .parts([wire, text, font_size, position, mark(at, FIELD_POSITION)])
}

/// The stroke width a preset draws at, in view units.
fn preset_width(preset: WireWidthPreset) -> f32 {
    match preset {
        WireWidthPreset::Smallest => 1.0,
        WireWidthPreset::Small => 2.0,
        WireWidthPreset::Medium => 4.0,
        WireWidthPreset::Large => 6.0,
    }
}

/// The ends of the drawn wire: centred, along the direction from `start`
/// to `end` (sheet Y runs down, as on the drawing); level when they meet.
fn wire_ends(start: (f64, f64), end: (f64, f64)) -> (Point, Point) {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let len = dx.hypot(dy);
    let (ux, uy) = if len > f64::EPSILON {
        ((dx / len) as f32, (dy / len) as f32)
    } else {
        (1.0, 0.0)
    };
    let centre = Point::new(VIEW_W / 2.0, VIEW_H * 0.45);
    let (half_x, half_y) = (VIEW_W * 0.32, VIEW_H * 0.28);
    (
        Point::new(centre.x - ux * half_x, centre.y - uy * half_y),
        Point::new(centre.x + ux * half_x, centre.y + uy * half_y),
    )
}

/// The wire to place: Start to End at its width preset and colour, with
/// the Start, End and Length callouts.
pub(super) fn wire(
    i18n: &I18n,
    (start, end): ((f64, f64), (f64, f64)),
    preset: WireWidthPreset,
    color: Option<iced::Color>,
) -> Illustration {
    let (a, b) = wire_ends(start, end);
    let paint = color.map_or(Paint::Ink(Ink::Secondary), Paint::Document);
    let band = Part::new(
        Primitive::Band {
            points: vec![a, b],
            width: preset_width(preset),
            cap: Cap::Round,
        },
        Style::filled(paint, Tint::Solid),
    )
    .linked(FIELD_WIDTH);
    let leader = |at: Point, dy: f32, id: signex_i18n::MessageId, field: FieldId| {
        Part::leader(Leader {
            at,
            label_at: Point::new(at.x, at.y + dy),
            label: t!(i18n, id).into(),
            anchor: Anchor::Middle,
        })
        .linked(field)
    };
    let offset = VIEW_H * 0.2;
    let (la, lb) = (Point::new(a.x, a.y + offset), Point::new(b.x, b.y + offset));
    let length = Part::callout(
        Callout::new(la, lb)
            .label(t!(i18n, ids::PANELS_PLACED_LENGTH))
            .extension(a, la)
            .extension(b, lb),
    )
    .linked(FIELD_LENGTH);
    Illustration::new(VIEW_W, VIEW_H)
        .height_em(HEIGHT_EM)
        .parts([
            band,
            length,
            leader(a, -offset, ids::PROPERTIES_PRE_PLACEMENT_START, FIELD_START),
            leader(b, -offset, ids::PANELS_PLACED_END, FIELD_END),
            mark(a, FIELD_START),
            mark(b, FIELD_END),
        ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bottom_left_label_runs_right_and_above_its_point() {
        let at = Point::new(100.0, 80.0);
        let (text_at, anchor) = label_text_at(at, (HAlign::Left, VAlign::Bottom));
        assert_eq!(anchor, Anchor::Start);
        assert!(text_at.x > at.x && text_at.y < at.y, "{text_at:?}");
        let (text_at, anchor) = label_text_at(at, (HAlign::Right, VAlign::Top));
        assert_eq!(anchor, Anchor::End);
        assert!(text_at.x < at.x && text_at.y > at.y, "{text_at:?}");
    }

    #[test]
    fn the_drawn_wire_follows_the_typed_direction() {
        let (a, b) = wire_ends((0.0, 0.0), (0.0, 10.0));
        assert!(
            (a.x - b.x).abs() < 1e-3 && b.y > a.y,
            "vertical down: {a:?} {b:?}"
        );
        let (a, b) = wire_ends((5.0, 5.0), (5.0, 5.0));
        assert!(
            (a.y - b.y).abs() < 1e-3 && b.x > a.x,
            "level when the ends meet"
        );
    }

    #[test]
    fn a_wider_preset_draws_a_wider_band() {
        let widths: Vec<f32> = WireWidthPreset::ALL.into_iter().map(preset_width).collect();
        assert!(widths.windows(2).all(|w| w[0] < w[1]), "{widths:?}");
    }

    #[test]
    fn each_illustration_links_its_callouts_to_the_fields() {
        let en = I18n::en_us();
        let label = net_label(&en, "SPI_CLK", (HAlign::Left, VAlign::Bottom));
        for field in [FIELD_NET_NAME, FIELD_FONT_SIZE, FIELD_POSITION] {
            assert!(
                label.visible_parts().any(|p| p.is_linked_to(&field)),
                "{field}"
            );
        }
        let wire = wire(&en, ((0.0, 0.0), (25.4, 0.0)), WireWidthPreset::Small, None);
        for field in [FIELD_START, FIELD_END, FIELD_LENGTH, FIELD_WIDTH] {
            assert!(
                wire.visible_parts().any(|p| p.is_linked_to(&field)),
                "{field}"
            );
        }
    }
}
