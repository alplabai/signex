//! The string illustration (#2523, mockup row 100): the text in its box, the Text
//! Height and the Position X/Y origin. The rotation is not shown.

use iced::{Point, Rectangle, Size};
use signex_i18n::{I18n, UserText, ids, t};
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Illustration, Part, Primitive, Style, TextRole,
};

use super::super::TextFacts;
use super::{VIEW_H, VIEW_W, canvas, cross, label};

pub(in crate::panels::pcb_primitive_properties) const TEXT: FieldId = FieldId::new("string.text");
pub(in crate::panels::pcb_primitive_properties) const POSITION: FieldId =
    FieldId::new("string.position");
pub(in crate::panels::pcb_primitive_properties) const HEIGHT: FieldId =
    FieldId::new("string.height");
pub(in crate::panels::pcb_primitive_properties) const LAYER: FieldId = FieldId::new("string.layer");

/// The longest text drawn; a longer one is cut.
const MAX_CHARS: usize = 32;
/// A mono glyph is about this share of its height wide.
const GLYPH_ASPECT: f32 = 0.62;
/// The box's left edge: room on its left for the Text Height callout.
const BOX_LEFT: f32 = 76.0;
/// The tallest box drawn, in view units.
const MAX_BOX_H: f32 = 40.0;

/// The illustration of the selected strings `f`.
pub(in crate::panels::pcb_primitive_properties) fn illustration(
    f: &TextFacts,
    i18n: &I18n,
) -> Illustration {
    let shown: String = f
        .text
        .as_deref()
        .unwrap_or_default()
        .chars()
        .take(MAX_CHARS)
        .collect();
    let chars = shown.chars().count().max(1) as f32;
    // The box as tall as the room to its right allows.
    let room = VIEW_W - BOX_LEFT - 16.0;
    let height = (room / (chars * GLYPH_ASPECT)).clamp(10.0, MAX_BOX_H);
    let width = (chars * GLYPH_ASPECT * height).min(room);
    let baseline = VIEW_H / 2.0 + height / 2.0;
    let top = baseline - height;
    let origin = Point::new(BOX_LEFT, baseline);
    let layer = f
        .layer
        .as_ref()
        .map_or_else(String::new, |l| l.label.clone());
    canvas()
        .part(
            Part::new(
                Primitive::Rect {
                    rect: Rectangle::new(Point::new(BOX_LEFT, top), Size::new(width, height)),
                },
                Style::dashed(),
            )
            .linked_all([TEXT, HEIGHT]),
        )
        .part(label(
            Point::new(BOX_LEFT + width / 2.0, top + height / 2.0),
            UserText::new(shown),
            Anchor::Middle,
            TextRole::Mono,
            &[TEXT, LAYER],
        ))
        .part(
            Part::callout(
                Callout::new(
                    Point::new(BOX_LEFT - 9.0, baseline),
                    Point::new(BOX_LEFT - 9.0, top),
                )
                .label(t!(i18n, ids::PCB_PRIM_TEXT_HEIGHT))
                .label_at(Point::new(BOX_LEFT - 14.0, top + height / 2.0), Anchor::End),
            )
            .linked(HEIGHT),
        )
        .parts(cross(origin, 5.0, &[POSITION]))
        .part(label(
            Point::new(origin.x + 4.0, origin.y + 12.0),
            t!(i18n, ids::PCB_PRIM_STRING_DRAWING_ORIGIN),
            Anchor::Start,
            TextRole::Caption,
            &[POSITION],
        ))
        .part(label(
            Point::new(VIEW_W - 6.0, VIEW_H - 8.0),
            UserText::new(layer),
            Anchor::End,
            TextRole::Note,
            &[LAYER],
        ))
}
