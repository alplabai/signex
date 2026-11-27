//! The Import Library dialog (mockup row 211, #3117–#3120): File ▸ Import ▸
//! Library… turns a library made in another tool into a native library
//! through an installed library converter, in three steps.
//!
//! 1. Source and options ([`source`]): the files, the converter that reads
//!    them, the database override, name, folder, mount scope, 3D models.
//! 2. Preview ([`preview`]): a `--dry-run` into a temp folder; its counts
//!    and report items.
//! 3. Import ([`progress`]): the run's phases, bar and current item, and
//!    Cancel, which stops after the current item and removes what was
//!    written. A finished import ends on its result card and loss report
//!    ([`result`]); the same report opens later from the Library Browser.
//!
//! The state is a pure model; the handlers are in
//! `app/dispatch/library/import_dialog/` and the run itself goes through
//! `Signex::start_library_import`. See docs/architecture/library-dialogs.md,
//! "Import Library dialog".

use std::path::PathBuf;

use signex_i18n::Translatable;

use crate::library::messages::{LibraryImportMsg, LibraryMessage};
use crate::library_import::report::Severity;
use crate::library_import::{Discovery, DiscoveryCache, RunOutcome};

pub mod notices;
pub mod preview;
pub mod progress;
pub mod result;
pub mod source;
mod view;

#[cfg(test)]
mod notices_tests;
#[cfg(test)]
mod report_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod update_tests;

#[cfg(all(test, unix))]
mod flow_tests;

pub use view::{ImportDialogContext, view};

/// Width and height of the dialog card in the main window.
pub(crate) const CARD_W: f32 = 760.0;
pub(crate) const CARD_H: f32 = 620.0;

/// The step the dialog shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Source,
    Preview,
    Import,
}

/// What the source picker offers: library files (several), or one
/// database link file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Files,
    Database,
}

/// The dry run of step 2.
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewState {
    /// Running into the temp folder.
    Running,
    /// Finished: what the converter would write.
    Ready(Box<RunOutcome>),
    /// It failed; why, and the converter's last error output.
    Failed(Box<notices::RunProblem>),
}

/// What became of the library folder after a run that wrote no library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderAfter {
    /// The folder the run created was removed.
    Removed,
    /// Files the run wrote may remain.
    Kept,
    /// An update's library was put back to its last import.
    Restored,
    /// An update's library could not be put back; why.
    NotRestored(String),
}

impl FolderAfter {
    pub fn of(failure: &crate::library_import::RunFailure) -> Self {
        match &failure.restored {
            Some(Ok(())) => Self::Restored,
            Some(Err(why)) => Self::NotRestored(why.clone()),
            None if failure.removed_output => Self::Removed,
            None => Self::Kept,
        }
    }
}

/// The import of step 3, once it ended without a library.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportEnd {
    /// Cancelled, and what became of the folder.
    Cancelled { folder: FolderAfter },
    /// It failed; the reason, what became of the folder, and the end of
    /// the converter's error output.
    Failed {
        reason: Translatable,
        folder: FolderAfter,
        stderr_tail: String,
    },
    /// Refused before the converter started: nothing was changed.
    Refused { reason: Translatable },
}

/// What the folder check behind Next and Import found.
#[derive(Debug, Clone, PartialEq)]
pub enum TargetCheck {
    /// Absent or empty: a new import takes it.
    Free,
    /// It cannot take the library; why.
    Refused(Translatable),
    /// It holds an imported library with no uncommitted changes, so an
    /// update can run over it; `local_commits` are commits since the last
    /// import that are not imports.
    Imported { local_commits: usize },
}

/// The open Import Library dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDialogState {
    pub step: Step,
    /// `None` while the converters are looked for.
    pub discovery: Option<Discovery>,
    pub source_kind: SourceKind,
    pub sources: Vec<PathBuf>,
    /// The converter chosen when several read the sources, by its path.
    pub converter: Option<PathBuf>,
    /// The database connection override, as typed (empty: the link's own).
    pub database: String,
    pub name: String,
    /// The name was typed, so a new pick no longer renames the library.
    pub name_edited: bool,
    pub location: String,
    /// The location was changed by hand, so it no longer follows the name.
    pub location_edited: bool,
    pub mount_for_all: bool,
    /// The project the dialog was opened for (Project tab ▸ Libraries):
    /// "This project" means it rather than the active project.
    pub project: Option<crate::app::ProjectId>,
    pub include_models: bool,
    /// Why the chosen folder cannot take the library (from the check Next
    /// runs); cleared by any edit.
    pub target_problem: Option<Translatable>,
    /// The chosen folder holds an imported library: the update offered,
    /// and whether it was taken (#3123). Cleared by any edit.
    pub update_offer: Option<source::UpdateOffer>,
    /// Next ran the folder check and waits for its answer.
    pub checking_target: bool,
    /// The run the folder check leads to.
    pub pending_run: Option<crate::library_import::RunMode>,
    /// The temp folder of the dry run, removed when the dialog moves on.
    pub preview_dir: Option<PathBuf>,
    pub preview: Option<PreviewState>,
    pub filter: String,
    /// Report severities the item list hides.
    pub hidden: Vec<Severity>,
    /// The report item selected in the list, by its index in the report.
    pub selected_item: Option<usize>,
    /// The run this dialog started and waits for.
    pub awaiting: Option<crate::library_import::RunMode>,
    /// Cancel was pressed while an item was in flight: the run stops at
    /// the next item boundary.
    pub cancel_pending: bool,
    /// The run was told to stop now (a second Cancel, or no item in
    /// flight); the footer has nothing left to press but waits for it.
    pub stop_requested: bool,
    /// A run the dialog no longer waits for is still stopping; Next waits.
    pub run_stopping: bool,
    /// A library import the dialog did not start is running; Next waits.
    pub other_run_active: bool,
    pub phases: Vec<progress::PhaseLine>,
    pub import_end: Option<ImportEnd>,
    /// The finished import (or a stored report) step 3 ends on.
    pub result: Option<Box<result::ImportResult>>,
    pub result_tab: result::ResultTab,
    /// What the last Save report… or Open report did.
    pub report_note: Option<result::ReportNote>,
}

