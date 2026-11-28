//! Rich text notes on a schematic sheet (#1114, mockup row 203): a note's
//! Typst source compiled in the notebook's world (the same Typst, bundled
//! fonts and `@signex/notebook` package, nothing read from the disk or the
//! network) and lowered to vector paths in mm, which the canvas, the SVG and
//! the PDF export all draw. A compile is pure: the same [`NoteInput`] gives
//! the same [`NoteDrawing`], so the caller caches by [`note_key`].
//! Rules: `docs/architecture/snxsch-note-format.md`, "Drawing".

mod draw;
mod place;
mod shape;

#[cfg(test)]
mod tests;

use std::hash::{Hash, Hasher};

use signex_types::schematic::{HAlign, TextNote};
use typst_layout::PagedDocument;

pub use place::PlacedNote;
pub use shape::{NoteDrawing, NoteLink, NotePaint, NotePathOp, NoteShape, NoteShapeKind};

use crate::compile::{Diagnostic, DiagnosticSeverity};
use crate::sources::NotebookSources;
use crate::world::NotebookWorld;

/// The file a note compiles as; diagnostics name it.
pub const NOTE_FILE: &str = "note.typ";
/// The Typst release every note compiles with. Part of the cache key, so an
/// upgrade recompiles every note.
pub const TYPST_VERSION: &str = "0.15.1";
/// A note that lowers to more shapes than this is refused with an error.
pub const MAX_NOTE_SHAPES: usize = 20_000;
/// Typographic points per mm.
pub const PT_PER_MM: f64 = 72.0 / 25.4;
/// A note's text size is the height of its capitals; a Typst text size is
/// its em. The bundled text face's capitals are about this much of the em.
const CAP_HEIGHT_EM: f64 = 0.7;
/// The text size a note with no size of its own compiles at, in mm.
const DEFAULT_TEXT_SIZE_MM: f64 = 1.27;
/// The lines the compile puts before the source: the page, the text size
/// and the paragraph alignment.
const PREAMBLE_LINES: u32 = 3;

/// What one note compiles from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteInput<'a> {
    /// The note's text: its Typst source.
    pub source: &'a str,
    /// The width the text wraps at, in mm; `None` lays every line out as
    /// long as it is (Fit text).
    pub width_mm: Option<f64>,
    /// The note's text size (cap height) in mm.
    pub text_size_mm: f64,
    /// Justify H: how paragraphs and lists align in the box.
    pub align: HAlign,
}

impl<'a> NoteInput<'a> {
    /// What `note` compiles from: its text, its Size section's text width,
    /// its text size and its Justify H.
    #[must_use]
    pub fn of(note: &'a TextNote) -> Self {
        Self {
            source: &note.text,
            width_mm: note.rich.text_width_mm(),
            text_size_mm: note.font_size,
            align: note.justify_h,
        }
    }
}

impl NoteInput<'_> {
    /// The Typst text size, in points.
    #[must_use]
    pub fn text_size_pt(&self) -> f64 {
        let size = if self.text_size_mm.is_finite() && self.text_size_mm > 0.0 {
            self.text_size_mm
        } else {
            DEFAULT_TEXT_SIZE_MM
        };
        size * PT_PER_MM / CAP_HEIGHT_EM
    }

    /// The width the page is laid out at, when it is a usable length.
    fn page_width_mm(&self) -> Option<f64> {
        self.width_mm.filter(|mm| mm.is_finite() && *mm > 0.0)
    }

    /// The file the world compiles: the page and text size, then the source.
    fn main_text(&self) -> String {
        let width = match self.page_width_mm() {
            Some(mm) => format!("{mm:.4}mm"),
            None => "auto".to_string(),
        };
        let align = match self.align {
            HAlign::Left => "left",
            HAlign::Center => "center",
            HAlign::Right => "right",
        };
        // The text's bottom edge is its descender, so the last line's descenders
        // stay inside the page the drawing is sized by.
        format!(
            "#set page(width: {width}, height: auto, margin: 0pt, fill: none)\n\
             #set text(size: {size:.4}pt, bottom-edge: \"descender\")\n\
             #set align({align})\n{source}",
            size = self.text_size_pt(),
            source = self.source,
        )
    }
}

