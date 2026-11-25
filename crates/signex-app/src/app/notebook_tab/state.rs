//! The notebook tab's state, kept per project for as long as the project is
//! loaded: closing the tab keeps unsaved text, which the tab shows again
//! when it reopens and which Save All and the close and quit prompts see
//! (the path is in the dirty set while the text differs from the file).

use std::path::PathBuf;
use std::sync::Arc;

use iced::widget::text_editor;
use signex_notebook::{Highlights, NotebookFile, NotebookSources, Outline, Template, outline};

use super::compile::CompileState;
use super::picker::Picker;

/// Source, Split or Preview (mockup row 195). Split is the view a tab opens
/// in (decided as drawn).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    Source,
    #[default]
    Split,
    Preview,
}

impl ViewMode {
    pub const ALL: [ViewMode; 3] = [ViewMode::Source, ViewMode::Split, ViewMode::Preview];

    /// The next view, as Ctrl+\ cycles them.
    pub fn next(self) -> Self {
        match self {
            ViewMode::Source => ViewMode::Split,
            ViewMode::Split => ViewMode::Preview,
            ViewMode::Preview => ViewMode::Source,
        }
    }

    /// The token a command names the view by.
    pub fn token(self) -> &'static str {
        match self {
            ViewMode::Source => "source",
            ViewMode::Split => "split",
            ViewMode::Preview => "preview",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|view| view.token() == token)
    }

    pub fn shows_source(self) -> bool {
        self != ViewMode::Preview
    }

    pub fn shows_preview(self) -> bool {
        self != ViewMode::Source
    }
}

/// The side panel: the outline of the document, page thumbnails, or the
/// references it makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidePanel {
    #[default]
    Outline,
    Pages,
    References,
}

impl SidePanel {
    pub const ALL: [SidePanel; 3] = [SidePanel::Outline, SidePanel::Pages, SidePanel::References];

    /// The token a command names the panel by.
    pub fn token(self) -> &'static str {
        match self {
            SidePanel::Outline => "outline",
            SidePanel::Pages => "pages",
            SidePanel::References => "references",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|panel| panel.token() == token)
    }
}

/// Where the notebook file stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Load {
    /// The read is running.
    Reading,
    /// There is no notebook file yet: the empty state.
    Missing,
    /// The file is open in the editor.
    Open,
    /// The file is there and could not be read; nothing is edited, so
    /// nothing can overwrite it.
    Failed(String),
}

/// The text in the editor (the body after the header) and what is read
/// from it after each edit, in `update`.
pub struct NotebookText {
    pub content: text_editor::Content,
    /// The Typst colours of every line.
    pub highlights: Arc<Highlights>,
    /// The outline of the text as typed, chapters included.
    pub outline: Outline,
    /// Bumped on every edit; a compile reports the one it read.
    pub generation: u64,
}

impl NotebookText {
    pub fn new(body: &str) -> Self {
        NotebookText {
            content: text_editor::Content::with_text(body),
            highlights: Arc::new(Highlights::of(body)),
            outline: Outline::default(),
            generation: 0,
        }
    }

    /// The body as typed.
    pub fn body(&self) -> String {
        self.content.text()
    }

    /// The cursor's 0-based line and column (bytes) in the body.
    pub fn cursor(&self) -> (usize, usize) {
        let position = self.content.cursor().position;
        (position.line, position.index)
    }
}

impl std::fmt::Debug for NotebookText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotebookText")
            .field("lines", &self.content.line_count())
            .field("generation", &self.generation)
            .finish()
    }
}

/// One project's notebook tab.
#[derive(Debug)]
pub struct NotebookTab {
    /// The notebook file: `<project>/notebook/design.snxnbk`.
    pub path: PathBuf,
    pub load: Load,
    /// The file as read or last saved; `None` for a notebook not saved yet.
    pub file: Option<NotebookFile>,
    /// The body the file on disk holds; `None` before the first save. The
    /// text is unsaved while it differs.
    pub saved_body: Option<String>,
    pub text: Option<NotebookText>,
    /// How many lines the header takes in the file: the editor shows the
    /// body and numbers its lines from one past them.
    pub header_lines: u32,
    /// The chapters the notebook includes, as read, by path inside the
    /// notebook folder; the notebook file itself is replaced by the text as
    /// typed before each compile.
    pub sources: NotebookSources,
    pub view: ViewMode,
    pub panel: SidePanel,
    pub compile: CompileState,
    /// The page the preview shows, 0-based.
    pub page: usize,
    pub block_menu: bool,
    pub picker: Option<Picker>,
    /// The empty state's template card that is picked.
    pub template: Template,
    /// A save is running.
    pub saving: bool,
    /// Why the last save failed, until the next one.
    pub save_error: Option<String>,
    /// When the last save landed, as the clock read then (`14:02`).
    pub saved_at: Option<String>,
}

impl NotebookTab {
    /// A tab whose file is being read.
    pub fn reading(path: PathBuf) -> Self {
        NotebookTab {
            sources: NotebookSources::new(super::open::MAIN_FILE, Vec::new()),
            path,
            load: Load::Reading,
            file: None,
            saved_body: None,
            text: None,
            header_lines: 0,
            view: ViewMode::default(),
            panel: SidePanel::default(),
            compile: CompileState::default(),
            page: 0,
            block_menu: false,
            picker: None,
            template: Template::DesignReport,
            saving: false,
            save_error: None,
            saved_at: None,
        }
    }

    /// True when the text in the editor is not what the file holds.
    pub fn is_unsaved(&self) -> bool {
        match &self.text {
            Some(text) => self.saved_body.as_deref() != Some(text.body().as_str()),
            None => false,
        }
    }

    /// True when the file was written by a newer Signex: it opens for
    /// viewing and Save is refused.
    pub fn is_read_only(&self) -> bool {
        self.file.as_ref().is_some_and(NotebookFile::is_read_only)
    }

    /// The sources a compile reads: the chapters as read and the notebook
    /// file as typed, its header lines blank so every line keeps its number.
    pub fn compile_sources(&self) -> NotebookSources {
        let mut sources = self.sources.clone();
        if let Some(text) = &self.text {
            let blank = "\n".repeat(self.header_lines as usize);
            let typed = format!("{blank}{}", text.body());
            // The main path is always inside the notebook folder.
            let _inserted = sources.insert(super::open::MAIN_FILE, typed);
        }
        sources
    }

    /// Read the outline and the colours again after the text changed.
    pub fn refresh_text(&mut self) {
        let sources = self.compile_sources();
        if let Some(text) = &mut self.text {
            text.highlights = Arc::new(Highlights::of(&text.body()));
            text.outline = outline(&sources);
            text.generation += 1;
        }
    }

    /// The 1-based line of the file for 0-based body line `line`.
    pub fn file_line(&self, line: usize) -> u32 {
        let line = u32::try_from(line).unwrap_or(u32::MAX);
        line.saturating_add(self.header_lines).saturating_add(1)
    }

    /// The 0-based body line for 1-based file line `line`; `None` for a
    /// line in the header.
    pub fn body_line(&self, line: u32) -> Option<usize> {
        let line = line.checked_sub(self.header_lines + 1)?;
        usize::try_from(line).ok()
    }
}
