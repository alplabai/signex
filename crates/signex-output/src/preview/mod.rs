//! Print preview. See `OUTPUT_PLAN.md` §6.
//!
//! Reuses the PDF pipeline's `PdfSurface`; instead of emitting a PDF file,
//! rasterises each page via `tiny-skia` into an Iced image for on-screen
//! display. Byte-for-byte matches what PDF export produces.

use crate::ExportContext;
use crate::pdf::PdfOptions;

mod rasterize;

pub struct PreviewRasterizer;

#[derive(Debug, Clone, Default)]
pub struct PreviewOptions {
    pub pdf: PdfOptions,
    pub dpi: f64,
}

/// A single rasterised page ready for Iced to display. Actual bitmap layout
/// is filled in when the rasteriser lands.
#[derive(Debug, Clone)]
pub struct PreviewPage {
    pub page_number: usize,
    pub width_px: u32,
    pub height_px: u32,
    pub rgba: Vec<u8>,
}

impl PreviewRasterizer {
    pub fn rasterize(
        &self,
        _ctx: &ExportContext,
        _opts: &PreviewOptions,
    ) -> Vec<PreviewPage> {
        todo!("preview rasteriser — implemented alongside PdfSurface in a follow-up PR")
    }
}
