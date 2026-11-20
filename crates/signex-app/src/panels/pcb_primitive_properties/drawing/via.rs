//! The Via illustration (mockup row 96): a small top view of the via on the shown
//! layer (Diameter, Drill, the mask opening or the tent) beside a
//! cross-section of the copper layers L1 (Top) to Ln (Bottom) with the span,
//! the fill, plug or cap, and the mask per side. Line art on the panel
//! ground; the callouts carry the field names below it.

use iced::{Point, Rectangle};
use signex_i18n::{Localized, ids, t};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_types::pcb_stack::StackMode;
use signex_types::pcb_via_protection::{PlugSide, ViaFill, ViaProtection};
use signex_widgets::Label;
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Fill, Illustration, Ink, Leader, Paint, Part, Primitive, Stroke,
    Style, TextLabel, TextRole, Tint, Weight,
};

use super::super::ViaFacts;
use super::super::rows::Look;
use super::{VIEW_H, canvas, layer_paint as paint};
use crate::styles::board_palette::BoardPalette;

/// The fields the callouts are linked to.
pub(in crate::panels::pcb_primitive_properties) const TYPE: FieldId = FieldId::new("via-type");
pub(in crate::panels::pcb_primitive_properties) const SPAN: FieldId = FieldId::new("via-span");
pub(in crate::panels::pcb_primitive_properties) const DIAMETER: FieldId =
    FieldId::new("via-diameter");
pub(in crate::panels::pcb_primitive_properties) const DRILL: FieldId = FieldId::new("via-drill");
pub(in crate::panels::pcb_primitive_properties) const PROTECTION: FieldId =
    FieldId::new("via-protection");
pub(in crate::panels::pcb_primitive_properties) const TENT: FieldId = FieldId::new("via-tent");
/// The mask opening: drawn only while a side has one.
const OPENING: FieldId = FieldId::new("via-mask-expansion");

/// The expansion drawn while the rule decides, in mm: the drawing shows an
/// opening, not the rule's value.
const RULE_EXPANSION_MM: f64 = 0.05;
/// Top view centre and the cross-section's left, right and barrel x.
const TOP_VIEW: (f32, f32) = (44.0, 64.0);
const SECTION_X: (f32, f32, f32) = (124.0, 258.0, 198.0);
/// The first and last copper row of the cross-section.
const SECTION_Y: (f32, f32) = (40.0, 112.0);

/// What the drawing reads off the facts.
struct Shape {
    /// Copper rows, the span's first and last row, and the row shown in
    /// the top view.
    rows: usize,
    span: (usize, usize),
    shown: usize,
    /// Diameter on each row, in mm.
    diameters: Vec<f64>,
    hole: f64,
    expansion: f64,
    hole_edge: bool,
    protection: ViaProtection,
}

impl Shape {
    fn of(f: &ViaFacts) -> Self {
        let rows = f.layers.len().max(2);
        let index = |layer: Option<CopperLayer>| {
            layer.and_then(|l| f.layers.iter().position(|c| c.layer == l))
        };
        let top = index(f.span_ends.0.as_ref().map(|c| c.layer)).unwrap_or(0);
        let bottom = index(f.span_ends.1.as_ref().map(|c| c.layer)).unwrap_or(rows - 1);
        let diameter = f.diameter.unwrap_or(0.6);
        let row_size = |i: usize| {
            let layer = f.layers.get(i).map(|c| c.layer);
            f.layer_sizes
                .iter()
                .find(|r| Some(r.layer) == layer)
                .and_then(|r| r.x)
                .unwrap_or(diameter)
        };
        let simple = f.mode != Some(StackMode::TopMiddleBottom) && f.mode != Some(StackMode::Full);
        let shown = if simple {
            top
        } else {
            index(f.view_layer)
                .or_else(|| index(f.layer_sizes.first().map(|r| r.layer)))
                .unwrap_or(top)
        };
        let manual = |m: Option<signex_types::pcb::MaskExpansion>| m.and_then(|m| m.manual_mm());
        let side = if shown + 1 == rows { f.bottom } else { f.top };
        Self {
            rows,
            span: (top.min(bottom), top.max(bottom)),
            shown,
            diameters: (0..rows).map(row_size).collect(),
            hole: f.hole.unwrap_or(diameter / 2.0),
            expansion: manual(side.expansion).unwrap_or(RULE_EXPANSION_MM).max(0.0),
            hole_edge: f.from_hole_edge == Some(true),
            protection: f.protection.unwrap_or_default(),
        }
    }

