//! Lowers a Bezier drawing (#1267) to the scene: its flattened segments in
//! the drawing's line style, then any arrowheads. The SVG/PDF export reads
//! the same `SchBezier::flatten` and `arrow_heads`.

use super::drawings::{filled_triangle, styled_line};
use super::*;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::ordered::OrderedItem;
use signex_types::schematic::{SchBezier, SchematicDefaults};

/// The `LineSegment::style` bits of a line style.
pub(super) fn line_style_bits(style: NetLineStyle) -> u32 {
    match style {
        NetLineStyle::Solid => 0,
        NetLineStyle::Dashed => LineSegment::STYLE_DASHED,
        NetLineStyle::Dotted => LineSegment::STYLE_DOTTED,
    }
}

/// Push `b` in its own colour, else `base` (the sheet's drawing colour).
pub(super) fn push_bezier(
    b: &SchBezier,
    base: Color,
    defaults: &SchematicDefaults,
    out: &mut Vec<OrderedItem>,
) {
    let color = to_rgba(resolve_stroke_color(&b.stroke_color, base));
    let width = defaults.shape_stroke_mm(b.width) as f32;
    let style = line_style_bits(b.line_style);
    out.extend(
        b.flatten()
            .windows(2)
            .map(|w| styled_line(w[0], w[1], width, color, style)),
    );
    out.extend(b.arrow_heads().iter().map(|h| filled_triangle(h, color)));
}
