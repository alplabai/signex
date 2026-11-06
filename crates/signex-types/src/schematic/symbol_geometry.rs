//! World-space geometry of a placed symbol, lowered from its library graphics.
//!
//! [`lower_symbol`] is the one place a `(LibSymbol, Symbol)` pair becomes
//! world-space primitives. The canvas draws that list and
//! [`primitives_bounds`] derives the hit-test / culling box from the same
//! list, so the box always matches what is drawn.

use super::{
    Aabb, FillType, Graphic, HAlign, LibPin, LibSymbol, PIN_LENGTH_MM, Point, SCHEMATIC_TEXT_MM,
    Symbol, SymbolTransform, VAlign, circumcircle, item_shown, readable_text_style,
};
use std::f64::consts::{FRAC_PI_2, TAU};

/// Segments a cubic bezier is sampled into.
const BEZIER_SEGMENTS: usize = 16;

/// Segments an arc is sampled into when it is filled.
const ARC_SEGMENTS: usize = 32;

/// One drawable piece of a placed symbol, in world (schematic, Y-down) mm.
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolPrimitive {
    /// Outline through `points`: rectangles, polylines, sampled beziers and
    /// text-box frames.
    Path {
        points: Vec<Point>,
        closed: bool,
        width: f64,
        fill: FillType,
    },
    Circle {
        center: Point,
        radius: f64,
        width: f64,
        fill: FillType,
    },
    /// Arc through three points. The sweep is the side that contains `mid`,
    /// which stays correct under mirroring.
    Arc {
        start: Point,
        mid: Point,
        end: Point,
        width: f64,
        fill: FillType,
    },
    Text {
        text: String,
        position: Point,
        rotation_deg: f64,
        size: f64,
        h_align: HAlign,
        v_align: VAlign,
    },
    /// Pin stub from its connection point (`tip`) to the body end, stroked at
    /// the symbol's library width ([`super::SchematicDefaults::symbol_stroke_mm`]).
    Pin { tip: Point, body: Point, width: f64 },
}

impl SymbolPrimitive {
    /// Stored stroke width (mm) of a stroked body primitive; `None` for text.
    #[must_use]
    pub fn stroke_width(&self) -> Option<f64> {
        match self {
            Self::Path { width, .. }
            | Self::Circle { width, .. }
            | Self::Arc { width, .. }
            | Self::Pin { width, .. } => Some(*width),
            Self::Text { .. } => None,
        }
    }
}

/// Library pins that belong to `sym`'s unit and body style.
pub fn instance_pins<'a>(lib: &'a LibSymbol, sym: &'a Symbol) -> impl Iterator<Item = &'a LibPin> {
    // The body is per instance, not per pin: resolve it once.
    let body = lib.effective_body(sym.unit, sym.body_style);
    lib.pins
        .iter()
        .filter(move |lp| item_shown(lp.unit, lp.body_style, sym.unit, body))
}

/// Unit direction of a pin in library space (Y-up), from its rotation.
pub fn pin_direction(rotation_deg: f64) -> (f64, f64) {
    match rotation_deg.rem_euclid(360.0).round() as i32 {
        0 | 360 => (1.0, 0.0),
        90 => (0.0, 1.0),
        180 => (-1.0, 0.0),
        270 => (0.0, -1.0),
        _ => {
            let rad = rotation_deg.to_radians();
            (rad.cos(), rad.sin())
        }
    }
}

/// Lower `sym`'s library graphics and visible pins to world space. Pins take
/// the symbol's library stroke width resolved against `defaults`.
pub fn lower_symbol(
    lib: &LibSymbol,
    sym: &Symbol,
    defaults: &super::SchematicDefaults,
) -> Vec<SymbolPrimitive> {
    let t = SymbolTransform::from_symbol(sym);
    let mut out = Vec::with_capacity(lib.graphics.len() + lib.pins.len());
    let body = lib.effective_body(sym.unit, sym.body_style);
    for lg in &lib.graphics {
        if item_shown(lg.unit, lg.body_style, sym.unit, body) {
            lower_graphic(&lg.graphic, &t, &mut out);
        }
    }
    let pin_width = defaults.symbol_stroke_mm(out.iter().filter_map(SymbolPrimitive::stroke_width));
    for lp in instance_pins(lib, sym).filter(|lp| lp.pin.visible) {
        let pin = &lp.pin;
        let (dx, dy) = pin_direction(pin.rotation);
        let len = if pin.length > 0.0 {
            pin.length
        } else {
            PIN_LENGTH_MM
        };
        let body = Point::new(pin.position.x + dx * len, pin.position.y + dy * len);
        out.push(SymbolPrimitive::Pin {
            tip: t.apply(pin.position),
            body: t.apply(body),
            width: pin_width,
        });
    }
    out
}

