//! The Pad illustration (#97): the pad's top view with its X and Y sizes,
//! the paste (dashed) and mask openings and the hole, beside a side view
//! through the board. A surface pad shows top copper only. Line art on the
//! panel ground; the callouts carry the field names below it.

use iced::{Point, Rectangle, Size};
use signex_i18n::{MessageId, ids};
use signex_types::pcb::PadShape;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_widgets::Label;
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Illustration, Ink, Paint, Part, Primitive, Stroke, Style, TextLabel,
    TextRole, Tint, Weight,
};

use super::super::PadFacts;
use super::super::rows::Look;
use super::{VIEW_H, canvas, layer_paint as paint};
use crate::styles::board_palette::BoardPalette;

/// The fields the callouts are linked to.
pub(in crate::panels::pcb_primitive_properties) const LAYER: FieldId = FieldId::new("pad-layer");
pub(in crate::panels::pcb_primitive_properties) const SIZE_X: FieldId = FieldId::new("pad-size-x");
pub(in crate::panels::pcb_primitive_properties) const SIZE_Y: FieldId = FieldId::new("pad-size-y");
pub(in crate::panels::pcb_primitive_properties) const SHAPE: FieldId = FieldId::new("pad-shape");
pub(in crate::panels::pcb_primitive_properties) const HOLE: FieldId = FieldId::new("pad-hole");
pub(in crate::panels::pcb_primitive_properties) const PASTE: FieldId = FieldId::new("pad-paste");
pub(in crate::panels::pcb_primitive_properties) const MASK: FieldId = FieldId::new("pad-mask");

/// The mask opening drawn around the copper, in mm: the board does not
/// store a pad's expansion, so the drawing shows an opening, not a value.
const MASK_MARGIN_MM: f64 = 0.05;
/// Top view centre and the room the pad is fitted into.
const TOP_VIEW: (f32, f32) = (64.0, 60.0);
const TOP_ROOM: f32 = 70.0;
/// The side view's left and right x, and the board's top and bottom y.
const SIDE_X: (f32, f32) = (150.0, 250.0);
const SIDE_Y: (f32, f32) = (52.0, 82.0);

/// The pad illustration for `f`.
pub(in crate::panels::pcb_primitive_properties) fn illustration(
    look: Look<'_>,
    f: &PadFacts,
) -> Illustration {
    let surface = !f.has_hole;
    let (w, h) = (f.size_x.unwrap_or(1.0), f.size_y.unwrap_or(1.0));
    let hole = f.hole.filter(|_| !surface);
    let shape = f.shape.as_ref().map_or(PadShape::Rect, |c| c.shape);
    let colour = match f.layer.as_ref().and_then(|(_, l)| *l) {
        Some(layer) => paint(layer),
        None => Paint::Document(BoardPalette::new().multi_layer),
    };
    canvas()
        .feature(PASTE, surface)
        .parts(top_view(look, f, (w, h), hole, shape, colour))
        .parts(side_view(look, (w, h), hole, surface, colour))
}

