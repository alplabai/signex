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
use crate::pdf::PdfOptions;
use crate::pdf::page_range::preview_page_indices;

mod rasterize;

pub use rasterize::{MAX_PREVIEW_EDGE_PX, preview_raster_size};

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
    /// The sheet number the page prints (N in "Sheet N of M").
    pub page_number: usize,
    /// The sheet count the page prints (M in "Sheet N of M"); it may exceed
    /// the number of previewed pages.
    pub sheet_count: usize,
    /// Width in pixels at the preview DPI.
    pub width_px: u32,
    /// Height in pixels at the preview DPI.
    pub height_px: u32,
    /// RGBA bytes (width_px × height_px × 4 bytes per pixel).
    pub rgba: Vec<u8>,
}

/// What a previewed page is, known without rasterising it: enough to lay out
/// the thumbnail rail and the page caption before any pixel is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewPageInfo {
    /// The sheet number the page prints (N in "Sheet N of M").
    pub page_number: usize,
    /// The sheet count the page prints (M in "Sheet N of M").
    pub sheet_count: usize,
    /// Width in pixels the page rasterises to at the preview DPI.
    pub width_px: u32,
    /// Height in pixels the page rasterises to at the preview DPI.
    pub height_px: u32,
}

impl PreviewRasterizer {
    /// The pages the options select, in print order, without rasterising any.
    ///
    /// Empty when the range selects nothing or the page is too small to
    /// rasterise. A page's *position* in this list is what
    /// [`Self::rasterize_pages`] takes.
    pub fn outline(&self, ctx: &ExportContext, opts: &PreviewOptions) -> Vec<PreviewPageInfo> {
        let (page_w_mm, page_h_mm) = opts.pdf.page_size.dimensions_mm(opts.pdf.orientation);
        let (width_px, height_px, _, _) = preview_raster_size(page_w_mm, page_h_mm, opts.dpi);
        if width_px < 1 || height_px < 1 {
            return Vec::new();
        }
        preview_page_indices(&opts.pdf.page_range, &ctx.sheets, ctx.current_sheet_index())
            .into_iter()
            .filter_map(|idx| ctx.sheets.get(idx))
            .map(|sheet| PreviewPageInfo {
                page_number: sheet.sheet_number,
                sheet_count: sheet.sheet_count,
                width_px,
                height_px,
            })
            .collect()
    }

    /// Rasterise only the pages at `positions` in [`Self::outline`], each
    /// returned with its position. A position past the end, or a page that
    /// fails to rasterise, is skipped.
    pub fn rasterize_pages(
        &self,
        ctx: &ExportContext,
        opts: &PreviewOptions,
        positions: &[usize],
    ) -> Vec<(usize, PreviewPage)> {
        let (page_w_mm, page_h_mm) = opts.pdf.page_size.dimensions_mm(opts.pdf.orientation);
        let sheet_indices =
            preview_page_indices(&opts.pdf.page_range, &ctx.sheets, ctx.current_sheet_index());
        let wanted: Vec<(usize, &crate::SheetSnapshot)> = positions
            .iter()
            .filter_map(|&pos| Some((pos, ctx.sheets.get(*sheet_indices.get(pos)?)?)))
            .collect();
        if wanted.is_empty() {
            return Vec::new();
        }
        let expr_tables = build_expression_tables(&ctx.sheets, ctx.netlist.as_ref());
        wanted
            .into_iter()
            .filter_map(|(pos, sheet)| {
                rasterize::rasterize_page(sheet, page_w_mm, page_h_mm, opts, ctx, &expr_tables)
                    .map(|page| (pos, page))
            })
            .collect()
    }