/// Lower a placed symbol whose library entry may be missing. A library entry
/// wins; a power port without one gets its built-in body (see
/// `power_port_lib_symbol`); any other symbol without an entry lowers to
/// nothing. A library entry that draws nothing also falls back for a power
/// port.
pub fn lower_placed_symbol(
    lib: Option<&LibSymbol>,
    sym: &Symbol,
    defaults: &super::SchematicDefaults,
) -> Vec<SymbolPrimitive> {
    if let Some(lib) = lib {
        let lowered = lower_symbol(lib, sym, defaults);
        if !lowered.is_empty() || !sym.is_power {
            return lowered;
        }
    }
    if !sym.is_power {
        return Vec::new();
    }
    lower_symbol(&super::power_port_lib_symbol(&sym.lib_id), sym, defaults)
}

fn lower_graphic(graphic: &Graphic, t: &SymbolTransform, out: &mut Vec<SymbolPrimitive>) {
    let path = |local: &[Point], closed: bool, width: f64, fill: FillType| SymbolPrimitive::Path {
        points: local.iter().map(|p| t.apply(*p)).collect(),
        closed,
        width,
        fill,
    };
    match graphic {
        Graphic::Polyline {
            points,
            width,
            fill,
        } => {
            if points.len() < 2 {
                return;
            }
            let (first, last) = (points[0], points[points.len() - 1]);
            let closed = points.len() > 2
                && (first.x - last.x).abs() < 0.001
                && (first.y - last.y).abs() < 0.001;
            out.push(path(points, closed, *width, *fill));
        }
        Graphic::Rectangle {
            start,
            end,
            width,
            fill,
        } => out.push(path(&rect_corners(*start, *end), true, *width, *fill)),
        Graphic::Circle {
            center,
            radius,
            width,
            fill,
        } => out.push(SymbolPrimitive::Circle {
            center: t.apply(*center),
            radius: *radius,
            width: *width,
            fill: *fill,
        }),
        Graphic::Arc {
            start,
            mid,
            end,
            width,
            fill,
        } => out.push(SymbolPrimitive::Arc {
            start: t.apply(*start),
            mid: t.apply(*mid),
            end: t.apply(*end),
            width: *width,
            fill: *fill,
        }),
        Graphic::Bezier {
            points,
            width,
            fill,
        } => {
            if let [p0, c1, c2, p3] = points.as_slice() {
                let samples = sample_bezier(*p0, *c1, *c2, *p3);
                out.push(path(&samples, false, *width, *fill));
            }
        }
        Graphic::Text {
            text,
            position,
            rotation,
            font_size,
            justify_h,
            justify_v,
            ..
        } => out.push(readable_text(
            t, text, *position, *rotation, *font_size, *justify_h, *justify_v,
        )),
        Graphic::TextBox {
            text,
            position,
            size,
            font_size,
            width,
            fill,
            ..
        } => {
            let far = Point::new(position.x + size.x, position.y + size.y);
            out.push(path(&rect_corners(*position, far), true, *width, *fill));
            // Library space is Y-up, so the frame's visual top-left is the
            // corner with the smallest x and the largest y.
            let top_left = Point::new(position.x.min(far.x), position.y.max(far.y));
            out.push(readable_text(
                t,
                text,
                top_left,
                0.0,
                *font_size,
                HAlign::Left,
                VAlign::Top,
            ));
        }
    }
}

