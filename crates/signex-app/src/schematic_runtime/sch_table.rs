//! Lowers a table drawing (#1300) to the scene: the filled header row, the
//! borders, then (through the text bucket) the title and cell texts. Every
//! cell sits where `SchTable::layout` puts it, which the hit test and the
//! SVG/PDF export read too. See docs/RENDERING_RULES.md, "Tables".

use std::collections::BTreeSet;

use super::drawings::styled_line;
use super::*;
use signex_gfx::primitive::ordered::OrderedItem;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_types::schematic::{
    SCHEMATIC_RENDER_MIN_STROKE_MM, SchTable, SchematicDefaults, TABLE_TITLE_GAP_EM, TableBorders,
    pin_columns, table_header_fill,
};

/// The family a table's cells draw in: monospace, so the fixed advance of
/// `SchTable::layout` fits the text.
const TABLE_FAMILY: &str = "Iosevka";

/// Push the header fill and the borders of `t`; `base` is the outline colour.
pub(super) fn push_table(
    t: &SchTable,
    base: Color,
    colors: &CanvasColors,
    defaults: &SchematicDefaults,
    out: &mut Vec<OrderedItem>,
) {
    let layout = t.layout();
    let o = layout.origin;
    if t.style.header_fill && layout.width > 0.0 {
        let paper = to_rgba(to_iced(&colors.paper));
        let tint = to_rgba(to_iced(&colors.net_label));
        let [r, g, b] =
            table_header_fill([paper[0], paper[1], paper[2]], [tint[0], tint[1], tint[2]]);
        let (x0, y0) = (o.x as f32, o.y as f32);
        let (x1, y1) = (
            (o.x + layout.width) as f32,
            (o.y + layout.row_height) as f32,
        );
        out.push(OrderedItem::Polygon(GpuPolygon {
            holes: Vec::new(),
            vertices: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
            fill_color: [r, g, b, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        }));
    }
    if matches!(t.style.borders, TableBorders::None) || layout.width <= 0.0 {
        return;
    }
    let color = to_rgba(base);
    let outline = defaults.shape_stroke_mm(0.0) as f32;
    let inner = SCHEMATIC_RENDER_MIN_STROKE_MM as f32;
    let p = |x: f64, y: f64| Point::new(o.x + x, o.y + y);
    let (w, h) = (layout.width, layout.height);
    out.push(styled_line(
        p(0.0, layout.row_height),
        p(w, layout.row_height),
        inner,
        color,
        0,
    ));
    if matches!(t.style.borders, TableBorders::All) {
        for r in 2..=layout.body_rows {
            let y = layout.row_height * r as f64;
            out.push(styled_line(p(0.0, y), p(w, y), inner, color, 0));
        }
        for x in layout.column_x.iter().skip(1) {
            out.push(styled_line(p(*x, 0.0), p(*x, h), inner, color, 0));
        }
    }
    for [a, b] in [
        [p(0.0, 0.0), p(w, 0.0)],
        [p(w, 0.0), p(w, h)],
        [p(w, h), p(0.0, h)],
        [p(0.0, h), p(0.0, 0.0)],
    ] {
        out.push(styled_line(a, b, outline, color, 0));
    }
}

fn cell_text(content: &str, at: Point, size_mm: f64, color: Color) -> TextInput {
    TextInput {
        content: content.to_string(),
        position: [at.x as f32, at.y as f32],
        size_mm: size_mm as f32,
        color: to_rgba(color),
        bold: false,
        italic: false,
        family: resolve_family(Some(TABLE_FAMILY), None),
        rotation_rad: 0.0,
        h_align: HAlign::Left,
        v_align: VAlign::Center,
    }
}

/// Push the title and every drawn cell of `t`. A bound pin table's net
/// cells that name a power net draw in `power`.
pub(super) fn push_cell_texts(
    t: &SchTable,
    ink: Color,
    power: Color,
    power_nets: &BTreeSet<String>,
    out: &mut Vec<TextInput>,
) {
    let layout = t.layout();
    let size = t.style.text_size.max(0.1);
    let pad = t.style.cell_padding.max(0.0);
    if let Some(title) = t.title.as_deref().filter(|s| !s.is_empty()) {
        let at = Point::new(layout.origin.x, layout.origin.y - size * TABLE_TITLE_GAP_EM);
        out.push(TextInput {
            v_align: VAlign::Bottom,
            ..cell_text(title, at, size, ink)
        });
    }
    for (j, &column) in layout.columns.iter().enumerate() {
        let is_net = t.columns[column].key == pin_columns::NET && t.source.is_bound();
        let header = t.columns[column].title.as_str();
        let rows = (0..layout.body_rows).map(|r| (Some(r), t.cell(r, column)));
        for (row, text) in std::iter::once((None, header)).chain(rows) {
            if text.is_empty() {
                continue;
            }
            let corner = layout.cell_origin(j, row);
            let at = Point::new(corner.x + pad, corner.y + layout.row_height / 2.0);
            let color = if row.is_some() && is_net && power_nets.contains(text) {
                power
            } else {
                ink
            };
            out.push(cell_text(text, at, size, color));
        }
    }
}
