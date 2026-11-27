//! The Export Job document tab (#1299, mockup row 114): it edits one
//! `.snxout` as a document (a dirty dot, Ctrl+S, Save All, the quit gate)
//! and runs it. The job itself is the draft in `DocumentState::output_jobs`;
//! what the tab shows of it (the page, the picked output and profile, the
//! open menus) lives here in [`JobTabUi`]. Edits are in `edit.rs`, runs and
//! the summary's actions in `actions.rs`. Rules: docs/OUTPUT_JOBS.md, "The
//! Export Job tab".

mod actions;
pub(super) mod edit;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use iced::Task;
use signex_types::output_job::{
    BeforeClose, GerberApertures, GerberFormat, GerberUnits, GerberZeros, OnErrors, OnWarnings,
    OutputKind,
};

use super::auto::AutoRuns;
use super::document::SaveSlots;
use super::pipeline::RunBook;
use crate::app::{Message, Signex};

/// What the tab's body shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabPage {
    /// The outputs, the picked output's form and the container.
    #[default]
    Settings,
    /// The profile's triggers, the command line and the recent auto runs.
    Automation,
    /// The run as a flow of nodes.
    Flow,
    /// The last run's summary and the history.
    Summary,
}

/// A profile name being typed: for Save as profile… or Rename….
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamingMode {
    SaveAs,
    Rename,
}

/// What one Export Job tab shows of its job.
#[derive(Debug, Clone, Default)]
pub struct JobTabUi {
    pub page: TabPage,
    /// The output whose form shows, by name.
    pub selected: Option<String>,
    /// The profile shown; the job's default (or first) when `None`.
    pub profile: Option<String>,
    pub profile_menu: bool,
    pub add_menu: bool,
    /// The profile name being typed, and why.
    pub naming: Option<(NamingMode, String)>,
    /// The read of the file is under way.
    pub loading: bool,
    /// Why the file could not be read.
    pub read_error: Option<String>,
    /// The flow node whose log shows: `None` for Validate.
    pub flow_node: Option<usize>,
    /// The history row whose summary shows; the newest run when `None`.
    pub history_pick: Option<usize>,
    /// A run asked for while the job had unsaved edits: it starts when the
    /// save lands (`true`: Run Selected).
    pub run_after_save: Option<bool>,
}

/// Everything the Export Job slice keeps outside the documents: the tabs'
/// view state, the runs, the writes in flight and the automation.
#[derive(Default)]
pub struct OutputJobUi {
    pub tabs: HashMap<PathBuf, JobTabUi>,
    pub runs: RunBook,
    pub(crate) saves: SaveSlots,
    pub auto: AutoRuns,
}

/// A trigger edit of the shown profile (mockup row 114, "Automation").
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerEdit {
    OnSaveToggled,
    SourcesEdited(String),
    WaitEdited(String),
    KeepUpToDateToggled,
    BeforeClosePicked(BeforeClose),
    TagEdited(String),
}

/// An edit of a Gerber output's settings (#1299, mockup row 114, the
/// Gerber output's form): one per setting.
#[derive(Debug, Clone, PartialEq)]
pub enum GerberEdit {
    UnitsPicked(GerberUnits),
    FormatPicked(GerberFormat),
    ZerosPicked(GerberZeros),
    /// A layer id ticked or unticked in the list; `None` is "Every layer".
    LayerToggled(Option<String>),
    AperturesPicked(GerberApertures),
}

/// A switch of the shown profile's container (mockup row 114, "Output
/// container").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagingSwitch {
    Zip,
    GroupFolders,
    ReleaseFolder,
    ClearFolder,
    Validate,
}