fn top_view(
    look: Look<'_>,
    f: &PadFacts,
    (w, h): (f64, f64),
    hole: Option<f64>,
    shape: PadShape,
    colour: Paint,
) -> Vec<Part> {
    let (cx, cy) = TOP_VIEW;
    let scale = TOP_ROOM / (w.max(h) + 2.0 * MASK_MARGIN_MM) as f32;
    let (pw, ph) = (w as f32 * scale, h as f32 * scale);
    let margin = (MASK_MARGIN_MM as f32 * scale).max(2.5);
    let mut parts = vec![
        Part::new(
            outline(shape, cx, cy, pw + 2.0 * margin, ph + 2.0 * margin),
            Style::dashed(),
        )
        .linked(MASK),
        Part::new(
            outline(shape, cx, cy, pw, ph),
            Style::edge().with_fill(colour, Tint::Light),
        )
        .linked_all([SIZE_X, SIZE_Y, SHAPE]),
        Part::new(
            outline(shape, cx, cy, pw - 2.0, ph - 2.0),
            Style::dashed().with_stroke_paint(paint(PcbLayer::Technical(TechnicalLayer::PasteTop))),
        )
        .linked(PASTE)
        .requires(PASTE),
    ];
    if let Some(hole) = hole {
        let r = (hole as f32 * scale / 2.0)
            .min(pw.min(ph) / 2.0 - 1.0)
            .max(1.0);
        parts.push(
            Part::new(
                Primitive::Circle {
                    center: Point::new(cx, cy),
                    radius: r,
                },
                Style::edge().with_fill(Paint::Ink(Ink::Panel), Tint::Solid),
            )
            .linked(HOLE),
        );
        parts.push(
            Part::callout(
                Callout::new(Point::new(cx - r, cy), Point::new(cx + r, cy))
                    .label(word(look, ids::PCB_PRIM_HOLE)),
            )
            .linked(HOLE),
        );
    }
    let top = cy - ph / 2.0 - margin - 8.0;
    parts.push(
        Part::callout(
            Callout::new(
                Point::new(cx - pw / 2.0, top),
                Point::new(cx + pw / 2.0, top),
            )
            .label(word(look, ids::PCB_PRIM_X_SIZE))
            .extension(
                Point::new(cx - pw / 2.0, cy),
                Point::new(cx - pw / 2.0, top - 3.0),
            )
            .extension(
                Point::new(cx + pw / 2.0, cy),
                Point::new(cx + pw / 2.0, top - 3.0),
            ),
        )
        .linked(SIZE_X),
    );
    let left = cx - pw / 2.0 - margin - 8.0;
    parts.push(
        Part::callout(
            Callout::new(
                Point::new(left, cy - ph / 2.0),
                Point::new(left, cy + ph / 2.0),
            )
            .label(word(look, ids::PCB_PRIM_Y_SIZE))
            .label_at(Point::new(left - 3.0, cy), Anchor::End)
            .extension(
                Point::new(cx, cy - ph / 2.0),
                Point::new(left - 3.0, cy - ph / 2.0),
            )
            .extension(
                Point::new(cx, cy + ph / 2.0),
                Point::new(left - 3.0, cy + ph / 2.0),
            ),
        )
        .linked(SIZE_Y),
    );
    let layer = f
        .layer
        .as_ref()
        .map(|(name, _)| Label::from(signex_i18n::UserText::new(name.clone())))
        .unwrap_or_else(|| word(look, ids::PCB_PRIM_TOP_VIEW));
    parts.push(note(Point::new(6.0, VIEW_H - 4.0), layer, Anchor::Start).linked(LAYER));
    parts
}

