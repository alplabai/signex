//! The notebook PDF export (mockup row 202, #2611): the stale-reference
//! check, the compile with the export's settings, the pages chosen, the
//! bookmarks and a PDF that is the same file every time. The dialog, the
//! `export_notebook_pdf` command and the Design Notebook PDF output of an
//! Output Job all call [`export_pdf`]; an export that runs the Python cells
//! first (mockup row 200) calls [`export_pdf_running_cells`]. Rules:
//! `docs/NOTEBOOK_PDF.md`.

mod bookmarks;
mod cells;
mod file_name;
mod options;
mod output_job;
mod stale;
mod title;

#[cfg(test)]
mod tests_cells;

pub use bookmarks::BookmarkCounts;
pub use cells::{ExportCells, export_pdf_running_cells};
pub use file_name::{OUTPUT_FOLDERS, default_file_name};
pub use options::{ExportOptions, PageSelection, Paper, StaleRule};
pub use output_job::{NOTEBOOK_PDF_OUTPUT_KIND, NotebookPdfOutput};
pub use stale::{DesignNames, StaleCheck};
pub use title::TitleBlock;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cell::PageCells;
use crate::compile::{
    CompileOptions, Diagnostic, DiagnosticSeverity, PageInfo, compile_with_inputs,
};
use crate::reference::Reference;
use crate::sources::NotebookSources;

/// Points per millimetre.
const PT_PER_MM: f64 = 72.0 / 25.4;
/// How far a page may be off a paper size and still be called it, in
/// points.
const PAPER_TOLERANCE_PT: f64 = 1.0;

/// A page size by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperName {
    A4,
    Letter,
    /// Any other size.
    Other,
}

/// The paper the pages are on, read from the first page.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PaperSize {
    pub name: PaperName,
    pub landscape: bool,
    pub width_mm: f64,
    pub height_mm: f64,
}

impl PaperSize {
    /// The paper of `page`.
    pub fn of(page: &PageInfo) -> Self {
        let (short, long) = if page.width_pt <= page.height_pt {
            (page.width_pt, page.height_pt)
        } else {
            (page.height_pt, page.width_pt)
        };
        let near = |w: f64, h: f64| {
            (short - w).abs() < PAPER_TOLERANCE_PT && (long - h).abs() < PAPER_TOLERANCE_PT
        };
        let name = if near(595.28, 841.89) {
            PaperName::A4
        } else if near(612.0, 792.0) {
            PaperName::Letter
        } else {
            PaperName::Other
        };
        Self {
            name,
            landscape: page.width_pt > page.height_pt,
            width_mm: page.width_pt / PT_PER_MM,
            height_mm: page.height_pt / PT_PER_MM,
        }
    }
}

/// What the dialog shows before anything is written: the notebook as it
/// compiles now, with no export settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotebookInspection {
    pub page_count: usize,
    /// The paper of the first page; `None` when the notebook did not
    /// compile.
    pub paper: Option<PaperSize>,
    pub title: TitleBlock,
    pub stale: StaleCheck,
    /// The compile's errors and warnings.
    pub diagnostics: Vec<Diagnostic>,
}

/// What an export wrote, for the done card and the command's answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExportSummary {
    /// Pages in the PDF.
    pub pages: usize,
    /// Pages of the notebook.
    pub page_count: usize,
    /// The first and last page exported, 1-based.
    pub first_page: usize,
    pub last_page: usize,
    pub paper: PaperSize,
    pub bytes: usize,
    pub references: usize,
    pub stale: Vec<Reference>,
    pub variant: String,
    /// Zero when the export left bookmarks out.
    pub bookmarks: BookmarkCounts,
    /// The SHA-256 of the file, lower-case hex.
    pub sha256: String,
    pub title: TitleBlock,
    /// The compile's warnings.
    pub warnings: Vec<Diagnostic>,
}

/// A finished export: the PDF and what it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct NotebookExport {
    pub pdf: Vec<u8>,
    pub summary: ExportSummary,
}

