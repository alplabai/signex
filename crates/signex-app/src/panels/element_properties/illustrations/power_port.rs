//! A power port: the picked style's glyph and the name in the power colour on
//! a wire stub, the hotspot marked, and the Font Size, Style and Position
//! callouts carrying their values (mockup row 45). The glyph is the built-in
//! body the canvas draws (`power_port_graphics`), so the drawing is the shape
//! on the sheet; the Style tiles draw the same glyph alone.

use iced::{Color, Point, Rectangle};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::schematic::{Graphic, PowerPortStyle, WireWidthPreset, power_port_graphics};
use signex_widgets::property_illustration::{
    Anchor, Callout, Cap, Illustration, Ink, Leader, Paint, Part, Primitive, Style, Text,
    TextLabel, TextRole, Tint, Weight,
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
/// Half the name's height, in character widths: the Font Size span.
const NAME_HALF_H_CHARS: f32 = 0.6;
/// Gap between the name's end and the Font Size dimension line.
const NAME_GAP: f32 = 10.0;
/// How far left of the stem the Style leader starts.
const STYLE_REACH: f32 = 14.0;
/// Where the left (Style) and right (Position) callout labels sit.
const LEFT_LABEL_X: f32 = 56.0;
const RIGHT_LABEL_X: f32 = 172.0;
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

/// The callout texts of a power port, each carrying the value its row
/// holds (mockup row 45): "Style · Bar", "Font Size 10", "Position x, y".
pub struct PowerPortCallouts {
    pub style: Text,
    pub font_size: Text,
    pub position: Text,
}

impl PowerPortCallouts {
    /// `style` is the style's name, `font_size` the Font Size row's points,
    /// `position` the Location X and Y as the rows show them (`None` gives
    /// the bare "Position").
    pub fn new(
        i18n: &I18n,
        style: &str,
        font_size: u32,
        position: Option<(String, String)>,
    ) -> Self {
        let position = match position {
            Some((x, y)) => {
                t!(i18n, ids::PROPERTIES_PART_ILLUSTRATION_POSITION, "x" => x, "y" => y)
            }
            None => t!(i18n, ids::PROPERTIES_POSITION),
        };
        Self {
            style: t!(i18n, ids::PROPERTIES_POWER_ILLUSTRATION_STYLE, "style" => style).into(),
            font_size: t!(i18n, ids::PROPERTIES_POWER_ILLUSTRATION_FONT_SIZE, "size" => font_size)
                .into(),
            position: position.into(),
        }
    }
}

/// The wire the port sits on: a band at the default wire width, running
/// from the hotspot away from the head to the drawing's edge.
fn wire_stub(hot: Point, hangs: bool) -> Part {
    let end = Point::new(HOT_X, if hangs { 4.0 } else { VIEW_H - 4.0 });
    Part::new(
        Primitive::Band {
            points: vec![hot, end],
            width: super::wire::band_width(WireWidthPreset::Small),
            cap: Cap::Flat,
        },
        Style::edge().with_fill(Paint::Ink(Ink::Secondary), Tint::Body),
    )
}

/// The Font Size dimension: a span over the name's height just past its
/// right end, labelled with the size.
fn font_size_callout(name_y: f32, name_right: f32, label: Text) -> Part {
    let half = CHAR_W * NAME_HALF_H_CHARS;
    let x = name_right + NAME_GAP;
    let callout = Callout::new(Point::new(x, name_y - half), Point::new(x, name_y + half))
        .label(label)
        .label_at(Point::new(x + NAME_GAP / 2.0, name_y), Anchor::Start)
        .extension(
            Point::new(name_right, name_y - half),
            Point::new(x, name_y - half),
        )
        .extension(
            Point::new(name_right, name_y + half),
            Point::new(x, name_y + half),
        );
    Part::callout(callout).linked(fields::FONT_SIZE)
}

/// The power port of `lib_id` named `name`, drawn in `power` (the theme's
/// power-port colour) on its wire stub, with `callouts` carrying the row
/// values.
pub fn power_port(
    lib_id: &str,
    name: &str,
    power: Color,
    callouts: PowerPortCallouts,
) -> Illustration {
    let hangs = PowerPortStyle::of(lib_id).hangs_down();
    let hot = Point::new(HOT_X, if hangs { 36.0 } else { 76.0 });
    let red = Paint::Document(power);
    let ink = Style::outline(Ink::Secondary, Weight::Heavy).with_stroke_paint(red);
    let (glyph_parts, top, bottom) = glyph(lib_id, hot, SCALE, ink);
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
    let head_mid = (top + bottom) / 2.0;
    let style_leader = Part::leader(Leader {
        at: Point::new(HOT_X - STYLE_REACH, head_mid),
        label_at: Point::new(LEFT_LABEL_X, head_mid),
        label: callouts.style,
        anchor: Anchor::End,
    })
    .linked(fields::STYLE);
    let position_leader = Part::leader(Leader {
        at: hot,
        label_at: Point::new(RIGHT_LABEL_X, hot.y),
        label: callouts.position,
        anchor: Anchor::Start,
    })
    .linked(fields::POSITION);
    Illustration::new(VIEW_W, VIEW_H)
        .height_em(HEIGHT_EM)
        .part(wire_stub(hot, hangs))
        .parts(glyph_parts)
        .part(name_label)
        .part(hotspot)
        .part(font_size_callout(name_y, HOT_X + half, callouts.font_size))
        .part(style_leader)
        .part(position_leader)
}