    fn in_span(&self, row: usize) -> bool {
        (self.span.0..=self.span.1).contains(&row)
    }

    fn tented(&self, row: usize) -> bool {
        if row + 1 == self.rows {
            self.protection.tented_bottom
        } else {
            self.protection.tented_top
        }
    }

    fn largest(&self) -> f64 {
        self.diameters.iter().copied().fold(self.hole, f64::max)
    }
}

/// The via illustration for `f`.
pub(in crate::panels::pcb_primitive_properties) fn illustration(
    look: Look<'_>,
    f: &ViaFacts,
) -> Illustration {
    let shape = Shape::of(f);
    let surface = (shape.shown == 0 || shape.shown + 1 == shape.rows) && shape.in_span(shape.shown);
    let opening = surface && !shape.tented(shape.shown);
    let mut drawing = canvas().feature(OPENING, opening);
    drawing = drawing.parts(top_view(look, f, &shape, surface));
    drawing.parts(cross_section(look, f, &shape))
}

fn top_view(look: Look<'_>, f: &ViaFacts, shape: &Shape, surface: bool) -> Vec<Part> {
    let (cx, cy) = TOP_VIEW;
    let centre = Point::new(cx, cy);
    let scale = 48.0 / (shape.largest() + 2.0 * shape.expansion);
    let copper = (shape.diameters[shape.shown] / 2.0 * scale) as f32;
    let hole = (shape.hole / 2.0 * scale) as f32;
    let reach = if shape.hole_edge { hole } else { copper };
    let opening = reach + ((shape.expansion * scale) as f32).max(2.5);
    let tent = surface && shape.tented(shape.shown);
    let p = shape.protection;
    let shown_layer = f.layers.get(shape.shown).map(|c| c.layer);
    let colour = match (f.mode, shown_layer) {
        (Some(StackMode::TopMiddleBottom | StackMode::Full), Some(l)) => paint(PcbLayer::Copper(l)),
        _ => Paint::Document(BoardPalette::new().multi_layer),
    };
    let mut parts = Vec::new();
    parts.push(
        Part::new(circle(centre, opening), Style::dashed())
            .linked(OPENING)
            .requires(OPENING),
    );
    parts
        .push(Part::new(circle(centre, copper), edge_filled(colour, Tint::Light)).linked(DIAMETER));
    let plugged_here = p.fill == ViaFill::Plugged
        && (p.plug_from == PlugSide::Both
            || (p.plug_from == PlugSide::Top) == (shape.shown + 1 != shape.rows));
    let hole_style = if p.capped && p.fill.is_filled() && surface {
        edge_filled(colour, Tint::Strong)
    } else if p.fill.is_filled() || plugged_here {
        edge_filled(fill_paint(p.fill), Tint::Strong)
    } else {
        edge_filled(Paint::Ink(Ink::Panel), Tint::Solid)
    };
    parts.push(Part::new(circle(centre, hole), hole_style).linked(DRILL));
    if tent {
        parts.push(
            Part::new(
                circle(centre, copper + 3.0),
                edge_filled(mask_paint(), Tint::Body),
            )
            .linked(TENT),
        );
    }
    let outer = opening.max(copper + 3.0);
    let top = cy - outer - 9.0;
    parts.push(
        Part::callout(
            Callout::new(Point::new(cx - copper, top), Point::new(cx + copper, top))
                .label(word(look, ids::PCB_PRIM_DIAMETER))
                .extension(
                    Point::new(cx - copper, cy),
                    Point::new(cx - copper, top - 3.0),
                )
                .extension(
                    Point::new(cx + copper, cy),
                    Point::new(cx + copper, top - 3.0),
                ),
        )
        .linked(DIAMETER),
    );
    let below = cy + outer + 9.0;
    parts.push(
        Part::callout(
            Callout::new(Point::new(cx - hole, below), Point::new(cx + hole, below))
                .label(word(look, ids::PCB_PRIM_DRILL))
                .label_at(Point::new(cx, below + 12.0), Anchor::Middle)
                .extension(
                    Point::new(cx - hole, cy),
                    Point::new(cx - hole, below + 3.0),
                )
                .extension(
                    Point::new(cx + hole, cy),
                    Point::new(cx + hole, below + 3.0),
                ),
        )
        .linked(DRILL),
    );
    parts.push(
        Part::leader(Leader {
            at: Point::new(cx + opening * 0.71, cy + opening * 0.71),
            label_at: Point::new(90.0, 104.0),
            label: word(look, ids::PCB_PRIM_EXPANSION),
            anchor: Anchor::Start,
        })
        .linked(OPENING)
        .requires(OPENING),
    );
    let caption = match (f.mode, shown_layer) {
        (Some(StackMode::TopMiddleBottom | StackMode::Full), Some(l)) => Label::from(
            crate::pcb_canvas::layer_style::copper_name(&look.ctx.i18n, l),
        ),
        _ => word(look, ids::PCB_PRIM_TOP_VIEW),
    };
    parts.push(note(Point::new(6.0, VIEW_H - 4.0), caption, Anchor::Start).linked(TYPE));
    parts
}

