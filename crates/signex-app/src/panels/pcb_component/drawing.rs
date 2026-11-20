//! The footprint illustration of the Component view (Graphite mockup row 94): the pads
//! in a light tint of the side's copper colour, the dashed courtyard, the
//! body outline, pin 1 and the designator, and a note naming the side. No
//! rotation is shown (review 2026-11-18). The geometry is footprint-local and
//! comes with the facts ([`PcbComponentFacts::drawing`]); this module only
//! fits it into the view box and links each part to the field it shows.

use iced::widget::container;
use iced::{Element, Length, Padding, Point as ViewPoint, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::pcb::Point;
use signex_types::pcb_layer::{BoardSide, CopperLayer, PcbLayer};
use signex_widgets::property_illustration::{
    Anchor, FieldId, Illustration, Ink, Leader, Paint, Part, Primitive, Style, TextLabel, TextRole,
    Tint, Weight, property_illustration,
};

use super::{PcbComponentFacts, PcbComponentMsg, Shared};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::board_palette::BoardPalette;
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The Footprint Name row: the outline, the pads and the courtyard.
pub const FOOTPRINT_FIELD: FieldId = FieldId::new("pcb_component.footprint");
/// The Designator row: the designator text.
pub const DESIGNATOR_FIELD: FieldId = FieldId::new("pcb_component.designator");
/// The Layer row: the pad tint and the side note.
pub const LAYER_FIELD: FieldId = FieldId::new("pcb_component.layer");
/// Pin 1's leader; no row shows it.
pub const PIN_ONE_FIELD: FieldId = FieldId::new("pcb_component.pin_one");

// The view box and the label places of the approved mockup row 94 drawing, in view
// units; the widget fits the box uniformly into its bounds.
const VIEW_W: f32 = 264.0;
const VIEW_H: f32 = 132.0;
/// The centre and the half size of the box the footprint is fitted into.
const FIT_CENTER: ViewPoint = ViewPoint::new(124.0, 66.0);
const FIT_HALF_W: f32 = 40.0;
const FIT_HALF_H: f32 = 46.0;
/// Where the Pin 1 label sits: left of the footprint, or right of it when
/// pin 1 is on the right half.
const PIN_LABEL_LEFT: ViewPoint = ViewPoint::new(52.0, 16.0);
const PIN_LABEL_RIGHT: ViewPoint = ViewPoint::new(196.0, 16.0);
const COURTYARD_LABEL: ViewPoint = ViewPoint::new(196.0, 104.0);
const SIDE_NOTE: ViewPoint = ViewPoint::new(6.0, 128.0);
/// The widget height, in captions: the view box at the panel's width.
const HEIGHT_EM: f32 = 12.0;
/// A footprint narrower than this (mm) is fitted as if it were this wide.
const MIN_EXTENT_MM: f64 = 1e-3;

/// A pad's copper as drawn, footprint-local mm.
#[derive(Debug, Clone, PartialEq)]
pub enum PadOutline {
    /// A round pad.
    Disc { center: Point, radius: f64 },
    /// Any other pad: its closed outline.
    Ring(Vec<Point>),
}

/// A footprint as its illustration draws it: footprint-local mm in the
/// board's axes (y down), mirrored on the bottom side, unturned.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentDrawing {
    pub side: BoardSide,
    pub pads: Vec<PadOutline>,
    /// The centre of the pad numbered 1, when there is one.
    pub pin_one: Option<Point>,
    /// The courtyard strokes; empty when the footprint has no courtyard
    /// drawings (none is made up).
    pub courtyard: Vec<(Point, Point)>,
    /// The fabrication and assembly outline strokes.
    pub outline: Vec<(Point, Point)>,
}

impl ComponentDrawing {
    /// Every point the drawing reaches.
    fn points(&self) -> impl Iterator<Item = Point> + '_ {
        let pads = self.pads.iter().flat_map(|pad| match pad {
            PadOutline::Disc { center, radius } => vec![
                Point::new(center.x - radius, center.y - radius),
                Point::new(center.x + radius, center.y + radius),
            ],
            PadOutline::Ring(points) => points.clone(),
        });
        let strokes = self
            .courtyard
            .iter()
            .chain(&self.outline)
            .flat_map(|(a, b)| [*a, *b]);
        pads.chain(strokes)
    }

    /// The middle of the courtyard's rightmost stroke, where its leader
    /// starts.
    fn courtyard_edge(&self) -> Option<Point> {
        self.courtyard
            .iter()
            .map(|(a, b)| Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0))
            .max_by(|p, q| p.x.total_cmp(&q.x))
    }
}

/// Footprint mm to view units: a uniform scale about the drawing's middle.
struct Fit {
    scale: f32,
    middle: Point,
}

impl Fit {
    fn of(drawing: &ComponentDrawing) -> Option<Self> {
        let mut points = drawing.points();
        let first = points.next()?;
        let (mut lo, mut hi) = (first, first);
        for p in points {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        let width = (hi.x - lo.x).max(MIN_EXTENT_MM);
        let height = (hi.y - lo.y).max(MIN_EXTENT_MM);
        let scale = (f64::from(FIT_HALF_W) * 2.0 / width).min(f64::from(FIT_HALF_H) * 2.0 / height);
        Some(Self {
            scale: scale as f32,
            middle: Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0),
        })
    }

