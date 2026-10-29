//! Annotate and Reset Duplicate Designators without file IO in `update` (#859).
//!
//! One run is three steps: a worker reads the project's sheets, ONE `update`
//! numbers them (open sheets in place through their engines, unopened sheets in
//! memory), and a second worker writes the unopened sheets. The policy for a
//! result that went stale in between, and the guards held meanwhile, are in
//! `docs/architecture/path-ops.md`, "Annotate and Reset Duplicate Designators".

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;

use super::super::super::*;
use crate::app::path_ops::{PathOpId, PathOpKind};
use crate::app::project_sheets::{self, ProjectSheetSet};
use crate::app::state::scope::CaseFold;
use crate::find_replace::ChildSheetSignature;
use crate::persist::{DocumentSaveJob, SaveOutcome};

/// What a project-wide run does.
#[derive(Debug, Clone, Copy)]
pub(crate) enum PassKind {
    Annotate(signex_engine::AnnotateMode),
    ResetDuplicates,
}

impl PassKind {
    /// The run's name in the Messages panel and in notices.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Annotate(_) => "Annotate",
            Self::ResetDuplicates => "Reset Duplicate Designators",
        }
    }
}

/// The undo group a run stamped, so the unopened sheets written afterwards can
/// be recorded on it.
#[derive(Debug, Clone, Copy)]
pub(super) struct RunUndo {
    pub group: signex_engine::UndoGroupId,
}

/// One unopened sheet, renumbered and encoded in `update`, waiting for its write.
pub(super) struct PendingWrite {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

/// What numbering a run changed in memory, and what is still owed to disk.
pub(super) struct PassApplied {
    pub writes: Vec<PendingWrite>,
    pub task: Task<Message>,
    pub undo: Option<RunUndo>,
}

impl PassApplied {
    pub(super) fn without_writes(task: Task<Message>) -> Self {
        Self {
            writes: Vec::new(),
            task,
            undo: None,
        }
    }
}

/// What the read worker found. Only `Debug`: the completion clones what it uses.
pub struct SheetsRead(ProjectSheetSet);

impl std::fmt::Debug for SheetsRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SheetsRead")
            .field("sheets", &self.0.sheets.len())
            .finish()
    }
}

/// How one unopened sheet's write ended.
#[derive(Debug, Clone)]
pub struct SheetWrite {
    pub path: PathBuf,
    pub outcome: Result<SaveOutcome, String>,
}

/// What the run captured before its read began; the result is applied only if
/// all of it still holds.
#[derive(Debug)]
struct ReadStage {
    epoch: u64,
    child_sheets: ChildSheetSignature,
    project: Option<ProjectId>,
    root: PathBuf,
}

#[derive(Debug)]
enum Stage {
    Reading(ReadStage),
    Writing {
        paths: Vec<PathBuf>,
        undo: Option<RunUndo>,
    },
}

#[derive(Debug)]
struct AnnotateJob {
    generation: u64,
    kind: PassKind,
    /// The registration in `pending_path_ops` that refuses renames, deletes,
    /// opens and saves of the run's sheets until it ends.
    guard: PathOpId,
    stage: Stage,
}

/// The run in flight. One at a time: a second request is refused with a notice
/// rather than queued, because a queued run would number the project as it was
/// when the first one started.
#[derive(Debug, Default)]
pub struct AnnotateRuns {
    generation: u64,
    job: Option<AnnotateJob>,
}

/// Why a read result was not applied.
enum Refused {
    /// The refusal already put its own notice and Messages line out.
    Reported,
    Because(String),
}

impl AnnotateRuns {
    /// Is a run reading, numbering or writing right now?
    pub(super) fn is_running(&self) -> bool {
        self.job.is_some()
    }

    /// The generation of the run in flight, so a test can address its completion.
    #[cfg(test)]
    pub(super) fn live_generation(&self) -> Option<u64> {
        self.job.as_ref().map(|job| job.generation)
    }
}

