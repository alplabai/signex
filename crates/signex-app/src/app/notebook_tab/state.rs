//! The notebook tab's view state, kept per project for as long as the
//! project is loaded, beside the notebook document it shows. The document
//! (the file, the body as typed, the chapters, where the file stands) is a
//! `signex_notebook::NotebookDocument` in `DocumentState::notebooks`; this
//! is the editor, the preview, the panels and the margin lane in
//! `UiState::notebook_tabs`. Closing the tab keeps unsaved text, which Save
//! All and the close and quit prompts see (the path is in the dirty set
//! while the body differs from the file).

use std::collections::HashMap;
use std::sync::Arc;

use iced::widget::text_editor;
use signex_notebook::{
    Highlights, NotebookDocument, Outline, Reference, ReferenceCard, Template, outline,
};

pub use signex_notebook::NotebookLoad as Load;

use super::compile::CompileState;
use super::picker::Picker;
use crate::app::state::ProjectId;

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

    /// Read the outline and the colours again after the body of `doc`
    /// changed.
    pub fn refresh(&mut self, doc: &NotebookDocument) {
        self.highlights = Arc::new(Highlights::of(&self.body()));
        self.outline = outline(&doc.compile_sources());
        self.generation += 1;
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

/// The margin lane's timeline (mockup row 199): the targets it was opened
/// for (its tabs) and the one shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineTarget {
    pub targets: Vec<Reference>,
    pub shown: Reference,
}

/// One project's notebook tab: what the editor, the preview, the side
/// panel and the margin lane show.
#[derive(Debug, Default)]
pub struct NotebookTab {
    /// The editor's text; `None` while the document has no body.
    pub text: Option<NotebookText>,
    pub view: ViewMode,
    pub panel: SidePanel,
    pub compile: CompileState,
    /// The page the preview shows, 0-based.
    pub page: usize,
    pub block_menu: bool,
    pub picker: Option<Picker>,
    /// The empty state's template card that is picked.
    pub template: Template,
    /// The timeline the margin lane shows, if one is open.
    pub timeline: Option<TimelineTarget>,
    /// The reference card the margin lane shows (mockup row 196), with the
    /// notebook's stale count for its Update all.
    pub card: Option<(ReferenceCard, usize)>,
}

impl NotebookTab {
    /// The editor's text for the body of `doc`, when it has one.
    pub fn show_body(&mut self, doc: &NotebookDocument) {
        self.text = doc.body.as_deref().map(NotebookText::new);
        if let Some(text) = &mut self.text {
            text.refresh(doc);
        }
    }
}

/// Every loaded project's notebook tab, boxed so a project with none costs
/// one pointer in `UiState`.
#[derive(Debug, Default)]
pub struct NotebookTabs(HashMap<ProjectId, NotebookTab>);

impl std::ops::Deref for NotebookTabs {
    type Target = HashMap<ProjectId, NotebookTab>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for NotebookTabs {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl crate::app::Signex {
    /// `id`'s notebook document and its tab, for an edit that changes both.
    pub(crate) fn notebook_parts_mut(
        &mut self,
        id: ProjectId,
    ) -> Option<(&mut NotebookDocument, &mut NotebookTab)> {
        let doc = self.document_state.notebooks.get_mut(&id)?;
        let tab = self.ui_state.notebook_tabs.get_mut(&id)?;
        Some((doc, tab))
    }

    /// `id`'s notebook document and its tab.
    pub(crate) fn notebook_parts(
        &self,
        id: ProjectId,
    ) -> Option<(&NotebookDocument, &NotebookTab)> {
        let doc = self.document_state.notebooks.get(&id)?;
        let tab = self.ui_state.notebook_tabs.get(&id)?;
        Some((doc, tab))
    }
}