/// The cache key of a compile: a hash of everything the drawing depends on
/// (the source, the width, the text size, the Typst release and the bundled
/// package). Equal inputs give equal keys within one run of the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoteKey(pub u64);

/// The key of `input`.
#[must_use]
pub fn note_key(input: &NoteInput<'_>) -> NoteKey {
    let mut hasher = std::hash::DefaultHasher::new();
    input.source.hash(&mut hasher);
    input.page_width_mm().map(f64::to_bits).hash(&mut hasher);
    input.text_size_pt().to_bits().hash(&mut hasher);
    input.align.hash(&mut hasher);
    TYPST_VERSION.hash(&mut hasher);
    crate::world::PACKAGE_VERSION.hash(&mut hasher);
    NoteKey(hasher.finish())
}

/// What a note compile gave: the drawing when there was no error, and every
/// diagnostic, lines counted in the note's own source (1-based).
#[derive(Debug, Clone)]
pub struct NoteCompilation {
    pub drawing: Option<NoteDrawing>,
    pub diagnostics: Vec<Diagnostic>,
}

impl NoteCompilation {
    /// True when the note did not compile.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }

    /// The first error, the one the editor shows under its line.
    #[must_use]
    pub fn first_error(&self) -> Option<&Diagnostic> {
        self.diagnostics
            .iter()
            .find(|d| d.severity == DiagnosticSeverity::Error)
    }
}

/// Compile `input` and lower it to vector paths. Pure.
#[must_use]
pub fn compile_note(input: &NoteInput<'_>) -> NoteCompilation {
    let sources = NotebookSources::new(NOTE_FILE, input.main_text());
    let cells = crate::cell::PageCells::default().to_json();
    let Some(world) = NotebookWorld::new(&sources, None, cells, &[]) else {
        return failed("the note could not be read as a Typst file".to_string());
    };
    let warned = typst::compile::<PagedDocument>(&world);
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let document = match warned.output {
        Ok(document) => Some(document),
        Err(errors) => {
            diagnostics.extend(errors.iter().map(|d| crate::compile::diagnostic(&world, d)));
            None
        }
    };
    diagnostics.extend(
        warned
            .warnings
            .iter()
            .map(|d| crate::compile::diagnostic(&world, d)),
    );
    for diagnostic in &mut diagnostics {
        in_note_lines(diagnostic);
    }
    diagnostics.sort_by_key(|d| (d.severity != DiagnosticSeverity::Error, d.line, d.column));
    let drawing = document.and_then(|document| match draw::lower(&document) {
        Ok(drawing) => Some(drawing),
        Err(error) => {
            diagnostics.insert(0, error_at_top(error));
            None
        }
    });
    NoteCompilation {
        drawing,
        diagnostics,
    }
}

/// A diagnostic of the compiled file counted in the note's own lines; one in
/// the lines the compile put first names the note's first line.
fn in_note_lines(diagnostic: &mut Diagnostic) {
    if diagnostic.file != NOTE_FILE || diagnostic.line == 0 {
        return;
    }
    if diagnostic.line > PREAMBLE_LINES {
        diagnostic.line -= PREAMBLE_LINES;
    } else {
        diagnostic.line = 1;
        diagnostic.column = 1;
    }
}

fn error_at_top(message: String) -> Diagnostic {
    Diagnostic {
        severity: DiagnosticSeverity::Error,
        file: NOTE_FILE.to_string(),
        line: 1,
        column: 1,
        message,
        hints: Vec::new(),
    }
}

fn failed(message: String) -> NoteCompilation {
    NoteCompilation {
        drawing: None,
        diagnostics: vec![error_at_top(message)],
    }
}
