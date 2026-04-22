//! PDF export via `pdf-writer`.
//!
//! See `OUTPUT_PLAN.md` §3. `PdfSurface` (in `surface.rs`) acts as a second
//! render target for the schematic — wires, symbols, labels, title block.
//! Screen (Iced Canvas) and PDF share one source of truth for page layout.

use pdf_writer::{Finish, Pdf, Rect, Ref};
use thiserror::Error;

use crate::template::TemplateId;
use crate::{ExportContext, Exporter, SubstitutionContext};
use signex_types::schematic::Point;

mod font;
mod layout;
mod page;
mod surface;

use font::{FontCatalog, PdfFont};
use surface::PdfSurface;

/// 1 mm in PDF points (1 pt = 1/72 inch).
const MM_TO_PT: f64 = 72.0 / 25.4;

pub struct PdfExporter;

#[derive(Debug, Clone)]
pub struct PdfOptions {
    pub page_size: PageSize,
    pub orientation: Orientation,
    pub colour_mode: ColourMode,
    pub page_range: PageRange,
    pub sheet_template: Option<TemplateId>,
    pub margins: Margins,
    pub scale: PdfScale,
    pub include_title_block: bool,
}

#[derive(Debug, Clone)]
pub struct PdfOutput {
    pub bytes: Vec<u8>,
    pub page_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageSize {
    IsoA0,
    IsoA1,
    IsoA2,
    IsoA3,
    IsoA4,
    IsoA5,
    AnsiA,
    AnsiB,
    AnsiC,
    AnsiD,
    AnsiE,
    UsLetter,
    UsLegal,
    Custom { width_mm: f64, height_mm: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColourMode {
    Colour,
    Grayscale,
    BlackAndWhite,
}

#[derive(Debug, Clone)]
pub enum PageRange {
    All,
    Current,
    Specific(Vec<usize>),
    Range(usize, usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub top_mm: f64,
    pub right_mm: f64,
    pub bottom_mm: f64,
    pub left_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PdfScale {
    FitToPage,
    OneToOne,
    Percent(f64),
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            page_size: PageSize::IsoA4,
            orientation: Orientation::Landscape,
            colour_mode: ColourMode::Colour,
            page_range: PageRange::All,
            sheet_template: None,
            margins: Margins {
                top_mm: 10.0,
                right_mm: 10.0,
                bottom_mm: 10.0,
                left_mm: 10.0,
            },
            scale: PdfScale::FitToPage,
            include_title_block: true,
        }
    }
}

#[derive(Debug, Error)]
pub enum PdfError {
    #[error("no sheets in export context")]
    NoSheets,

    #[error("page range references sheet {0} but project only has {1} sheets")]
    OutOfRange(usize, usize),

    #[error("font subsetting failed: {0}")]
    Font(String),
}

impl Exporter for PdfExporter {
    type Options = PdfOptions;
    type Output = PdfOutput;
    type Error = PdfError;

    fn export(
        &self,
        ctx: &ExportContext,
        opts: &Self::Options,
    ) -> Result<Self::Output, Self::Error> {
        if ctx.sheets.is_empty() {
            return Err(PdfError::NoSheets);
        }

        let sheet_indices = resolve_page_range(&opts.page_range, ctx.sheets.len())?;
        let (page_w_mm, page_h_mm) = opts.page_size.dimensions_mm(opts.orientation);
        let page_w_pt = (page_w_mm * MM_TO_PT) as f32;
        let page_h_pt = (page_h_mm * MM_TO_PT) as f32;

        let mut pdf = Pdf::new();

        let catalog_id = Ref::new(1);
        let page_tree_id = Ref::new(2);

        // Reserve one Ref per page, starting at 3 (after catalog + page tree).
        let page_refs: Vec<Ref> = (0..sheet_indices.len())
            .map(|i| Ref::new(3 + i as i32))
            .collect();

        // Reserve content stream Refs after page Refs.
        let content_refs: Vec<Ref> = (0..sheet_indices.len())
            .map(|i| Ref::new(3 + sheet_indices.len() as i32 + i as i32))
            .collect();

        pdf.catalog(catalog_id).pages(page_tree_id);
        pdf.pages(page_tree_id)
            .kids(page_refs.iter().copied())
            .count(page_refs.len() as i32);

        // Build each page with content.
        for (idx, &sheet_idx) in sheet_indices.iter().enumerate() {
            let sheet = &ctx.sheets[sheet_idx];
            let content_ref = content_refs[idx];

            // Emit content stream for this page.
            let content_bytes = build_page_content(
                sheet,
                opts,
                ctx,
                page_w_pt,
                page_h_pt,
            )?;

            pdf.stream(content_ref, &content_bytes);

            // Create the page object referencing the content stream.
            let mut page = pdf.page(page_refs[idx]);
            page.parent(page_tree_id)
                .media_box(Rect::new(0.0, 0.0, page_w_pt, page_h_pt))
                .contents(content_ref);

            // Minimal resources for fonts — pdf-writer handles Type1 fonts internally.
            page.resources();

            page.finish();
        }

        let bytes = pdf.finish();

        Ok(PdfOutput {
            bytes,
            page_count: page_refs.len(),
        })
    }
}

/// Build a content stream for a single page.
fn build_page_content(
    sheet: &crate::SheetSnapshot,
    opts: &PdfOptions,
    ctx: &ExportContext,
    page_w_pt: f32,
    page_h_pt: f32,
) -> Result<Vec<u8>, PdfError> {
    let mut surface = PdfSurface::new();

    // Convert schematic mm to PDF pt. Y-flip for PDF (bottom-left origin).
    // Note: OneToOne scale (FitToPage is a no-op for now).
    let mm_to_pt = MM_TO_PT;

    // Set default black color for all drawings.
    surface.set_stroke_color(0.0, 0.0, 0.0);

    // Draw schematic content.
    // Wires: stroke as lines (0.15 mm default, black).
    for wire in &sheet.schematic.wires {
        let w = if wire.stroke_width > 0.0 {
            (wire.stroke_width * mm_to_pt) as f32
        } else {
            (0.15 * mm_to_pt) as f32
        };
        let x1 = (wire.start.x * mm_to_pt) as f32;
        let y1 = page_h_pt - (wire.start.y * mm_to_pt) as f32;
        let x2 = (wire.end.x * mm_to_pt) as f32;
        let y2 = page_h_pt - (wire.end.y * mm_to_pt) as f32;
        surface.stroke_line(x1, y1, x2, y2, w);
    }

    // Symbols: bbox (10mm square default) + reference text.
    for sym in &sheet.schematic.symbols {
        // Compute symbol bbox: if it has pins, use their bounding box.
        // Otherwise, use a default 10mm × 10mm square.
        let (bbox_x1, bbox_y1, bbox_x2, bbox_y2) = if let Some(lib_sym) =
            sheet.schematic.lib_symbols.values().find(|ls| ls.id == sym.lib_id)
        {
            // Compute bbox from library symbol graphics.
            let mut x_min: f64 = 0.0;
            let mut x_max: f64 = 0.0;
            let mut y_min: f64 = 0.0;
            let mut y_max: f64 = 0.0;
            for lib_g in &lib_sym.graphics {
                match &lib_g.graphic {
                    signex_types::schematic::Graphic::Rectangle {
                        start,
                        end,
                        ..
                    } => {
                        x_min = x_min.min(start.x).min(end.x);
                        x_max = x_max.max(start.x).max(end.x);
                        y_min = y_min.min(start.y).min(end.y);
                        y_max = y_max.max(start.y).max(end.y);
                    }
                    signex_types::schematic::Graphic::Polyline { points, .. } => {
                        for pt in points {
                            x_min = x_min.min(pt.x);
                            x_max = x_max.max(pt.x);
                            y_min = y_min.min(pt.y);
                            y_max = y_max.max(pt.y);
                        }
                    }
                    _ => {}
                }
            }
            // Add symbol position offset.
            let w = (x_max - x_min).max(10.0);
            let h = (y_max - y_min).max(10.0);
            (
                sym.position.x - w / 2.0,
                sym.position.y - h / 2.0,
                sym.position.x + w / 2.0,
                sym.position.y + h / 2.0,
            )
        } else {
            // Default 10mm box.
            (
                sym.position.x - 5.0,
                sym.position.y - 5.0,
                sym.position.x + 5.0,
                sym.position.y + 5.0,
            )
        };

        let x = (bbox_x1 * mm_to_pt) as f32;
        let y = page_h_pt - (bbox_y2 * mm_to_pt) as f32;
        let w = ((bbox_x2 - bbox_x1) * mm_to_pt) as f32;
        let h = ((bbox_y2 - bbox_y1) * mm_to_pt) as f32;
        surface.stroke_rect(x, y, w, h, (0.1 * mm_to_pt) as f32);

        // Reference text at symbol center.
        if !sym.reference.is_empty() {
            let cx = ((bbox_x1 + bbox_x2) / 2.0 * mm_to_pt) as f32;
            let cy = page_h_pt - ((bbox_y1 + bbox_y2) / 2.0 * mm_to_pt) as f32;
            surface.text_at(cx, cy, "F1", 9.0, &sym.reference);
        }
    }

    // Labels: text at position.
    for label in &sheet.schematic.labels {
        let x = (label.position.x * mm_to_pt) as f32;
        let y = page_h_pt - (label.position.y * mm_to_pt) as f32;
        let size = if label.font_size > 0.0 {
            (label.font_size * mm_to_pt) as f32
        } else {
            9.0 // default
        };
        surface.text_at(x, y, "F1", size, &label.text);
    }

    // Title block (if enabled and template present).
    if opts.include_title_block {
        if let Some(template_id) = &opts.sheet_template {
            if let Some(template) = crate::template::load_builtin(template_id) {
                let sub_ctx = SubstitutionContext {
                    metadata: &ctx.metadata,
                    filename: sheet.path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                    sheet_name: sheet.sheet_name.clone(),
                    sheet_number: sheet.sheet_number,
                    sheet_count: sheet.sheet_count,
                    signex_version: env!("CARGO_PKG_VERSION"),
                };

                // Draw title block frame (bottom-right).
                // Position: page_w_pt - title_block_width
                let tb_width_pt = (template.title_block.width_mm * mm_to_pt) as f32;
                let tb_height_pt = (template.title_block.height_mm * mm_to_pt) as f32;
                let tb_x = page_w_pt - tb_width_pt;
                let tb_y = page_h_pt - tb_height_pt;
                surface.stroke_rect(
                    tb_x,
                    tb_y,
                    tb_width_pt,
                    tb_height_pt,
                    (0.2 * mm_to_pt) as f32,
                );

                // Emit title block fields.
                for field in &template.title_block.fields {
                    let resolved = crate::resolve(&field.default_text, &sub_ctx);
                    let fx = tb_x + (field.x_mm * mm_to_pt) as f32;
                    let fy = tb_y + (field.y_mm * mm_to_pt) as f32;
                    let font = PdfFont::for_style(field.font_style);
                    let font_name = if font == PdfFont::Helvetica {
                        "F1"
                    } else {
                        "F2"
                    };
                    let size = (field.font_size_mm * mm_to_pt) as f32;
                    surface.text_at(fx, fy, font_name, size as f32, &resolved);
                }
            }
        }
    }

    Ok(surface.finish())
}

/// Resolve a `PageRange` against the project's sheet count into a concrete
/// list of zero-based sheet indices to export.
fn resolve_page_range(range: &PageRange, sheet_count: usize) -> Result<Vec<usize>, PdfError> {
    match range {
        PageRange::All => Ok((0..sheet_count).collect()),
        PageRange::Current => Ok(vec![0]),
        PageRange::Specific(pages) => {
            let mut out = Vec::with_capacity(pages.len());
            for &p in pages {
                if p == 0 || p > sheet_count {
                    return Err(PdfError::OutOfRange(p, sheet_count));
                }
                out.push(p - 1);
            }
            Ok(out)
        }
        PageRange::Range(start, end) => {
            if *start == 0 || *end == 0 || *start > sheet_count || *end > sheet_count {
                return Err(PdfError::OutOfRange(
                    (*start).max(*end).max(1),
                    sheet_count,
                ));
            }
            if start <= end {
                Ok((start - 1..*end).collect())
            } else {
                Ok((end - 1..*start).collect())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_types::schematic::SchematicSheet;

    use super::*;
    use crate::{ExportContext, ProjectMetadata, SheetSnapshot};

    fn empty_sheet() -> SchematicSheet {
        SchematicSheet {
            uuid: uuid::Uuid::nil(),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            root_sheet_page: "1".into(),
            symbols: vec![],
            wires: vec![],
            junctions: vec![],
            labels: vec![],
            child_sheets: vec![],
            no_connects: vec![],
            text_notes: vec![],
            buses: vec![],
            bus_entries: vec![],
            drawings: vec![],
            no_erc_directives: vec![],
            title_block: Default::default(),
            lib_symbols: Default::default(),
        }
    }

    fn sample_ctx(sheet_count: usize) -> ExportContext {
        ExportContext {
            sheets: (0..sheet_count)
                .map(|i| SheetSnapshot {
                    path: PathBuf::from(format!("sheet_{i}.kicad_sch")),
                    schematic: empty_sheet(),
                    sheet_name: format!("Sheet{i}"),
                    sheet_number: i + 1,
                    sheet_count,
                })
                .collect(),
            metadata: ProjectMetadata::default(),
        }
    }

    #[test]
    fn produces_valid_pdf_header() {
        let ctx = sample_ctx(1);
        let out = PdfExporter
            .export(&ctx, &PdfOptions::default())
            .expect("export");
        assert!(out.bytes.starts_with(b"%PDF-"), "missing %PDF- header");
        assert!(out.bytes.ends_with(b"%%EOF\n") || out.bytes.ends_with(b"%%EOF"));
        assert_eq!(out.page_count, 1);
    }

    #[test]
    fn multi_sheet_produces_multi_page() {
        let ctx = sample_ctx(4);
        let out = PdfExporter
            .export(&ctx, &PdfOptions::default())
            .expect("export");
        assert_eq!(out.page_count, 4);
    }

    #[test]
    fn empty_context_errors() {
        let ctx = ExportContext {
            sheets: vec![],
            metadata: ProjectMetadata::default(),
        };
        let err = PdfExporter.export(&ctx, &PdfOptions::default()).unwrap_err();
        assert!(matches!(err, PdfError::NoSheets));
    }

    #[test]
    fn page_range_specific() {
        let ctx = sample_ctx(5);
        let opts = PdfOptions {
            page_range: PageRange::Specific(vec![1, 3, 5]),
            ..Default::default()
        };
        let out = PdfExporter.export(&ctx, &opts).unwrap();
        assert_eq!(out.page_count, 3);
    }

    #[test]
    fn page_range_range_inclusive() {
        let ctx = sample_ctx(5);
        let opts = PdfOptions {
            page_range: PageRange::Range(2, 4),
            ..Default::default()
        };
        let out = PdfExporter.export(&ctx, &opts).unwrap();
        assert_eq!(out.page_count, 3); // 2, 3, 4
    }

    #[test]
    fn page_range_out_of_bounds() {
        let ctx = sample_ctx(3);
        let opts = PdfOptions {
            page_range: PageRange::Specific(vec![1, 99]),
            ..Default::default()
        };
        let err = PdfExporter.export(&ctx, &opts).unwrap_err();
        assert!(matches!(err, PdfError::OutOfRange(99, 3)));
    }

    #[test]
    fn page_size_reflected_in_media_box() {
        let ctx = sample_ctx(1);
        let opts = PdfOptions {
            page_size: PageSize::IsoA4,
            orientation: Orientation::Portrait,
            ..Default::default()
        };
        let out = PdfExporter.export(&ctx, &opts).unwrap();
        // A4 portrait = 210 × 297 mm = 595.28 × 841.89 pt.
        let bytes = String::from_utf8_lossy(&out.bytes);
        assert!(bytes.contains("595"), "width not reflected in MediaBox");
        assert!(bytes.contains("841"), "height not reflected in MediaBox");
    }

    #[test]
    fn exports_schematic_content() {
        use signex_types::schematic::{Wire, Symbol, Label, LabelType};
        use std::collections::HashMap;
        use uuid::Uuid;

        let mut sheet = empty_sheet();

        // Add one wire.
        sheet.wires.push(Wire {
            uuid: Uuid::nil(),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 10.0),
            stroke_width: 0.15,
        });

        // Add one symbol.
        sheet.symbols.push(Symbol {
            uuid: Uuid::nil(),
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            value: "10k".to_string(),
            position: Point::new(50.0, 50.0),
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            unit: 1,
            is_power: false,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: HashMap::new(),
            custom_properties: vec![],
            pin_uuids: HashMap::new(),
            instances: vec![],
            footprint: String::new(),
            datasheet: String::new(),
        });

        // Add one label.
        sheet.labels.push(Label {
            uuid: Uuid::nil(),
            text: "VCC".to_string(),
            position: Point::new(20.0, 20.0),
            rotation: 0.0,
            label_type: LabelType::Net,
            shape: String::new(),
            font_size: 0.0,
            justify: signex_types::schematic::HAlign::Center,
        });

        let mut ctx = sample_ctx(1);
        ctx.sheets[0].schematic = sheet;

        let out = PdfExporter
            .export(&ctx, &PdfOptions::default())
            .expect("export");

        let bytes = String::from_utf8_lossy(&out.bytes);
        // Check for content stream operators: 'm' (moveto), 'l' (lineto), 'S' (stroke),
        // 're' (rect), 'Tj' (show text).
        let has_graphics = bytes.contains(" l\n") || bytes.contains(" re\n") || bytes.contains(" Tj");
        assert!(
            has_graphics,
            "exported PDF should contain graphics operators"
        );
    }
}
