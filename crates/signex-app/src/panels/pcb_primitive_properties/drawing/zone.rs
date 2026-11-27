//! The area illustration (#2523, mockup row 99): a corner of the pour around a pad of
//! its net, joined directly or, with Thermal Relief on, through spokes across
//! an air gap (#2580), and a pad of another net cleared by the Clearance; a
//! shelved pour as its dashed outline alone; a keepout or a pour cutout
//! (#2871) as its outline and what it keeps out.

use iced::{Point, Rectangle, Size};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Illustration, Ink, Leader, Paint, Part, Primitive, Stroke, Style,
    TextRole, Tint, Weight,
};

use super::super::{ZoneFacts, ZoneKind};
use super::{VIEW_H, VIEW_W, canvas, label, layer_paint};
use crate::styles::board_palette::BoardPalette;

/// The fields the illustration shows, shared with their rows.
pub(in crate::panels::pcb_primitive_properties) const NET: FieldId = FieldId::new("area.net");
pub(in crate::panels::pcb_primitive_properties) const LAYER: FieldId = FieldId::new("area.layer");
pub(in crate::panels::pcb_primitive_properties) const KEEPS_OUT: FieldId =
    FieldId::new("area.keeps_out");
pub(in crate::panels::pcb_primitive_properties) const FILL_MODE: FieldId =
    FieldId::new("pour.fill_mode");
pub(in crate::panels::pcb_primitive_properties) const CLEARANCE: FieldId =
    FieldId::new("pour.clearance");
pub(in crate::panels::pcb_primitive_properties) const THERMAL: FieldId =
    FieldId::new("pour.thermal");
pub(in crate::panels::pcb_primitive_properties) const SPOKE_GAP: FieldId =
    FieldId::new("pour.spoke_gap");
pub(in crate::panels::pcb_primitive_properties) const CORNERS: FieldId =
    FieldId::new("pour.corners");
pub(in crate::panels::pcb_primitive_properties) const SHELVE: FieldId = FieldId::new("pour.shelve");

/// View units per mm of the drawn pads and gaps.
const SCALE: f32 = 22.0;
/// The drawn pads: 1.6 mm across with a 0.8 mm hole.
const PAD_R: f32 = 0.8 * SCALE;
const HOLE_R: f32 = 0.4 * SCALE;
/// The clearance drawn when the pour follows the board's.
const BOARD_CLEARANCE_MM: f32 = 0.2;
/// The pour corner: its top-left; it runs on past the right of the view
/// and down to above the note.
const LEFT: f32 = 20.0;
const TOP: f32 = 18.0;
const FILL_BOTTOM_INSET: f32 = 22.0;
/// Where the pads sit: one of the pour's net, one of another net.
const PAD_Y: f32 = 62.0;
const OWN_PAD_X: f32 = 86.0;
const OTHER_PAD_X: f32 = 196.0;

/// The illustration of the selected areas `f`.
pub(in crate::panels::pcb_primitive_properties) fn illustration(
    f: &ZoneFacts,
    i18n: &I18n,
) -> Illustration {
    let paint = layer_paint(
        f.layer
            .as_ref()
            .map_or(PcbLayer::Copper(CopperLayer::Top), |l| l.layer),
    );
    let layer_name = f
        .layer
        .as_ref()
        .map_or_else(String::new, |l| l.label.clone());
    let relief_on = f.thermal.is_some_and(|(on, _, _)| on);
    let art = canvas().feature(THERMAL, relief_on);
    match f.kind {
        Some(ZoneKind::Keepout) => {
            let caption = t!(i18n, ids::PCB_PRIM_POUR_DRAWING_KEEPOUT);
            return keepout(art, paint, layer_name, caption);
        }
        Some(ZoneKind::Cutout) => {
            let caption = t!(i18n, ids::PCB_PRIM_CUTOUT_DRAWING);
            return keepout(art, paint, layer_name, caption);
        }
        _ => {}
    }
    let net = f
        .common
        .net
        .as_ref()
        .map_or_else(String::new, |n| n.label.clone());
    if f.shelved == Some(true) {
        return art.part(outline(true)).parts(pads(net, i18n)).part(label(
            Point::new(VIEW_W / 2.0, VIEW_H - 8.0),
            t!(i18n, ids::PCB_PRIM_POUR_DRAWING_SHELVED),
            Anchor::Middle,
            TextRole::Note,
            &[SHELVE],
        ));
    }
    let clearance = f
        .clearance
        .filter(|c| c.is_finite() && *c > 0.0)
        .map_or(BOARD_CLEARANCE_MM, |c| c as f32);
    let gap = (clearance * SCALE).clamp(3.0, 20.0);
    let edge = Point::new(OTHER_PAD_X + PAD_R, PAD_Y);
    let edge_out = Point::new(edge.x + gap, PAD_Y);
    let note = t!(
        i18n,
        ids::PCB_PRIM_POUR_DRAWING_NOTE,
        "net" => net.clone(),
        "layer" => layer_name
    );
    // The fill first; the clearance ring knocks it out to the panel ground;
    // the outline and pads go on top.
    art.part(
        Part::new(
            Primitive::Rect {
                rect: Rectangle::new(
                    Point::new(LEFT, TOP),
                    Size::new(VIEW_W - LEFT, fill_bottom() - TOP),
                ),
            },
            Style::filled(paint, Tint::Body),
        )
        .linked_all([FILL_MODE, NET, LAYER]),
    )
    .part(
        Part::new(
            Primitive::Circle {
                center: Point::new(OTHER_PAD_X, PAD_Y),
                radius: PAD_R + gap,
            },
            Style::filled(Paint::Ink(Ink::Panel), Tint::Solid),
        )
        .linked(CLEARANCE),
    )
    .parts(relief_copper(f.thermal, paint))
    .part(outline(false))
    .parts(pads(net, i18n))
    .parts(relief(i18n, relief_on))
    .part(Part::callout(Callout::new(edge, edge_out)).linked(CLEARANCE))
    .part(
        Part::leader(Leader {
            at: edge_out,
            label_at: Point::new(VIEW_W - 6.0, TOP + 8.0),
            label: t!(i18n, ids::PCB_PRIM_CLEARANCE).into(),
            anchor: Anchor::End,
        })
        .linked(CLEARANCE),
    )
    .part(label(
        Point::new(VIEW_W - 6.0, VIEW_H - 8.0),
        note,
        Anchor::End,
        TextRole::Note,
        &[NET, LAYER, FILL_MODE],
    ))
}

