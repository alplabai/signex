//! Export Design Notebook PDF (mockup row 202, #2611): the dialog over the
//! project's notebook, its stale-reference check, and the
//! `export_notebook_pdf` command. The export itself is
//! `signex_notebook::export`; this slice reads the notebook files and writes
//! the PDF off the update thread (the export write path, crash-safe) and
//! keeps the dialog's state. Nothing here changes a document, so there is
//! no undo step. Rules: `docs/NOTEBOOK_PDF.md`.

mod dialog;
mod jobs;
mod operands;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use signex_notebook::AnnotationKind;
use signex_notebook::export::{
    ExportOptions, ExportSummary, NotebookExport, NotebookInspection, PageSelection, Paper,
    StaleRule,
};

pub(crate) use operands::export_notebook_pdf;

use crate::app::agent::WaitingCommand;
use crate::app::contracts::{ExportMsg, ExportWriteOutcome};
use crate::app::{Message, ProjectId};

impl From<NotebookPdfMsg> for Message {
    fn from(msg: NotebookPdfMsg) -> Self {
        Message::Export(ExportMsg::NotebookPdf(msg))
    }
}

/// What happened in Export Design Notebook PDF, namespaced under
/// `ExportMsg::NotebookPdf` and routed to `dispatch_notebook_pdf_message`.
#[derive(Debug, Clone)]
pub enum NotebookPdfMsg {
    /// `export_notebook_pdf` in a window, File ▸ Export ▸ Design Notebook
    /// PDF… or the notebook toolbar's Export PDF…: the dialog opens on the
    /// notebook of the project in view. The notebook tab passes the page it
    /// shows.
    Requested {
        current_page: Option<usize>,
    },
    /// The notebook was read and compiled for the dialog, or why it could
    /// not be; an answer for an earlier opening is dropped.
    Inspected {
        generation: u64,
        result: Arc<Result<NotebookInspection, String>>,
    },
    PagesPicked(PageChoice),
    /// The Range field was edited.
    RangeEdited(String),
    PaperPicked(Paper),
    VariantPicked(String),
    IncludeToggled(IncludeFlag),
    KindToggled(AnnotationKind),
    StaleRulePicked(StaleRule),
    /// The Save to field was edited.
    PathEdited(String),
    /// Browse…: the save dialog.
    BrowseRequested,
    /// The save dialog answered with `path`.
    PathChosen {
        generation: u64,
        path: PathBuf,
    },
    /// Review… on the stale banner: each stale reference to Messages.
    ReviewRequested,
    /// Export PDF or Export anyway.
    ExportRequested,
    /// The PDF was made off the update thread, or why it could not be.
    Built {
        request: Box<BuildRequest>,
        result: Arc<Result<BuiltPdf, String>>,
    },
    /// The PDF's write finished.
    Written {
        request: Box<BuildRequest>,
        summary: Box<ExportSummary>,
        outcome: Box<ExportWriteOutcome>,
    },
    /// The done card's Open PDF.
    OpenPdfRequested,
    /// The done card's Show in folder.
    ShowInFolderRequested,
    /// Open PDF or Show in folder did not launch: why.
    OpenFailed(String),
    /// Cancel, Close, Esc or the header's close button.
    Closed,
}

/// The Pages control: every page, the page the notebook tab shows, or the
/// Range field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageChoice {
    All,
    Current,
    Range,
}

/// The Include ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncludeFlag {
    TitleBlock,
    Contents,
    Bookmarks,
    PythonSource,
}

impl IncludeFlag {
    pub const ALL: [Self; 4] = [
        Self::TitleBlock,
        Self::Contents,
        Self::Bookmarks,
        Self::PythonSource,
    ];

    /// The setting in `options` this tick is.
    pub fn get(self, options: &ExportOptions) -> bool {
        match self {
            Self::TitleBlock => options.title_block,
            Self::Contents => options.contents,
            Self::Bookmarks => options.bookmarks,
            Self::PythonSource => options.python_source,
        }
    }

    fn toggle(self, options: &mut ExportOptions) {
        let flag = match self {
            Self::TitleBlock => &mut options.title_block,
            Self::Contents => &mut options.contents,
            Self::Bookmarks => &mut options.bookmarks,
            Self::PythonSource => &mut options.python_source,
        };
        *flag = !*flag;
    }
}

/// Where the dialog is.
#[derive(Debug, Clone)]
pub enum Stage {
    /// The notebook is being read and compiled.
    Reading,
    /// The notebook file could not be read: why.
    Unreadable(String),
    /// Ready to export; the stale banner shows when the inspection found
    /// stale references.
    Ready(Box<NotebookInspection>),
    /// The PDF is being made and written.
    Exporting(Box<NotebookInspection>),
    /// The done card.
    Done(Box<ExportDone>),
}

/// What the done card shows.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportDone {
    pub path: PathBuf,
    pub summary: ExportSummary,
    /// Whether the checksum is the one of the last export to the same file
    /// this session; `None` for the first.
    pub same_as_last: Option<bool>,
}

