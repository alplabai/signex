//! The line-art illustrations of the primitive panels (#2523, m95-m102):
//! each kind's drawing as `property_illustration` data, on the mockup's
//! 264 x 140 view box. Strokes and labels take the theme's ink tokens and a
//! layer colour tints the object; callouts carry the keys of the fields
//! they show (`keys`), so hovering either lights the other. The drawings
//! are schematic: they show what each field sizes, not the selection's
//! geometry.

pub(super) mod arc;
pub(super) mod dimension;
pub(super) mod graphic;
pub(super) mod pad;
pub(super) mod text;
pub(super) mod track;
pub(super) mod via;
pub(super) mod zone;

use iced::Point;
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_widgets::Label;
use signex_widgets::property_illustration::{
    Anchor, Callout, Cap, FieldId, Illustration, Ink, Paint, Part, Primitive, Style, TextLabel,
    TextRole, Tint, Weight,
};

use crate::styles::board_palette::BoardPalette;

/// The view box of every primitive illustration (the mockup's `viewBox`).
pub(super) const VIEW_W: f32 = 264.0;
pub(super) const VIEW_H: f32 = 140.0;
/// The widget height in captions: the view box at the dock's usual width.
const HEIGHT_EM: f32 = 14.0;

/// The keys that link a drawing part to its field row; one per row.
pub(super) mod keys {
    use signex_widgets::property_illustration::FieldId;

    pub const NET: FieldId = FieldId::new("net");
    pub const LAYER: FieldId = FieldId::new("layer");
    pub const WIDTH: FieldId = FieldId::new("w");
    pub const ENDS: FieldId = FieldId::new("ends");
    pub const START: FieldId = FieldId::new("p1");
    pub const END: FieldId = FieldId::new("p2");
    pub const LENGTH: FieldId = FieldId::new("len");
    pub const CENTRE: FieldId = FieldId::new("ctr");
    pub const RADIUS: FieldId = FieldId::new("r");
    pub const ANGLES: FieldId = FieldId::new("angles");
    pub const TEXT: FieldId = FieldId::new("ftxt");
    pub const TEXT_HEIGHT: FieldId = FieldId::new("fth");
}

/// An empty illustration on the view box.
pub(super) fn canvas() -> Illustration {
    Illustration::new(VIEW_W, VIEW_H).height_em(HEIGHT_EM)
}

/// The board palette colour of `layer`, as a document paint.
pub(super) fn layer_paint(layer: PcbLayer) -> Paint {
    Paint::Document(BoardPalette::new().layer(layer))
}

