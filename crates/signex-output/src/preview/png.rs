//! Sheets as PNG files (#1299): the print preview's rasteriser at a chosen
//! resolution, one page at a time, each encoded as PNG.

use tiny_skia::{IntSize, Pixmap};

use super::{PreviewOptions, PreviewPage, PreviewRasterizer, preview_raster_size};
use crate::ExportContext;

/// One page as a PNG file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetPng {
    /// The sheet number the page prints.
    pub page_number: usize,
    pub width_px: u32,
    pub height_px: u32,
    pub bytes: Vec<u8>,
}

/// The PNG encoding of a rasterised page; `None` when its pixels do not
/// match its size or the encoder fails.
pub fn encode_png(page: &PreviewPage) -> Option<Vec<u8>> {
    let size = IntSize::from_wh(page.width_px, page.height_px)?;
    let pixmap = Pixmap::from_vec(page.rgba.clone(), size)?;
    pixmap.encode_png().ok()
}

/// Whether the page of `opts` at its resolution is past
/// [`super::MAX_PREVIEW_EDGE_PX`], so it is drawn at a lower one.
pub fn png_resolution_capped(opts: &PreviewOptions) -> bool {
    let (page_w_mm, page_h_mm) = opts.pdf.page_size.dimensions_mm(opts.pdf.orientation);
    preview_raster_size(page_w_mm, page_h_mm, opts.dpi).3
}

impl PreviewRasterizer {
    /// Every page the options select as PNG, in print order. Drawn and
    /// encoded one page at a time, so only one page's pixels are held. A
    /// page that cannot be drawn or encoded is left out; the caller compares
    /// the count with [`Self::outline`].
    pub fn render_png(&self, ctx: &ExportContext, opts: &PreviewOptions) -> Vec<SheetPng> {
        (0..self.outline(ctx, opts).len())
            .filter_map(|position| {
                let (_, page) = self
                    .rasterize_pages(ctx, opts, &[position])
                    .into_iter()
                    .next()?;
                Some(SheetPng {
                    page_number: page.page_number,
                    width_px: page.width_px,
                    height_px: page.height_px,
                    bytes: encode_png(&page)?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three_page_ctx() -> ExportContext {
        let mut ctx = crate::pdf::tests::sample_ctx(3);
        for (sheet, number) in ctx.sheets.iter_mut().zip(1..) {
            sheet.sheet_number = number;
            sheet.sheet_count = 3;
        }
        ctx
    }

    fn at(dpi: f64) -> PreviewOptions {
        PreviewOptions {
            dpi,
            ..Default::default()
        }
    }

    #[test]
    fn every_page_is_a_png_of_its_size_at_the_resolution() {
        let ctx = three_page_ctx();
        let pages = PreviewRasterizer.render_png(&ctx, &at(20.0));
        let numbers: Vec<usize> = pages.iter().map(|p| p.page_number).collect();
        assert_eq!(numbers, [1, 2, 3]);
        let outline = PreviewRasterizer.outline(&ctx, &at(20.0));
        for (page, info) in pages.iter().zip(&outline) {
            assert!(page.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
            let decoded = Pixmap::decode_png(&page.bytes).expect("a PNG");
            assert_eq!(
                (decoded.width(), decoded.height()),
                (info.width_px, info.height_px)
            );
            assert_eq!(
                (page.width_px, page.height_px),
                (info.width_px, info.height_px)
            );
        }
        let finer = PreviewRasterizer.render_png(&ctx, &at(40.0));
        assert!(
            finer[0].width_px > pages[0].width_px,
            "more dpi, more pixels"
        );
    }

    #[test]
    fn the_same_pages_encode_to_the_same_bytes() {
        let ctx = three_page_ctx();
        assert_eq!(
            PreviewRasterizer.render_png(&ctx, &at(20.0)),
            PreviewRasterizer.render_png(&ctx, &at(20.0))
        );
    }

    #[test]
    fn a_resolution_past_the_pixel_cap_is_reported() {
        assert!(!png_resolution_capped(&at(96.0)));
        assert!(png_resolution_capped(&at(100_000.0)));
    }

    #[test]
    fn a_page_whose_pixels_do_not_match_its_size_is_not_encoded() {
        let page = PreviewPage {
            page_number: 1,
            sheet_count: 1,
            width_px: 4,
            height_px: 4,
            rgba: vec![255; 4 * 4 * 4 - 1],
        };
        assert_eq!(encode_png(&page), None);
    }
}