/// Body text placed like a field: folded to a readable angle with its
/// justification swapped on a turn or mirror (`readable_text_style`).
fn readable_text(
    t: &SymbolTransform,
    text: &str,
    position: Point,
    rotation: f64,
    font_size: f64,
    justify_h: HAlign,
    justify_v: VAlign,
) -> SymbolPrimitive {
    let (rotation_deg, h_align, v_align) = readable_text_style(
        t.rotation_deg + rotation,
        justify_h,
        justify_v,
        t.mirror_x,
        t.mirror_y,
    );
    SymbolPrimitive::Text {
        text: text.to_owned(),
        position: t.apply(position),
        rotation_deg,
        size: text_size(font_size),
        h_align,
        v_align,
    }
}

fn text_size(font_size: f64) -> f64 {
    if font_size > 0.0 {
        font_size
    } else {
        SCHEMATIC_TEXT_MM
    }
}

fn rect_corners(a: Point, b: Point) -> [Point; 4] {
    [
        Point::new(a.x, a.y),
        Point::new(b.x, a.y),
        Point::new(b.x, b.y),
        Point::new(a.x, b.y),
    ]
}

fn sample_bezier(p0: Point, c1: Point, c2: Point, p3: Point) -> Vec<Point> {
    (0..=BEZIER_SEGMENTS)
        .map(|i| {
            let s = i as f64 / BEZIER_SEGMENTS as f64;
            let u = 1.0 - s;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * s, 3.0 * u * s * s, s * s * s);
            Point::new(
                a * p0.x + b * c1.x + c * c2.x + d * p3.x,
                a * p0.y + b * c1.y + c * c2.y + d * p3.y,
            )
        })
        .collect()
}

/// World box enclosing every primitive, strokes included; `None` when the
/// list is empty.
pub fn primitives_bounds(primitives: &[SymbolPrimitive]) -> Option<Aabb> {
    primitives_bounds_in(primitives, None)
}

/// [`primitives_bounds`] with the symbol's text measured in `family`, the
/// resolved face it draws in (the sheet default font).
pub fn primitives_bounds_in(primitives: &[SymbolPrimitive], family: Option<&str>) -> Option<Aabb> {
    primitives
        .iter()
        .filter_map(|p| primitive_bounds(p, family))
        .reduce(|a, b| a.union(&b))
}

fn primitive_bounds(primitive: &SymbolPrimitive, family: Option<&str>) -> Option<Aabb> {
    match primitive {
        SymbolPrimitive::Path { points, width, .. } => {
            points_bounds(points).map(|b| b.expand(width * 0.5))
        }
        SymbolPrimitive::Circle {
            center,
            radius,
            width,
            ..
        } => Some(points_bounds(&[*center])?.expand(radius + width * 0.5)),
        SymbolPrimitive::Arc {
            start,
            mid,
            end,
            width,
            ..
        } => Some(arc_bounds(*start, *mid, *end).expand(width * 0.5)),
        SymbolPrimitive::Text {
            text,
            position,
            rotation_deg,
            size,
            h_align,
            v_align,
        } => Some(text_aabb(
            text,
            *size,
            (position.x, position.y),
            *rotation_deg,
            *h_align,
            *v_align,
            family,
        )),
        SymbolPrimitive::Pin { tip, body, .. } => points_bounds(&[*tip, *body]),
    }
}

fn points_bounds(points: &[Point]) -> Option<Aabb> {
    points
        .iter()
        .map(|p| Aabb::new(p.x, p.y, p.x, p.y))
        .reduce(|a, b| a.union(&b))
}

/// Tight box of the arc from `start` through `mid` to `end`: its end points
/// plus every axis extreme the sweep passes.
fn arc_bounds(start: Point, mid: Point, end: Point) -> Aabb {
    let ends = points_bounds(&[start, mid, end]).expect("three points");
    let Some((cx, cy, r)) = circumcircle(start, mid, end) else {
        return ends;
    };
    let angle = |p: Point| (p.y - cy).atan2(p.x - cx);
    let (a0, am, a1) = (angle(start), angle(mid), angle(end));
    let turn = |from: f64, to: f64| (to - from).rem_euclid(TAU);
    let (from, span) = if turn(a0, am) < turn(a0, a1) {
        (a0, turn(a0, a1))
    } else {
        (a1, turn(a1, a0))
    };
    (0..4)
        .map(|k| k as f64 * FRAC_PI_2)
        .filter(|&c| turn(from, c) <= span)
        .fold(ends, |b, c| {
            let (x, y) = (cx + r * c.cos(), cy + r * c.sin());
            b.union(&Aabb::new(x, y, x, y))
        })
}

