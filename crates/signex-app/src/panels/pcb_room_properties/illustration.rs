//! Room Properties' line-art (#2457, BB2): the room's shape in its colour,
//! its member parts (outside ones in the error ink), the margin outline
//! while Keep parts inside is on, and the Width, Height, X, Y, Layer and
//! Margin callouts linked to their fields both ways. No handles and no
//! editing on the drawing: the fields edit.

use iced::{Element, Point, Rectangle, Size};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::pcb::RoomLayer;
use signex_types::theme::ThemeTokens;
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, IllustrationMsg, Ink, Paint, Part, Primitive, Style, TextLabel,
    TextRole, Tint, Weight, property_illustration,
};

use super::{RoomFacts, room_colour};
use crate::styles::TypeScale;

const TS: TypeScale = TypeScale::DEFAULT;
/// The view box: the mock's 280 by 160 drawing.
const VIEW_W: f32 = 280.0;
const VIEW_H: f32 = 160.0;
/// Room left round the drawing for the callouts, view units.
const PAD_X: f32 = 44.0;
const PAD_TOP: f32 = 30.0;
const PAD_BOTTOM: f32 = 30.0;
/// The feature switch the margin outline needs.
const KEEP: &str = "keep";

/// Board mm to the view box.
struct Fit {
    origin: [f64; 2],
    scale: f64,
}

impl Fit {
    fn point(&self, x: f64, y: f64) -> Point {
        Point::new(
            PAD_X + ((x - self.origin[0]) * self.scale) as f32,
            PAD_TOP + ((y - self.origin[1]) * self.scale) as f32,
        )
    }

    fn rect(&self, b: [f64; 4]) -> Rectangle {
        let lo = self.point(b[0], b[1]);
        let hi = self.point(b[2], b[3]);
        Rectangle::new(
            lo,
            Size::new((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0)),
        )
    }
}

/// The illustration of `facts`, sending hover events.
pub(super) fn view<'a>(
    facts: &RoomFacts,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, IllustrationMsg> {
    property_illustration(drawing(facts, i18n), &facts.hover, tokens, TS.caption)
}

