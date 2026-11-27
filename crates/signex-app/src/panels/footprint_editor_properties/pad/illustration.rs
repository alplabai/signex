//! The pad's property illustration (mockup row 125): copper, hole, paste and solder
//! mask outlines of the current state on the panel ground, with callouts
//! named after the fields. No rotation is drawn. A switched-off paste or
//! mask drops out through `Part::requires`. The layer chips of a
//! through-hole pad draw each layer's pad in the same line art.

use iced::widget::{Row, container};
use iced::{Element, Length, Point, Rectangle, Size, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_widgets::property_illustration::{
    Anchor, Callout, Cap, FieldId, Illustration, Ink, Leader, Paint, Part, Primitive, Style,
    TextLabel, TextRole, Tint, Weight, property_illustration,
};
use signex_widgets::{Label, text};

use super::super::super::PanelMsg;
use super::form::{Form, PadClass};
use super::layer_title;
use crate::library::editor::footprint::{PadSide, PadStackTab};
use crate::styles::board_palette::BoardPalette;
use crate::styles::{PillTone, Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The fields the drawing parts are linked to.
pub(super) const SIZE_X: FieldId = FieldId::new("pad-size-x");
pub(super) const SIZE_Y: FieldId = FieldId::new("pad-size-y");
pub(super) const SHAPE: FieldId = FieldId::new("pad-shape");
pub(super) const CORNER: FieldId = FieldId::new("pad-corner-radius");
pub(super) const HOLE: FieldId = FieldId::new("pad-hole");
pub(super) const SLOT: FieldId = FieldId::new("pad-slot");
pub(super) const PASTE: FieldId = FieldId::new("pad-paste");
pub(super) const MASK: FieldId = FieldId::new("pad-mask");
pub(super) const MASK_BOTTOM: FieldId = FieldId::new("pad-mask-bottom");
/// The thermal relief rows (#2593): the air gap, and the spokes' width and
/// count.
pub(super) const THERMAL_GAP: FieldId = FieldId::new("pad-thermal-gap");
pub(super) const THERMAL_SPOKE: FieldId = FieldId::new("pad-thermal-spoke");
pub(super) const THERMAL_SPOKES: FieldId = FieldId::new("pad-thermal-spokes");
/// Feature switches: a part that requires one drops out while it is off.
const PASTE_ON: FieldId = FieldId::new("pad-paste-on");
const MASK_ON: FieldId = FieldId::new("pad-mask-on");
const THERMAL_ON: FieldId = FieldId::new("pad-thermal-on");
/// The gap and spoke width drawn while the pour decides them, mm: a library
/// pad has no pour to read them from.
const RULE_GAP_MM: f64 = 0.5;
const RULE_SPOKE_MM: f64 = 0.25;

/// The view box of the approved drawings, and the pad's centre in it.
const VIEW: Size = Size::new(264.0, 140.0);
const CENTRE: Point = Point::new(104.0, 80.0);
/// Largest drawn pad, in view units.
const FIT: (f64, f64) = (110.0, 60.0);
/// Where the labels of the right and left leaders start or end.
const RIGHT_LABEL_X: f32 = 168.0;
const LEFT_LABEL_X: f32 = 58.0;
/// Expansions the drawing shows while the rule decides, mm: it shows an
/// opening, not the rule's value (a library pad has no board rule).
const RULE_MASK_MM: f64 = 0.05;
const RULE_PASTE_MM: f64 = -0.05;
/// Smallest drawn mask ring, in view units, so the opening reads.
const MIN_MASK_RING: f32 = 2.5;
/// Glyph view box of a layer chip.
const CHIP_VIEW: Size = Size::new(24.0, 16.0);
const CHIP_GLYPH_EM: f32 = 1.4;

/// A drawn copper or hole outline: a rounded rectangle (a circle or a
/// stadium when the radius reaches half the short side) or a polygon.
#[derive(Debug, Clone)]
enum Outline {
    Rounded { half: Size, radius: f32 },
    Polygon(Vec<Point>),
}

impl Outline {
    /// The pad copper of `values` at `scale` view units per mm.
    fn copper(form: &Form<'_>, scale: f64) -> Self {
        use signex_library::PadShape;
        let v = form.values;
        let (w, h) = (v.size_x_mm.max(0.001), v.size_y_mm.max(0.001));
        let half = Size::new((w * scale / 2.0) as f32, (h * scale / 2.0) as f32);
        let short = half.width.min(half.height);
        match &v.shape {
            PadShape::Rect => Outline::Rounded { half, radius: 0.0 },
            PadShape::Round | PadShape::Oval => Outline::Rounded {
                half,
                radius: short,
            },
            PadShape::RoundRect { radius_ratio } => Outline::Rounded {
                half,
                radius: (*radius_ratio as f32 * 2.0 * short).clamp(0.0, short),
            },
            PadShape::Chamfered { chamfer_ratio, .. } => {
                let c = (*chamfer_ratio as f32 * 2.0 * short).clamp(0.0, short);
                let (x, y) = (half.width, half.height);
                Outline::Polygon(vec![
                    Point::new(-x + c, -y),
                    Point::new(x - c, -y),
                    Point::new(x, -y + c),
                    Point::new(x, y - c),
                    Point::new(x - c, y),
                    Point::new(-x + c, y),
                    Point::new(-x, y - c),
                    Point::new(-x, -y + c),
                ])
            }
            PadShape::Trapezoid { delta } => {
                use signex_types::pcb::Point as Mm;
                match signex_types::pcb_pad_shape::trapezoid_corners(
                    Mm::new(w, h),
                    Mm::new(delta[0], delta[1]),
                ) {
                    // Model y is up; the drawing's y is down.
                    Ok(corners) => Outline::Polygon(
                        corners
                            .iter()
                            .map(|c| Point::new((c.x * scale) as f32, -(c.y * scale) as f32))
                            .collect(),
                    ),
                    Err(_) => Outline::Rounded { half, radius: 0.0 },
                }
            }
            _ => Outline::Rounded { half, radius: 0.0 },
        }
    }

    /// A round hole of `diameter` view units.
    fn circle(diameter: f32) -> Self {
        let r = diameter / 2.0;
        Outline::Rounded {
            half: Size::new(r, r),
            radius: r,
        }
    }

    fn half(&self) -> Size {
        match self {
            Outline::Rounded { half, .. } => *half,
            Outline::Polygon(points) => points.iter().fold(Size::ZERO, |s, p| {
                Size::new(s.width.max(p.x.abs()), s.height.max(p.y.abs()))
            }),
        }
    }

    /// This outline grown by `by` view units on every side (shrunk when
    /// negative), never below a sliver.
    fn offset(&self, by: f32) -> Self {
        match self {
            Outline::Rounded { half, radius } => Outline::Rounded {
                half: Size::new((half.width + by).max(1.0), (half.height + by).max(1.0)),
                radius: (radius + by).max(0.0),
            },
            Outline::Polygon(points) => {
                let half = self.half();
                let sx = ((half.width + by) / half.width.max(0.1)).max(0.1);
                let sy = ((half.height + by) / half.height.max(0.1)).max(0.1);
                Outline::Polygon(
                    points
                        .iter()
                        .map(|p| Point::new(p.x * sx, p.y * sy))
                        .collect(),
                )
            }
        }
    }

    /// The primitive drawing this outline about `centre`.
    fn at(&self, centre: Point) -> Primitive {
        match self {
            Outline::Rounded { half, radius } => {
                let rect = Rectangle::new(
                    Point::new(centre.x - half.width, centre.y - half.height),
                    Size::new(half.width * 2.0, half.height * 2.0),
                );
                if *radius <= 0.0 {
                    Primitive::Rect { rect }
                } else if (half.width - half.height).abs() < f32::EPSILON && *radius >= half.width {
                    Primitive::Circle {
                        center: centre,
                        radius: half.width,
                    }
                } else {
                    Primitive::RoundedRect {
                        rect,
                        radius: radius.min(half.width.min(half.height)),
                    }
                }
            }
            Outline::Polygon(points) => Primitive::Polyline {
                points: points
                    .iter()
                    .map(|p| Point::new(centre.x + p.x, centre.y + p.y))
                    .collect(),
                is_closed: true,
            },
        }
    }
}

/// The copper layer an SMD pad sits on.
fn smd_layer(side: PadSide) -> CopperLayer {
    match side {
        PadSide::Bottom => CopperLayer::Bottom,
        _ => CopperLayer::Top,
    }
}

fn copper_paint(class: PadClass) -> Paint {
    let palette = BoardPalette::new();
    match class {
        PadClass::Smd(side) => Paint::Document(palette.layer(PcbLayer::Copper(smd_layer(side)))),
        _ => Paint::Document(palette.multi_layer),
    }
}

fn mask_paint(class: PadClass) -> Paint {
    let layer = match class {
        PadClass::Smd(PadSide::Bottom) => TechnicalLayer::SolderMaskBottom,
        _ => TechnicalLayer::SolderMaskTop,
    };
    Paint::Document(BoardPalette::new().layer(PcbLayer::Technical(layer)))
}

/// A hole: knocked out to the panel ground, its plated wall dashed.
fn hole_style(plated: bool) -> Style {
    let style = if plated {
        Style::dashed()
    } else {
        // A non-plated hole's edge is the part itself: the full ink.
        Style {
            stroke: Style::dashed().stroke.map(|mut s| {
                s.paint = Paint::Ink(Ink::Secondary);
                s.weight = Weight::Regular;
                s
            }),
            fill: None,
        }
    };
    style.with_fill(Paint::Ink(Ink::Panel), Tint::Solid)
}

fn word(form: &Form<'_>, id: MessageId) -> Label<'static> {
    Label::from(t!(form.i18n, id))
}

fn mm(v: f64) -> String {
    format!("{v:.3}")
}

/// The pad illustration of `form`.
pub(super) fn pad_illustration(form: &Form<'_>) -> Illustration {
    let v = form.values;
    let class = form.class;
    let (paste_on, mask_on) = match class {
        PadClass::Smd(PadSide::Bottom) => {
            (v.stack.paste_enabled_bottom, !v.stack.mask_tented_bottom)
        }
        PadClass::Smd(_) => (v.stack.paste_enabled_top, !v.stack.mask_tented_top),
        _ => (
            false,
            !v.stack.mask_tented_top || !v.stack.mask_tented_bottom,
        ),
    };
    let drill = v.drill_diameter_mm.unwrap_or(0.0).max(0.0);
    let (w, h) = match class {
        PadClass::Npth => (drill.max(0.001), drill.max(0.001)),
        _ => (v.size_x_mm.max(0.001), v.size_y_mm.max(0.001)),
    };
    let scale = (FIT.0 / w).min(FIT.1 / h).min(90.0);
    let copper = match class {
        PadClass::Npth => Outline::circle((w * scale) as f32),
        _ => Outline::copper(form, scale),
    };
    let half = copper.half();
    let (mask_mm, paste_mm) = match class {
        PadClass::Smd(PadSide::Bottom) => (v.stack.mask_margin_bottom, v.stack.paste_margin_bottom),
        PadClass::Smd(_) => (v.stack.mask_margin_top, v.stack.paste_margin_top),
        _ => (v.stack.mask_margin_top, None),
    };
    let me = (mask_mm.unwrap_or(RULE_MASK_MM) * scale) as f32;
    let me = if me >= 0.0 { me.max(MIN_MASK_RING) } else { me };
    let pe = (paste_mm.unwrap_or(RULE_PASTE_MM) * scale) as f32;
    let c = CENTRE;

    let mut drawing = Illustration::new(VIEW.width, VIEW.height)
        .feature(PASTE_ON, paste_on)
        .feature(MASK_ON, mask_on)
        .feature(THERMAL_ON, v.stack.thermal_relief);
    let mask_fields = match class {
        PadClass::Smd(_) => vec![MASK],
        _ => vec![MASK, MASK_BOTTOM],
    };
    // The spokes under the copper, so the pad covers their inner ends.
    if class.has_copper_stack() {
        drawing = drawing.parts(relief_parts(form, &copper, c, scale));
    }

    // Copper (or the bare hole of an NPTH).
    if class == PadClass::Npth {
        drawing = drawing.part(Part::new(copper.at(c), hole_style(false)).linked(HOLE));
    } else {
        let fill = Style::edge().with_fill(copper_paint(class), Tint::Body);
        drawing =
            drawing.part(Part::new(copper.at(c), fill).linked_all([SIZE_X, SIZE_Y, SHAPE, CORNER]));
    }
    // The hole of a through-hole pad: round, or a slot along its length.
    match class {
        PadClass::ThroughHole => {
            let hole = Outline::circle((drill * scale) as f32);
            drawing = drawing.part(Part::new(hole.at(c), hole_style(true)).linked(HOLE));
        }
        PadClass::Slot => {
            let length = v.drill_slot_length_mm.unwrap_or(drill).max(drill);
            let slot = Outline::Rounded {
                half: Size::new((length * scale / 2.0) as f32, (drill * scale / 2.0) as f32),
                radius: (drill * scale / 2.0) as f32,
            };
            drawing =
                drawing.part(Part::new(slot.at(c), hole_style(true)).linked_all([HOLE, SLOT]));
        }
        _ => {}
    }
    // Paste (SMD only) and solder mask outlines.
    if matches!(class, PadClass::Smd(_)) {
        drawing = drawing.part(
            Part::new(copper.offset(pe).at(c), Style::dashed())
                .linked(PASTE)
                .requires(PASTE_ON),
        );
    }
    let mask_style =
        Style::outline(Ink::Secondary, Weight::Thin).with_stroke_paint(mask_paint(class));
    drawing = drawing.part(
        Part::new(copper.offset(me).at(c), mask_style)
            .linked_all(mask_fields.clone())
            .requires(MASK_ON),
    );

    drawing = drawing.parts(size_callouts(form, class, c, half, me.max(0.0), (w, h)));
    drawing = drawing.parts(leaders(form, class, c, half, (me, pe), mask_fields));
    let layer = match class {
        PadClass::Smd(side) => {
            crate::pcb_canvas::layer_style::copper_name(form.i18n, smd_layer(side))
        }
        _ => t!(form.i18n, ids::PCB_LAYER_MULTI_LAYER),
    };
    let title = t!(
        form.i18n,
        ids::EDITORS_FP_PAD_CALLOUT_TITLE,
        "number" => v.designator.as_str(),
        "layer" => layer.into_string()
    );
    drawing.part(Part::label(
        TextLabel {
            at: Point::new(4.0, 12.0),
            text: Label::from(title),
            anchor: Anchor::Start,
            role: TextRole::Note,
        },
        Ink::Faint,
    ))
}

/// The thermal relief of a plated pad while it is on (#2593): the air gap
/// as a dashed ring round the copper and the spokes across it, at the pad's
/// own values or, on the rule, at a nominal gap and width.
fn relief_parts(form: &Form<'_>, copper: &Outline, c: Point, scale: f64) -> Vec<Part> {
    let thermal = form.values.stack.thermal;
    let gap = (thermal.air_gap.unwrap_or(RULE_GAP_MM) * scale).max(3.0) as f32;
    let width = (thermal.spoke_width.unwrap_or(RULE_SPOKE_MM) * scale).clamp(1.5, 10.0) as f32;
    let spokes = thermal
        .spokes
        .unwrap_or(signex_types::pad_thermal::DEFAULT_SPOKES);
    let half = copper.half();
    let reach = half.width.max(half.height) + gap + 4.0;
    let mut parts = vec![
        Part::new(copper.offset(gap).at(c), Style::dashed())
            .linked(THERMAL_GAP)
            .requires(THERMAL_ON),
    ];
    let paint = copper_paint(form.class);
    for k in 0..spokes {
        // Counter-clockwise from +x; the drawing's y is down.
        let turn = std::f32::consts::TAU * f32::from(k) / f32::from(spokes);
        let end = Point::new(c.x + reach * turn.cos(), c.y - reach * turn.sin());
        parts.push(
            Part::new(
                Primitive::Band {
                    points: vec![c, end],
                    width,
                    cap: Cap::Flat,
                },
                Style::edge().with_fill(paint, Tint::Body),
            )
            .linked_all([THERMAL_SPOKE, THERMAL_SPOKES])
            .requires(THERMAL_ON),
        );
    }
    parts
}

/// Size X above the pad and Size Y on its right (Size Ø for a hole).
fn size_callouts(
    form: &Form<'_>,
    class: PadClass,
    c: Point,
    half: Size,
    ring: f32,
    (w, h): (f64, f64),
) -> Vec<Part> {
    let (left, right) = (c.x - half.width, c.x + half.width);
    let (top, bottom) = (c.y - half.height, c.y + half.height);
    let above = top - ring - 10.0;
    let (x_label, x_field) = if class == PadClass::Npth {
        (ids::EDITORS_FP_PAD_CALLOUT_DIAMETER, HOLE)
    } else {
        (ids::EDITORS_FP_PAD_CALLOUT_SIZE_X, SIZE_X)
    };
    let mut parts = vec![
        Part::callout(
            Callout::new(Point::new(left, above), Point::new(right, above))
                .label(Label::from(t!(form.i18n, x_label, "value" => mm(w))))
                .extension(Point::new(left, top + 2.0), Point::new(left, above - 3.0))
                .extension(Point::new(right, top + 2.0), Point::new(right, above - 3.0)),
        )
        .linked(x_field),
    ];
    if class != PadClass::Npth {
        let beside = right + ring + 10.0;
        parts.push(
            Part::callout(
                Callout::new(Point::new(beside, top), Point::new(beside, bottom))
                    .label(Label::from(
                        t!(form.i18n, ids::EDITORS_FP_PAD_CALLOUT_SIZE_Y, "value" => mm(h)),
                    ))
                    .label_at(Point::new(beside + 5.0, c.y + 3.0), Anchor::Start)
                    .extension(Point::new(right + 2.0, top), Point::new(beside + 3.0, top))
                    .extension(
                        Point::new(right + 2.0, bottom),
                        Point::new(beside + 3.0, bottom),
                    ),
            )
            .linked(SIZE_Y),
        );
    }
    parts
}

/// The leaders: corner radius, paste and mask of an SMD pad; the hole and
/// the mask of a through-hole pad or a hole.
fn leaders(
    form: &Form<'_>,
    class: PadClass,
    c: Point,
    half: Size,
    (me, pe): (f32, f32),
    mask_fields: Vec<FieldId>,
) -> Vec<Part> {
    let v = form.values;
    let right = |at: Point, y: f32, label: Label<'static>| {
        Part::leader(Leader {
            at,
            label_at: Point::new(RIGHT_LABEL_X, y),
            label,
            anchor: Anchor::Start,
        })
    };
    let left = |at: Point, y: f32, label: Label<'static>| {
        Part::leader(Leader {
            at,
            label_at: Point::new(LEFT_LABEL_X, y),
            label,
            anchor: Anchor::End,
        })
    };
    let mut parts = Vec::new();
    match class {
        PadClass::Smd(side) => {
            if let signex_library::PadShape::RoundRect { .. } = v.shape
                && let Some(pct) = v.stack.corner_radius_pct.filter(|p| *p > 0.0)
            {
                let corner = Point::new(c.x + half.width * 0.9, c.y - half.height * 0.9);
                let label = t!(form.i18n, ids::EDITORS_FP_PAD_CALLOUT_CORNER, "value" => format!("{pct:.0}"));
                parts.push(right(corner, 30.0, Label::from(label)).linked(CORNER));
            }
            let (paste, mask) = match side {
                PadSide::Bottom => (
                    TechnicalLayer::PasteBottom,
                    TechnicalLayer::SolderMaskBottom,
                ),
                _ => (TechnicalLayer::PasteTop, TechnicalLayer::SolderMaskTop),
            };
            let paste_at = Point::new(c.x - half.width / 2.0, c.y + half.height + pe);
            parts.push(
                left(paste_at, 126.0, Label::from(layer_title(form.i18n, paste)))
                    .linked(PASTE)
                    .requires(PASTE_ON),
            );
            let mask_at = Point::new(c.x + half.width + me * 0.7, c.y + half.height + me * 0.7);
            parts.push(
                right(mask_at, 112.0, Label::from(layer_title(form.i18n, mask)))
                    .linked(MASK)
                    .requires(MASK_ON),
            );
            let tented = match side {
                PadSide::Bottom => v.stack.mask_tented_bottom,
                _ => v.stack.mask_tented_top,
            };
            if tented {
                let note = t!(
                    form.i18n,
                    ids::EDITORS_FP_PAD_CALLOUT_TENTED,
                    "layer" => layer_title(form.i18n, mask).into_string()
                );
                parts.push(
                    Part::label(
                        TextLabel {
                            at: Point::new(VIEW.width - 4.0, 126.0),
                            text: Label::from(note),
                            anchor: Anchor::End,
                            role: TextRole::Note,
                        },
                        Ink::Faint,
                    )
                    .linked(MASK),
                );
            }
        }
        _ => {
            let drill = (v.drill_diameter_mm.unwrap_or(0.0) / 2.0) as f32;
            let (hole_id, hole_at) = match class {
                PadClass::Slot => (
                    ids::EDITORS_FP_PAD_CALLOUT_SLOT_PLATED,
                    Point::new(c.x + half.width * 0.6, c.y),
                ),
                PadClass::Npth => (
                    ids::EDITORS_FP_PAD_CALLOUT_HOLE_NPTH,
                    Point::new(c.x + half.width * 0.7, c.y + half.height * 0.7),
                ),
                _ => (
                    ids::EDITORS_FP_PAD_CALLOUT_HOLE_PLATED,
                    Point::new(c.x + drill, c.y),
                ),
            };
            parts.push(right(hole_at, 112.0, word(form, hole_id)).linked(HOLE));
            let mask_at = Point::new(c.x - half.width - me * 0.7, c.y + half.height + me * 0.7);
            parts.push(
                left(
                    mask_at,
                    126.0,
                    word(form, ids::EDITORS_FP_PAD_CALLOUT_SOLDER_MASK),
                )
                .linked_all(mask_fields)
                .requires(MASK_ON),
            );
        }
    }
    parts
}

/// One layer's pad, drawn small for a layer chip: the same copper and hole
/// on every layer while the model holds one shape for all of them.
fn chip_glyph(form: &Form<'_>) -> Illustration {
    let v = form.values;
    let class = form.class;
    let drill = v.drill_diameter_mm.unwrap_or(0.0).max(0.0);
    let (w, h) = match class {
        PadClass::Npth => (drill.max(0.001), drill.max(0.001)),
        _ => (v.size_x_mm.max(0.001), v.size_y_mm.max(0.001)),
    };
    let scale = (20.0 / w).min(12.0 / h);
    let c = Point::new(CHIP_VIEW.width / 2.0, CHIP_VIEW.height / 2.0);
    let mut glyph = Illustration::new(CHIP_VIEW.width, CHIP_VIEW.height).height_em(CHIP_GLYPH_EM);
    match class {
        PadClass::Npth => {
            let hole = Outline::circle((w * scale) as f32);
            glyph = glyph.part(Part::new(hole.at(c), hole_style(false)));
        }
        _ => {
            let fill = Style::edge().with_fill(copper_paint(class), Tint::Strong);
            glyph = glyph.part(Part::new(Outline::copper(form, scale).at(c), fill));
            let hole = match (class, v.drill_slot_length_mm) {
                (PadClass::Slot, Some(length)) => Outline::Rounded {
                    half: Size::new(
                        (length.max(drill) * scale / 2.0) as f32,
                        (drill * scale / 2.0) as f32,
                    ),
                    radius: (drill * scale / 2.0) as f32,
                },
                _ => Outline::circle((drill * scale) as f32),
            };
            glyph = glyph.part(Part::new(hole.at(c), hole_style(true)));
        }
    }
    glyph
}

/// The Layers row under the illustration of a through-hole pad or a hole:
/// Top, the inner layers and Bottom, each with its pad drawn. Full Stack
/// names the inner layers by position (L1 is Top, Ln is Bottom).
pub(super) fn layer_chips<'a>(form: &Form<'_>) -> Element<'a, PanelMsg> {
    let inner =
        if form.class.has_copper_stack() && form.values.pad_stack_tab == PadStackTab::FullStack {
            ids::EDITORS_FP_PAD_LAYER_INNER_RANGE
        } else {
            ids::EDITORS_FP_PAD_LAYER_INNER
        };
    let ink = crate::styles::tokens::resolve(form.tokens).ink4;
    let mut chips: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center)
        .push(
            text(t!(form.i18n, ids::EDITORS_FP_PAD_LAYERS))
                .size(TS.caption)
                .color(ink)
                .boxed(),
        );
    for id in [
        ids::EDITORS_FP_PAD_LAYER_TOP,
        inner,
        ids::EDITORS_FP_PAD_LAYER_BOTTOM,
    ] {
        let glyph = property_illustration(chip_glyph(form), form.hover, form.tokens, TS.caption)
            .map(super::super::illustration::hover_msg);
        let chip = iced::widget::row![
            container(glyph).width(Length::Fixed(TS.caption * 2.0)),
            text(t!(form.i18n, id)).size(TS.caption),
        ]
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center);
        chips = chips.push(
            container(chip)
                .padding([0.0, SP.s])
                .style(crate::styles::pill(form.tokens, PillTone::Neutral))
                .boxed(),
        );
    }
    container(chips)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}