fn cross_section(look: Look<'_>, f: &ViaFacts, shape: &Shape) -> Vec<Part> {
    let (x0, x1, vx) = SECTION_X;
    let step = (SECTION_Y.1 - SECTION_Y.0) / (shape.rows - 1) as f32;
    let ys: Vec<f32> = (0..shape.rows)
        .map(|i| SECTION_Y.0 + step * i as f32)
        .collect();
    let scale = 56.0 / shape.largest();
    let p = shape.protection;
    let mut parts = Vec::new();
    let class = look.t(super::super::via::class_label(p.class()));
    let header = match &f.drill_pair {
        Some(pair) => t!(
            look.ctx.i18n,
            ids::PCB_PRIM_VIA_SECTION_HEADER,
            "pair" => pair.clone(),
            "class" => class.into_string()
        ),
        None => class,
    };
    parts.push(
        note(Point::new(x1, 12.0), header.into(), Anchor::End).linked_all([SPAN, PROTECTION]),
    );
    parts.push(Part::new(
        Primitive::Rect {
            rect: Rectangle::new(
                Point::new(x0, ys[0]),
                iced::Size::new(x1 - x0, ys[shape.rows - 1] - ys[0]),
            ),
        },
        Style::filled(Paint::Ink(Ink::Faint), Tint::Wash),
    ));
    for (i, &y) in ys.iter().enumerate() {
        let layer = f.layers.get(i).map(|c| c.layer).unwrap_or(CopperLayer::Top);
        let colour = paint(PcbLayer::Copper(layer));
        let n = t!(look.ctx.i18n, ids::PCB_PRIM_LAYER_POSITION, "n" => i + 1);
        parts.push(note(Point::new(x0 - 4.0, y + 3.0), n.into(), Anchor::End));
        let d = (shape.diameters[i] * scale) as f32;
        let plane = stroke_style(colour, Weight::Regular);
        if shape.in_span(i) {
            parts.push(Part::new(line((x0, y), (vx - d / 2.0 - 5.0, y)), plane));
            parts.push(Part::new(line((vx + d / 2.0 + 5.0, y), (x1, y)), plane));
            parts.push(
                Part::new(
                    rect(vx - d / 2.0, y - 2.0, d, 4.0),
                    edge_filled(colour, Tint::Body),
                )
                .linked(DIAMETER),
            );
        } else {
            parts.push(Part::new(line((x0, y), (x1, y)), plane));
        }
    }
    let (top, bottom) = (ys[shape.span.0] - 2.0, ys[shape.span.1] + 2.0);
    let hr = (shape.hole / 2.0 * scale) as f32;
    let depth = bottom - top;
    if p.fill.is_filled() {
        parts.push(
            Part::new(
                rect(vx - hr, top, 2.0 * hr, depth),
                filled(fill_paint(p.fill)),
            )
            .linked(PROTECTION),
        );
    }
    if p.fill == ViaFill::Plugged {
        let plug = depth
            * if p.plug_from == PlugSide::Both {
                0.32
            } else {
                0.45
            };
        if p.plug_from != PlugSide::Bottom {
            parts.push(Part::new(
                rect(vx - hr, top, 2.0 * hr, plug),
                filled(fill_paint(p.fill)),
            ));
        }
        if p.plug_from != PlugSide::Top {
            parts.push(Part::new(
                rect(vx - hr, bottom - plug, 2.0 * hr, plug),
                filled(fill_paint(p.fill)),
            ));
        }
    }
    for x in [vx - hr, vx + hr] {
        parts.push(Part::new(line((x, top), (x, bottom)), Style::edge()).linked_all([DRILL, SPAN]));
    }
    if p.capped && p.fill.is_filled() {
        for (row, y) in [(0, ys[0] - 5.0), (shape.rows - 1, ys[shape.rows - 1] + 2.0)] {
            if shape.in_span(row) {
                let d = (shape.diameters[row] * scale) as f32;
                let layer = f
                    .layers
                    .get(row)
                    .map(|c| c.layer)
                    .unwrap_or(CopperLayer::Top);
                parts.push(
                    Part::new(
                        rect(vx - d / 2.0, y, d, 3.0),
                        edge_filled(paint(PcbLayer::Copper(layer)), Tint::Strong),
                    )
                    .linked(PROTECTION),
                );
            }
        }
    }
    for (row, y) in [
        (0, ys[0] - 10.0),
        (shape.rows - 1, ys[shape.rows - 1] + 7.0),
    ] {
        let reach = shape.in_span(row);
        let open = reach && !shape.tented(row);
        let ink = if reach {
            mask_paint()
        } else {
            Paint::Ink(Ink::Faint)
        };
        let style = stroke_style(ink, Weight::Heavy);
        if open {
            let edge = if shape.hole_edge {
                shape.hole
            } else {
                shape.diameters[row]
            };
            let half = ((edge / 2.0 + shape.expansion) * scale) as f32;
            parts.push(Part::new(line((x0, y), (vx - half, y)), style).linked(TENT));
            parts.push(Part::new(line((vx + half, y), (x1, y)), style).linked(TENT));
        } else {
            parts.push(Part::new(line((x0, y), (x1, y)), style).linked(TENT));
        }
    }
    parts.push(
        Part::callout(Callout::new(
            Point::new(x1 - 4.0, top),
            Point::new(x1 - 4.0, bottom),
        ))
        .linked(SPAN),
    );
    parts.push(
        note(
            Point::new(x1, VIEW_H - 4.0),
            fill_summary(look, p).into(),
            Anchor::End,
        )
        .linked(PROTECTION),
    );
    parts
}