/// The open dialog.
#[derive(Debug, Clone)]
pub struct NotebookPdfDialog {
    pub project: ProjectId,
    pub project_dir: PathBuf,
    pub project_name: String,
    /// The notebook file, a path inside the project's `notebook/` folder.
    pub notebook: String,
    /// Base and the project's variants, the Variant list.
    pub variants: Vec<String>,
    /// The page the notebook tab showed when the dialog opened, 1-based.
    pub current_page: usize,
    pub choice: PageChoice,
    pub range_text: String,
    pub options: ExportOptions,
    /// The Save to field: a path, relative to the project folder or not.
    pub path_text: String,
    /// The field was edited or browsed: the default name no longer fills it.
    pub path_edited: bool,
    pub stage: Stage,
    /// Numbers this opening; a late answer for an earlier one is dropped.
    pub generation: u64,
}

impl NotebookPdfDialog {
    /// The page selection the Pages control and the Range field make;
    /// `None` while the Range field names no page.
    pub fn page_selection(&self) -> Option<PageSelection> {
        match self.choice {
            PageChoice::All => Some(PageSelection::All),
            PageChoice::Current => Some(PageSelection::Current {
                page: self.current_page,
            }),
            PageChoice::Range => PageSelection::parse(&self.range_text, self.current_page)
                .filter(|selection| matches!(selection, PageSelection::Range { .. })),
        }
    }

    /// The inspection the dialog shows, while there is one.
    pub fn inspection(&self) -> Option<&NotebookInspection> {
        match &self.stage {
            Stage::Ready(inspection) | Stage::Exporting(inspection) => Some(inspection),
            Stage::Reading | Stage::Unreadable(_) | Stage::Done(_) => None,
        }
    }
}

/// The slice's state in `UiState`: the open dialog, the numbering of its
/// openings, and the checksum of the last export of each file this session
/// (the done card says whether a new export matches it).
#[derive(Debug, Clone, Default)]
pub struct NotebookPdfState {
    pub dialog: Option<NotebookPdfDialog>,
    pub generation: u64,
    pub last_checksums: BTreeMap<PathBuf, String>,
}

/// One export under way: where it came from and where it goes.
#[derive(Debug, Clone)]
pub struct BuildRequest {
    pub project_dir: PathBuf,
    pub project_name: String,
    pub notebook: String,
    /// The file; `None` takes the default name once the title block is read.
    pub path: Option<PathBuf>,
    /// A command's `overwrite`: an existing file is replaced only with it.
    /// `None` for the dialog, where the person picked the file.
    pub overwrite: Option<bool>,
    /// A headless or agent run: it reads and writes only inside the
    /// project, through no link (#2172, #3020).
    pub confined: bool,
    pub options: ExportOptions,
    /// The dialog opening it was started from; `None` for the command.
    pub generation: Option<u64>,
    /// The command waiting for the answer; `None` for the dialog.
    pub waiting: Option<WaitingCommand>,
}

/// A made PDF and the file it goes to.
#[derive(Debug, Clone)]
pub struct BuiltPdf {
    pub path: PathBuf,
    pub export: NotebookExport,
}

impl crate::app::Signex {
    pub(crate) fn dispatch_notebook_pdf_message(
        &mut self,
        msg: NotebookPdfMsg,
    ) -> iced::Task<Message> {
        use NotebookPdfMsg as M;
        match msg {
            M::Requested { current_page } => return self.open_notebook_pdf(current_page),
            M::Inspected { generation, result } => {
                self.apply_notebook_inspection(generation, result)
            }
            M::PagesPicked(choice) => self.edit_notebook_pdf(|d| d.choice = choice),
            M::RangeEdited(text) => self.edit_notebook_pdf(|d| {
                d.range_text = text;
                d.choice = PageChoice::Range;
            }),
            M::PaperPicked(paper) => self.edit_notebook_pdf(|d| d.options.paper = paper),
            M::VariantPicked(variant) => self.edit_notebook_pdf(|d| d.options.variant = variant),
            M::IncludeToggled(flag) => self.edit_notebook_pdf(|d| flag.toggle(&mut d.options)),
            M::KindToggled(kind) => self.edit_notebook_pdf(|d| {
                if !d.options.annotations.remove(&kind) {
                    d.options.annotations.insert(kind);
                }
            }),
            M::StaleRulePicked(rule) => self.edit_notebook_pdf(|d| d.options.stale = rule),
            M::PathEdited(text) => self.edit_notebook_pdf(|d| {
                d.path_text = text;
                d.path_edited = true;
            }),
            M::BrowseRequested => return self.browse_notebook_pdf_path(),
            M::PathChosen { generation, path } => self.edit_notebook_pdf(|d| {
                if d.generation == generation {
                    d.path_text = path.display().to_string();
                    d.path_edited = true;
                }
            }),
            M::ReviewRequested => self.review_notebook_stale(),
            M::ExportRequested => return self.export_notebook_pdf_from_dialog(),
            M::Built { request, result } => return self.write_notebook_pdf(*request, result),
            M::Written {
                request,
                summary,
                outcome,
            } => return self.finish_notebook_pdf(*request, *summary, *outcome),
            M::OpenPdfRequested => return self.open_exported_notebook_pdf(false),
            M::ShowInFolderRequested => return self.open_exported_notebook_pdf(true),
            M::OpenFailed(why) => self.report_notebook_pdf_open_failed(&why),
            M::Closed => self.ui_state.notebook_pdf.dialog = None,
        }
        iced::Task::none()
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_operands;
