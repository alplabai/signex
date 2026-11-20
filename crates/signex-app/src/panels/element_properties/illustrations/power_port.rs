//! A power port: the picked style's glyph and the name in the power colour on
//! a wire stub, the hotspot marked, and the Name, Font Size, Style and
//! Position callouts. The glyph is the built-in body the canvas draws
//! (`power_port_graphics`), so the drawing is the shape on the sheet; the
//! Style tiles draw the same glyph alone.

use iced::{Color, Point, Rectangle};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::schematic::{Graphic, PowerPortStyle, power_port_graphics};
use signex_widgets::property_illustration::{
    Anchor, Illustration, Ink, Leader, Paint, Part, Primitive, Style, Text, TextLabel, TextRole,
    Tint, Weight,
};

use super::{HEIGHT_EM, fields};

const VIEW_W: f32 = 264.0;
const VIEW_H: f32 = 120.0;
/// View units per mm of the glyph in the main drawing.
const SCALE: f32 = 12.0;
/// The x of the hotspot and the wire stub.
const HOT_X: f32 = 132.0;
/// Rough width of one character of the name, view units.
const CHAR_W: f32 = 7.0;
/// Side of a Style tile's view box, and its glyph scale.
const TILE: f32 = 40.0;
const TILE_SCALE: f32 = 8.0;

/// The glyph of `lib_id` at `origin` (its connection point), `scale` view
/// units per mm, in `style`; with the glyph's top and bottom y.
fn glyph(lib_id: &str, origin: Point, scale: f32, style: Style) -> (Vec<Part>, f32, f32) {
    // Library space is y up; the drawing is y down.
    let at = |p: &signex_types::schematic::Point| {
        Point::new(origin.x + p.x as f32 * scale, origin.y - p.y as f32 * scale)
    };
    let mut parts = Vec::new();
    let (mut top, mut bottom) = (origin.y, origin.y);
    let mut reach = |y: f32| {
        top = top.min(y);
        bottom = bottom.max(y);
    };
    for graphic in power_port_graphics(lib_id) {
        let primitive = match &graphic {
            Graphic::Polyline { points, .. } => {
                let points: Vec<Point> = points.iter().map(at).collect();
                points.iter().for_each(|p| reach(p.y));
                Primitive::Polyline {
                    points,
                    is_closed: false,
                }
            }
            Graphic::Circle { center, radius, .. } => {
                let center = at(center);
                let radius = *radius as f32 * scale;
                reach(center.y - radius);
                reach(center.y + radius);
                Primitive::Circle { center, radius }
            }
            Graphic::Rectangle { start, end, .. } => {
                let (a, b) = (at(start), at(end));
                reach(a.y);
                reach(b.y);
                Primitive::Rect {
                    rect: Rectangle::new(
                        Point::new(a.x.min(b.x), a.y.min(b.y)),
                        iced::Size::new((a.x - b.x).abs(), (a.y - b.y).abs()),
                    ),
                }
            }
            Graphic::Arc {
                start, mid, end, ..
            } => {
                let points: Vec<Point> = [start, mid, end].into_iter().map(at).collect();
                points.iter().for_each(|p| reach(p.y));
                Primitive::Polyline {
                    points,
                    is_closed: false,
                }
            }
            // A built-in power body carries no text of its own.
            _ => continue,
        };
        parts.push(Part::new(primitive, style).linked(fields::STYLE));
    }
    (parts, top, bottom)
}

/// The glyph of `lib_id` alone, centred on a Style tile, in `ink`.
pub fn style_glyph(lib_id: &str, ink: Ink) -> Illustration {
    let style = Style::outline(ink, Weight::Heavy);
    let (_, top, bottom) = glyph(lib_id, Point::ORIGIN, TILE_SCALE, style);
    // Shift the connection point so the glyph's extent is centred.
    let origin = Point::new(TILE / 2.0, TILE / 2.0 - (top + bottom) / 2.0);
    let (parts, _, _) = glyph(lib_id, origin, TILE_SCALE, style);
    Illustration::new(TILE, TILE).height_em(2.4).parts(parts)
}

/// The power port of `lib_id` named `name`, drawn in `power` (the theme's
/// power-port colour).
pub fn power_port(lib_id: &str, name: &str, power: Color, i18n: &I18n) -> Illustration {
    let hangs = PowerPortStyle::of(lib_id).hangs_down();
    let hot = Point::new(HOT_X, if hangs { 36.0 } else { 76.0 });
    let red = Paint::Document(power);
    let ink = Style::outline(Ink::Secondary, Weight::Heavy).with_stroke_paint(red);
    let (glyph_parts, top, bottom) = glyph(lib_id, hot, SCALE, ink);
    // The stub runs away from the head, as the wire the port sits on.
    let stub_end = if hangs { 4.0 } else { VIEW_H - 4.0 };
    let stub = Part::new(
        Primitive::Line {
            from: hot,
            to: Point::new(HOT_X, stub_end),
        },
        Style::outline(Ink::Secondary, Weight::Regular),
    );
    let name_y = if hangs { bottom + 12.0 } else { top - 12.0 };
    let half = CHAR_W * name.chars().count().max(1) as f32 / 2.0;
    let name_label = Part::new(
        Primitive::Label(TextLabel {
            at: Point::new(HOT_X, name_y),
            text: UserText::new(name.to_string()).into(),
            anchor: Anchor::Middle,
            role: TextRole::Mono,
        }),
        Style::outline(Ink::Secondary, Weight::Thin).with_stroke_paint(red),
    )
    .linked_all([fields::NAME, fields::FONT_SIZE]);
    let hotspot = Part::new(
        Primitive::Circle {
            center: hot,
            radius: 2.5,
        },
        Style::filled(Paint::Ink(Ink::Secondary), Tint::Solid),
    )
    .linked(fields::POSITION);
    let leader = |at: Point, label_at: Point, text: Text, anchor: Anchor| {
        Part::leader(Leader {
            at,
            label_at,
            label: text,
            anchor,
        })
    };
    let head_mid = (top + bottom) / 2.0;
    Illustration::new(VIEW_W, VIEW_H)
        .height_em(HEIGHT_EM)
        .part(stub)
        .parts(glyph_parts)
        .part(name_label)
        .part(hotspot)
        .part(
            leader(
                Point::new(HOT_X - half - 2.0, name_y),
                Point::new(56.0, name_y),
                t!(i18n, ids::PROPERTIES_NAME).into(),
                Anchor::End,
            )
            .linked(fields::NAME),
        )
        .part(
            leader(
                Point::new(HOT_X + half + 2.0, name_y),
                Point::new(208.0, name_y),
                t!(i18n, ids::PROPERTIES_FONT_SIZE).into(),
                Anchor::Start,
            )
            .linked(fields::FONT_SIZE),
        )
        .part(
            leader(
                Point::new(HOT_X + 14.0, head_mid),
                Point::new(208.0, head_mid),
                t!(i18n, ids::PROPERTIES_STYLE).into(),
                Anchor::Start,
            )
            .linked(fields::STYLE),
        )
        .part(
            leader(
                hot,
                Point::new(56.0, hot.y),
                t!(i18n, ids::PROPERTIES_POSITION).into(),
                Anchor::End,
            )
            .linked(fields::POSITION),
        )
}