/// "Fill: None", "Fill: Resin · capped".
fn fill_summary(look: Look<'_>, p: ViaProtection) -> Localized {
    let fill = look.t(match p.fill {
        ViaFill::None => ids::PCB_PRIM_NONE,
        ViaFill::Plugged => ids::PCB_PRIM_FILL_PLUG,
        ViaFill::Resin => ids::PCB_PRIM_FILL_RESIN,
        ViaFill::Conductive => ids::PCB_PRIM_FILL_CONDUCTIVE,
    });
    let id = if p.capped && p.fill.is_filled() {
        ids::PCB_PRIM_FILL_CAPPED_SUMMARY
    } else {
        ids::PCB_PRIM_FILL_SUMMARY
    };
    t!(look.ctx.i18n, id, "fill" => fill.into_string())
}

fn word(look: Look<'_>, id: signex_i18n::MessageId) -> Label<'static> {
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

fn circle(center: Point, radius: f32) -> Primitive {
    Primitive::Circle { center, radius }
}

fn line(from: (f32, f32), to: (f32, f32)) -> Primitive {
    Primitive::Line {
        from: Point::new(from.0, from.1),
        to: Point::new(to.0, to.1),
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Primitive {
    Primitive::Rect {
        rect: Rectangle::new(Point::new(x, y), iced::Size::new(w.max(0.0), h.max(0.0))),
    }
}

fn mask_paint() -> Paint {
    paint(PcbLayer::Technical(TechnicalLayer::SolderMaskTop))
}

/// What fills a filled or plugged hole: conductive paste reads as copper.
fn fill_paint(fill: ViaFill) -> Paint {
    match fill {
        ViaFill::Conductive => paint(PcbLayer::Copper(CopperLayer::Top)),
        _ => Paint::Ink(Ink::Tertiary),
    }
}

fn edge_filled(paint: Paint, tint: Tint) -> Style {
    Style::edge().with_fill(paint, tint)
}

fn filled(paint: Paint) -> Style {
    Style {
        stroke: None,
        fill: Some(Fill {
            paint,
            tint: Tint::Strong,
        }),
    }
}

fn stroke_style(paint: Paint, weight: Weight) -> Style {
    Style {
        stroke: Some(Stroke {
            paint,
            weight,
            is_dashed: false,
        }),
        fill: None,
    }
}
