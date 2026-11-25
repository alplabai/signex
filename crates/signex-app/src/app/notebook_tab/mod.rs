//! The Design Notebook tab (#2795, mockup row 195): the fourth document
//! mode, beside sheets, boards and libraries. The project's
//! `notebook/design.snxnbk` in a Typst code editor with line numbers,
//! highlighting and gutter marks, its compiled pages beside it (recompiled
//! as you type, the last good compile kept under an error), an Outline,
//! Pages and References side panel, the insert toolbar, the Problems strip
//! and the compile status in the footer. With no notebook yet the tab shows
//! the empty state: two templates and Import .typ; nothing is written to
//! disk before Save. Behaviour: `docs/DESIGN_NOTEBOOK_TAB.md`.
//!
//! The file, the outline, the highlighting, the insert text and the compile
//! are `signex-notebook`'s; this module holds the tab's state, runs the
//! reads, compiles and saves in tasks, and draws.

mod commands;
mod compile;
mod edit;
mod highlighter;
mod keys;
mod open;
mod picker;
mod save;
mod state;
mod update;
mod view;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_edit;
#[cfg(test)]
mod tests_view;

use std::path::PathBuf;
use std::sync::Arc;

use iced::widget::text_editor;
use signex_notebook::{Insertion, Template};

pub(crate) use commands::{notebook_compile, notebook_insert, notebook_view};
pub use compile::{CompileOutcome, CompileState, ShownPage};
pub use open::NotebookRead;
pub use picker::Picker;
pub use state::{Load, NotebookTab, NotebookText, SidePanel, ViewMode};

use crate::app::state::ProjectId;

/// What happened on a project's Design Notebook. Namespaced under
/// `ProjectMsg::Notebook`; each names its project, so an edit never lands
/// on a project that replaced it. The project-level requests (open,
/// generate, export, badges) are resolved by `notebook_project`.
#[derive(Debug, Clone)]
pub enum NotebookMsg {
    /// Project ▸ Design Notebook, Add New ▸ Design Notebook, a Projects
    /// row or a command: open or focus the notebook tab of `project`, or of
    /// the active document's project.
    OpenRequested {
        project: Option<ProjectId>,
    },
    /// Project ▸ Design Notebook ▸ Generate from Project… or the tab's
    /// Generate button (mockup row 201).
    GenerateRequested {
        project: Option<ProjectId>,
    },
    /// File ▸ Export ▸ Design Notebook PDF… or the tab's Export PDF…
    /// button (mockup row 202).
    ExportPdfRequested {
        project: Option<ProjectId>,
    },
    /// View ▸ Notebook Badges (mockup row 197).
    BadgesToggled,
    /// Generate design notebook (#2945, mockup row 201): its options dialog
    /// and its runs, routed to the `notebook_generate` slice.
    Generate(crate::app::NotebookGenerateMsg),
    /// The read of the notebook file and its chapters finished.
    Read {
        project_id: ProjectId,
        result: Result<Arc<NotebookRead>, String>,
    },
    /// Something happened in the code editor: a key, a click, a paste.
    TextActed {
        project_id: ProjectId,
        action: text_editor::Action,
    },
    /// Source, Split or Preview was picked.
    ViewPicked {
        project_id: ProjectId,
        view: ViewMode,
    },
    /// Ctrl+\ on the tab: the next of Source, Split and Preview.
    ViewCycled {
        project_id: ProjectId,
    },
    /// Outline, Pages or References was picked.
    PanelPicked {
        project_id: ProjectId,
        panel: SidePanel,
    },
    /// An outline row, a reference row or a Problems row was pressed: the
    /// cursor moves to its line in the notebook file.
    LinePicked {
        project_id: ProjectId,
        line: u32,
    },
    /// A toolbar button or a Block menu row: insert at the cursor.
    InsertPressed {
        project_id: ProjectId,
        insertion: Insertion,
    },
    /// The Block button: its menu (Note, Issue, Decision) opens or closes.
    BlockMenuToggled {
        project_id: ProjectId,
    },
    /// Insert reference, or `@` typed in the editor: the picker opens.
    PickerOpened {
        project_id: ProjectId,
    },
    PickerQueryChanged {
        project_id: ProjectId,
        query: String,
    },
    /// Up or Down in the picker.
    PickerMoved {
        project_id: ProjectId,
        down: bool,
    },
    /// Enter in the picker, or a row pressed: insert the row's call.
    PickerChosen {
        project_id: ProjectId,
        row: Option<usize>,
    },
    PickerClosed {
        project_id: ProjectId,
    },
    /// A compile finished.
    Compiled {
        project_id: ProjectId,
        outcome: Arc<CompileOutcome>,
    },
    /// The page navigator: the previous or the next page.
    PageStepped {
        project_id: ProjectId,
        forward: bool,
    },
    /// A page thumbnail was pressed, 0-based.
    PagePicked {
        project_id: ProjectId,
        page: usize,
    },
    /// A page drawn for the preview after a page change landed.
    PageDrawn {
        project_id: ProjectId,
        generation: u64,
        page: usize,
        image: Arc<signex_notebook::PageImage>,
    },
    /// F8, or Go to error: the cursor moves to the next problem.
    NextProblemRequested {
        project_id: ProjectId,
    },
    /// Save, or Ctrl+S on the tab.
    SavePressed {
        project_id: ProjectId,
    },
    /// A save finished; `written` is the file text it wrote.
    Saved {
        project_id: ProjectId,
        written: String,
        result: Result<(), String>,
    },
    /// The empty state: a template card was picked.
    TemplateSelected {
        project_id: ProjectId,
        template: Template,
    },
    /// The empty state's Create from template: a new notebook from
    /// `template`, unsaved.
    TemplatePicked {
        project_id: ProjectId,
        template: Template,
    },
    /// The empty state's Import .typ files…: the picker opens.
    ImportPressed {
        project_id: ProjectId,
    },
    /// The picker closed (`None`: cancelled).
    ImportPicked {
        project_id: ProjectId,
        paths: Option<Vec<PathBuf>>,
    },
    /// The picked `.typ` files were read, in order.
    ImportRead {
        project_id: ProjectId,
        result: Result<String, String>,
    },
    /// `notebook_compile`'s compile finished: answer the caller waiting on
    /// it.
    CommandCompiled {
        project_id: ProjectId,
        waiting: Box<crate::app::agent::WaitingCommand>,
        path: PathBuf,
        answer: Arc<Result<serde_json::Value, String>>,
    },
    /// Esc on the tab: the picker, else the Block menu, closes.
    EscapePressed {
        project_id: ProjectId,
    },
}