/// The drawing as data.
pub(super) fn drawing(facts: &RoomFacts, i18n: &I18n) -> Illustration {
    let room = &facts.room;
    let mut out = Illustration::new(VIEW_W, VIEW_H).feature(KEEP, room.keep_parts_inside);
    let Some(b) = room.outline.bounds() else {
        return out;
    };
    let (w, h) = (b.w_mm.max(f64::MIN_POSITIVE), b.h_mm.max(f64::MIN_POSITIVE));
    let scale =
        (((VIEW_W - 2.0 * PAD_X) as f64) / w).min(((VIEW_H - PAD_TOP - PAD_BOTTOM) as f64) / h);
    let fit = Fit {
        origin: [b.x_mm, b.y_mm],
        scale,
    };
    let colour = Paint::Document(room_colour(&room.colour));
    let ring: Vec<Point> = room
        .outline
        .corners()
        .iter()
        .map(|p| fit.point(p.x, p.y))
        .collect();
    out = out.part(
        Part::new(
            Primitive::Polyline {
                points: ring,
                is_closed: true,
            },
            Style::dashed()
                .with_stroke_paint(colour)
                .with_fill(colour, Tint::Wash),
        )
        .linked_all(["w", "h"]),
    );
    for part in &facts.parts {
        let ink = if part.outside_nm > 0 {
            Ink::Error
        } else {
            Ink::Secondary
        };
        out = out.part(Part::new(
            Primitive::Rect {
                rect: fit.rect(part.body),
            },
            Style::outline(ink, Weight::Thin),
        ));
    }
    let m = room.margin_mm.max(0.0);
    if m > 0.0 && 2.0 * m < w && 2.0 * m < h {
        out = out.part(
            Part::new(
                Primitive::Rect {
                    rect: fit.rect([b.x_mm + m, b.y_mm + m, b.x_mm + w - m, b.y_mm + h - m]),
                },
                Style::dashed().with_stroke_paint(Paint::Ink(Ink::Accent)),
            )
            .linked("margin")
            .requires(KEEP),
        );
    }
    let (lo, hi) = (fit.point(b.x_mm, b.y_mm), fit.point(b.x_mm + w, b.y_mm + h));
    let mm = |v: f64| format!("{v:.3}");
    let width = Callout::new(Point::new(lo.x, lo.y - 10.0), Point::new(hi.x, lo.y - 10.0))
        .label(UserText::new(format!(
            "{} {}",
            t!(i18n, ids::ROOMS_CALLOUT_WIDTH),
            mm(w)
        )))
        .extension(Point::new(lo.x, lo.y - 2.0), Point::new(lo.x, lo.y - 13.0))
        .extension(Point::new(hi.x, lo.y - 2.0), Point::new(hi.x, lo.y - 13.0));
    let height = Callout::new(Point::new(hi.x + 10.0, lo.y), Point::new(hi.x + 10.0, hi.y))
        .label(UserText::new(format!(
            "{} {}",
            t!(i18n, ids::ROOMS_CALLOUT_HEIGHT),
            mm(h)
        )))
        .label_at(Point::new(hi.x + 14.0, (lo.y + hi.y) / 2.0), Anchor::Start)
        .extension(Point::new(hi.x + 2.0, lo.y), Point::new(hi.x + 13.0, lo.y))
        .extension(Point::new(hi.x + 2.0, hi.y), Point::new(hi.x + 13.0, hi.y));
    out = out
        .part(Part::callout(width).linked("w"))
        .part(Part::callout(height).linked("h"));
    let label = |at: Point, anchor: Anchor, text: String| TextLabel {
        at,
        text: UserText::new(text).into(),
        anchor,
        role: TextRole::Caption,
    };
    let x = format!("{} {}", t!(i18n, ids::ROOMS_CALLOUT_X), mm(b.x_mm));
    let y = format!("{} {}", t!(i18n, ids::ROOMS_CALLOUT_Y), mm(b.y_mm));
    let bottom = VIEW_H - 10.0;
    out = out
        .part(
            Part::label(
                label(Point::new(8.0, bottom), Anchor::Start, x),
                Ink::Secondary,
            )
            .linked("x"),
        )
        .part(
            Part::label(
                label(Point::new(86.0, bottom), Anchor::Start, y),
                Ink::Secondary,
            )
            .linked("y"),
        );
    let layer = match room.layer {
        RoomLayer::Top => t!(i18n, ids::ROOMS_LAYER_TOP),
        RoomLayer::Bottom => t!(i18n, ids::ROOMS_LAYER_BOTTOM),
        RoomLayer::Both => t!(i18n, ids::ROOMS_LAYER_TOP_BOTTOM),
    };
    out = out.part(
        Part::label(
            label(
                Point::new(VIEW_W - 8.0, 12.0),
                Anchor::End,
                layer.into_string(),
            ),
            Ink::Secondary,
        )
        .linked("layer"),
    );
    let margin = format!(
        "{} {} {}",
        t!(i18n, ids::ROOMS_CALLOUT_MARGIN),
        mm(m),
        t!(i18n, ids::ROOMS_UNIT_MM)
    );
    out.part(
        Part::label(
            label(Point::new(VIEW_W - 8.0, bottom), Anchor::End, margin),
            Ink::Secondary,
        )
        .linked("margin")
        .requires(KEEP),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb::{Room, RoomOutline};
    use signex_widgets::property_illustration::FieldId;
    use uuid::Uuid;

    fn facts(keep: bool) -> RoomFacts {
        let mut room = Room::drawn(
            Uuid::from_u128(1),
            "dcdc".into(),
            "#A058C8",
            RoomOutline::Rect {
                x_mm: 20.0,
                y_mm: 2.0,
                w_mm: 24.0,
                h_mm: 33.0,
            },
        );
        room.keep_parts_inside = keep;
        RoomFacts {
            room,
            parts: Vec::new(),
            sheets: Vec::new(),
            drafts: Default::default(),
            hover: Default::default(),
        }
    }

    /// The margin outline and callout show only while Keep parts inside is
    /// on; the size callouts always.
    #[test]
    fn the_margin_is_drawn_only_while_keep_is_on() {
        let i18n = signex_i18n::I18n::en_us_ref();
        let margin = FieldId::new("margin");
        let on = drawing(&facts(true), i18n);
        let off = drawing(&facts(false), i18n);
        assert!(on.visible_parts().any(|p| p.is_linked_to(&margin)));
        assert!(!off.visible_parts().any(|p| p.is_linked_to(&margin)));
        for drawing in [&on, &off] {
            for id in ["w", "h", "x", "y", "layer"] {
                assert!(
                    drawing
                        .visible_parts()
                        .any(|p| p.is_linked_to(&FieldId::new(id))),
                    "{id}"
                );
            }
        }
    }
}