fn fill_bottom() -> f32 {
    VIEW_H - FILL_BOTTOM_INSET
}

/// The pour's corner, dashed and empty while shelved.
fn outline(shelved: bool) -> Part {
    let style = if shelved {
        Style {
            stroke: Some(Stroke {
                paint: Paint::Ink(Ink::Secondary),
                weight: Weight::Regular,
                is_dashed: true,
            }),
            fill: None,
        }
    } else {
        Style::edge()
    };
    Part::new(
        Primitive::Polyline {
            points: vec![
                Point::new(VIEW_W, TOP),
                Point::new(LEFT, TOP),
                Point::new(LEFT, fill_bottom()),
            ],
            is_closed: false,
        },
        style,
    )
    .linked_all([CORNERS, LAYER, SHELVE])
}

/// The two through-hole pads, each named under it: the pour's net, and
/// another net.
fn pads(net: String, i18n: &I18n) -> Vec<Part> {
    let multi = Paint::Document(BoardPalette::new().multi_layer);
    let mut parts = Vec::new();
    for x in [OWN_PAD_X, OTHER_PAD_X] {
        let center = Point::new(x, PAD_Y);
        parts.push(Part::new(
            Primitive::Circle {
                center,
                radius: PAD_R,
            },
            Style::edge().with_fill(multi, Tint::Strong),
        ));
        parts.push(Part::new(
            Primitive::Circle {
                center,
                radius: HOLE_R,
            },
            Style::outline(Ink::Secondary, Weight::Thin)
                .with_fill(Paint::Ink(Ink::Panel), Tint::Solid),
        ));
    }
    parts.push(label(
        Point::new(OWN_PAD_X, PAD_Y + PAD_R + 9.0),
        UserText::new(net),
        Anchor::Middle,
        TextRole::Note,
        &[NET],
    ));
    parts.push(label(
        Point::new(OTHER_PAD_X, PAD_Y + PAD_R + 22.0),
        t!(i18n, ids::PCB_PRIM_POUR_DRAWING_OTHER_NET),
        Anchor::Middle,
        TextRole::Note,
        &[],
    ));
    parts
}

/// The air gap round the pad of the pour's net, knocked out to the panel
/// ground, and the four spokes across it, drawn at the stored gap and
/// width; shown while Thermal Relief is on.
fn relief_copper(thermal: Option<(bool, f64, f64)>, paint: Paint) -> Vec<Part> {
    let (spoke_mm, gap_mm) = thermal.map_or((0.0, 0.0), |(_, s, g)| (s, g));
    let gap = (gap_mm as f32 * SCALE).clamp(3.0, 12.0);
    let width = (spoke_mm as f32 * SCALE).clamp(2.0, 8.0);
    let reach = PAD_R + gap + 3.0;
    let ring = Part::new(
        Primitive::Circle {
            center: Point::new(OWN_PAD_X, PAD_Y),
            radius: PAD_R + gap,
        },
        Style::filled(Paint::Ink(Ink::Panel), Tint::Solid),
    )
    .linked(SPOKE_GAP)
    .requires(THERMAL);
    let spoke = |rect: Rectangle| {
        Part::new(Primitive::Rect { rect }, Style::filled(paint, Tint::Body))
            .linked(SPOKE_GAP)
            .requires(THERMAL)
    };
    vec![
        ring,
        spoke(Rectangle::new(
            Point::new(OWN_PAD_X - reach, PAD_Y - width / 2.0),
            Size::new(2.0 * reach, width),
        )),
        spoke(Rectangle::new(
            Point::new(OWN_PAD_X - width / 2.0, PAD_Y - reach),
            Size::new(width, 2.0 * reach),
        )),
    ]
}