impl From<NotebookMsg> for crate::app::Message {
    fn from(msg: NotebookMsg) -> Self {
        crate::app::Message::Project(crate::app::ProjectMsg::Notebook(msg))
    }
}

impl NotebookMsg {
    /// The project the message is for; `None` for a project-level request
    /// that names none, and for the badges toggle.
    pub fn project_id(&self) -> Option<ProjectId> {
        use NotebookMsg as M;
        match self {
            M::OpenRequested { project }
            | M::GenerateRequested { project }
            | M::ExportPdfRequested { project } => *project,
            M::BadgesToggled | M::Generate(_) => None,
            M::Read { project_id, .. }
            | M::TextActed { project_id, .. }
            | M::ViewPicked { project_id, .. }
            | M::ViewCycled { project_id }
            | M::PanelPicked { project_id, .. }
            | M::LinePicked { project_id, .. }
            | M::InsertPressed { project_id, .. }
            | M::BlockMenuToggled { project_id }
            | M::PickerOpened { project_id }
            | M::PickerQueryChanged { project_id, .. }
            | M::PickerMoved { project_id, .. }
            | M::PickerChosen { project_id, .. }
            | M::PickerClosed { project_id }
            | M::Compiled { project_id, .. }
            | M::PageStepped { project_id, .. }
            | M::PagePicked { project_id, .. }
            | M::PageDrawn { project_id, .. }
            | M::NextProblemRequested { project_id }
            | M::SavePressed { project_id }
            | M::Saved { project_id, .. }
            | M::TemplateSelected { project_id, .. }
            | M::TemplatePicked { project_id, .. }
            | M::ImportPressed { project_id }
            | M::ImportPicked { project_id, .. }
            | M::ImportRead { project_id, .. }
            | M::CommandCompiled { project_id, .. }
            | M::EscapePressed { project_id } => Some(*project_id),
        }
    }
}
