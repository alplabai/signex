//! Page layout helpers: bounding box, fit-to-page scale, coordinate mapping.
//!
//! Both the PDF exporter and the preview rasteriser use the same logic so
//! that preview thumbnails match the exported PDF exactly.

use super::{Margins, Orientation, PageSize, PdfOptions, PdfScale};
use crate::SheetSnapshot;

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

/// The sheet's paper `(width_mm, height_mm)`, read from its stored paper size
/// the way the canvas reads it: named sizes are landscape unless marked
/// portrait.
pub fn sheet_paper_mm(sheet: &SheetSnapshot) -> (f64, f64) {
    let paper = sheet.schematic.paper_size.as_str();
    let size = PageSize::from_standard_str(paper);
    let orientation = match size {
        PageSize::Custom { .. } => Orientation::Landscape,
        _ => PageSize::default_orientation_for_standard(paper),
    };
    size.dimensions_mm(orientation)
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
    /// Build a transform for the given sheet, page dimensions and options.
    ///
    /// With the sheet frame on (`include_title_block`), the sheet's whole
    /// paper rectangle is laid out on the page, so content keeps its place
    /// relative to the frame and title block exactly as on the canvas
    /// (#1134). Without it, the content's bounding box is.
    ///
    /// `mm_per_unit` is the number of mm in one output unit:
    /// - PDF: `25.4 / 72.0` (one point = 1/72 inch = 25.4/72 mm)
    /// - Pixels: `25.4 / dpi`
    pub fn new(
        sheet: &SheetSnapshot,
        page_w_mm: f64,
        page_h_mm: f64,
        opts: &PdfOptions,
        units_per_mm: f64,
    ) -> Self {
        let bbox = if opts.include_title_block {
            let (w, h) = sheet_paper_mm(sheet);
            (0.0, 0.0, w, h)
        } else {
            schematic_bbox(sheet)
        };
        Self::for_box(
            bbox,
            page_w_mm,
            page_h_mm,
            &opts.margins,
            &opts.scale,
            units_per_mm,
        )
    }

    /// Place the sheet-space box `bbox` at the top-left margin corner,
    /// scaled per `scale_mode`.
    pub fn for_box(
        bbox: (f64, f64, f64, f64),
        page_w_mm: f64,
        page_h_mm: f64,
        margins: &Margins,
        scale_mode: &PdfScale,
        units_per_mm: f64,
    ) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{Point, Wire};

    fn sheet_with_wire(paper: &str) -> SheetSnapshot {
        let mut schematic = signex_types::schematic::SchematicSheet {
            paper_size: paper.into(),
            ..Default::default()
        };
        schematic.wires.push(Wire {
            start: Point::new(100.0, 50.0),
            end: Point::new(150.0, 80.0),
            stroke_width: 0.15,
            ..Default::default()
        });
        SheetSnapshot {
            path: "/p/a.snxsch".into(),
            schematic,
            sheet_name: "A".into(),
            sheet_number: 1,
            sheet_count: 1,
            placement: Default::default(),
        }
    }

    #[test]
    fn named_paper_sizes_read_like_the_canvas() {
        assert_eq!(sheet_paper_mm(&sheet_with_wire("A4")), (297.0, 210.0));
        assert_eq!(sheet_paper_mm(&sheet_with_wire("A3")), (420.0, 297.0));
        assert_eq!(sheet_paper_mm(&sheet_with_wire("Letter")), (279.4, 215.9));
        assert_eq!(sheet_paper_mm(&sheet_with_wire("")), (297.0, 210.0));
    }

    #[test]
    fn with_the_frame_on_the_sheet_paper_fills_the_printable_area() {
        let sheet = sheet_with_wire("A4");
        let opts = PdfOptions::default();
        assert!(opts.include_title_block);
        let xform = PageTransform::new(&sheet, 297.0, 210.0, &opts, 1.0);
        // The sheet's top-left corner sits at the margin corner, and the
        // sheet is scaled to fit inside the margins.
        assert!((xform.x(0.0) - 10.0).abs() < 1e-4);
        assert!((xform.px_y(0.0) - 10.0).abs() < 1e-4);
        let scale = ((297.0 - 20.0) / 297.0_f64).min((210.0 - 20.0) / 210.0);
        assert!((xform.mm_to_unit - scale).abs() < 1e-9);
    }

    #[test]
    fn with_the_frame_off_the_content_box_is_laid_out() {
        let sheet = sheet_with_wire("A4");
        let opts = PdfOptions {
            include_title_block: false,
            ..PdfOptions::default()
        };
        let xform = PageTransform::new(&sheet, 297.0, 210.0, &opts, 1.0);
        // The wire's top-left end lands on the margin corner at 1:1.
        assert!((xform.x(100.0) - 10.0).abs() < 1e-4);
        assert!((xform.px_y(50.0) - 10.0).abs() < 1e-4);
        assert!((xform.mm_to_unit - 1.0).abs() < 1e-9);
    }
}
