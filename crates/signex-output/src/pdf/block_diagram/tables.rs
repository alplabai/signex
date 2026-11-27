//! A page of rows (mockup row 171, "Export PDF…": the Power tree and Budget
//! table pages): a heading, a header row and one line per row, cut into as
//! many pages as the rows need. The caller words and fills the cells; this
//! only lays them out.

use super::super::MM_TO_PT;
use super::super::board::stroke;
use super::super::font::{PdfFont, best_alias_for_text, normalize_pdf_text, text_advance_pt};
use super::super::surface::PdfSurface;

/// One table page as the caller words it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BlockDiagramTable {
    pub heading: String,
    pub columns: Vec<String>,
    pub rows: Vec<BlockDiagramTableRow>,
}

/// One row: its cells in column order, how far it is indented under the
/// row before, and whether it heads a group (printed bold).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BlockDiagramTableRow {
    pub cells: Vec<String>,
    pub indent: u8,
    pub strong: bool,
}

const MARGIN_MM: f64 = 15.0;
const HEADING_PT: f32 = 14.0;
const TEXT_PT: f32 = 9.0;
const LINE_PT: f32 = 14.0;
/// Indent per level, PDF points.
const INDENT_PT: f32 = 12.0;
/// The space between two columns, PDF points.
const GAP_PT: f32 = 8.0;

/// The rows that fit one page of `page_mm` under its heading and header.
pub(super) fn rows_per_page(page_mm: (f64, f64)) -> usize {
    let usable = (page_mm.1 - 2.0 * MARGIN_MM) * MM_TO_PT - f64::from(HEADING_PT + 2.0 * LINE_PT);
    (usable / f64::from(LINE_PT)).floor().max(1.0) as usize
}

/// Lay `rows` (one page's share of `table`) out on `s`.
pub(super) fn draw_table_page(
    s: &mut PdfSurface<'_>,
    table: &BlockDiagramTable,
    rows: &[BlockDiagramTableRow],
    page_mm: (f64, f64),
    monochrome: bool,
) {
    let margin = (MARGIN_MM * MM_TO_PT) as f32;
    let (page_w, page_h) = ((page_mm.0 * MM_TO_PT) as f32, (page_mm.1 * MM_TO_PT) as f32);
    let regular = PdfFont::RobotoRegular.alias();
    let bold = PdfFont::RobotoBold.alias();
    let ink = if monochrome { 0.0 } else { 0.15 };
    s.set_fill_color(ink, ink, ink);
    let mut y = page_h - margin - HEADING_PT;
    let heading = normalize_pdf_text(&table.heading);
    s.text_at(
        margin,
        y,
        best_alias_for_text(bold, &heading),
        HEADING_PT,
        &heading,
    );
    y -= LINE_PT * 1.5;
    let widths = column_widths(table, page_w - 2.0 * margin);
    write_cells(s, margin, y, &widths, &table.columns, 0, bold);
    let rule = y - LINE_PT * 0.35;
    stroke(
        s,
        &[(margin, rule), (page_w - margin, rule)],
        false,
        0.5,
        (ink, ink, ink),
    );
    for row in rows {
        y -= LINE_PT;
        let font = if row.strong { bold } else { regular };
        write_cells(s, margin, y, &widths, &row.cells, row.indent, font);
    }
}

/// Each column's share of `width`: the widest text in it, scaled to fit.
fn column_widths(table: &BlockDiagramTable, width: f32) -> Vec<f32> {
    let count = table.columns.len().max(1);
    let mut widest = vec![0.0f32; count];
    let rows = std::iter::once((&table.columns, 0u8))
        .chain(table.rows.iter().map(|row| (&row.cells, row.indent)));
    for (cells, indent) in rows {
        for (at, cell) in cells.iter().enumerate().take(count) {
            let lead = if at == 0 {
                f32::from(indent) * INDENT_PT
            } else {
                0.0
            };
            let text = normalize_pdf_text(cell);
            let w = lead + text_advance_pt(PdfFont::RobotoBold.alias(), &text, TEXT_PT) + GAP_PT;
            widest[at] = widest[at].max(w);
        }
    }
    let total: f32 = widest.iter().sum::<f32>().max(1.0);
    let fit = (width / total).min(1.0);
    widest.into_iter().map(|w| w * fit).collect()
}

fn write_cells(
    s: &mut PdfSurface<'_>,
    left: f32,
    y: f32,
    widths: &[f32],
    cells: &[String],
    indent: u8,
    font: &str,
) {
    let mut x = left;
    for (at, (cell, width)) in cells.iter().zip(widths).enumerate() {
        let lead = if at == 0 {
            f32::from(indent) * INDENT_PT
        } else {
            0.0
        };
        let text = normalize_pdf_text(cell);
        if !text.trim().is_empty() {
            s.text_at(
                x + lead,
                y,
                best_alias_for_text(font, &text),
                TEXT_PT,
                &text,
            );
        }
        x += width;
    }
}
