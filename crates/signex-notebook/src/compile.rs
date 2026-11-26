//! The compile pipeline: notebook sources to Typst pages, page images for
//! the preview and a PDF that is the same file, byte for byte, every time
//! the same notebook is exported.

use std::num::NonZeroUsize;

use typst::diag::{Severity, SourceDiagnostic};
use typst::foundations::Smart;
use typst::layout::PageRanges;
use typst::utils::Scalar;
use typst::{World, WorldExt};
use typst_layout::PagedDocument;

use crate::cell::PageCells;
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
    /// What the Python cells print: their last results and whether their
    /// source prints. Empty: every cell shows as not run yet. Compiling
    /// never runs a cell (`cell::run_cells` does, on Run, Run all and
    /// export).
    pub cells: PageCells,
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
    compile_with_inputs(sources, options, &[])
}

/// How long one compile may run. Typst caps a `while` loop but not a
/// `for` loop, so a notebook can loop for hours; past this the compile is
/// reported as stopped, so the preview and the command line go on (#3020).
pub const COMPILE_TIME_LIMIT: std::time::Duration = std::time::Duration::from_secs(60);

/// The stack a compile runs on: Typst recurses deeply on nested content.
const COMPILE_STACK: usize = 64 * 1024 * 1024;

/// [`compile`] with `inputs` as Typst's `sys.inputs`, `(key, text)` pairs;
/// the PDF export passes its settings this way (`docs/NOTEBOOK_PDF.md`).
/// Stopped after [`COMPILE_TIME_LIMIT`].
pub fn compile_with_inputs(
    sources: &NotebookSources,
    options: &CompileOptions,
    inputs: &[(String, String)],
) -> Compilation {
    compile_within(sources, options, inputs, COMPILE_TIME_LIMIT)
}

/// [`compile_with_inputs`] with its time limit. The compile runs on a
/// thread of its own; one still running at `limit` is reported as an
/// error and left behind, since Typst cannot be interrupted: the caller
/// answers, the process can exit, and the thread ends with its loop.
pub fn compile_within(
    sources: &NotebookSources,
    options: &CompileOptions,
    inputs: &[(String, String)],
    limit: std::time::Duration,
) -> Compilation {
    let (sent, received) = std::sync::mpsc::channel();
    let (sources_c, options_c, inputs_c) = (sources.clone(), options.clone(), inputs.to_vec());
    let spawned = std::thread::Builder::new()
        .name("notebook-compile".into())
        .stack_size(COMPILE_STACK)
        .spawn(move || {
            let _gone = sent.send(compile_now(&sources_c, &options_c, &inputs_c));
        });
    if spawned.is_err() {
        return compile_now(sources, options, inputs);
    }
    match received.recv_timeout(limit) {
        Ok(compilation) => compilation,
        Err(_) => Compilation {
            notebook: None,
            diagnostics: vec![Diagnostic {
                severity: DiagnosticSeverity::Error,
                file: String::new(),
                line: 0,
                column: 0,
                message: format!(
                    "the notebook did not finish compiling within {} s and was stopped",
                    limit.as_secs()
                ),
                hints: vec!["a loop that never ends, or one too long, is the usual cause".into()],
            }],
        },
    }
}

/// The compile itself, on the calling thread.
fn compile_now(
    sources: &NotebookSources,
    options: &CompileOptions,
    inputs: &[(String, String)],
) -> Compilation {
    let cells = options.cells.to_json();
    let Some(world) = NotebookWorld::new(sources, options.today, cells, inputs) else {
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

    /// The bookmarks the PDF of pages `first..=last` (1-based) carries:
    /// each bookmarked contents heading, and each captioned figure or table
    /// (an `#attachment`, a bench `#measurement` or a plain `figure`), read
    /// from the laid-out pages rather than the source (#3021).
    pub fn bookmarks(&self, first: usize, last: usize) -> crate::export::BookmarkCounts {
        use typst::foundations::{NativeElement, StyleChain};
        use typst::introspection::Introspector;
        use typst::model::{FigureElem, FigureKind, HeadingElem, TableElem};
        let styles = StyleChain::default();
        let introspector = self.document.introspector();
        let on_pages = |content: &typst::foundations::Content| {
            content
                .location()
                .and_then(|location| introspector.page(location))
                .is_some_and(|page| (first..=last).contains(&page.get()))
        };
        let mut counts = crate::export::BookmarkCounts::default();
        for content in introspector.query(&HeadingElem::ELEM.select()) {
            let Some(heading) = content.to_packed::<HeadingElem>() else {
                continue;
            };
            let outlined = heading.outlined.get(styles);
            let bookmarked = heading.bookmarked.get(styles).unwrap_or(outlined);
            // A figure's own bookmark heading stays out of the contents; it
            // is counted with its figure below.
            if !bookmarked || !outlined || !on_pages(&content) {
                continue;
            }
            if heading.resolve_level(styles).get() <= 1 {
                counts.sections += 1;
            } else {
                counts.subsections += 1;
            }
        }
        for content in introspector.query(&FigureElem::ELEM.select()) {
            let Some(figure) = content.to_packed::<FigureElem>() else {
                continue;
            };
            if figure.caption.get_ref(styles).is_none() || !on_pages(&content) {
                continue;
            }
            let table = matches!(
                figure.kind.get_ref(styles),
                Smart::Custom(FigureKind::Elem(kind)) if *kind == TableElem::ELEM
            );
            if table {
                counts.tables += 1;
            } else {
                counts.figures += 1;
            }
        }
        counts
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
        self.pdf_pages(ident, None)
    }

    /// [`Self::pdf`] of pages `first` to `last` only (1-based, both
    /// included); `None` writes every page. Every page is written as a
    /// tagged PDF; Typst cannot tag a page range, so a range is written
    /// untagged.
    pub fn pdf_pages(
        &self,
        ident: &str,
        pages: Option<(usize, usize)>,
    ) -> Result<Vec<u8>, Vec<String>> {
        let every = self.page_count();
        let range = pages.filter(|&(first, last)| (first, last) != (1, every));
        let page_ranges = range.map(|(first, last)| {
            PageRanges::new(vec![NonZeroUsize::new(first)..=NonZeroUsize::new(last)])
        });
        let options = typst_pdf::PdfOptions {
            ident: Smart::Custom(ident.to_string()),
            creator: Smart::Custom(Some(PDF_CREATOR.to_string())),
            timestamp: None,
            tagged: page_ranges.is_none(),
            page_ranges,
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

/// The text runs of `frame`, each followed by a space. A run that goes on
/// where the one before it ended (a glyph a fallback font drew, `ΔY`)
/// joins it, and a no-break space reads as a space.
fn frame_text(frame: &typst::layout::Frame, out: &mut String) {
    let mut end: Option<typst::layout::Point> = None;
    for (at, item) in frame.items() {
        match item {
            typst::layout::FrameItem::Group(group) => {
                frame_text(&group.frame, out);
                end = None;
            }
            typst::layout::FrameItem::Text(run) => {
                let touches = end.is_some_and(|end| {
                    (end.y - at.y).abs().to_pt() < 0.01 && (end.x - at.x).abs().to_pt() < 0.01
                });
                if touches && out.ends_with(' ') {
                    out.pop();
                }
                out.push_str(&run.text.as_str().replace('\u{a0}', " "));
                out.push(' ');
                end = Some(typst::layout::Point::new(at.x + run.width(), at.y));
            }
            _ => {}
        }
    }
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}
