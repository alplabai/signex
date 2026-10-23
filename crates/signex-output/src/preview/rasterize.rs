//! `tiny-skia` CPU rasteriser for the preview pipeline via SVG context.

use super::PreviewOptions;
use super::PreviewPage;
use crate::expression::{ExpressionTables, sheet_cell_value};
use crate::svg::{SvgEvaluatorInputs, SvgRenderContext};
use crate::{ExportContext, SheetSnapshot};

/// Longest side, in pixels, a preview bitmap may have. A Custom page can be
/// metres wide; at screen DPI that would be gigabytes of RGBA allocated
/// synchronously. Past this bound the preview is drawn at a lower effective
/// DPI (the export itself is unaffected).
pub const MAX_PREVIEW_EDGE_PX: u32 = 4096;

/// Pixel size and effective pixels-per-mm for a preview of the given page,
/// scaled down so neither side exceeds [`MAX_PREVIEW_EDGE_PX`]. The `bool` is
/// `true` when the requested DPI had to be reduced.
pub fn preview_raster_size(page_w_mm: f64, page_h_mm: f64, dpi: f64) -> (u32, u32, f64, bool) {
    let mut px_per_mm = dpi / 25.4;
    let longest_px = page_w_mm.max(page_h_mm) * px_per_mm;
    let capped = longest_px > f64::from(MAX_PREVIEW_EDGE_PX);
    if capped {
        px_per_mm = f64::from(MAX_PREVIEW_EDGE_PX) / page_w_mm.max(page_h_mm);
    }
    let w = ((page_w_mm * px_per_mm) as u32).min(MAX_PREVIEW_EDGE_PX);
    let h = ((page_h_mm * px_per_mm) as u32).min(MAX_PREVIEW_EDGE_PX);
    (w, h, px_per_mm, capped)
}

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
    let (page_w_px, page_h_px, px_per_mm, _) = preview_raster_size(page_w_mm, page_h_mm, opts.dpi);

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
    let rgba =
        svg_ctx.rasterize_rgba_with_colour_mode(page_w_px, page_h_px, opts.pdf.colour_mode)?;
    Some(PreviewPage {
        page_number: sheet.sheet_number,
        width_px: page_w_px,
        height_px: page_h_px,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_5080_mm_custom_page_stays_within_the_pixel_cap() {
        let (w, h, _, capped) = preview_raster_size(5080.0, 5080.0, 144.0);
        assert!(capped);
        assert!(w <= MAX_PREVIEW_EDGE_PX && h <= MAX_PREVIEW_EDGE_PX);
        assert!(u64::from(w) * u64::from(h) * 4 <= 70_000_000);
    }

    #[test]
    fn the_cap_keeps_the_aspect_ratio() {
        let (w, h, _, capped) = preview_raster_size(5080.0, 2540.0, 200.0);
        assert!(capped);
        assert_eq!(w, MAX_PREVIEW_EDGE_PX);
        assert!((i64::from(w) - 2 * i64::from(h)).abs() <= 2);
    }

    #[test]
    fn an_ordinary_page_is_not_downscaled() {
        let (w, h, ppm, capped) = preview_raster_size(297.0, 210.0, 144.0);
        assert!(!capped);
        assert_eq!((w, h), (1683, 1190));
        assert!((ppm - 144.0 / 25.4).abs() < 1e-9);
    }
}
