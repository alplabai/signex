//! The Import Library dialog's handlers (mockup row 211, #3117–#3121). The
//! dialog's model is `crate::library::import_dialog`; the converter search,
//! the pickers and the folder check run as tasks, and both runs (the dry
//! run and the import) go through [`Signex::start_library_import`]. The
//! result's report actions are in [`report`].

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use signex_i18n::{Translatable, ids};

use super::*;
use crate::library::import_dialog::notices::RunProblem;
use crate::library::import_dialog::progress::{ends_item, item_in_flight};
use crate::library::import_dialog::result::{ImportResult, Mounted, RequiredLibrary};
use crate::library::import_dialog::{
    self as dialog, FolderAfter, ImportDialogMsg, ImportDialogState, ImportEnd, PreviewState,
    SourceKind, Step, TargetCheck,
};
use crate::library_import::run_state::{ImportFinished, MountTarget};
use crate::library_import::snapshot::default_libraries_root;
use crate::library_import::{MountScope, ProgressEvent, RunMode};

mod report;
mod target;

pub(crate) use target::check_target;

/// The temp folders of dry runs are named with this prefix; only such a
/// folder is ever removed by the dialog.
const PREVIEW_DIR_PREFIX: &str = "signex-library-preview-";

fn dialog_message(m: ImportDialogMsg) -> Message {
    Message::Library(dialog::msg(m))
}

/// Remove a dry run's temp folder on a worker; anything not named like one
/// is left alone.
pub(super) fn remove_preview_dir(dir: PathBuf) -> Task<Message> {
    let ours = dir
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with(PREVIEW_DIR_PREFIX));
    if !ours {
        return Task::none();
    }
    Task::future(async move {
        let shown = dir.display().to_string();
        let removed = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(&dir)).await;
        if let Ok(Err(e)) = removed
            && e.kind() != std::io::ErrorKind::NotFound
        {
            log::warn!("Library import: could not remove the preview {shown}: {e}");
        }
    })
    .discard()
}

