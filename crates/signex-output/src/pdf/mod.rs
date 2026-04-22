//! PDF export via `pdf-writer`.
//!
//! See `OUTPUT_PLAN.md` §3. `PdfSurface` (in `surface.rs`) will eventually
//! act as a second render target for the existing `signex-render` scene
//! graph — screen and PDF share one source of truth. For now the exporter
//! only emits blank pages at the correct geometry; scene content integration
//! lands in a follow-up commit.

use pdf_writer::{Finish, Pdf, Rect, Ref};
use thiserror::Error;

use crate::template::TemplateId;
use crate::{ExportContext, Exporter};

mod font;
mod layout;
mod page;
mod surface;

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

        pdf.catalog(catalog_id).pages(page_tree_id);
        pdf.pages(page_tree_id)
            .kids(page_refs.iter().copied())
            .count(page_refs.len() as i32);

        for &page_ref in &page_refs {
            let mut page = pdf.page(page_ref);
            page.parent(page_tree_id)
                .media_box(Rect::new(0.0, 0.0, page_w_pt, page_h_pt))
                .resources();
            page.finish();
        }

        let bytes = pdf.finish();

        Ok(PdfOutput {
            bytes,
            page_count: page_refs.len(),
        })
    }
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
                    path: PathBuf::from(format!("sheet_{i}.standard_sch")),
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
}