/// What happened on an Export Job tab, namespaced under
/// `OutputJobMsg::Tab`. Each names the tab's job file.
#[derive(Debug, Clone)]
pub enum JobTabMsg {
    PagePicked {
        job: PathBuf,
        page: TabPage,
    },
    OutputPicked {
        job: PathBuf,
        name: String,
    },
    OutputEnabledToggled {
        job: PathBuf,
        name: String,
    },
    OutputTicked {
        job: PathBuf,
        name: String,
    },
    AddMenuToggled {
        job: PathBuf,
    },
    OutputAdded {
        job: PathBuf,
        kind: OutputKind,
    },
    OutputRemoved {
        job: PathBuf,
        name: String,
    },
    OutputFileEdited {
        job: PathBuf,
        name: String,
        text: String,
    },
    OutputDpiEdited {
        job: PathBuf,
        name: String,
        text: String,
    },
    GerberEdited {
        job: PathBuf,
        name: String,
        edit: GerberEdit,
    },
    OdbFolderToggled {
        job: PathBuf,
        name: String,
    },
    ProfileMenuToggled {
        job: PathBuf,
    },
    ProfilePicked {
        job: PathBuf,
        name: String,
    },
    ProfileNamingStarted {
        job: PathBuf,
        mode: NamingMode,
    },
    ProfileNameEdited {
        job: PathBuf,
        text: String,
    },
    ProfileNamingConfirmed {
        job: PathBuf,
    },
    ProfileNamingCancelled {
        job: PathBuf,
    },
    ProfileDuplicated {
        job: PathBuf,
    },
    ProfileDeleted {
        job: PathBuf,
    },
    ProfileMadeDefault {
        job: PathBuf,
    },
    VariantPicked {
        job: PathBuf,
        variant: String,
    },
    FolderEdited {
        job: PathBuf,
        text: String,
    },
    NamingEdited {
        job: PathBuf,
        text: String,
    },
    PackagingToggled {
        job: PathBuf,
        switch: PackagingSwitch,
    },
    OnErrorsPicked {
        job: PathBuf,
        policy: OnErrors,
    },
    OnWarningsPicked {
        job: PathBuf,
        policy: OnWarnings,
    },
    TriggerEdited {
        job: PathBuf,
        edit: TriggerEdit,
    },
    RunPressed {
        job: PathBuf,
        selected: bool,
    },
    StopPressed {
        job: PathBuf,
    },
    AnswerPressed {
        job: PathBuf,
        go_on: bool,
    },
    AskNextTimePressed {
        job: PathBuf,
    },
    ShowInCheckPressed {
        job: PathBuf,
    },
    FlowNodePicked {
        job: PathBuf,
        node: Option<usize>,
    },
    HistoryPicked {
        job: PathBuf,
        index: usize,
    },
    OpenFolderPressed {
        job: PathBuf,
    },
    CopyPathPressed {
        job: PathBuf,
    },
    CopyCommandPressed {
        job: PathBuf,
    },
    ViewInGerberPressed {
        job: PathBuf,
    },
    FolderOpenFailed {
        folder: PathBuf,
        why: String,
    },
}

impl From<JobTabMsg> for Message {
    fn from(msg: JobTabMsg) -> Self {
        Message::from(super::OutputJobMsg::Tab(msg))
    }
}

impl Signex {
    pub(crate) fn output_jobs(&self) -> &OutputJobUi {
        &self.ui_state.output_jobs
    }

    pub(crate) fn output_jobs_mut(&mut self) -> &mut OutputJobUi {
        &mut self.ui_state.output_jobs
    }

    /// Whether an Export Job tab shows `job`.
    pub(crate) fn has_output_job_tab(&self, job: &Path) -> bool {
        self.document_state
            .tabs
            .iter()
            .any(|tab| tab.kind.is_output_job() && tab.path == job)
    }

    /// A run of `job` started: its tab shows the flow.
    pub(crate) fn show_run_flow(&mut self, job: &Path) {
        if let Some(ui) = self.output_jobs_mut().tabs.get_mut(job) {
            ui.page = TabPage::Flow;
            ui.flow_node = None;
            ui.history_pick = None;
        }
    }

    /// A run of `job` ended: a complete run collapses into its summary; a
    /// stopped one keeps the flow with its banner.
    pub(crate) fn show_run_end(&mut self, job: &Path, complete: bool) {
        if let Some(ui) = self.output_jobs_mut().tabs.get_mut(job)
            && ui.page == TabPage::Flow
            && complete
        {
            ui.page = TabPage::Summary;
            ui.history_pick = None;
        }
    }

