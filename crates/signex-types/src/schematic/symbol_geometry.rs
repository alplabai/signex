//! World-space geometry of a placed symbol, lowered from its library graphics.
//!
//! [`lower_symbol`] is the one place a `(LibSymbol, Symbol)` pair becomes
//! world-space primitives. The canvas draws that list and
//! [`primitives_bounds`] derives the hit-test / culling box from the same
//! list, so the box always matches what is drawn.

use super::{
    Aabb, FillType, Graphic, HAlign, LibPin, LibSymbol, PIN_LENGTH_MM, Point, SCHEMATIC_TEXT_MM,
    Symbol, SymbolTransform, VAlign, circumcircle, readable_text_style, shown_on,
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
    /// Pin stub from its connection point (`tip`) to the body end.
    Pin { tip: Point, body: Point },
}

/// Library pins that belong to `sym`'s unit and body style.
pub fn instance_pins<'a>(lib: &'a LibSymbol, sym: &'a Symbol) -> impl Iterator<Item = &'a LibPin> {
    lib.pins
        .iter()
        .filter(move |lp| shown_on(lib, lp.unit, lp.body_style, sym))
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

/// Lower `sym`'s library graphics and visible pins to world space.
pub fn lower_symbol(lib: &LibSymbol, sym: &Symbol) -> Vec<SymbolPrimitive> {
    let t = SymbolTransform::from_symbol(sym);
    let mut out = Vec::with_capacity(lib.graphics.len() + lib.pins.len());
    for lg in &lib.graphics {
        if shown_on(lib, lg.unit, lg.body_style, sym) {
            lower_graphic(&lg.graphic, &t, &mut out);
        }
    }
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
        });
    }
    out
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
    primitives
        .iter()
        .filter_map(primitive_bounds)
        .reduce(|a, b| a.union(&b))
}

fn primitive_bounds(primitive: &SymbolPrimitive) -> Option<Aabb> {
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
        )),
        SymbolPrimitive::Pin { tip, body } => points_bounds(&[*tip, *body]),
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
/// with the text about the anchor (Y-down). Glyphs are estimated at 0.6 em.
pub fn text_aabb(
    text: &str,
    font_size: f64,
    anchor: (f64, f64),
    rotation_deg: f64,
    h_align: HAlign,
    v_align: VAlign,
) -> Aabb {
    let chars = text.chars().count().max(1) as f64;
    let w = text_height_mm(font_size) * 0.6 * chars;
    text_aabb_with_width(w, font_size, anchor, rotation_deg, h_align, v_align)
}

/// [`text_aabb`] for a caller-measured text width `width_mm` (a face other
/// than the monospace estimate).
pub fn text_aabb_with_width(
    width_mm: f64,
    font_size: f64,
    anchor: (f64, f64),
    rotation_deg: f64,
    h_align: HAlign,
    v_align: VAlign,
) -> Aabb {
    let h = text_height_mm(font_size);
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