impl Signex {
    pub(super) fn handle_import_dialog(&mut self, msg: ImportDialogMsg) -> Task<Message> {
        let root = default_libraries_root();
        match msg {
            ImportDialogMsg::Requested => self.open_import_dialog(None),
            ImportDialogMsg::RequestedForProject(id) => self.open_import_dialog(Some(id)),
            ImportDialogMsg::DiscoveryFinished(cache) => {
                let discovery = cache.as_ref().map(|c| c.discovery.clone());
                if let Some(d) = self.library.import_dialog.as_mut() {
                    d.discovery = Some(discovery.unwrap_or_default());
                }
                if let Some(cache) = cache {
                    self.library.import.discovered = Some(*cache);
                }
                Task::none()
            }
            ImportDialogMsg::RescanPressed => {
                self.with_import_dialog(|d| d.discovery = None);
                self.discover_converters()
            }
            ImportDialogMsg::SourceKindPicked(kind) => self.edit_import_dialog(|d| {
                if d.source_kind != kind {
                    d.source_kind = kind;
                    d.apply_sources(Vec::new(), None);
                }
            }),
            ImportDialogMsg::BrowsePressed => self.pick_import_sources(),
            ImportDialogMsg::SourcesPicked(Some(paths)) if !paths.is_empty() => {
                self.edit_import_dialog(|d| d.apply_sources(paths, root.as_deref()))
            }
            ImportDialogMsg::SourcesPicked(_)
            | ImportDialogMsg::DatabasePicked(None)
            | ImportDialogMsg::LocationPicked(None) => Task::none(),
            ImportDialogMsg::ConverterPicked(path) => {
                self.edit_import_dialog(|d| d.converter = Some(path))
            }
            ImportDialogMsg::DatabaseEdited(text) => self.edit_import_dialog(|d| d.database = text),
            ImportDialogMsg::DatabaseBrowsePressed => self.pick_import_database(),
            ImportDialogMsg::DatabasePicked(Some(path)) => {
                self.edit_import_dialog(|d| d.database = path.display().to_string())
            }
            ImportDialogMsg::NameEdited(name) => self.edit_import_dialog(|d| {
                d.name_edited = true;
                d.set_name(name, root.as_deref());
            }),
            ImportDialogMsg::LocationEdited(text) => {
                self.edit_import_dialog(|d| d.set_location(text))
            }
            ImportDialogMsg::LocationBrowsePressed => self.pick_import_location(),
            ImportDialogMsg::LocationPicked(Some(path)) => {
                self.edit_import_dialog(|d| d.set_location(path.display().to_string()))
            }
            ImportDialogMsg::MountForAllToggled => {
                self.edit_import_dialog(|d| d.mount_for_all = !d.mount_for_all)
            }
            ImportDialogMsg::ModelsToggled => {
                self.edit_import_dialog(|d| d.include_models = !d.include_models)
            }
            ImportDialogMsg::NextPressed => {
                self.check_import_target(Step::Source, RunMode::Preview)
            }
            ImportDialogMsg::ImportPressed => self.import_pressed(),
            ImportDialogMsg::TargetChecked { location, check } => {
                self.import_target_checked(location, check)
            }
            ImportDialogMsg::UpdateAccepted => self.import_update_accepted(),
            ImportDialogMsg::BackPressed => self.import_dialog_back(),
            ImportDialogMsg::FilterEdited(text) => self.edit_import_dialog(|d| d.filter = text),
            ImportDialogMsg::SeverityToggled(severity) => {
                self.edit_import_dialog(|d| d.toggle_severity(severity))
            }
            ImportDialogMsg::ItemSelected(index) => {
                self.edit_import_dialog(|d| d.selected_item = Some(index))
            }
            ImportDialogMsg::CancelPressed | ImportDialogMsg::Closed => self.import_dialog_closed(),
            ImportDialogMsg::ResultTabPicked(tab) => {
                self.edit_import_dialog(|d| d.result_tab = tab)
            }
            ImportDialogMsg::SaveReportPressed => self.pick_import_report_target(),
            ImportDialogMsg::ReportSavePicked(target) => self.save_import_report(target),
            ImportDialogMsg::ReportSaved(saved) => self.import_report_saved(saved),
            ImportDialogMsg::OpenReportPressed => self.open_import_report(),
            ImportDialogMsg::ReportOpened(opened) => self.import_report_opened(opened),
            ImportDialogMsg::OpenInBrowserPressed => self.open_imported_library(),
            ImportDialogMsg::LibraryMenuToggled(file) => {
                let open = self.library.import_menu.as_ref() == Some(&file);
                self.library.import_menu = (!open).then_some(file);
                Task::none()
            }
            ImportDialogMsg::StoredReportRequested(file) => self.read_stored_import_report(file),
            ImportDialogMsg::StoredReportRead {
                library_file,
                report,
            } => self.stored_import_report_read(library_file, report),
        }
    }

    fn edit_import_dialog(&mut self, edit: impl FnOnce(&mut ImportDialogState)) -> Task<Message> {
        self.with_import_dialog(edit);
        Task::none()
    }

    fn with_import_dialog(&mut self, edit: impl FnOnce(&mut ImportDialogState)) {
        if let Some(d) = self.library.import_dialog.as_deref_mut() {
            edit(d);
        }
    }

    /// Open the dialog on step 1 and look for converters; opened for a
    /// project, "This project" is preselected and means it. An open dialog
    /// stays as it is.
    fn open_import_dialog(&mut self, project: Option<crate::app::ProjectId>) -> Task<Message> {
        if self.library.import_dialog.is_some() {
            return Task::none();
        }
        let mut state = ImportDialogState::new();
        if let Some(id) = project.filter(|&id| self.document_state.project_by_id(id).is_some()) {
            state.project = Some(id);
            state.mount_for_all = false;
        }
        self.library.import_dialog = Some(Box::new(state));
        self.discover_converters()
    }