/// The pad of the pour's net: "direct connect" while Thermal Relief is off;
/// the spoke and gap leaders, which need the thermal feature, while on.
fn relief(i18n: &I18n, relief_on: bool) -> Vec<Part> {
    let mut parts = Vec::new();
    if !relief_on {
        parts.push(label(
            Point::new(OWN_PAD_X, PAD_Y - PAD_R - 7.0),
            t!(i18n, ids::PCB_PRIM_POUR_DRAWING_DIRECT),
            Anchor::Middle,
            TextRole::Note,
            &[THERMAL],
        ));
    }
    parts.extend([
        Part::leader(Leader {
            at: Point::new(OWN_PAD_X + PAD_R * 0.7, PAD_Y - PAD_R * 0.7),
            label_at: Point::new(OWN_PAD_X + 34.0, TOP + 6.0),
            label: t!(i18n, ids::PCB_PRIM_POUR_DRAWING_SPOKE).into(),
            anchor: Anchor::Start,
        })
        .linked(SPOKE_GAP)
        .requires(THERMAL),
        Part::leader(Leader {
            at: Point::new(OWN_PAD_X - PAD_R, PAD_Y),
            label_at: Point::new(LEFT + 6.0, fill_bottom() - 6.0),
            label: t!(i18n, ids::PCB_PRIM_POUR_DRAWING_GAP).into(),
            anchor: Anchor::Start,
        })
        .linked(SPOKE_GAP)
        .requires(THERMAL),
    ]);
    parts
}

/// A keepout or a pour cutout (#2871): its outline with a light tint, and
/// `caption`, what it keeps out.
fn keepout(
    art: Illustration,
    paint: Paint,
    layer_name: String,
    caption: Localized,
) -> Illustration {
    let (left, right) = (40.0, VIEW_W - 24.0);
    let (top, bottom) = (14.0, VIEW_H - FILL_BOTTOM_INSET);
    let shape = Part::new(
        Primitive::Polyline {
            points: vec![
                Point::new(left, top),
                Point::new(right - 20.0, top),
                Point::new(right, top + 20.0),
                Point::new(right, bottom),
                Point::new(left, bottom),
            ],
            is_closed: true,
        },
        Style::edge().with_fill(paint, Tint::Light),
    )
    .linked_all([KEEPS_OUT, LAYER]);
    art.part(shape)
        .part(label(
            Point::new((left + right) / 2.0, (top + bottom) / 2.0),
            caption,
            Anchor::Middle,
            TextRole::Caption,
            &[KEEPS_OUT],
        ))
        .part(label(
            Point::new(VIEW_W - 6.0, VIEW_H - 8.0),
            UserText::new(layer_name),
            Anchor::End,
            TextRole::Note,
            &[LAYER],
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Thermal Relief off: no gap, spoke or leader shows; "direct connect"
    /// does.
    #[test]
    fn the_spoke_and_gap_leaders_stay_out_while_thermal_relief_is_off() {
        let off = Some((false, 0.254, 0.508));
        let art = canvas()
            .parts(relief_copper(off, Paint::Ink(Ink::Secondary)))
            .parts(relief(&I18n::en_us(), false))
            .feature(THERMAL, false);
        assert!(art.visible_parts().all(|p| !p.is_linked_to(&SPOKE_GAP)));
        assert!(art.visible_parts().any(|p| p.is_linked_to(&THERMAL)));
    }

    /// #2580: Thermal Relief on draws the gap ring, the spokes and their two
    /// leaders, and no "direct connect".
    #[test]
    fn thermal_relief_on_draws_the_gap_the_spokes_and_their_leaders() {
        let on = Some((true, 0.254, 0.508));
        let art = canvas()
            .parts(relief_copper(on, Paint::Ink(Ink::Secondary)))
            .parts(relief(&I18n::en_us(), true))
            .feature(THERMAL, true);
        let spoke_gap = art
            .visible_parts()
            .filter(|p| p.is_linked_to(&SPOKE_GAP))
            .count();
        assert_eq!(spoke_gap, 5, "the ring, two spokes and two leaders");
        assert!(art.visible_parts().all(|p| !p.is_linked_to(&THERMAL)));
    }

    #[test]
    fn a_shelved_pour_draws_its_outline_dashed_and_nothing_filled() {
        let shelved = outline(true);
        let stroke = shelved.style.stroke.expect("stroked");
        assert!(stroke.is_dashed && shelved.style.fill.is_none());
        assert!(!outline(false).style.stroke.expect("stroked").is_dashed);
    }
}
