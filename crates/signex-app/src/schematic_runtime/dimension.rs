//! Lowers a dimension drawing (#1267) to the scene: extension lines, the
//! dimension line, its arrowheads and the measured text, all placed by
//! `SchDimension::geometry` (the SVG/PDF export reads the same geometry).

use super::drawings::{filled_triangle, styled_line};
use super::*;
use signex_gfx::primitive::ordered::OrderedItem;
use signex_types::schematic::{DIMENSION_TEXT_MM, SchDimension, SchematicDefaults};

/// Push `d`'s lines and heads in its own colour, else `base`.
pub(super) fn push_dimension(
    d: &SchDimension,
    base: Color,
    defaults: &SchematicDefaults,
    out: &mut Vec<OrderedItem>,
) {
    let color = to_rgba(resolve_stroke_color(&d.stroke_color, base));
    let width = defaults.shape_stroke_mm(d.width) as f32;
    let g = d.geometry();
    let mut segments = vec![g.line, g.extensions[0], g.extensions[1]];
    segments.extend(g.outside_tails.iter().copied());
    if !d.arrows.is_filled() {
        for head in &g.arrows {
            segments.push([head[1], head[0]]);
            segments.push([head[0], head[2]]);
        }
    }
    out.extend(
        segments
            .iter()
            .map(|[a, b]| styled_line(*a, *b, width, color, 0)),
    );
    if d.arrows.is_filled() {
        out.extend(g.arrows.iter().map(|h| filled_triangle(h, color)));
    }
}

/// The measured (or override) text of `d`, on the outer side of its line.
pub(super) fn label_text(d: &SchDimension, base: Color) -> TextInput {
    let g = d.geometry();
    let color = resolve_stroke_color(&d.stroke_color, base);
    TextInput {
        content: d.label(),
        position: [g.text_at.x as f32, g.text_at.y as f32],
        size_mm: DIMENSION_TEXT_MM as f32,
        color: to_rgba(color),
        bold: false,
        italic: false,
        family: resolve_family(Some("Iosevka"), None),
        rotation_rad: g.text_angle_deg.to_radians() as f32,
        h_align: HAlign::Center,
        v_align: if g.text_hangs {
            VAlign::Top
        } else {
            VAlign::Bottom
        },
    }
}