    fn point(&self, p: Point) -> ViewPoint {
        ViewPoint::new(
            FIT_CENTER.x + (p.x - self.middle.x) as f32 * self.scale,
            FIT_CENTER.y + (p.y - self.middle.y) as f32 * self.scale,
        )
    }

    fn length(&self, mm: f64) -> f32 {
        mm as f32 * self.scale
    }
}

/// The illustration of the one selected component; `None` with several
/// selected or a footprint with nothing to draw.
pub fn illustration(facts: &PcbComponentFacts, i18n: &I18n) -> Option<Illustration> {
    let drawing = facts.drawing.as_ref()?;
    let fit = Fit::of(drawing)?;
    let copper = BoardPalette::new().layer(PcbLayer::Copper(match drawing.side {
        BoardSide::Top => CopperLayer::Top,
        BoardSide::Bottom => CopperLayer::Bottom,
    }));
    let line = |(a, b): &(Point, Point), style: Style| {
        Part::new(
            Primitive::Line {
                from: fit.point(*a),
                to: fit.point(*b),
            },
            style,
        )
        .linked(FOOTPRINT_FIELD)
    };
    let mut parts: Vec<Part> = drawing
        .courtyard
        .iter()
        .map(|stroke| line(stroke, Style::dashed()))
        .collect();
    let pad_style = Style::outline(Ink::Secondary, Weight::Thin)
        .with_fill(Paint::Document(copper), Tint::Strong);
    parts.extend(drawing.pads.iter().map(|pad| {
        let primitive = match pad {
            PadOutline::Disc { center, radius } => Primitive::Circle {
                center: fit.point(*center),
                radius: fit.length(*radius),
            },
            PadOutline::Ring(points) => Primitive::Polyline {
                points: points.iter().map(|p| fit.point(*p)).collect(),
                is_closed: true,
            },
        };
        Part::new(primitive, pad_style).linked_all([FOOTPRINT_FIELD, LAYER_FIELD])
    }));
    parts.extend(
        drawing
            .outline
            .iter()
            .map(|stroke| line(stroke, Style::edge())),
    );
    if let Shared::Same(designator) = &facts.designator {
        let label = TextLabel {
            at: FIT_CENTER,
            text: UserText::new(designator.clone()).into(),
            anchor: Anchor::Middle,
            role: TextRole::Mono,
        };
        parts.push(Part::label(label, Ink::Secondary).linked(DESIGNATOR_FIELD));
    }
    if let Some(pin) = drawing.pin_one {
        let at = fit.point(pin);
        let (label_at, anchor) = if at.x <= FIT_CENTER.x {
            (PIN_LABEL_LEFT, Anchor::End)
        } else {
            (PIN_LABEL_RIGHT, Anchor::Start)
        };
        let leader = Leader {
            at,
            label_at,
            label: t!(i18n, ids::PCB_COMPONENT_PIN_ONE).into(),
            anchor,
        };
        parts.push(Part::leader(leader).linked(PIN_ONE_FIELD));
    }
    if let Some(edge) = drawing.courtyard_edge() {
        let leader = Leader {
            at: fit.point(edge),
            label_at: COURTYARD_LABEL,
            label: t!(i18n, ids::PCB_COMPONENT_COURTYARD).into(),
            anchor: Anchor::Start,
        };
        parts.push(Part::leader(leader).linked(FOOTPRINT_FIELD));
    }
    let side = match drawing.side {
        BoardSide::Top => ids::PCB_COMPONENT_SIDE_TOP,
        BoardSide::Bottom => ids::PCB_COMPONENT_SIDE_BOTTOM,
    };
    let note = TextLabel {
        at: SIDE_NOTE,
        text: t!(i18n, side).into(),
        anchor: Anchor::Start,
        role: TextRole::Note,
    };
    parts.push(Part::label(note, Ink::Faint).linked(LAYER_FIELD));
    Some(
        Illustration::new(VIEW_W, VIEW_H)
            .height_em(HEIGHT_EM)
            .parts(parts),
    )
}

/// The illustration on the panel ground under the search field, its hover
/// routed to the panel state.
pub(super) fn view<'a>(
    ctx: &'a PanelContext,
    facts: &'a PcbComponentFacts,
) -> Option<Element<'a, PanelMsg>> {
    let drawn = illustration(facts, &ctx.i18n)?;
    let widget = property_illustration(drawn, &facts.illustration_hover, &ctx.tokens, TS.caption)
        .map(|msg| PanelMsg::PcbComponent(PcbComponentMsg::IllustrationHovered(msg)));
    Some(
        container(widget)
            .padding(Padding {
                top: SP.xs,
                right: SP.m,
                bottom: SP.s,
                left: SP.m,
            })
            .width(Length::Fill)
            .boxed(),
    )
}
