//! Print preview. See `OUTPUT_PLAN.md` §6.
//!
//! Rasterises pages via `tiny-skia` into RGBA pixel buffers for on-screen
//! display. The same scene graph rendering logic as PDF export but to pixels
//! instead of PDF operators.
//!
//! **Note on text rendering:** tiny-skia has no built-in text rasterisation.
//! We render text as hollow rectangles with estimated dimensions
//! (char_count × 0.6 × font_size_px) so users recognise text placement in
//! the preview even without glyph rendering. This is acceptable for preview
//! fidelity; the actual PDF renders glyphs correctly via font subsetting.

use crate::ExportContext;
use crate::expression::build_expression_tables;
use crate::pdf::{PageRange, PdfOptions};

mod rasterize;

pub struct PreviewRasterizer;

#[derive(Debug, Clone)]
pub struct PreviewOptions {
    pub pdf: PdfOptions,
    /// Screen resolution in DPI. Default: 96 (standard screen DPI).
    pub dpi: f64,
}

impl Default for PreviewOptions {
    fn default() -> Self {
        Self {
            pdf: PdfOptions::default(),
            dpi: 96.0,
        }
    }
}

/// A single rasterised page ready for Iced to display.
#[derive(Debug, Clone)]
pub struct PreviewPage {
    /// 1-based page number in the export.
    pub page_number: usize,
    /// Width in pixels at the preview DPI.
    pub width_px: u32,
    /// Height in pixels at the preview DPI.
    pub height_px: u32,
    /// RGBA bytes (width_px × height_px × 4 bytes per pixel).
    pub rgba: Vec<u8>,
}

impl PreviewRasterizer {
    /// Rasterise all sheets in the export context to RGBA bitmaps.
    ///
    /// Returns a vec of preview pages, one per sheet in sheet order.
    /// If a page fails to rasterise (e.g., dimensions too small), it is skipped.
    pub fn rasterize(
        &self,
        ctx: &ExportContext,
        opts: &PreviewOptions,
    ) -> Vec<PreviewPage> {
        let (page_w_mm, page_h_mm) = opts.pdf.page_size.dimensions_mm(opts.pdf.orientation);
        let expr_tables = build_expression_tables(&ctx.sheets);
        let sheet_indices = resolve_page_range_preview(&opts.pdf.page_range, ctx.sheets.len());

        sheet_indices
            .into_iter()
            .filter_map(|idx| ctx.sheets.get(idx))
            .filter_map(|sheet| {
                rasterize::rasterize_page(sheet, page_w_mm, page_h_mm, opts, ctx, &expr_tables)
            })
            .collect()
    }
}

fn resolve_page_range_preview(range: &PageRange, sheet_count: usize) -> Vec<usize> {
    match range {
        PageRange::All => (0..sheet_count).collect(),
        PageRange::Current => {
            if sheet_count > 0 {
                vec![0]
            } else {
                Vec::new()
            }
        }
        PageRange::Specific(pages) => pages
            .iter()
            .copied()
            .filter(|p| *p > 0 && *p <= sheet_count)
            .map(|p| p - 1)
            .collect(),
        PageRange::Range(start, end) => {
            if *start == 0 || *end == 0 || *start > sheet_count || *end > sheet_count {
                return Vec::new();
            }
            if start <= end {
                (start - 1..*end).collect()
            } else {
                (end - 1..*start).collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_page_structure() {
        // Test that PreviewPage can be constructed and has the right fields.
        let page = PreviewPage {
            page_number: 1,
            width_px: 100,
            height_px: 200,
            rgba: vec![255; 100 * 200 * 4], // White page
        };

        assert_eq!(page.page_number, 1);
        assert_eq!(page.width_px, 100);
        assert_eq!(page.height_px, 200);
        assert_eq!(page.rgba.len(), 100 * 200 * 4);
    }

    #[test]
    fn preview_options_default() {
        let opts = PreviewOptions::default();
        assert_eq!(opts.dpi, 96.0);
    }
}
