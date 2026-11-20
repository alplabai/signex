//! The body shapes of a placed symbol that take the default body fill (the
//! "Fill symbol bodies" preference). The canvas, the SVG and the PDF read
//! the one list, so they fill the same shapes.

use super::symbol_geometry::{polyline_is_closed, rect_corners};
use super::{FillType, Graphic, LibSymbol, Point, Symbol, SymbolTransform, item_shown};

/// A body shape to fill, in world (schematic, Y-down) mm.
#[derive(Debug, Clone, PartialEq)]
pub enum BodyFillShape {
    Polygon(Vec<Point>),
    Circle { center: Point, radius: f64 },
}

/// The body shapes of `sym` that take the default body fill: the closed
/// rectangles, closed polylines and circles of its unit and body style that
/// the symbol leaves unfilled ([`FillType::None`]). A shape with its own
/// fill keeps that fill. Open polylines, arcs, beziers, text-box frames, pins
/// and text never take it, and neither does a power port or a symbol whose
/// library entry is missing.
pub fn default_body_fill_shapes(lib: Option<&LibSymbol>, sym: &Symbol) -> Vec<BodyFillShape> {
    let Some(lib) = lib else {
        return Vec::new();
    };
    if sym.is_power {
        return Vec::new();
    }
    let t = SymbolTransform::from_symbol(sym);
    let body = lib.effective_body(sym.unit, sym.body_style);
    let world = |points: &[Point]| points.iter().map(|p| t.apply(*p)).collect::<Vec<_>>();
    lib.graphics
        .iter()
        .filter(|lg| item_shown(lg.unit, lg.body_style, sym.unit, body))
        .filter_map(|lg| match &lg.graphic {
            Graphic::Rectangle {
                start,
                end,
                fill: FillType::None,
                ..
            } => Some(BodyFillShape::Polygon(world(&rect_corners(*start, *end)))),
            Graphic::Polyline {
                points,
                fill: FillType::None,
                ..
            } if polyline_is_closed(points) => Some(BodyFillShape::Polygon(world(points))),
            Graphic::Circle {
                center,
                radius,
                fill: FillType::None,
                ..
            } => Some(BodyFillShape::Circle {
                center: t.apply(*center),
                radius: *radius,
            }),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
#[path = "body_fill_tests.rs"]
mod tests;
