//! PDF export via `pdf-writer`.
//!
//! See `OUTPUT_PLAN.md` §3. `PdfSurface` (in `surface.rs`) acts as a second
//! render target for the existing `signex-render` scene graph — screen and
//! PDF share one source of truth for how a schematic looks.

use thiserror::Error;

use crate::template::TemplateId;
use crate::{ExportContext, Exporter};

mod font;
mod layout;
mod page;
mod surface;

/// Stub. Real impl arrives in a follow-up PR that wires up `pdf-writer`.
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
    #[error("pdf-writer emission failed")]
    Emission,

    #[error("font subsetting failed: {0}")]
    Font(String),

    #[error("unsupported page size configuration")]
    UnsupportedPageSize,
}

impl Exporter for PdfExporter {
    type Options = PdfOptions;
    type Output = PdfOutput;
    type Error = PdfError;

    fn export(
        &self,
        _ctx: &ExportContext,
        _opts: &Self::Options,
    ) -> Result<Self::Output, Self::Error> {
        todo!("pdf pipeline — implemented in a follow-up PR on feature/v0.8-output")
    }
}
