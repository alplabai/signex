//! `tiny-skia` CPU rasteriser for the preview pipeline via SVG context.

use crate::svg::{SvgEvaluatorInputs, SvgRenderContext};
use crate::expression::{ExpressionTables, sheet_cell_value};
use crate::{ExportContext, SheetSnapshot};
use super::PreviewOptions;
use super::PreviewPage;

/// Rasterise a single sheet to an RGBA bitmap.
///
/// Uses the same `PageTransform` (FitToPage scale + margin offset) as the PDF
/// exporter so the preview thumbnail matches the exported document exactly.
pub fn rasterize_page(
    sheet: &SheetSnapshot,
    page_w_mm: f64,
    page_h_mm: f64,
    opts: &PreviewOptions,
    _ctx: &ExportContext,
    expr_tables: &ExpressionTables,
) -> Option<PreviewPage> {
    // mm → pixels: dpi / 25.4
    let px_per_mm = opts.dpi / 25.4;

    let page_w_px = (page_w_mm * px_per_mm) as u32;
    let page_h_px = (page_h_mm * px_per_mm) as u32;

    if page_w_px < 1 || page_h_px < 1 {
        return None;
    }

    let cell = sheet_cell_value(sheet);
    let eval_inputs = SvgEvaluatorInputs {
        global_refdes: &expr_tables.global_refdes,
        net_name_by_symbol_pin: &expr_tables.net_name_by_symbol_pin,
        cell: &cell,
    };

    let svg_ctx = SvgRenderContext::from_sheet(
        sheet,
        &opts.pdf,
        page_w_mm,
        page_h_mm,
        px_per_mm,
        Some(&eval_inputs),
    );
    let rgba = svg_ctx.rasterize_rgba_with_colour_mode(
        page_w_px,
        page_h_px,
        opts.pdf.colour_mode,
    )?;
    Some(PreviewPage {
        page_number: sheet.sheet_number,
        width_px: page_w_px,
        height_px: page_h_px,
        rgba,
    })
}
