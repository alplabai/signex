//! The sketch pad illustration (#126): the copper profile from the sketch,
//! the solder mask band around it and the paste outline inside it, on the
//! top side.

use iced::{Point, Rectangle};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_widgets::property_illustration::{
    Anchor, FieldId, Illustration, Leader, Paint, Part, Primitive, Style, TextRole, Tint,
};

use super::super::super::SketchPadAttrSummary;
use super::super::pad::layer_title;
use super::drawing::{canvas, centred, corner_note, label};
use super::expansion::literal_mm;
use crate::pcb_canvas::layer_style::layer_name;
use crate::styles::board_palette::BoardPalette;

pub(super) const DESIGNATOR: FieldId = FieldId::new("sketch_pad.designator");
pub(super) const PASTE_TOP: FieldId = FieldId::new("sketch_pad.paste_top");
pub(super) const MASK_TOP: FieldId = FieldId::new("sketch_pad.mask_top");
pub(super) const PASTE_BOTTOM: FieldId = FieldId::new("sketch_pad.paste_bottom");
pub(super) const MASK_BOTTOM: FieldId = FieldId::new("sketch_pad.mask_bottom");

/// A pad of unknown size is drawn this big (mm).
const FALLBACK_PAD_MM: [f64; 2] = [1.55, 0.6];
/// How wide the mask band and how deep the paste inset are drawn when the
/// design rule sets them (the panel does not know the rule's value), in
/// view units.
const RULE_MASK_BAND: f32 = 4.0;
const RULE_PASTE_INSET: f32 = 4.0;
const CENTRE: Point = Point::new(120.0, 76.0);

fn grown(rect: Rectangle, by: f32) -> Rectangle {
    Rectangle {
        x: rect.x - by,
        y: rect.y - by,
        width: (rect.width + 2.0 * by).max(1.0),
        height: (rect.height + 2.0 * by).max(1.0),
    }
}

pub(super) fn drawing(pad: &SketchPadAttrSummary, i18n: &I18n) -> Illustration {
    let palette = BoardPalette::new();
    let copper = Paint::Document(palette.layer(PcbLayer::Copper(CopperLayer::Top)));
    let mask = Paint::Document(palette.layer(PcbLayer::Technical(TechnicalLayer::SolderMaskTop)));
    let size = pad.size_mm.unwrap_or(FALLBACK_PAD_MM);
    let scale = (124.0 / size[0]).min(48.0 / size[1]).min(80.0);
    let body = centred(CENTRE, (size[0] * scale) as f32, (size[1] * scale) as f32);
    let radius = pad.corner_radius_pct.map_or(0.0, |pct| {
        body.height.min(body.width) * (pct as f32 / 100.0)
    });
    let to_view = |mm: f64| (mm * scale) as f32;
    let mut drawing = canvas();
    let mask_open = !pad.mask_top_tented;
    if mask_open {
        let band = pad
            .mask_top_expr
            .as_deref()
            .and_then(literal_mm)
            .map_or(RULE_MASK_BAND, to_view);
        drawing = drawing.part(
            Part::new(
                Primitive::RoundedRect {
                    rect: grown(body, band),
                    radius: radius + band.max(0.0),
                },
                Style::filled(mask, Tint::Light),
            )
            .linked(MASK_TOP),
        );
    }
    drawing = drawing
        .part(
            Part::new(
                Primitive::RoundedRect { rect: body, radius },
                Style::edge().with_fill(copper, Tint::Body),
            )
            .linked(DESIGNATOR),
        )
        .part(
            label(
                CENTRE,
                UserText::new(pad.number.as_str()),
                Anchor::Middle,
                TextRole::Mono,
            )
            .linked(DESIGNATOR),
        );
    if pad.paste_top_enabled {
        let inset = pad
            .paste_top_expr
            .as_deref()
            .and_then(literal_mm)
            .map_or(-RULE_PASTE_INSET, to_view);
        drawing = drawing.part(
            Part::new(
                Primitive::RoundedRect {
                    rect: grown(body, inset),
                    radius: (radius + inset).max(0.0),
                },
                Style::dashed(),
            )
            .linked(PASTE_TOP),
        );
        drawing = drawing.part(
            Part::leader(Leader {
                at: Point::new(body.x + 10.0, body.y + body.height - 6.0),
                label_at: Point::new(4.0, 124.0),
                label: layer_title(i18n, TechnicalLayer::PasteTop).into(),
                anchor: Anchor::Start,
            })
            .linked(PASTE_TOP),
        );
    }
    let mask_note = if mask_open {
        Part::leader(Leader {
            at: Point::new(body.x + body.width + 2.0, body.y + body.height + 2.0),
            label_at: Point::new(260.0, 124.0),
            label: layer_title(i18n, TechnicalLayer::SolderMaskTop).into(),
            anchor: Anchor::End,
        })
    } else {
        label(
            Point::new(260.0, 126.0),
            t!(i18n, ids::EDITORS_ROLE_SKETCH_PAD_MASK_OFF),
            Anchor::End,
            TextRole::Note,
        )
    };
    let size_note = t!(
        i18n,
        ids::EDITORS_ROLE_SKETCH_PAD_SIZE,
        "width" => format!("{:.3}", size[0]),
        "height" => format!("{:.3}", size[1])
    );
    let side = t!(
        i18n,
        ids::EDITORS_ROLE_SKETCH_PAD_CORNER,
        "layer" => layer_name(i18n, PcbLayer::Copper(CopperLayer::Top)).into_string(),
        "number" => pad.number.as_str()
    );
    let mut drawing = drawing
        .part(mask_note.linked(MASK_TOP))
        .part(corner_note(side));
    if pad.size_mm.is_some() {
        drawing = drawing.part(label(
            Point::new(CENTRE.x, body.y - 14.0),
            size_note,
            Anchor::Middle,
            TextRole::Note,
        ));
    }
    drawing
}