/// Why nothing was exported.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ExportError {
    /// The stale rule is Fail and these references are stale.
    #[error("{} stale reference(s): {}", .0.stale.len(), names(&.0.stale))]
    Stale(StaleCheck),
    /// The notebook did not compile.
    #[error("the notebook does not compile: {}", first_error(.0))]
    Compile(Vec<Diagnostic>),
    /// The page selection names no page of the notebook.
    #[error("the notebook has {page_count} page(s), and the selection names none of them")]
    NoPages { page_count: usize },
    /// The PDF writer refused.
    #[error("the PDF could not be written: {}", .0.join("; "))]
    Pdf(Vec<String>),
}

/// Compile `sources` as they are and check their references with
/// `is_live`, for the dialog.
pub fn inspect(
    sources: &NotebookSources,
    is_live: impl Fn(&Reference) -> bool,
) -> NotebookInspection {
    let title = main_title(sources);
    let options = CompileOptions {
        today: title.date,
        ..CompileOptions::default()
    };
    let compilation = compile_with_inputs(sources, &options, &[]);
    let pages = compilation
        .notebook
        .as_ref()
        .map(|notebook| notebook.pages())
        .unwrap_or_default();
    NotebookInspection {
        page_count: pages.len(),
        paper: pages.first().map(PaperSize::of),
        title,
        stale: StaleCheck::run(sources, is_live),
        diagnostics: compilation.diagnostics,
    }
}

/// Export `sources` with `options`. `ident` names the notebook for good
/// (the project and the notebook path) and becomes the PDF's id; `is_live`
/// says whether a reference still names something in the design. Pure:
/// the same sources, options, ident and design give the same bytes.
pub fn export_pdf(
    sources: &NotebookSources,
    options: &ExportOptions,
    ident: &str,
    is_live: impl Fn(&Reference) -> bool,
) -> Result<NotebookExport, ExportError> {
    export_with_cells(sources, options, ident, is_live, PageCells::default())
}

/// [`export_pdf`] with the cells printing `cells`.
fn export_with_cells(
    sources: &NotebookSources,
    options: &ExportOptions,
    ident: &str,
    is_live: impl Fn(&Reference) -> bool,
    cells: PageCells,
) -> Result<NotebookExport, ExportError> {
    let stale = StaleCheck::run(sources, is_live);
    if options.stale == StaleRule::Fail && !stale.stale.is_empty() {
        return Err(ExportError::Stale(stale));
    }
    let title = main_title(sources);
    let compile_options = CompileOptions {
        today: title.date,
        cells,
    };
    let inputs = options.typst_inputs(&stale.stale_names());
    let compilation = compile_with_inputs(sources, &compile_options, &inputs);
    let Some(notebook) = compilation.notebook else {
        return Err(ExportError::Compile(compilation.diagnostics));
    };
    let pages = notebook.pages();
    let page_count = pages.len();
    let (first_page, last_page) = options
        .pages
        .resolve(page_count)
        .ok_or(ExportError::NoPages { page_count })?;
    let pdf = notebook
        .pdf_pages(ident, Some((first_page, last_page)))
        .map_err(ExportError::Pdf)?;
    // Counted on the pages exported, so `--pages` and the figures an
    // `#attachment` or a `#measurement` makes are counted (#3021).
    let bookmarks = if options.bookmarks {
        notebook.bookmarks(first_page, last_page)
    } else {
        BookmarkCounts::default()
    };
    let summary = ExportSummary {
        pages: last_page + 1 - first_page,
        page_count,
        first_page,
        last_page,
        paper: PaperSize::of(&pages[first_page - 1]),
        bytes: pdf.len(),
        references: stale.references,
        stale: stale.stale,
        variant: options.variant.clone(),
        bookmarks,
        sha256: sha256_hex(&pdf),
        title,
        warnings: compilation.diagnostics,
    };
    Ok(NotebookExport { pdf, summary })
}

/// The SHA-256 of `bytes`, lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The title block of the notebook file itself.
fn main_title(sources: &NotebookSources) -> TitleBlock {
    sources
        .get(sources.main())
        .map(|bytes| TitleBlock::of(&String::from_utf8_lossy(bytes)))
        .unwrap_or_default()
}

fn names(references: &[Reference]) -> String {
    references
        .iter()
        .map(Reference::name)
        .collect::<Vec<_>>()
        .join(", ")
}

fn first_error(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
        .map_or_else(String::new, |d| {
            format!("{}:{}:{}: {}", d.file, d.line, d.column, d.message)
        })
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_options;