    /// The converter search on a worker, answered from the session's cache
    /// when nothing it depends on changed.
    fn discover_converters(&mut self) -> Task<Message> {
        let cache = self.library.import.discovered.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    crate::library_import::discovery::discover(cache.as_ref())
                })
                .await
            },
            |found| match found {
                Ok(cache) => {
                    dialog_message(ImportDialogMsg::DiscoveryFinished(Some(Box::new(cache))))
                }
                Err(e) => {
                    log::warn!("Library import: the converter search failed: {e}");
                    dialog_message(ImportDialogMsg::DiscoveryFinished(None))
                }
            },
        )
    }

    fn pick_import_sources(&mut self) -> Task<Message> {
        let Some(d) = self.library.import_dialog.as_deref() else {
            return Task::none();
        };
        let i18n = &self.ui_state.i18n;
        let mut picker = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::LIBRARY_IMPORT_DIALOG_PICK_TITLE);
        for filter in d.picker_filters() {
            picker = picker.add_filter(filter.label, &filter.extensions);
        }
        match d.source_kind {
            SourceKind::Files => Task::perform(picker.pick_files(), |paths| {
                dialog_message(ImportDialogMsg::SourcesPicked(paths))
            }),
            SourceKind::Database => Task::perform(picker.pick_file(), |path| {
                dialog_message(ImportDialogMsg::SourcesPicked(path.map(|p| vec![p])))
            }),
        }
    }

    fn pick_import_database(&mut self) -> Task<Message> {
        let picker = self.ui_state.io.picker.dialog().title_in(
            &self.ui_state.i18n,
            ids::LIBRARY_IMPORT_DIALOG_DATABASE_PICK_TITLE,
        );
        Task::perform(picker.pick_file(), |path| {
            dialog_message(ImportDialogMsg::DatabasePicked(path))
        })
    }

    fn pick_import_location(&mut self) -> Task<Message> {
        let picker = self.ui_state.io.picker.dialog().title_in(
            &self.ui_state.i18n,
            ids::LIBRARY_IMPORT_DIALOG_LOCATION_PICK_TITLE,
        );
        Task::perform(picker.pick_folder(), |path| {
            dialog_message(ImportDialogMsg::LocationPicked(path))
        })
    }

    /// Next (from step 1) and Import (from step 2) first check the folder
    /// on a worker; the run starts from [`Self::import_target_checked`].
    fn check_import_target(&mut self, from: Step, mode: RunMode) -> Task<Message> {
        let Some(d) = self.library.import_dialog.as_deref_mut() else {
            return Task::none();
        };
        if d.step != from || d.awaiting.is_some() || d.checking_target || d.blocker().is_some() {
            return Task::none();
        }
        // The engine takes one run at a time. A dry run this dialog left
        // (Back, Close) is still stopping; any other run is not its own.
        if self.library.import.running.is_some() {
            if !d.run_stopping {
                d.other_run_active = true;
            }
            return Task::none();
        }
        d.checking_target = true;
        d.pending_run = Some(mode);
        let location = d.location_path();
        Task::perform(
            async move {
                let checked = location.clone();
                let check = tokio::task::spawn_blocking(move || check_target(&checked))
                    .await
                    .unwrap_or_else(|e| {
                        TargetCheck::Refused(
                            Translatable::new(ids::LIBRARY_IMPORT_DIALOG_TARGET_UNREADABLE)
                                .arg("path", location.display().to_string())
                                .arg("error", e.to_string()),
                        )
                    });
                (location, check)
            },
            |(location, check)| dialog_message(ImportDialogMsg::TargetChecked { location, check }),
        )
    }

    /// Import on step 2: the dry run's temp folder goes, then the folder
    /// check, then the import.
    fn import_pressed(&mut self) -> Task<Message> {
        let ready = self
            .library
            .import_dialog
            .as_deref()
            .is_some_and(|d| d.step == Step::Preview && d.preview_outcome().is_some());
        if !ready {
            return Task::none();
        }
        let cleanup = self.take_preview_dir();
        Task::batch([
            cleanup,
            self.check_import_target(Step::Preview, RunMode::Import),
        ])
    }

    /// Start the dry run (into a new temp folder) or the import (into the
    /// chosen folder), and move to its step when the engine took it.
    fn start_dialog_run(&mut self, mode: RunMode) -> Task<Message> {
        let Some(d) = self.library.import_dialog.as_deref() else {
            return Task::none();
        };
        let for_all = mode == RunMode::Preview || d.mount_for_all;
        // The project the dialog was opened for, while it is loaded.
        let project = d
            .project
            .and_then(|id| self.document_state.project_by_id(id))
            .map(|p| p.path.clone());
        let output = match mode {
            RunMode::Preview => {
                std::env::temp_dir().join(format!("{PREVIEW_DIR_PREFIX}{}", uuid::Uuid::now_v7()))
            }
            RunMode::Import | RunMode::Update => d.location_path(),
        };
        let Some(job) = d.job(mode, output.clone()) else {
            return Task::none();
        };
        let task = match project {
            Some(path) if !for_all => {
                self.start_library_import_into(job, MountTarget::Project(path))
            }
            _ if for_all || self.document_state.active_project.is_none() => {
                self.start_library_import(job, MountScope::AllProjects)
            }
            _ => self.start_library_import(job, MountScope::ThisProject),
        };
        let started = self
            .library
            .import
            .running
            .as_ref()
            .is_some_and(|run| run.output == output && run.mode == mode);
        if let (true, Some(d)) = (started, self.library.import_dialog.as_deref_mut()) {
            d.awaiting = Some(mode);
            d.cancel_pending = false;
            d.phases.clear();
            d.import_end = None;
            if mode == RunMode::Preview {
                d.step = Step::Preview;
                d.preview = Some(PreviewState::Running);
                d.preview_dir = Some(output);
                d.selected_item = None;
            } else {
                d.step = Step::Import;
            }
        }
        task
    }

    /// Back from step 2 (or from a failed import): a running dry run is
    /// cancelled and its folder removed.
    fn import_dialog_back(&mut self) -> Task<Message> {
        let Some(d) = self.library.import_dialog.as_deref() else {
            return Task::none();
        };
        let importing = d.step == Step::Import && d.import_end.is_none();
        if d.step == Step::Source || importing {
            return Task::none();
        }
        self.stop_dialog_preview();
        let cleanup = self.take_preview_dir();
        self.with_import_dialog(|d| {
            d.step = Step::Source;
            d.preview = None;
            d.import_end = None;
            d.phases.clear();
        });
        cleanup
    }

    /// Cancel, the header close, Esc and the window's close: a running
    /// import is cancelled (the dialog stays until it has stopped);
    /// otherwise the dialog closes, cancelling a dry run and removing its
    /// folder.
    fn import_dialog_closed(&mut self) -> Task<Message> {
        let Some(d) = self.library.import_dialog.as_deref() else {
            return Task::none();
        };
        if d.awaiting == Some(RunMode::Import) {
            self.request_dialog_cancel();
            return Task::none();
        }
        self.stop_dialog_preview();
        let cleanup = self.take_preview_dir();
        self.library.import_dialog = None;
        cleanup
    }

    /// The dialog's window went away without a close request: as Closed,
    /// except that a running import keeps the dialog (the sync shows the
    /// window again).
    pub(crate) fn import_dialog_window_vanished(&mut self) -> Task<Message> {
        let importing = self
            .library
            .import_dialog
            .as_deref()
            .is_some_and(|d| d.awaiting == Some(RunMode::Import));
        if importing {
            return Task::none();
        }
        self.stop_dialog_preview();
        let cleanup = self.take_preview_dir();
        self.library.import_dialog = None;
        cleanup
    }

    /// Cancel the import: at once when no item is in flight, else at the
    /// next item boundary ([`Self::import_dialog_progress`]). A second
    /// Cancel (or Esc, or Close) while it waits stops the run now, so a
    /// converter stuck on one item can always be stopped.
    fn request_dialog_cancel(&mut self) {
        let Some(d) = self.library.import_dialog.as_deref_mut() else {
            return;
        };
        let now = d.cancel_pending || {
            d.cancel_pending = true;
            self.library
                .import
                .running
                .as_ref()
                .is_none_or(|run| !item_in_flight(run.progress.as_ref()))
        };
        if now && let Some(run) = &self.library.import.running {
            d.stop_requested = true;
            run.cancel.store(true, Ordering::Relaxed);
            log::info!("Library import: cancelling {}", run.output.display());
        }
    }

    /// A running dry run is told to stop; its result is no longer awaited.
    fn stop_dialog_preview(&mut self) {
        let Some(d) = self.library.import_dialog.as_deref_mut() else {
            return;
        };
        if d.awaiting != Some(RunMode::Preview) {
            return;
        }
        d.awaiting = None;
        if let Some(run) = &self.library.import.running
            && run.mode == RunMode::Preview
        {
            run.cancel.store(true, Ordering::Relaxed);
            d.run_stopping = true;
        }
    }

    /// The dry run's temp folder, removed on a worker.
    fn take_preview_dir(&mut self) -> Task<Message> {
        self.library
            .import_dialog
            .as_deref_mut()
            .and_then(|d| d.preview_dir.take())
            .map_or_else(Task::none, remove_preview_dir)
    }

    /// A progress event of the running import, with the one before it.
    /// A Cancel waiting for the current item takes effect once it is done.
    pub(super) fn import_dialog_progress(
        &mut self,
        previous: Option<&ProgressEvent>,
        event: &ProgressEvent,
    ) {
        let Some(d) = self.library.import_dialog.as_deref_mut() else {
            return;
        };
        if d.awaiting.is_none() {
            return;
        }
        d.record_progress(event);
        if d.cancel_pending
            && ends_item(previous, event)
            && let Some(run) = &self.library.import.running
        {
            run.cancel.store(true, Ordering::Relaxed);
            log::info!("Library import: cancelling {}", run.output.display());
        }
    }

    /// Where the finished import the dialog shows was mounted, and the
    /// libraries it requires.
    pub(super) fn import_dialog_mounted(
        &mut self,
        mounted: Mounted,
        required: Vec<RequiredLibrary>,
    ) {
        let facts = self
            .library
            .import_dialog
            .as_deref_mut()
            .and_then(|d| d.result.as_deref_mut())
            .and_then(|r| r.run.as_mut());
        if let Some(facts) = facts {
            facts.mounted = Some(mounted);
            facts.required = required;
        }
    }

    /// A run ended. `mode` is the run's own. Returns whether the dialog
    /// shows the result, so no notice repeats it.
    pub(super) fn import_dialog_run_finished(
        &mut self,
        finished: &ImportFinished,
        mode: Option<RunMode>,
    ) -> bool {
        let Some(d) = self.library.import_dialog.as_deref_mut() else {
            return false;
        };
        // Whatever ended, the engine is free for the next run.
        d.run_stopping = false;
        d.other_run_active = false;
        if mode.is_none() || d.awaiting != mode {
            return false;
        }
        d.awaiting = None;
        d.cancel_pending = false;
        d.stop_requested = false;
        match (mode, &finished.result) {
            (Some(RunMode::Preview), Ok(outcome)) => {
                d.preview = Some(PreviewState::Ready(Box::new(outcome.run.clone())));
            }
            (Some(RunMode::Preview), Err(failure)) => {
                d.preview = Some(PreviewState::Failed(Box::new(RunProblem::of(failure))));
            }
            (Some(mode), Ok(outcome)) => {
                let output = d.location_path();
                d.show_result(ImportResult::of_run(mode, outcome, &output));
            }
            (None, Ok(_)) => {}
            (_, Err(failure)) => {
                let folder = FolderAfter::of(failure);
                d.import_end = Some(match *failure.error {
                    // Refused before the converter started: the folder,
                    // which may hold the user's files, was not touched.
                    ref error if failure.refused => ImportEnd::Refused {
                        reason: error.to_translatable(),
                    },
                    crate::library_import::ImportError::Cancelled => {
                        ImportEnd::Cancelled { folder }
                    }
                    _ => {
                        let problem = RunProblem::of(failure);
                        ImportEnd::Failed {
                            reason: problem.reason,
                            folder,
                            stderr_tail: problem.stderr_tail,
                        }
                    }
                });
            }
        }
        true
    }
}
