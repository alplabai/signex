//! Generate design notebook (#2945, mockup row 201): one catalog command,
//! `generate_design_notebook`, reached from command search, the options
//! dialog and the command line (`signex notebook generate`), all running
//! [`Signex::start_notebook_generate`]. The checks, the BOM and the notebook
//! reads run off the UI thread (`job`); the report itself is built by
//! `signex_notebook::generate`. In the app the report waits unsaved
//! ([`NotebookGenerateUi::pending`]); with `save` it is written to
//! `notebook/generated/design-report.snxnbk`, never over an existing one
//! unless `update`. Design note: `docs/DESIGN_NOTEBOOK_GENERATE.md`.

mod job;
mod request;
mod update;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use signex_notebook::generate::{
    ChecksSource, GenerateOptions, GeneratedNotebook, Section, Template,
};
use signex_types::check::CheckReport;

use crate::app::agent::WaitingCommand;

pub(crate) use update::generate_design_notebook;

/// The command line that does what the dialog does, as its footer names it.
pub const GENERATE_COMMAND_LINE: &str = "signex notebook generate";

impl From<NotebookGenerateMsg> for crate::app::Message {
    fn from(msg: NotebookGenerateMsg) -> Self {
        crate::app::NotebookMsg::Generate(msg).into()
    }
}

/// What happened in Generate design notebook, namespaced under
/// `ProjectMsg::Notebook(NotebookMsg::Generate(..))`.
#[derive(Debug, Clone)]
pub enum NotebookGenerateMsg {
    /// The command ran with no operands: the options dialog opens for the
    /// project in view.
    DialogOpened,
    VariantPicked(String),
    TemplatePicked(Template),
    DocumentNumberEdited(String),
    RevisionEdited(String),
    SectionToggled(Section),
    ChecksSourcePicked(ChecksSource),
    GenerateClicked,
    DialogCancelled,
    /// The off-thread run finished.
    Generated(Box<GenerateFinished>),
}

/// One run of Generate: the project and what to make of it.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateRequest {
    /// The project file.
    pub project: PathBuf,
    pub options: GenerateOptions,
    /// The variant the BOM is built for; `None`: the project's active one.
    pub variant: Option<String>,
    /// Write `notebook/generated/design-report.snxnbk`.
    pub save: bool,
    /// Write over a report that is already there.
    pub update: bool,
}

/// What a run made: the report, and the check runs it made on the way.
#[derive(Debug, Clone)]
pub struct GenerateOutcome {
    pub notebook: GeneratedNotebook,
    /// Each check run now, with the file it checked (the project, or the
    /// board for DRC), kept as the newest of its check.
    pub ran: Vec<(PathBuf, CheckReport)>,
}

/// A finished run, and the command call it answers, if any.
#[derive(Debug, Clone)]
pub struct GenerateFinished {
    pub request: GenerateRequest,
    /// The command call waiting for this run; `None` for the dialog.
    pub(crate) waiting: Option<WaitingCommand>,
    pub outcome: Result<GenerateOutcome, String>,
}

/// The options dialog, while open.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateDialog {
    /// The project file.
    pub project: PathBuf,
    /// The project file's name, for the header.
    pub project_file: String,
    pub revision: Option<String>,
    /// The project's requirements, counted for the section hint.
    pub requirements: usize,
    pub variants: Vec<String>,
    pub variant: String,
    pub options: GenerateOptions,
    /// The document number and revision as typed.
    pub document_number: String,
    pub document_revision: String,
    /// A run started from the dialog is under way.
    pub running: bool,
    /// Why the last run made nothing.
    pub problem: Option<String>,
}

impl GenerateDialog {
    /// The request the dialog's Generate makes: never saved, since the
    /// report opens unsaved (Save writes it).
    pub fn request(&self) -> GenerateRequest {
        let typed = |text: &str| Some(text.trim().to_string()).filter(|t| !t.is_empty());
        GenerateRequest {
            project: self.project.clone(),
            options: GenerateOptions {
                document: typed(&self.document_number),
                revision: typed(&self.document_revision),
                ..self.options.clone()
            },
            variant: Some(self.variant.clone()),
            save: false,
            update: false,
        }
    }

    /// Generate is offered with at least one section and no run under way.
    pub fn can_generate(&self) -> bool {
        !self.running && !self.options.sections.is_empty()
    }
}

/// The slice's state in the Model.
#[derive(Debug, Clone, Default)]
pub struct NotebookGenerateUi {
    pub dialog: Option<GenerateDialog>,
    /// The newest report made in the app, unsaved, for the notebook tab to
    /// open; its Save writes it.
    pub pending: Option<GeneratedNotebook>,
}
