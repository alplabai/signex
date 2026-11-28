//! What the rich note slice keeps (#1114, mockup row 203): the compiles by
//! hash key, the drawings the canvas shows, and the in-place editor while it
//! is open. All UI state: the document holds only the note's text and its
//! Frame and Size sections.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use iced::widget::text_editor;
use signex_notebook::Diagnostic;
use signex_notebook::note::{NoteCompilation, NoteDrawing, NoteKey};

use crate::schematic_runtime::rich_note::RichNoteRenders;

/// Compiles kept before the cache starts again from empty.
pub(crate) const CACHE_LIMIT: usize = 512;

/// One compile's result.
#[derive(Debug, Clone, Default)]
pub struct NoteOutcome {
    pub drawing: Option<Arc<NoteDrawing>>,
    pub diagnostics: Vec<Diagnostic>,
    /// How long the compile took.
    pub elapsed_ms: u64,
}

impl NoteOutcome {
    /// The outcome of `compiled`, which took `elapsed_ms`.
    #[must_use]
    pub fn of(compiled: NoteCompilation, elapsed_ms: u64) -> Self {
        Self {
            drawing: compiled.drawing.map(Arc::new),
            diagnostics: compiled.diagnostics,
            elapsed_ms,
        }
    }

    /// The first error, which the editor shows under its line.
    #[must_use]
    pub fn first_error(&self) -> Option<&Diagnostic> {
        self.diagnostics
            .iter()
            .find(|d| d.severity == signex_notebook::DiagnosticSeverity::Error)
    }
}

/// The in-place editor over the sheet (mockup row 203): the source on the
/// left, the note as it will draw on the right. Nothing reaches the sheet
/// before Apply.
#[derive(Debug, Clone)]
pub struct RichNoteEditor {
    /// The sheet the note is on.
    pub sheet: Option<PathBuf>,
    pub note: uuid::Uuid,
    /// The note's text when the editor opened: Cancel goes back to it.
    pub original: String,
    pub content: text_editor::Content,
    /// The Typst colours of the text, as the notebook editor reads them.
    pub highlights: Arc<signex_notebook::Highlights>,
    /// Moves with every edit; a compile of an older text is dropped.
    pub generation: u64,
    /// The newest compile of the typed text.
    pub preview: Option<Arc<NoteOutcome>>,
    /// The last compile of the typed text that gave a drawing: the preview
    /// keeps it under an error.
    pub last_good: Option<Arc<NoteDrawing>>,
}

impl RichNoteEditor {
    /// An editor on `note` (`text` as stored), the cursor at byte `at`.
    #[must_use]
    pub fn open(sheet: Option<PathBuf>, note: uuid::Uuid, text: &str, at: usize) -> Self {
        let mut content = text_editor::Content::with_text(text);
        let (line, index) = line_and_index(text, at);
        content.move_to(text_editor::Cursor {
            position: iced::advanced::text::Position { line, index },
            selection: None,
        });
        Self {
            sheet,
            note,
            original: text.to_string(),
            content,
            highlights: Arc::new(signex_notebook::Highlights::of(text)),
            generation: 0,
            preview: None,
            last_good: None,
        }
    }

    /// The text as typed.
    #[must_use]
    pub fn text(&self) -> String {
        self.content.text()
    }

    /// The cursor's 0-based line and byte index in its line.
    #[must_use]
    pub fn cursor(&self) -> (usize, usize) {
        let position = self.content.cursor().position;
        (position.line, position.index)
    }

    /// The text changed: a new generation, the colours read again.
    pub fn edited(&mut self) -> u64 {
        self.generation += 1;
        self.highlights = Arc::new(signex_notebook::Highlights::of(&self.text()));
        self.generation
    }

    /// Whether the typed text differs from the note's.
    #[must_use]
    pub fn is_edited(&self) -> bool {
        self.text() != self.original
    }

    /// The drawing the preview shows: the newest one that compiled.
    #[must_use]
    pub fn shown_drawing(&self) -> Option<&NoteDrawing> {
        self.last_good.as_deref()
    }
}

/// Everything the slice keeps; boxed on `UiState`.
#[derive(Debug, Default)]
pub struct RichNotes {
    /// Compiles by key: an unchanged note never compiles again, and the same
    /// source on many sheets compiles once.
    pub(crate) compiled: HashMap<NoteKey, Arc<NoteOutcome>>,
    /// Keys a running task compiles.
    pub(crate) pending: HashSet<NoteKey>,
    /// What the canvas draws, by note.
    pub renders: RichNoteRenders,
    /// The key whose errors were said in Messages, by note: each failing
    /// text is reported once.
    pub(crate) reported: HashMap<uuid::Uuid, NoteKey>,
    /// The editor, while open.
    pub editor: Option<RichNoteEditor>,
    /// A search hit on a note of a sheet still opening: the editor opens on
    /// it at that byte once the sheet shows.
    pub(crate) open_at: Option<(uuid::Uuid, usize)>,
    /// How many notes were compiled this session.
    pub compiles: u64,
}

impl RichNotes {
    /// Keep `outcome` for `key`; the cache starts again when it is full.
    pub(crate) fn keep(&mut self, key: NoteKey, outcome: Arc<NoteOutcome>) {
        if self.compiled.len() >= CACHE_LIMIT && !self.compiled.contains_key(&key) {
            self.compiled.clear();
        }
        self.pending.remove(&key);
        self.compiled.insert(key, outcome);
    }
}

/// What a toolbar button wraps the selection in (mockup row 203).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Wrap {
    Bold,
    Italic,
    Heading,
    List,
    Math,
    Link,
    Table,
}

impl Wrap {
    pub const ALL: [Self; 7] = [
        Self::Bold,
        Self::Italic,
        Self::Heading,
        Self::List,
        Self::Math,
        Self::Link,
        Self::Table,
    ];

    /// The word an operand and an answer write.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bold => "bold",
            Self::Italic => "italic",
            Self::Heading => "heading",
            Self::List => "list",
            Self::Math => "math",
            Self::Link => "link",
            Self::Table => "table",
        }
    }

    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|wrap| wrap.as_str() == word)
    }

    /// The Typst that wraps `selected`; with nothing selected, the empty
    /// markup to type into (no words, so nothing needs translating).
    #[must_use]
    pub fn around(self, selected: Option<&str>) -> String {
        let text = selected.unwrap_or_default();
        match self {
            Self::Bold => format!("*{text}*"),
            Self::Italic => format!("_{text}_"),
            Self::Heading => format!("= {text}"),
            Self::List if text.is_empty() => "- ".to_string(),
            Self::List => text
                .lines()
                .map(|line| format!("- {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
            Self::Math => format!("${text}$"),
            Self::Link => format!("#link(\"{text}\")"),
            Self::Table if text.is_empty() => "#table(columns: 2, [], [])".to_string(),
            Self::Table => format!("#table(columns: 1, [{text}])"),
        }
    }
}

/// The 0-based line and byte index in that line of byte `at` of `text`
/// (clamped to the text, and to a character boundary).
#[must_use]
pub fn line_and_index(text: &str, at: usize) -> (usize, usize) {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    let before = &text[..at];
    let line = before.matches('\n').count();
    let start = before.rfind('\n').map_or(0, |i| i + 1);
    (line, at - start)
}