/// Points along the arc from `start` through `mid` to `end`, in that order,
/// for filling it (closed by the chord) or exporting it as a polyline.
/// Collinear points give the three points themselves.
pub fn arc_polyline(start: Point, mid: Point, end: Point) -> Vec<Point> {
    let Some((cx, cy, r)) = circumcircle(start, mid, end) else {
        return vec![start, mid, end];
    };
    let angle = |p: Point| (p.y - cy).atan2(p.x - cx);
    let (a0, am, a1) = (angle(start), angle(mid), angle(end));
    let turn = |from: f64, to: f64| (to - from).rem_euclid(TAU);
    // Signed sweep from `start` to `end` that passes `mid`.
    let sweep = if turn(a0, am) < turn(a0, a1) {
        turn(a0, a1)
    } else {
        -turn(a1, a0)
    };
    (0..=ARC_SEGMENTS)
        .map(|i| {
            let a = a0 + sweep * i as f64 / ARC_SEGMENTS as f64;
            Point::new(cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// Glyph height the text boxes use: the font size, never below the
/// schematic default.
pub fn text_height_mm(font_size: f64) -> f64 {
    font_size.max(SCHEMATIC_TEXT_MM)
}

/// Box of `text` anchored at `anchor` with the given justification, rotated
/// with the text about the anchor (Y-down). The width is the shaped advance
/// in `family`, the resolved face the text draws in (`None` is the bundled
/// default; see [`crate::font::family_width_mm`]).
pub fn text_aabb(
    text: &str,
    font_size: f64,
    anchor: (f64, f64),
    rotation_deg: f64,
    h_align: HAlign,
    v_align: VAlign,
    family: Option<&str>,
) -> Aabb {
    let w = crate::font::family_width_mm(text, family, text_height_mm(font_size));
    let lines = crate::font::line_count(text);
    text_aabb_lines(
        w,
        lines,
        font_size,
        anchor,
        rotation_deg,
        h_align,
        v_align,
        family,
    )
}

/// [`text_aabb`] of a one-line text for a caller-measured width `width_mm`
/// (a face other than the monospace estimate).
pub fn text_aabb_with_width(
    width_mm: f64,
    font_size: f64,
    anchor: (f64, f64),
    rotation_deg: f64,
    h_align: HAlign,
    v_align: VAlign,
    family: Option<&str>,
) -> Aabb {
    text_aabb_lines(
        width_mm,
        1,
        font_size,
        anchor,
        rotation_deg,
        h_align,
        v_align,
        family,
    )
}

/// [`text_aabb_with_width`] of a text drawn in `lines` lines, `width_mm` being
/// its widest. The canvas aligns the whole block, so the box is `lines` line
/// boxes tall and Top/Center/Bottom place that block, not the first line
/// (#2120).
#[allow(clippy::too_many_arguments)]
pub fn text_aabb_lines(
    width_mm: f64,
    lines: usize,
    font_size: f64,
    anchor: (f64, f64),
    rotation_deg: f64,
    h_align: HAlign,
    v_align: VAlign,
    family: Option<&str>,
) -> Aabb {
    // The drawn line box, which the text is aligned by (#2099).
    let h = crate::font::line_box_mm(text_height_mm(font_size), family) * lines.max(1) as f64;
    let w = width_mm;
    let ox = match h_align {
        HAlign::Left => 0.0,
        HAlign::Center => -w * 0.5,
        HAlign::Right => -w,
    };
    let oy = match v_align {
        VAlign::Top => 0.0,
        VAlign::Center => -h * 0.5,
        VAlign::Bottom => -h,
    };
    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    let corners = [(ox, oy), (ox + w, oy), (ox, oy + h), (ox + w, oy + h)].map(|(lx, ly)| {
        Point::new(
            anchor.0 + lx * cos - ly * sin,
            anchor.1 + lx * sin + ly * cos,
        )
    });
    points_bounds(&corners).expect("four corners")
}

#[cfg(test)]
#[path = "symbol_geometry_tests.rs"]
mod tests;
