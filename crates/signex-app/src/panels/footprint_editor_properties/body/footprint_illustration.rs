//! The Footprint sections' illustration: the pads with a light fill, the
//! courtyard dashed around them, a Courtyard leader and the Expansion
//! callout between the pads and the courtyard.

use iced::{Point, Rectangle};
use signex_i18n::{I18n, ids, t};
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, Ink, Leader, Paint, Part, Primitive, Style, Tint, Weight,
};

use super::fields;
use crate::panels::{FootprintEditorPanelContext, PadOutlineSummary};

/// The view box, in illustration units (the mockup's 264 × 140).
const VIEW_W: f32 = 264.0;
const VIEW_H: f32 = 140.0;
/// Room kept free left and right of the drawing for the callout labels.
const MARGIN_X: f32 = 60.0;
/// Room kept free above and below for the leader and the callout.
const MARGIN_Y: f32 = 24.0;
/// How far the Expansion callout runs under the courtyard.
const CALLOUT_DROP: f32 = 12.0;
/// The leader's elbow, up and right of the courtyard corner.
const LEADER_RUN: f32 = 10.0;
/// A rotation within this many degrees of a quarter turn draws upright.
const UPRIGHT_TOLERANCE_DEG: f64 = 1e-6;

/// Footprint millimetres to view units: uniform scale, centred.
struct Map {
    scale: f32,
    origin: Point,
    min: [f64; 2],
}

impl Map {
    fn fit(extent: [f64; 4]) -> Self {
        let [x0, y0, x1, y1] = extent;
        let width = ((x1 - x0) as f32).max(f32::EPSILON);
        let height = ((y1 - y0) as f32).max(f32::EPSILON);
        let scale = ((VIEW_W - 2.0 * MARGIN_X) / width).min((VIEW_H - 2.0 * MARGIN_Y) / height);
        let origin = Point::new(
            (VIEW_W - width * scale) / 2.0,
            (VIEW_H - height * scale) / 2.0,
        );
        Self {
            scale,
            origin,
            min: [x0, y0],
        }
    }

    fn point(&self, [x, y]: [f64; 2]) -> Point {
        Point::new(
            self.origin.x + (x - self.min[0]) as f32 * self.scale,
            self.origin.y + (y - self.min[1]) as f32 * self.scale,
        )
    }

    fn rect(&self, [x0, y0, x1, y1]: [f64; 4]) -> Rectangle {
        let a = self.point([x0, y0]);
        let b = self.point([x1, y1]);
        Rectangle::new(a, iced::Size::new(b.x - a.x, b.y - a.y))
    }
}

fn grow(extent: Option<[f64; 4]>, [x, y]: [f64; 2]) -> Option<[f64; 4]> {
    Some(match extent {
        Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
        None => [x, y, x, y],
    })
}

/// The extent of the pads' turned corners.
fn pads_extent(pads: &[PadOutlineSummary]) -> Option<[f64; 4]> {
    pads.iter().flat_map(|pad| pad.corners_mm).fold(None, grow)
}

/// The illustration of `fp`'s pads and courtyard; `None` without either.
pub(super) fn illustration(fp: &FootprintEditorPanelContext, i18n: &I18n) -> Option<Illustration> {
    let pads = pads_extent(&fp.pad_outlines);
    let extent = match (pads, fp.courtyard_extent_mm) {
        (Some(p), Some([x0, y0, x1, y1])) => grow(grow(Some(p), [x0, y0]), [x1, y1])?,
        (Some(p), None) => p,
        (None, Some(c)) => c,
        (None, None) => return None,
    };
    let map = Map::fit(extent);
    let mut ill =
        Illustration::new(VIEW_W, VIEW_H).feature(fields::COURTYARD, fp.auto_fit_courtyard);
    for pad in &fp.pad_outlines {
        ill = ill.parts(pad_parts(pad, &map));
    }
    if let Some(courtyard) = fp.courtyard_extent_mm {
        ill = ill.parts(courtyard_parts(courtyard, pads, &map, i18n));
    }
    Some(ill)
}

/// One pad: its copper with a light fill, and its hole knocked out.
fn pad_parts(pad: &PadOutlineSummary, map: &Map) -> Vec<Part> {
    use signex_library::PadShape;
    let copper = Style::edge().with_fill(Paint::Ink(Ink::Secondary), Tint::Body);
    let quarter_turns = pad.rotation_deg / 90.0;
    let is_upright = (quarter_turns - quarter_turns.round()).abs() < UPRIGHT_TOLERANCE_DEG;
    let corners = pad.corners_mm.map(|c| map.point(c));
    let bounds = corners
        .iter()
        .skip(1)
        .fold(Rectangle::new(corners[0], iced::Size::ZERO), |r, p| {
            r.union(&Rectangle::new(*p, iced::Size::ZERO))
        });
    let short = bounds.width.min(bounds.height);
    let body = match (&pad.shape, is_upright) {
        (PadShape::Round | PadShape::Oval, true) => Primitive::RoundedRect {
            rect: bounds,
            radius: short / 2.0,
        },
        (PadShape::RoundRect { radius_ratio }, true) => Primitive::RoundedRect {
            rect: bounds,
            radius: short * (*radius_ratio as f32),
        },
        (_, true) => Primitive::Rect { rect: bounds },
        (_, false) => Primitive::Polyline {
            points: corners.to_vec(),
            is_closed: true,
        },
    };
    let mut parts = vec![Part::new(body, copper)];
    if pad.has_drill {
        let hole = Style::outline(Ink::Secondary, Weight::Thin)
            .with_fill(Paint::Ink(Ink::Panel), Tint::Solid);
        parts.push(Part::new(
            Primitive::Circle {
                center: bounds.center(),
                radius: short / 4.0,
            },
            hole,
        ));
    }
    parts
}

/// The dashed courtyard, its leader and the Expansion callout from the pads'
/// right edge to the courtyard's.
fn courtyard_parts(
    courtyard: [f64; 4],
    pads: Option<[f64; 4]>,
    map: &Map,
    i18n: &I18n,
) -> Vec<Part> {
    let rect = map.rect(courtyard);
    let corner = Point::new(rect.x + rect.width, rect.y);
    let mut parts = vec![
        Part::new(Primitive::Rect { rect }, Style::dashed()).linked(fields::COURTYARD),
        Part::leader(Leader {
            at: corner,
            label_at: Point::new(corner.x + LEADER_RUN, corner.y - LEADER_RUN),
            label: t!(i18n, ids::EDITORS_FP_PROPS_ILLUSTRATION_COURTYARD).into(),
            anchor: Anchor::Start,
        })
        .linked(fields::COURTYARD),
    ];
    if let Some([_, _, pads_right, pads_bottom]) = pads {
        let y = rect.y + rect.height + CALLOUT_DROP;
        let from = Point::new(map.point([pads_right, pads_bottom]).x, y);
        let to = Point::new(rect.x + rect.width, y);
        let callout = Callout::new(from, to)
            .label(t!(i18n, ids::EDITORS_FP_PROPS_EXPANSION))
            .label_at(Point::new(to.x + LEADER_RUN / 2.0, y), Anchor::Start)
            .extension(map.point([pads_right, pads_bottom]), from)
            .extension(Point::new(to.x, rect.y + rect.height), to);
        parts.push(
            Part::callout(callout)
                .linked(fields::EXPANSION)
                .requires(fields::COURTYARD),
        );
    }
    parts
}