fn side_view(
    look: Look<'_>,
    (w, h): (f64, f64),
    hole: Option<f64>,
    surface: bool,
    colour: Paint,
) -> Vec<Part> {
    let (x0, x1) = SIDE_X;
    let (y0, y1) = SIDE_Y;
    let mid = (x0 + x1) / 2.0;
    let scale = 60.0 / w.max(h) as f32;
    let pw = w as f32 * scale;
    let mut parts = vec![Part::new(
        rect(x0, y0, x1 - x0, y1 - y0),
        Style::filled(Paint::Ink(Ink::Faint), Tint::Wash),
    )];
    let top = paint(PcbLayer::Copper(CopperLayer::Top));
    let copper = if surface { top } else { colour };
    parts.push(
        Part::new(
            rect(mid - pw / 2.0, y0 - 3.0, pw, 3.0),
            Style::edge().with_fill(copper, Tint::Body),
        )
        .linked_all([SIZE_X, LAYER]),
    );
    let mask = paint(PcbLayer::Technical(TechnicalLayer::SolderMaskTop));
    let gap = pw / 2.0 + 3.0;
    let mask_style = stroke(mask, Weight::Heavy);
    parts.push(Part::new(line((x0, y0 - 1.0), (mid - gap, y0 - 1.0)), mask_style).linked(MASK));
    parts.push(Part::new(line((mid + gap, y0 - 1.0), (x1, y0 - 1.0)), mask_style).linked(MASK));
    parts.push(
        Part::new(
            rect(mid - pw / 2.0 + 1.0, y0 - 7.0, pw - 2.0, 3.0),
            Style::dashed().with_stroke_paint(paint(PcbLayer::Technical(TechnicalLayer::PasteTop))),
        )
        .linked(PASTE)
        .requires(PASTE),
    );
    if let Some(hole) = hole {
        let bottom = paint(PcbLayer::Copper(CopperLayer::Bottom));
        parts.push(
            Part::new(
                rect(mid - pw / 2.0, y1, pw, 3.0),
                Style::edge().with_fill(bottom, Tint::Body),
            )
            .linked(SIZE_X),
        );
        parts.push(Part::new(line((x0, y1 + 4.0), (mid - gap, y1 + 4.0)), mask_style).linked(MASK));
        parts.push(Part::new(line((mid + gap, y1 + 4.0), (x1, y1 + 4.0)), mask_style).linked(MASK));
        let r = (hole as f32 * scale / 2.0).min(pw / 2.0 - 1.0).max(1.0);
        parts.push(
            Part::new(
                rect(mid - r, y0 - 3.0, 2.0 * r, y1 - y0 + 6.0),
                Style::filled(Paint::Ink(Ink::Panel), Tint::Solid),
            )
            .linked(HOLE),
        );
        for x in [mid - r, mid + r] {
            parts.push(Part::new(line((x, y0 - 3.0), (x, y1 + 3.0)), Style::edge()).linked(HOLE));
        }
    }
    parts.push(note(
        Point::new(x1, VIEW_H - 4.0),
        word(look, ids::PCB_PRIM_SIDE_VIEW),
        Anchor::End,
    ));
    parts
}

/// The pad's outline at `(cx, cy)`, `w` × `h` wide.
fn outline(shape: PadShape, cx: f32, cy: f32, w: f32, h: f32) -> Primitive {
    let (w, h) = (w.max(1.0), h.max(1.0));
    let r = Rectangle::new(Point::new(cx - w / 2.0, cy - h / 2.0), Size::new(w, h));
    match shape {
        PadShape::Circle => Primitive::Circle {
            center: Point::new(cx, cy),
            radius: w.min(h) / 2.0,
        },
        PadShape::Oval => Primitive::RoundedRect {
            rect: r,
            radius: w.min(h) / 2.0,
        },
        PadShape::RoundRect => Primitive::RoundedRect {
            rect: r,
            radius: w.min(h) / 4.0,
        },
        PadShape::Rect | PadShape::Trapezoid | PadShape::Custom => Primitive::Rect { rect: r },
    }
}

fn word(look: Look<'_>, id: MessageId) -> Label<'static> {
    Label::from(look.t(id))
}

fn note(at: Point, text: Label<'static>, anchor: Anchor) -> Part {
    Part::label(
        TextLabel {
            at,
            text,
            anchor,
            role: TextRole::Note,
        },
        Ink::Tertiary,
    )
}

fn line(from: (f32, f32), to: (f32, f32)) -> Primitive {
    Primitive::Line {
        from: Point::new(from.0, from.1),
        to: Point::new(to.0, to.1),
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Primitive {
    Primitive::Rect {
        rect: Rectangle::new(Point::new(x, y), Size::new(w.max(0.0), h.max(0.0))),
    }
}

fn stroke(paint: Paint, weight: Weight) -> Style {
    Style {
        stroke: Some(Stroke {
            paint,
            weight,
            is_dashed: false,
        }),
        fill: None,
    }
}