    /// Rasterise all sheets in the export context to RGBA bitmaps.
    ///
    /// Returns a vec of preview pages, one per sheet in sheet order.
    /// If a page fails to rasterise (e.g., dimensions too small), it is skipped.
    pub fn rasterize(&self, ctx: &ExportContext, opts: &PreviewOptions) -> Vec<PreviewPage> {
        let all: Vec<usize> = (0..self.outline(ctx, opts).len()).collect();
        self.rasterize_pages(ctx, opts, &all)
            .into_iter()
            .map(|(_, page)| page)
            .collect()
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
            sheet_count: 1,
            width_px: 100,
            height_px: 200,
            rgba: vec![255; 100 * 200 * 4], // White page
        };

        assert_eq!(page.page_number, 1);
        assert_eq!(page.width_px, 100);
        assert_eq!(page.height_px, 200);
        assert_eq!(page.rgba.len(), 100 * 200 * 4);
    }

    fn five_page_ctx() -> ExportContext {
        let mut ctx = crate::pdf::tests::sample_ctx(5);
        for (sheet, number) in ctx.sheets.iter_mut().zip(1..) {
            sheet.sheet_number = number;
            sheet.sheet_count = 5;
        }
        ctx
    }

    fn small_opts() -> PreviewOptions {
        PreviewOptions {
            dpi: 10.0,
            ..Default::default()
        }
    }

    #[test]
    fn the_outline_lists_every_page_without_rasterising() {
        let outline = PreviewRasterizer.outline(&five_page_ctx(), &small_opts());
        let numbers: Vec<usize> = outline.iter().map(|p| p.page_number).collect();
        assert_eq!(numbers, [1, 2, 3, 4, 5]);
        assert!(outline.iter().all(|p| p.width_px > 0 && p.height_px > 0));
    }

    #[test]
    fn only_the_requested_positions_are_rasterised() {
        let ctx = five_page_ctx();
        let pages = PreviewRasterizer.rasterize_pages(&ctx, &small_opts(), &[3, 1]);
        let got: Vec<(usize, usize)> = pages.iter().map(|(pos, p)| (*pos, p.page_number)).collect();
        assert_eq!(got, [(3, 4), (1, 2)]);
        let outline = PreviewRasterizer.outline(&ctx, &small_opts());
        for (pos, page) in &pages {
            assert_eq!(page.width_px, outline[*pos].width_px);
            assert_eq!(
                page.rgba.len(),
                (page.width_px * page.height_px * 4) as usize
            );
        }
    }

    #[test]
    fn a_position_past_the_end_is_skipped() {
        let pages = PreviewRasterizer.rasterize_pages(&five_page_ctx(), &small_opts(), &[9]);
        assert!(pages.is_empty());
    }

    #[test]
    fn rasterize_all_matches_the_outline() {
        let ctx = five_page_ctx();
        let all = PreviewRasterizer.rasterize(&ctx, &small_opts());
        assert_eq!(
            all.len(),
            PreviewRasterizer.outline(&ctx, &small_opts()).len()
        );
    }

    #[test]
    fn preview_options_default() {
        let opts = PreviewOptions::default();
        assert_eq!(opts.dpi, 96.0);
    }

    #[test]
    fn a_page_carries_its_sheet_number_and_the_sheet_count() {
        let mut ctx = crate::pdf::tests::sample_ctx(2);
        for (sheet, number) in ctx.sheets.iter_mut().zip([1, 4]) {
            sheet.sheet_number = number;
            sheet.sheet_count = 5;
        }
        let opts = PreviewOptions {
            dpi: 10.0,
            ..Default::default()
        };
        let pages: Vec<(usize, usize)> = PreviewRasterizer
            .rasterize(&ctx, &opts)
            .iter()
            .map(|page| (page.page_number, page.sheet_count))
            .collect();
        assert_eq!(pages, [(1, 5), (4, 5)]);
    }

    #[test]
    fn current_page_previews_the_active_sheet() {
        let ctx = crate::pdf::tests::sample_ctx(4);
        assert_eq!(
            preview_page_indices(&crate::PageRange::Current, &ctx.sheets, 2),
            [2]
        );
    }
}