impl Signex {
    /// Start a project-wide run: refuse what cannot run, register the guard
    /// and return the task that reads the sheets. Nothing is read in `update`.
    pub(super) fn begin_project_run(&mut self, kind: PassKind) -> Task<Message> {
        let label = kind.label();
        if self.ui_state.annotate_runs.job.is_some() {
            self.refuse_run(
                label,
                "another Annotate or Reset Duplicate Designators run is still going. \
                 Try again once it has finished.",
            );
            return Task::none();
        }
        let Some(active) = self.document_state.active_path.clone() else {
            crate::diagnostics::log_info(format!("{label}: no sheet is open, nothing to do"));
            return Task::none();
        };
        let plan = project_sheets::plan_project_sheets_for(&self.document_state, &active);
        let mut known: Vec<PathBuf> = plan.pages().to_vec();
        known.push(plan.root().to_path_buf());
        if self.refuse_annotate_while_project_moves(known.iter().map(PathBuf::as_path), label)
            || self.refuse_annotate_while_path_op(known.iter().map(PathBuf::as_path), label)
        {
            return Task::none();
        }
        let guard = self.guard_run_paths(&known, plan.fold());
        let stage = Stage::Reading(ReadStage {
            epoch: self.document_state.documents_epoch,
            child_sheets: self.child_sheet_signature(),
            project: self.document_state.document_project(&active).map(|p| p.id),
            root: plan.root().to_path_buf(),
        });
        let runs = &mut self.ui_state.annotate_runs;
        runs.generation += 1;
        let generation = runs.generation;
        runs.job = Some(AnnotateJob {
            generation,
            kind,
            guard,
            stage,
        });
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || SheetsRead(plan.assemble())),
            move |read| {
                Message::Annotate(AnnotateMsg::PassSheetsRead {
                    generation,
                    result: Arc::new(read),
                })
            },
        )
    }

    /// Register `sheets` (the last one is the root) so a rename, delete, open
    /// or save of any of them is refused while the run holds them.
    fn guard_run_paths(&mut self, sheets: &[PathBuf], fold: CaseFold) -> PathOpId {
        let (primary, rest) = sheets.split_last().expect("a run always has a root sheet");
        self.document_state.pending_path_ops.begin_with_dependents(
            PathOpKind::ProjectRewrite,
            vec![primary.clone()],
            rest.to_vec(),
            fold,
        )
    }

    fn refuse_run(&mut self, label: &str, why: &str) {
        let detail = format!("{label} was not run: {why}");
        crate::diagnostics::log_warning(detail.clone());
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::annotate(detail));
    }

    /// Take the run's own job when `generation` is the live one.
    fn take_run(&mut self, generation: u64, reading: bool) -> Option<AnnotateJob> {
        let job = self.ui_state.annotate_runs.job.take()?;
        let wanted =
            job.generation == generation && matches!(job.stage, Stage::Reading(_)) == reading;
        if wanted {
            return Some(job);
        }
        self.ui_state.annotate_runs.job = Some(job);
        None
    }

    /// The read worker reported. Number the project in this one `update`, or
    /// change nothing and say why.
    pub(crate) fn handle_annotate_sheets_read(
        &mut self,
        generation: u64,
        result: &Result<SheetsRead, String>,
    ) -> Task<Message> {
        let Some(job) = self.take_run(generation, true) else {
            return Task::none();
        };
        let Stage::Reading(read) = &job.stage else {
            self.document_state.pending_path_ops.finish(job.guard);
            return Task::none();
        };
        // Released first: the checks below ask whether ANOTHER operation holds
        // the sheets, and the run's own registration would answer yes.
        self.document_state.pending_path_ops.finish(job.guard);
        let label = job.kind.label();
        let set = match self.accept_read(job.kind, read, result) {
            Ok(set) => set,
            Err(Refused::Reported) => return Task::none(),
            Err(Refused::Because(why)) => {
                self.refuse_run(label, &format!("{why}. Nothing was changed; run it again."));
                return Task::none();
            }
        };
        let applied = match job.kind {
            PassKind::Annotate(mode) => self.apply_annotate(mode, set),
            PassKind::ResetDuplicates => self.apply_reset_duplicates(set),
        };
        self.start_writes(job.generation, job.kind, applied)
    }

    /// Everything the numbering relies on must still hold when the sheets
    /// arrive; otherwise the whole run is dropped (all or nothing: a partial
    /// run could hand two sheets the same designator). The returned set has
    /// every open sheet read from its live engine, so an edit made while the
    /// worker read is numbered, not overwritten.
    fn accept_read(
        &mut self,
        kind: PassKind,
        read: &ReadStage,
        result: &Result<SheetsRead, String>,
    ) -> Result<ProjectSheetSet, Refused> {
        let sheets = match result {
            Ok(sheets) => &sheets.0,
            Err(why) => {
                return Err(Refused::Because(format!(
                    "the sheets could not be read ({why})"
                )));
            }
        };
        self.check_unchanged(read)?;
        let mut set = ProjectSheetSet {
            sheets: sheets.sheets.clone(),
            pages_outside_the_hierarchy: sheets.pages_outside_the_hierarchy.clone(),
            unreadable: sheets.unreadable.clone(),
            root: sheets.root.clone(),
            fold: sheets.fold,
        };
        for (path, sheet) in &mut set.sheets {
            if let Some(engine) = self.document_state.engines.get(path) {
                *sheet = engine.document().clone();
            }
        }
        let label = kind.label();
        let paths = || set.sheets.keys().map(PathBuf::as_path);
        if self.refuse_annotate_while_project_moves(paths(), label)
            || self.refuse_annotate_while_path_op(paths(), label)
        {
            return Err(Refused::Reported);
        }
        Ok(set)
    }

    fn check_unchanged(&self, read: &ReadStage) -> Result<(), Refused> {
        let ds = &self.document_state;
        let stale = |why: &str| Err(Refused::Because(why.to_string()));
        if ds.documents_epoch != read.epoch {
            return stale(
                "a document was opened, saved, closed, renamed or deleted while the sheets were \
                 being read",
            );
        }
        if self.child_sheet_signature() != read.child_sheets {
            return stale(
                "a sheet symbol was placed, removed or changed while the sheets were being read",
            );
        }
        let Some(active) = ds.active_path.as_deref() else {
            return stale("no sheet is open any more");
        };
        let project = ds.document_project(active).map(|p| p.id);
        if project != read.project
            || project_sheets::resolve_netlist_root_for(ds, active) != read.root
        {
            return stale("the active project changed while the sheets were being read");
        }
        Ok(())
    }

    /// Hand the unopened sheets to the write worker, or finish the run when
    /// there are none.
    fn start_writes(
        &mut self,
        generation: u64,
        kind: PassKind,
        applied: PassApplied,
    ) -> Task<Message> {
        let PassApplied { writes, task, undo } = applied;
        if writes.is_empty() {
            let refresh = self.finish_run_view();
            return Task::batch([task, refresh]);
        }
        // The sheets are about to change on disk behind the engines: a read of
        // them already in flight (project Replace) is out of date from here, not
        // from the write's completion.
        self.document_state.note_document_written_behind_engines();
        let paths: Vec<PathBuf> = writes.iter().map(|w| w.path.clone()).collect();
        let fold = self.run_fold();
        let guard = self.guard_run_paths(&paths, fold);
        let hold = self.document_state.autosave.hold_for_save();
        let jobs: Vec<(PathBuf, DocumentSaveJob)> = writes
            .into_iter()
            .map(|w| {
                let job = self
                    .document_state
                    .document_saves
                    .begin(w.path.clone(), w.bytes);
                (w.path, job)
            })
            .collect();
        self.ui_state.annotate_runs.job = Some(AnnotateJob {
            generation,
            kind,
            guard,
            stage: Stage::Writing { paths, undo },
        });
        Task::batch([task, write_task(generation, jobs, hold)])
    }

    fn run_fold(&self) -> CaseFold {
        crate::app::state::project_case::fold_of(self.document_state.active_document_project())
    }

    /// The write worker reported: settle every sheet, release the guard.
    pub(crate) fn handle_annotate_sheets_written(
        &mut self,
        generation: u64,
        result: &Result<Vec<SheetWrite>, String>,
    ) -> Task<Message> {
        let Some(job) = self.take_run(generation, false) else {
            return Task::none();
        };
        let Stage::Writing { paths, undo } = job.stage else {
            return Task::none();
        };
        for path in &paths {
            self.document_state.document_saves.finish(path);
        }
        self.document_state.pending_path_ops.finish(job.guard);
        let sheets: Vec<SheetWrite> = match result {
            Ok(done) => done.clone(),
            // The worker died: nothing is known to have landed.
            Err(why) => paths
                .iter()
                .map(|path| SheetWrite {
                    path: path.clone(),
                    outcome: Err(why.clone()),
                })
                .collect(),
        };
        let mut tasks = self.settle_written(job.kind, undo, sheets);
        tasks.push(self.finish_run_view());
        Task::batch(tasks)
    }

    /// Bookkeeping per written sheet. Returns the autosave tasks it owes.
    fn settle_written(
        &mut self,
        kind: PassKind,
        undo: Option<RunUndo>,
        sheets: Vec<SheetWrite>,
    ) -> Vec<Task<Message>> {
        let label = kind.label();
        let mut tasks = Vec::new();
        let mut written: Vec<PathBuf> = Vec::new();
        let mut failed: Vec<PathBuf> = Vec::new();
        for SheetWrite { path, outcome } in sheets {
            let problem = match outcome {
                Ok(SaveOutcome::Written) => None,
                Ok(SaveOutcome::Superseded) => Some("a newer write to it landed first".to_string()),
                Err(why) => Some(why),
            };
            if let Some(why) = problem {
                crate::diagnostics::log_error(
                    &format!("{label} could not write '{}'", path.display()),
                    &anyhow::anyhow!(why),
                );
                failed.push(path);
                continue;
            }
            // The bytes landed and no engine holds the sheet (the guard kept
            // it from being opened), so there is no editor state to compare.
            if matches!(kind, PassKind::ResetDuplicates)
                && !self.document_state.engines.contains_key(&path)
            {
                self.document_state.mark_document_clean(&path);
            }
            // Rewritten outside the document save path: the hook drops a
            // recovery copy of the old content and moves `documents_epoch`.
            tasks.push(self.autosave_after_disk_write(&path));
            written.push(path);
        }
        self.forget_unwritten_sheets(undo, &failed);
        self.report_written(label, &written, &failed);
        tasks
    }

    /// The group listed every sheet owed to disk from numbering on, so an undo
    /// during the write can name them; the ones that never landed come off.
    fn forget_unwritten_sheets(&mut self, undo: Option<RunUndo>, failed: &[PathBuf]) {
        let Some(undo) = undo else { return };
        if let Some(group) = self.document_state.undo_groups.get_mut(&undo.group) {
            group.disk_only.retain(|path| !failed.contains(path));
        }
    }

    fn report_written(&mut self, label: &str, written: &[PathBuf], failed: &[PathBuf]) {
        if !written.is_empty() {
            crate::diagnostics::log_info(format!(
                "{label}: wrote {} unopened sheet file(s) to disk",
                written.len()
            ));
        }
        if failed.is_empty() {
            return;
        }
        let names: Vec<String> = failed.iter().map(|p| p.display().to_string()).collect();
        let detail = format!(
            "{label} could not write {} unopened sheet(s) and left them as they were on disk: \
             {}. The open sheets were still renumbered, so run it again once the cause is fixed.",
            failed.len(),
            names.join(", ")
        );
        crate::diagnostics::log_warning(detail.clone());
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::annotate(detail));
    }

    /// The run is over: the preview rows are stale (disk and memory both
    /// changed), so reload them.
    fn finish_run_view(&mut self) -> Task<Message> {
        self.refresh_panel_ctx();
        self.invalidate_annotate_preview();
        self.refresh_annotate_preview()
    }
}

/// The task that runs every write on one worker, in order.
fn write_task(
    generation: u64,
    jobs: Vec<(PathBuf, DocumentSaveJob)>,
    hold: crate::autosave::SaveHold,
) -> Task<Message> {
    Task::perform(
        crate::app::path_op_jobs::run_blocking(move || {
            jobs.into_iter()
                .map(|(path, job)| SheetWrite {
                    path,
                    outcome: job.run(),
                })
                .collect::<Vec<_>>()
        }),
        move |result| {
            Message::Annotate(AnnotateMsg::PassSheetsWritten {
                generation,
                result: Arc::new(result),
                hold: hold.clone(),
            })
        },
    )
}
