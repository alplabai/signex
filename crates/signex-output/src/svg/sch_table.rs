//! A schematic table (#1300) in the SVG page model (and so in the PDF and
//! the print preview): title line, filled header, cell text and borders,
//! all placed by `SchTable::layout`. See docs/RENDERING_RULES.md, "Tables".

use std::collections::BTreeSet;

use super::drawings::segments_path;
use super::geometry::rect_path;
use super::*;
use crate::pdf::layout::PageTransform;
use crate::pdf::palette::SchematicPalette;
use signex_types::schematic::{
    Point, SCHEMATIC_RENDER_MIN_STROKE_MM, SchTable, SchematicDefaults, TableBorders, pin_columns,
    table_header_fill,
};

const FONT: &str = "F3";

pub(super) fn push_table(
    out: &mut Vec<SvgElement>,
    table: &SchTable,
    xform: &PageTransform,
    palette: &SchematicPalette,
    defaults: &SchematicDefaults,
    power_nets: &BTreeSet<String>,
) {
    let layout = table.layout();
    let text_mm = table.style.text_size.max(0.1);
    let pad = table.style.cell_padding.max(0.0);
    let size_pt = super::text::em_for_glyph_height(FONT, (text_mm * xform.mm_to_unit) as f32);
    let ink = palette.note_text;
    let o = layout.origin;

    if let Some(title) = table.title.as_deref().filter(|t| !t.is_empty()) {
        out.push(SvgElement::Text {
            x: xform.x(o.x),
            y: xform.px_y(o.y - text_mm * signex_types::schematic::TABLE_TITLE_GAP_EM),
            font_alias: FONT,
            size_pt,
            align: SvgTextAlign::Left,
            v_align: SvgTextVAlign::Bottom,
            rotation_deg: 0.0,
            fill_rgb: ink,
            text: title.to_string(),
        });
    }

    if table.style.header_fill && layout.width > 0.0 {
        let p = palette.paper;
        let t = palette.net_label;
        let [r, g, b] = table_header_fill([p.0, p.1, p.2], [t.0, t.1, t.2]);
        out.push(rect_path(
            xform.x(o.x),
            xform.px_y(o.y),
            (layout.width * xform.mm_to_unit) as f32,
            (layout.row_height * xform.mm_to_unit) as f32,
            SvgStyle {
                stroke_alpha: 1.0,
                stroke_rgb: None,
                fill_rgb: Some((r, g, b)),
                stroke_width: 0.0,
            },
        ));
    }

    for (j, &column) in layout.columns.iter().enumerate() {
        let is_net = table.columns[column].key == pin_columns::NET && table.source.is_bound();
        let header = &table.columns[column].title;
        let rows = (0..layout.body_rows).map(|r| (Some(r), table.cell(r, column)));
        for (row, text) in std::iter::once((None, header.as_str())).chain(rows) {
            if text.is_empty() {
                continue;
            }
            let at = layout.cell_origin(j, row);
            let fill_rgb = if row.is_some() && is_net && power_nets.contains(text) {
                palette.power_label
            } else {
                ink
            };
            out.push(SvgElement::Text {
                x: xform.x(at.x + pad),
                y: xform.px_y(at.y + layout.row_height / 2.0),
                font_alias: FONT,
                size_pt,
                align: SvgTextAlign::Left,
                v_align: SvgTextVAlign::Center,
                rotation_deg: 0.0,
                fill_rgb,
                text: text.to_string(),
            });
        }
    }

    let outline_mm = defaults.shape_stroke_mm(0.0);
    let (lines, outline) = border_segments(table);
    if !lines.is_empty() {
        out.push(segments_path(
            &lines,
            xform,
            palette.symbol_stroke,
            SCHEMATIC_RENDER_MIN_STROKE_MM,
        ));
    }
    if !outline.is_empty() {
        out.push(segments_path(
            &outline,
            xform,
            palette.symbol_stroke,
            outline_mm,
        ));
    }
}

/// The inner rules and the outline a table's [`TableBorders`] asks for, in
/// sheet mm.
pub(crate) fn border_segments(table: &SchTable) -> (Vec<[Point; 2]>, Vec<[Point; 2]>) {
    let layout = table.layout();
    let o = layout.origin;
    let (w, h) = (layout.width, layout.height);
    if matches!(table.style.borders, TableBorders::None) || w <= 0.0 {
        return (Vec::new(), Vec::new());
    }
    let p = |x: f64, y: f64| Point::new(o.x + x, o.y + y);
    let outline = vec![
        [p(0.0, 0.0), p(w, 0.0)],
        [p(w, 0.0), p(w, h)],
        [p(w, h), p(0.0, h)],
        [p(0.0, h), p(0.0, 0.0)],
    ];
    let header_rule = [p(0.0, layout.row_height), p(w, layout.row_height)];
    let mut lines = vec![header_rule];
    if matches!(table.style.borders, TableBorders::All) {
        for r in 2..=layout.body_rows {
            let y = layout.row_height * r as f64;
            lines.push([p(0.0, y), p(w, y)]);
        }
        for x in layout.column_x.iter().skip(1) {
            lines.push([p(*x, 0.0), p(*x, h)]);
        }
    }
    (lines, outline)
}