    /// Route an Export Job tab message. Every message is listed, so a new
    /// one does not compile unrouted.
    pub(crate) fn dispatch_job_tab_message(&mut self, msg: JobTabMsg) -> Task<Message> {
        use JobTabMsg as M;
        match msg {
            M::PagePicked { job, page } => self.job_tab_ui(&job, |ui| ui.page = page),
            M::OutputPicked { job, name } => self.job_tab_ui(&job, |ui| ui.selected = Some(name)),
            M::AddMenuToggled { job } => self.job_tab_ui(&job, |ui| {
                ui.add_menu = !ui.add_menu;
                ui.profile_menu = false;
            }),
            M::ProfileMenuToggled { job } => self.job_tab_ui(&job, |ui| {
                ui.profile_menu = !ui.profile_menu;
                ui.add_menu = false;
            }),
            M::ProfilePicked { job, name } => self.job_tab_ui(&job, |ui| {
                ui.profile = Some(name);
                ui.profile_menu = false;
            }),
            M::ProfileNamingStarted { job, mode } => self.start_profile_naming(&job, mode),
            M::ProfileNameEdited { job, text } => self.job_tab_ui(&job, |ui| {
                if let Some((_, typed)) = ui.naming.as_mut() {
                    *typed = text;
                }
            }),
            M::ProfileNamingCancelled { job } => self.job_tab_ui(&job, |ui| ui.naming = None),
            M::FlowNodePicked { job, node } => self.job_tab_ui(&job, |ui| ui.flow_node = node),
            M::HistoryPicked { job, index } => self.job_tab_ui(&job, |ui| {
                ui.history_pick = Some(index);
                ui.page = TabPage::Summary;
            }),
            M::OutputEnabledToggled { .. }
            | M::OutputTicked { .. }
            | M::OutputAdded { .. }
            | M::OutputRemoved { .. }
            | M::OutputFileEdited { .. }
            | M::OutputDpiEdited { .. }
            | M::OdbFolderToggled { .. }
            | M::GerberEdited { .. }
            | M::ProfileNamingConfirmed { .. }
            | M::ProfileDuplicated { .. }
            | M::ProfileDeleted { .. }
            | M::ProfileMadeDefault { .. }
            | M::VariantPicked { .. }
            | M::FolderEdited { .. }
            | M::NamingEdited { .. }
            | M::PackagingToggled { .. }
            | M::OnErrorsPicked { .. }
            | M::OnWarningsPicked { .. }
            | M::TriggerEdited { .. }
            | M::AskNextTimePressed { .. } => self.edit_job_from_tab(msg),
            M::RunPressed { job, selected } => self.run_job_from_tab(&job, selected),
            M::StopPressed { job } => self.stop_job_run_of(&job),
            M::AnswerPressed { job, go_on } => self.answer_job_from_tab(&job, go_on),
            M::ShowInCheckPressed { job } => self.show_job_check(&job),
            M::OpenFolderPressed { job } => self.open_job_run_folder(&job),
            M::CopyPathPressed { job } => self.copy_job_run_path(&job),
            M::CopyCommandPressed { job } => self.copy_job_command(&job),
            M::ViewInGerberPressed { job } => self.view_job_run_in_gerber(&job, None),
            M::FolderOpenFailed { folder, why } => {
                crate::diagnostics::log_warning(format!(
                    "Export Job: {} could not be opened: {why}",
                    folder.display()
                ));
                Task::none()
            }
        }
    }

    /// Change the view state of `job`'s tab.
    fn job_tab_ui(&mut self, job: &Path, change: impl FnOnce(&mut JobTabUi)) -> Task<Message> {
        if let Some(ui) = self.output_jobs_mut().tabs.get_mut(job) {
            change(ui);
        }
        Task::none()
    }

    /// Save as profile… or Rename…: the name field opens with the shown
    /// profile's name.
    fn start_profile_naming(&mut self, job: &Path, mode: NamingMode) -> Task<Message> {
        let shown = self.shown_profile_name(job).unwrap_or_default();
        self.job_tab_ui(job, |ui| {
            ui.naming = Some((mode, shown));
            ui.profile_menu = false;
        })
    }

    /// The profile `job`'s tab shows: the picked one while it exists, else
    /// the default, else the first.
    pub(crate) fn shown_profile_name(&self, job: &Path) -> Option<String> {
        let draft = self.document_state.output_jobs.get(job)?;
        let picked = self
            .output_jobs()
            .tabs
            .get(job)
            .and_then(|ui| ui.profile.as_deref())
            .and_then(|name| draft.profile(name));
        picked
            .or_else(|| draft.default_profile())
            .or_else(|| draft.profiles.first())
            .map(|profile| profile.name.clone())
    }
}