pub(super) fn at(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

/// A wide stroke through `points` drawn as an outline with a light tint of
/// `paint`, linked to `fields` (the first is what hovering it reports).
pub(super) fn band(points: Vec<Point>, width: f32, paint: Paint, fields: &[FieldId]) -> Part {
    Part::new(
        Primitive::Band {
            points,
            width,
            cap: Cap::Round,
        },
        Style::edge().with_fill(paint, Tint::Body),
    )
    .linked_all(fields.iter().cloned())
}

/// A small cross marking a point (a centre, an end, the pointer).
pub(super) fn cross(centre: Point, arm: f32, fields: &[FieldId]) -> [Part; 2] {
    let line = |from: Point, to: Point| {
        Part::new(
            Primitive::Line { from, to },
            Style::outline(Ink::Secondary, Weight::Regular),
        )
        .linked_all(fields.iter().cloned())
    };
    [
        line(at(centre.x - arm, centre.y), at(centre.x + arm, centre.y)),
        line(at(centre.x, centre.y - arm), at(centre.x, centre.y + arm)),
    ]
}

/// A dimension callout from `from` to `to`, its label at `label_at`.
pub(super) fn callout(
    from: Point,
    to: Point,
    label: Option<(Label<'static>, Point, Anchor)>,
    fields: &[FieldId],
) -> Part {
    let mut c = Callout::new(from, to);
    if let Some((text, place, anchor)) = label {
        c = c.label(text).label_at(place, anchor);
    }
    Part::callout(c).linked_all(fields.iter().cloned())
}

/// A free label in the callout ink: a ray's name, an object's name.
pub(super) fn label(
    place: Point,
    text: impl Into<Label<'static>>,
    anchor: Anchor,
    role: TextRole,
    fields: &[FieldId],
) -> Part {
    Part::label(
        TextLabel {
            at: place,
            text: text.into(),
            anchor,
            role,
        },
        Ink::Faint,
    )
    .linked_all(fields.iter().cloned())
}

/// A dashed construction line (a ray, the run still to place).
pub(super) fn dashed(from: Point, to: Point, fields: &[FieldId]) -> Part {
    Part::new(Primitive::Line { from, to }, Style::dashed()).linked_all(fields.iter().cloned())
}

/// The note in the lower right corner: "{layer} · round ends".
pub(super) fn layer_note(i18n: &I18n, layer: &Label<'_>) -> Part {
    let note = t!(i18n, ids::PCB_PRIM_DRAWING_LAYER_ENDS, "layer" => layer.as_str().to_string());
    label(
        at(VIEW_W - 6.0, VIEW_H - 8.0),
        note,
        Anchor::End,
        TextRole::Note,
        &[keys::LAYER, keys::ENDS],
    )
}

/// The name a drawing gives a layer: an inner copper layer by its place in
/// the copper stack (L1 is the top), any other layer by `name`.
pub(super) fn layer_label(
    i18n: &I18n,
    layer: PcbLayer,
    copper_place: Option<usize>,
    name: &str,
) -> Label<'static> {
    match (layer, copper_place) {
        (PcbLayer::Copper(CopperLayer::Inner(_)), Some(place)) => {
            t!(i18n, ids::PCB_PRIM_DRAWING_INNER_LAYER, "number" => place + 1).into()
        }
        _ => UserText::new(name.to_string()).into(),
    }
}

/// `value` in mm as a callout reads it: three decimals.
pub(super) fn mm3(value: f64) -> String {
    format!("{value:.3}")
}

/// What the drawing tests share: a panel common to a selection of `items`.
#[cfg(test)]
pub(super) mod fixture {
    use pcb_geom::ItemRef;

    use crate::panels::pcb_primitive_properties::{Editability, PrimitiveCommon, PrimitiveTarget};

    pub fn common(items: Vec<ItemRef>) -> PrimitiveCommon {
        PrimitiveCommon {
            target: PrimitiveTarget {
                board: "board.snxpcb".into(),
                items,
            },
            editability: Editability::Editable,
            net_info: Default::default(),
            nets: Vec::new(),
            net: None,
            drafts: Default::default(),
            show_more: false,
            search: String::new(),
            picking_net: false,
            placement_paused: false,
            hover: Default::default(),
            unstored_open: false,
            locked_count: None,
        }
    }

    /// Whether some visible part linked to `key` lights while `key` is
    /// hovered, and none lights while nothing is.
    pub fn lights(
        drawing: &signex_widgets::property_illustration::Illustration,
        key: &signex_widgets::property_illustration::FieldId,
    ) -> bool {
        use signex_widgets::property_illustration::{IllustrationHover, IllustrationMsg};
        let mut hover = IllustrationHover::default();
        let dark = drawing
            .visible_parts()
            .all(|p| !hover.is_part_highlighted(p));
        hover.update(IllustrationMsg::FieldHovered(key.clone()));
        dark && drawing
            .visible_parts()
            .any(|p| hover.is_part_highlighted(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb_layer::InnerCopper;

    /// The text without the bidi isolation marks round its arguments.
    fn plain(label: &Label<'_>) -> String {
        label.as_str().replace(['\u{2068}', '\u{2069}'], "")
    }

    #[test]
    fn an_inner_copper_layer_is_named_by_its_place_and_others_by_name() {
        let i18n = I18n::en_us();
        let inner = PcbLayer::Copper(CopperLayer::Inner(InnerCopper::new(1).unwrap()));
        assert_eq!(plain(&layer_label(&i18n, inner, Some(1), "Inner 1")), "L2");
        let top = PcbLayer::Copper(CopperLayer::Top);
        assert_eq!(
            plain(&layer_label(&i18n, top, Some(0), "Top Layer")),
            "Top Layer"
        );
    }
}
