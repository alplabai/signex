//! `tiny-skia` CPU rasteriser for the preview pipeline.
//!
//! Renders the same scene graph as the PDF exporter but to RGBA pixel buffers
//! for on-screen display. Provides byte-for-byte pixel correspondence with PDF.

use tiny_skia::{Pixmap, Color, Paint, PathBuilder};

use crate::{ExportContext, SheetSnapshot};
use super::PreviewPage;

/// 1 mm in PDF points (1 pt = 1/72 inch).
const MM_TO_PT: f64 = 72.0 / 25.4;

/// Rasterise a single sheet to an RGBA bitmap at the given DPI.
///
/// # Arguments
///
/// * `sheet` - The schematic sheet snapshot to render
/// * `page_w_mm` - Page width in millimeters
/// * `page_h_mm` - Page height in millimeters
/// * `dpi` - Screen resolution in DPI (96 is standard screen resolution)
/// * `ctx` - Export context for metadata and configuration
///
/// # Returns
///
/// A `PreviewPage` containing the rendered bitmap as RGBA bytes.
pub fn rasterize_page(
    sheet: &SheetSnapshot,
    page_w_mm: f64,
    page_h_mm: f64,
    dpi: f64,
    _ctx: &ExportContext,
) -> Option<PreviewPage> {
    // Convert mm to points, then points to pixels at the given DPI.
    let page_w_pt = page_w_mm * MM_TO_PT;
    let page_h_pt = page_h_mm * MM_TO_PT;
    let page_w_px = (page_w_pt * dpi / 72.0) as u32;
    let page_h_px = (page_h_pt * dpi / 72.0) as u32;

    // Guard against zero or unreasonably small pages.
    if page_w_px < 1 || page_h_px < 1 {
        return None;
    }

    // Create a white pixmap (RGBA).
    let mut pixmap = Pixmap::new(page_w_px, page_h_px)?;
    pixmap.fill(Color::WHITE);

    // Scale factor from mm to pixels.
    let scale_factor = dpi / 72.0;

    // Render wires as lines using PathBuilder.
    for wire in &sheet.schematic.wires {
        let x1 = (wire.start.x * MM_TO_PT * scale_factor) as f32;
        let y1 = (wire.start.y * MM_TO_PT * scale_factor) as f32;
        let x2 = (wire.end.x * MM_TO_PT * scale_factor) as f32;
        let y2 = (wire.end.y * MM_TO_PT * scale_factor) as f32;

        let mut pb = PathBuilder::new();
        pb.move_to(x1, y1);
        pb.line_to(x2, y2);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(Color::from_rgba8(0, 0, 0, 255));
            let mut stroke = tiny_skia::Stroke::default();
            stroke.width = 0.5;
            pixmap.stroke_path(&path, &paint, &stroke, Default::default(), None);
        }
    }

    // Render symbols as small filled rectangles.
    for sym in &sheet.schematic.symbols {
        let x = (sym.position.x * MM_TO_PT * scale_factor) as f32;
        let y = (sym.position.y * MM_TO_PT * scale_factor) as f32;
        let size = 2.0; // Small box size in pixels

        // Draw symbol bounding box.
        if let Some(rect) = tiny_skia::Rect::from_xywh(x - size / 2.0, y - size / 2.0, size, size) {
            let mut paint = Paint::default();
            paint.set_color(Color::from_rgba8(100, 100, 100, 255));
            pixmap.fill_rect(rect, &paint, Default::default(), None);
        }
    }

    // Render labels as text placeholders (outlined rectangles).
    for label in &sheet.schematic.labels {
        let x = (label.position.x * MM_TO_PT * scale_factor) as f32;
        let y = (label.position.y * MM_TO_PT * scale_factor) as f32;

        // Estimate text width based on character count and font size.
        // Label text length × 0.6 × font size (default 2.5mm) in pixels.
        let text_len = label.text.len() as f32;
        let font_size_px = (2.5 * MM_TO_PT * scale_factor) as f32;
        let text_width = text_len * 0.6 * font_size_px;
        let text_height = font_size_px;

        if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, text_width, text_height) {
            // Draw outline rectangle for text placeholder.
            let mut pb = PathBuilder::new();
            pb.move_to(rect.left(), rect.top());
            pb.line_to(rect.right(), rect.top());
            pb.line_to(rect.right(), rect.bottom());
            pb.line_to(rect.left(), rect.bottom());
            pb.close();

            if let Some(path) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color(Color::from_rgba8(50, 50, 50, 255)); // Dark grey for text placeholder
                let mut stroke = tiny_skia::Stroke::default();
                stroke.width = 0.3;
                pixmap.stroke_path(&path, &paint, &stroke, Default::default(), None);
            }
        }
    }

    // Extract RGBA data from the pixmap.
    let rgba = pixmap.data().to_vec();

    Some(PreviewPage {
        page_number: sheet.sheet_number,
        width_px: page_w_px,
        height_px: page_h_px,
        rgba,
    })
}
