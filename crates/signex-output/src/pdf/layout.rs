//! Page layout helpers: bounding box, fit-to-page scale, coordinate mapping.
//!
//! Both the PDF exporter and the preview rasteriser use the same logic so
//! that preview thumbnails match the exported PDF exactly.

use crate::SheetSnapshot;
use super::{Margins, PdfScale};

/// Bounding box of all schematic content (wires, symbols, labels) in mm.
///
/// Returns `(x_min, y_min, x_max, y_max)`. Falls back to `(0, 0, 100, 100)`
/// when the sheet is empty.
pub fn schematic_bbox(sheet: &SheetSnapshot) -> (f64, f64, f64, f64) {
    let mut x_min = f64::INFINITY;
    let mut y_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_max = f64::NEG_INFINITY;

    let mut extend = |x: f64, y: f64| {
        x_min = x_min.min(x);
        x_max = x_max.max(x);
        y_min = y_min.min(y);
        y_max = y_max.max(y);
    };

    for wire in &sheet.schematic.wires {
        extend(wire.start.x, wire.start.y);
        extend(wire.end.x, wire.end.y);
    }

    for sym in &sheet.schematic.symbols {
        // Expand by 5 mm each side to approximate the symbol body.
        extend(sym.position.x - 5.0, sym.position.y - 5.0);
        extend(sym.position.x + 5.0, sym.position.y + 5.0);
    }

    for label in &sheet.schematic.labels {
        extend(label.position.x, label.position.y);
    }

    if x_min.is_infinite() || x_max.is_infinite() {
        (0.0, 0.0, 100.0, 100.0)
    } else {
        (x_min, y_min, x_max, y_max)
    }
}

/// Scale factor that fits the content bounding box into the printable area
/// (page minus margins).
///
/// - Never upscales beyond `1.0`.
/// - Treats content width/height < 1 mm as 1 mm to avoid division by zero.
pub fn fit_to_page_scale(
    page_w_mm: f64,
    page_h_mm: f64,
    margins: &Margins,
    bbox: (f64, f64, f64, f64),
) -> f64 {
    let (x1, y1, x2, y2) = bbox;
    let printable_w = (page_w_mm - margins.left_mm - margins.right_mm).max(1.0);
    let printable_h = (page_h_mm - margins.top_mm - margins.bottom_mm).max(1.0);
    let content_w = (x2 - x1).max(1.0);
    let content_h = (y2 - y1).max(1.0);
    (printable_w / content_w)
        .min(printable_h / content_h)
        .min(1.0)
}

/// Resolved coordinate transform for a single page.
///
/// Converts schematic coordinates (mm, top-left origin, Y down) to the
/// layout coordinate space (any unit, origin and axis direction
/// determined by the caller).
#[derive(Debug, Clone, Copy)]
pub struct PageTransform {
    /// Multiplier: schematic mm → output units (pt or px).
    pub mm_to_unit: f64,
    /// X translation in output units applied after scaling.
    pub translate_x: f64,
    /// Y translation in output units applied after scaling.
    pub translate_y: f64,
}

impl PageTransform {
    /// Build a transform for the given sheet, page dimensions, and PDF scale
    /// option.
    ///
    /// `mm_per_unit` is the number of mm in one output unit:
    /// - PDF: `25.4 / 72.0` (one point = 1/72 inch = 25.4/72 mm)
    /// - Pixels: `25.4 / dpi`
    pub fn new(
        sheet: &SheetSnapshot,
        page_w_mm: f64,
        page_h_mm: f64,
        margins: &Margins,
        scale_mode: &PdfScale,
        units_per_mm: f64,
    ) -> Self {
        let bbox = schematic_bbox(sheet);
        let (bbox_x1, bbox_y1, _, _) = bbox;

        let content_scale = match scale_mode {
            PdfScale::FitToPage => fit_to_page_scale(page_w_mm, page_h_mm, margins, bbox),
            PdfScale::OneToOne => 1.0,
            PdfScale::Percent(p) => p / 100.0,
        };

        let mm_to_unit = units_per_mm * content_scale;

        // Place the top-left of the content bbox at the top-left margin corner.
        let translate_x = margins.left_mm * units_per_mm - bbox_x1 * mm_to_unit;
        let translate_y = margins.top_mm * units_per_mm - bbox_y1 * mm_to_unit;

        Self {
            mm_to_unit,
            translate_x,
            translate_y,
        }
    }

    /// Map a schematic X coordinate to output units.
    #[inline]
    pub fn x(&self, sch_x: f64) -> f32 {
        (sch_x * self.mm_to_unit + self.translate_x) as f32
    }

    /// Map a schematic Y coordinate to a **PDF Y** coordinate.
    ///
    /// PDF origin is bottom-left, Y increases upward, so we flip.
        #[allow(dead_code)]
    #[inline]
    pub fn pdf_y(&self, sch_y: f64, page_h_units: f32) -> f32 {
        page_h_units - (sch_y * self.mm_to_unit + self.translate_y) as f32
    }

    /// Map a schematic Y coordinate to a **pixel Y** coordinate.
    ///
    /// Pixels origin is top-left, Y increases downward — same as schematic,
    /// no flip needed.
    #[inline]
    pub fn px_y(&self, sch_y: f64) -> f32 {
        (sch_y * self.mm_to_unit + self.translate_y) as f32
    }
}