impl ImportDialogState {
    /// A fresh dialog, converters not looked for yet.
    pub fn new() -> Self {
        Self {
            step: Step::Source,
            discovery: None,
            source_kind: SourceKind::Files,
            sources: Vec::new(),
            converter: None,
            database: String::new(),
            name: String::new(),
            name_edited: false,
            location: String::new(),
            location_edited: false,
            mount_for_all: true,
            project: None,
            include_models: true,
            target_problem: None,
            update_offer: None,
            checking_target: false,
            pending_run: None,
            preview_dir: None,
            preview: None,
            filter: String::new(),
            hidden: Vec::new(),
            selected_item: None,
            awaiting: None,
            cancel_pending: false,
            stop_requested: false,
            run_stopping: false,
            other_run_active: false,
            phases: Vec::new(),
            import_end: None,
            result: None,
            result_tab: result::ResultTab::Result,
            report_note: None,
        }
    }
}

impl Default for ImportDialogState {
    fn default() -> Self {
        Self::new()
    }
}

/// Events of the Import Library dialog.
#[derive(Debug, Clone)]
pub enum ImportDialogMsg {
    /// File ▸ Import ▸ Library…, the palette, or the Library Browser's
    /// Import… was chosen.
    Requested,
    /// Import library… in a project's Libraries section: "This project"
    /// is preselected, meaning that project.
    RequestedForProject(crate::app::ProjectId),
    /// The converter search finished on its worker; `None` when the worker
    /// itself failed (shown as no converter found).
    DiscoveryFinished(Option<Box<DiscoveryCache>>),
    /// "Look again" was pressed in the not-installed state.
    RescanPressed,
    SourceKindPicked(SourceKind),
    BrowsePressed,
    /// The source picker closed; `None` when dismissed.
    SourcesPicked(Option<Vec<PathBuf>>),
    /// A converter chip was pressed (several read the sources).
    ConverterPicked(PathBuf),
    DatabaseEdited(String),
    DatabaseBrowsePressed,
    DatabasePicked(Option<PathBuf>),
    NameEdited(String),
    LocationEdited(String),
    LocationBrowsePressed,
    LocationPicked(Option<PathBuf>),
    MountForAllToggled,
    ModelsToggled,
    NextPressed,
    /// The folder check behind Next or Import finished.
    TargetChecked {
        location: PathBuf,
        check: TargetCheck,
    },
    /// Update this library was pressed on the offer in step 1.
    UpdateAccepted,
    BackPressed,
    ImportPressed,
    FilterEdited(String),
    SeverityToggled(Severity),
    ItemSelected(usize),
    /// The footer's Cancel: closes steps 1 and 2, cancels a running import.
    CancelPressed,
    /// The header close, Esc or the window's close button.
    Closed,
    /// Result or Loss report was picked at the end of step 3.
    ResultTabPicked(result::ResultTab),
    SaveReportPressed,
    /// The save picker closed; `None` when dismissed.
    ReportSavePicked(Option<PathBuf>),
    /// The report copy finished: the file written, or why not.
    ReportSaved(Result<PathBuf, (PathBuf, String)>),
    OpenReportPressed,
    /// The viewer launch for the report finished; why it failed.
    ReportOpened(Result<(), String>),
    OpenInBrowserPressed,
    /// A library row in the Library Browser was right-clicked: its menu
    /// opened, or closed when it was open. By the library's `.snxlib`.
    LibraryMenuToggled(PathBuf),
    /// View import report in that menu was pressed.
    StoredReportRequested(PathBuf),
    /// The stored report of the library `library_file` was read.
    StoredReportRead {
        library_file: PathBuf,
        report: Result<Box<crate::library_import::ImportReport>, String>,
    },
}

/// The message the `import_library` command sends.
pub(crate) fn requested_message() -> crate::app::Message {
    crate::app::Message::Library(msg(ImportDialogMsg::Requested))
}

/// `m` as the library message the dialog's widgets send.
pub(crate) fn msg(m: ImportDialogMsg) -> LibraryMessage {
    LibraryMessage::Import(LibraryImportMsg::Dialog(m))
}
