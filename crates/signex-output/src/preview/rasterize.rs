//! `tiny-skia` CPU rasteriser for the preview pipeline.
//!
//! Renders the same scene graph as the PDF exporter to RGBA pixel buffers
//! using the same `PageTransform` so thumbnails match the exported PDF exactly.

use tiny_skia::{Color, Paint, PathBuilder, Pixmap};

use crate::pdf::layout::PageTransform;
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
) -> Option<PreviewPage> {
    // mm → pixels: dpi / 25.4
    let px_per_mm = opts.dpi / 25.4;

    let page_w_px = (page_w_mm * px_per_mm) as u32;
    let page_h_px = (page_h_mm * px_per_mm) as u32;

    if page_w_px < 1 || page_h_px < 1 {
        return None;
    }

    let mut pixmap = Pixmap::new(page_w_px, page_h_px)?;
    pixmap.fill(Color::WHITE);

    // Build the same coordinate transform as the PDF exporter.
    // `units_per_mm` for pixels = dpi / 25.4.
    let xform = PageTransform::new(
        sheet,
        page_w_mm,
        page_h_mm,
        &opts.pdf.margins,
        &opts.pdf.scale,
        px_per_mm,
    );

    // Draw wires.
    for wire in &sheet.schematic.wires {
        let x1 = xform.x(wire.start.x);
        let y1 = xform.px_y(wire.start.y);
        let x2 = xform.x(wire.end.x);
        let y2 = xform.px_y(wire.end.y);

        let mut pb = PathBuilder::new();
        pb.move_to(x1, y1);
        pb.line_to(x2, y2);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(Color::BLACK);
            let mut stroke = tiny_skia::Stroke::default();
            // Wire width: 0.15mm default, scale up for visibility. At 96 DPI, 0.15mm ≈ 0.45px (barely visible); use 1.2px minimum.
            stroke.width = ((0.15 * xform.mm_to_unit as f64).max(1.2)) as f32;
            pixmap.stroke_path(&path, &paint, &stroke, Default::default(), None);
        }
    }

    // Draw symbols as bounding-box rectangles with darker stroke for visibility.
    for sym in &sheet.schematic.symbols {
        let x = xform.x(sym.position.x - 5.0);
        let y = xform.px_y(sym.position.y - 5.0);
        let side = (10.0 * xform.mm_to_unit) as f32;

        let mut pb = PathBuilder::new();
        pb.move_to(x, y);
        pb.line_to(x + side, y);
        pb.line_to(x + side, y + side);
        pb.line_to(x, y + side);
        pb.close();
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(Color::from_rgba8(60, 60, 60, 255));
            let mut stroke = tiny_skia::Stroke::default();
            stroke.width = (0.2 * xform.mm_to_unit).max(0.8) as f32;
            pixmap.stroke_path(&path, &paint, &stroke, Default::default(), None);
        }
    }

    // Draw labels as outlined text-placeholder rectangles with tighter bounds.
    for label in &sheet.schematic.labels {
        let x = xform.x(label.position.x);
        let y = xform.px_y(label.position.y);
        let font_size_px = (2.5 * xform.mm_to_unit) as f32;
        // Tighter estimate: monospace character width ≈ 0.5 × height.
        let text_w = label.text.len() as f32 * 0.5 * font_size_px;
        let text_h = font_size_px;

        let mut pb = PathBuilder::new();
        pb.move_to(x, y);
        pb.line_to(x + text_w, y);
        pb.line_to(x + text_w, y + text_h);
        pb.line_to(x, y + text_h);
        pb.close();
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            // Brown/tan for text labels to distinguish from wires/symbols.
            paint.set_color(Color::from_rgba8(120, 80, 40, 255));
            let mut stroke = tiny_skia::Stroke::default();
            stroke.width = 0.6;
            pixmap.stroke_path(&path, &paint, &stroke, Default::default(), None);
        }
    }

    let rgba = pixmap.data().to_vec();
    Some(PreviewPage {
        page_number: sheet.sheet_number,
        width_px: page_w_px,
        height_px: page_h_px,
        rgba,
    })
}
