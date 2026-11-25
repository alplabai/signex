//! The compile pipeline: notebook sources to Typst pages, page images for
//! the preview and a PDF that is the same file, byte for byte, every time
//! the same notebook is exported.

use typst::diag::{Severity, SourceDiagnostic};
use typst::foundations::Smart;
use typst::utils::Scalar;
use typst::{World, WorldExt};
use typst_layout::PagedDocument;

use crate::date::IsoDate;
use crate::sources::NotebookSources;
use crate::world::NotebookWorld;

/// The PDF `/Creator` every notebook export carries. Fixed, so a Signex
/// update alone does not change the file.
pub const PDF_CREATOR: &str = "Signex Design Notebook";

/// What a compile may depend on besides the sources.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompileOptions {
    /// The date Typst's `datetime.today()` returns. `None` makes that call
    /// an error, so no page depends on the clock unless the caller says so
    /// (the export passes the title block's revision date).
    pub today: Option<IsoDate>,
}

/// How bad a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    /// The notebook did not compile.
    Error,
    /// It compiled; something deserves a look.
    Warning,
}

/// One compile error or warning, where it is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    /// The file, as a path inside the notebook folder; empty when the
    /// problem is not in a file.
    pub file: String,
    /// 1-based line; 0 when it is not in a file.
    pub line: u32,
    /// 1-based column, in characters; 0 when it is not in a file.
    pub column: u32,
    pub message: String,
    /// The compiler's hints, in order.
    pub hints: Vec<String>,
}

impl Diagnostic {
    /// Errors first, then by file, line and column.
    fn order(&self) -> (bool, &str, u32, u32) {
        let is_warning = self.severity != DiagnosticSeverity::Error;
        (is_warning, self.file.as_str(), self.line, self.column)
    }
}

/// The size of one compiled page.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct PageInfo {
    /// 1-based physical page number.
    pub number: usize,
    /// Width in typographic points.
    pub width_pt: f64,
    /// Height in typographic points.
    pub height_pt: f64,
}

/// One page drawn for the preview: `width` x `height` pixels, RGBA, eight
/// bits per channel, premultiplied (pages are opaque, so it is plain RGBA).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The compiled pages of a notebook.
#[derive(Debug, Clone)]
pub struct CompiledNotebook {
    document: PagedDocument,
}

/// What a compile gave: the pages when there was no error, and every
/// diagnostic, errors first, each group in source order.
#[derive(Debug, Clone)]
pub struct Compilation {
    pub notebook: Option<CompiledNotebook>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Compilation {
    /// True when the notebook did not compile.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }
}

/// Compile `sources` to pages. Pure: the result depends only on the
/// sources, the options and the bundled package and fonts.
pub fn compile(sources: &NotebookSources, options: &CompileOptions) -> Compilation {
    let Some(world) = NotebookWorld::new(sources, options.today) else {
        return Compilation {
            notebook: None,
            diagnostics: vec![Diagnostic {
                severity: DiagnosticSeverity::Error,
                file: sources.main().to_string(),
                line: 0,
                column: 0,
                message: "the notebook path does not name a file".to_string(),
                hints: Vec::new(),
            }],
        };
    };
    let warned = typst::compile::<PagedDocument>(&world);
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let notebook = match warned.output {
        Ok(document) => Some(CompiledNotebook { document }),
        Err(errors) => {
            diagnostics.extend(errors.iter().map(|d| diagnostic(&world, d)));
            None
        }
    };
    diagnostics.extend(warned.warnings.iter().map(|d| diagnostic(&world, d)));
    diagnostics.sort_by(|a, b| a.order().cmp(&b.order()));
    Compilation {
        notebook,
        diagnostics,
    }
}

impl CompiledNotebook {
    /// How many pages the notebook has.
    pub fn page_count(&self) -> usize {
        self.document.pages().len()
    }

    /// The text the pages print, each run of text followed by a space and
    /// each page by a line break: what a test or a search reads.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for page in self.document.pages() {
            frame_text(&page.frame, &mut out);
            out.push('\n');
        }
        out
    }

    /// The number and size of every page.
    pub fn pages(&self) -> Vec<PageInfo> {
        self.document
            .pages()
            .iter()
            .enumerate()
            .map(|(index, page)| PageInfo {
                number: index + 1,
                width_pt: page.frame.width().to_pt(),
                height_pt: page.frame.height().to_pt(),
            })
            .collect()
    }

    /// Page `index` (0-based) drawn at `pixels_per_pt` pixels per point,
    /// on white; `None` past the last page or for a scale that is not
    /// positive and finite.
    pub fn render_page(&self, index: usize, pixels_per_pt: f64) -> Option<PageImage> {
        if !(pixels_per_pt.is_finite() && pixels_per_pt > 0.0) {
            return None;
        }
        let page = self.document.pages().get(index)?;
        let options = typst_render::RenderOptions {
            pixel_per_pt: Scalar::new(pixels_per_pt),
            render_bleed: false,
        };
        let pixmap = typst_render::render(page, &options);
        Some(PageImage {
            width: pixmap.width(),
            height: pixmap.height(),
            rgba: pixmap.data().to_vec(),
        })
    }

    /// The notebook as PDF. `ident` names the notebook for good (the
    /// project and the notebook path); its hash becomes the PDF's id. No
    /// creation time is written and the creator is [`PDF_CREATOR`], so the
    /// same notebook gives the same bytes. Headings become bookmarks.
    pub fn pdf(&self, ident: &str) -> Result<Vec<u8>, Vec<String>> {
        let options = typst_pdf::PdfOptions {
            ident: Smart::Custom(ident.to_string()),
            creator: Smart::Custom(Some(PDF_CREATOR.to_string())),
            timestamp: None,
            ..typst_pdf::PdfOptions::default()
        };
        typst_pdf::pdf(&self.document, &options)
            .map_err(|errors| errors.iter().map(|e| e.message.to_string()).collect())
    }
}

/// `source` placed in its file, line and column.
fn diagnostic(world: &NotebookWorld, source: &SourceDiagnostic) -> Diagnostic {
    let severity = match source.severity {
        Severity::Error => DiagnosticSeverity::Error,
        Severity::Warning => DiagnosticSeverity::Warning,
    };
    let (file, line, column) = source
        .span
        .id()
        .and_then(|id| {
            let start = world.range(source.span)?.start;
            let text = world.source(id).ok()?;
            let (line, column) = text.lines().byte_to_line_column(start)?;
            let file = NotebookWorld::path_of(id);
            Some((file, to_u32(line + 1), to_u32(column + 1)))
        })
        .unwrap_or_default();
    Diagnostic {
        severity,
        file,
        line,
        column,
        message: source.message.to_string(),
        hints: source.hints.iter().map(|h| h.v.to_string()).collect(),
    }
}

fn frame_text(frame: &typst::layout::Frame, out: &mut String) {
    for (_, item) in frame.items() {
        match item {
            typst::layout::FrameItem::Group(group) => frame_text(&group.frame, out),
            typst::layout::FrameItem::Text(run) => {
                out.push_str(&run.text);
                out.push(' ');
            }
            _ => {}
        }
    }
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}
