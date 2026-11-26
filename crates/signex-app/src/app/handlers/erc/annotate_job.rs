//! Annotate and Reset Duplicate Designators without file IO in `update` (#859).
//!
//! One run is two steps: a worker reads the project's sheets, then ONE `update`
//! numbers them in memory. Open sheets change through their engines; unopened
//! sheets load into parked, unsaved engines, so no run writes a file (#2760).
//! The policy for a result that went stale in between, and the guards held
//! meanwhile, are in `docs/architecture/path-ops.md`, "Annotate and Reset
//! Duplicate Designators".

use signex_i18n::RenderTranslatable;
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;

use super::super::super::*;
use super::reprefix_confirm::ReprefixConfirm;
use crate::app::path_ops::{PathOpId, PathOpKind};
use crate::app::project_sheets::{self, ProjectSheetSet};
use crate::app::state::scope::CaseFold;
use crate::find_replace::ChildSheetSignature;

/// What a project-wide run does.
#[derive(Debug, Clone, Copy)]
pub(crate) enum PassKind {
    Annotate(signex_engine::AnnotateMode),
    ResetDuplicates,
    /// Re-prefix to standard (#1756): read here, planned and applied by
    /// `reprefix_job`. Its confirm rides in the job, not in this `Copy` tag.
    Reprefix,
}

impl PassKind {
    /// The run's name in the Messages panel and in notices.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Annotate(_) => "Annotate",
            Self::ResetDuplicates => "Reset Duplicate Designators",
            Self::Reprefix => "Re-prefix to standard",
        }
    }
}

/// What the read worker found. Only `Debug`: the completion clones what it uses.
pub struct SheetsRead(ProjectSheetSet);

impl SheetsRead {
    /// Read `plan`'s sheets. Runs on a worker.
    pub(super) fn assemble(plan: &project_sheets::ProjectSheetPlan) -> Self {
        Self(plan.assemble())
    }
}

impl std::fmt::Debug for SheetsRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SheetsRead")
            .field("sheets", &self.0.sheets.len())
            .finish()
    }
}

/// What the run captured before its read began; the result is applied only if
/// all of it still holds.
#[derive(Debug)]
pub(super) struct ReadStage {
    epoch: u64,
    child_sheets: ChildSheetSignature,
    project: Option<ProjectId>,
    root: PathBuf,
    /// The sheet a `document` operand named (#2470): its project is numbered
    /// whatever tab is active. `None`: the active sheet's.
    pub(super) anchor: Option<PathBuf>,
}

#[derive(Debug)]
struct AnnotateJob {
    generation: u64,
    kind: PassKind,
    /// The registration in `pending_path_ops` that refuses renames, deletes
    /// and opens of the run's sheets until it ends.
    guard: PathOpId,
    read: ReadStage,
    /// The confirm a Re-prefix run applies once its sheets are read.
    reprefix: Option<ReprefixConfirm>,
    /// The caller of the operand form, answered when the run ends (#2470).
    waiting: Option<crate::app::agent::WaitingCommand>,
}

/// The run in flight (Annotate, Reset Duplicate Designators or Re-prefix). One
/// at a time: a second request is refused with a notice rather than queued,
/// because a queued run would number the project as it was when the first one
/// started.
#[derive(Debug, Default)]
pub struct AnnotateRuns {
    generation: u64,
    job: Option<AnnotateJob>,
}

/// Why a read result was not applied.
pub(super) enum Refused {
    /// The refusal already put its own notice and Messages line out.
    Reported,
    Because(String),
}

impl AnnotateRuns {
    /// Is a run reading its sheets right now?
    pub(crate) fn is_running(&self) -> bool {
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
        self.begin_run(kind, None)
    }

    /// [`Self::begin_project_run`], carrying the Re-prefix confirm the run
    /// applies.
    pub(super) fn begin_run(
        &mut self,
        kind: PassKind,
        reprefix: Option<ReprefixConfirm>,
    ) -> Task<Message> {
        self.begin_run_at(kind, reprefix, None, None)
            .unwrap_or_else(|_| Task::none())
    }

    /// [`Self::begin_run`] on the project of `anchor` (a sheet a `document`
    /// operand named, #2470), else of the active sheet, answering `waiting`
    /// when the run ends. `Err` is why it did not start, already logged.
    pub(super) fn begin_run_at(
        &mut self,
        kind: PassKind,
        reprefix: Option<ReprefixConfirm>,
        anchor: Option<PathBuf>,
        waiting: Option<crate::app::agent::WaitingCommand>,
    ) -> Result<Task<Message>, String> {
        let label = kind.label();
        if self.ui_state.annotate_runs.job.is_some() {
            let why = "another Annotate or Reset Duplicate Designators run is still going. \
                       Try again once it has finished.";
            self.refuse_run(label, why);
            return Err(format!("{label}: {why}"));
        }
        let Some((read, plan)) = self.capture_sheet_read_at(anchor) else {
            let line = format!("{label}: no sheet is open, nothing to do");
            crate::diagnostics::log_info(line.clone());
            return Err(line);
        };
        let mut known: Vec<PathBuf> = plan.pages().to_vec();
        known.push(plan.root().to_path_buf());
        if self.refuse_annotate_while_project_moves(known.iter().map(PathBuf::as_path), label)
            || self.refuse_annotate_while_path_op(known.iter().map(PathBuf::as_path), label)
        {
            return Err(self.notice_line(label));
        }
        let guard = self.guard_run_paths(&known, plan.fold());
        let runs = &mut self.ui_state.annotate_runs;
        runs.generation += 1;
        let generation = runs.generation;
        runs.job = Some(AnnotateJob {
            generation,
            kind,
            guard,
            read,
            reprefix,
            waiting,
        });
        Ok(Task::perform(
            crate::app::path_op_jobs::run_blocking(move || SheetsRead(plan.assemble())),
            move |read| {
                Message::Annotate(AnnotateMsg::PassSheetsRead {
                    generation,
                    result: Arc::new(read),
                })
            },
        ))
    }

    /// The error notice a refusal just raised, as one en-US line.
    pub(super) fn notice_line(&self, label: &str) -> String {
        self.document_state.error_notice.as_ref().map_or_else(
            || format!("{label}: nothing was changed"),
            crate::app::state::ErrorNotice::detail_line_en_us,
        )
    }

    /// What a read of the active project must capture so its result can be
    /// checked afterwards, and the plan the worker reads from. `None` with no
    /// sheet open. Nothing is read here.
    pub(super) fn capture_sheet_read(
        &self,
    ) -> Option<(ReadStage, project_sheets::ProjectSheetPlan)> {
        self.capture_sheet_read_at(None)
    }

    /// [`Self::capture_sheet_read`] for the project of `anchor`, else of the
    /// active sheet.
    fn capture_sheet_read_at(
        &self,
        anchor: Option<PathBuf>,
    ) -> Option<(ReadStage, project_sheets::ProjectSheetPlan)> {
        let sheet = anchor
            .clone()
            .or_else(|| self.document_state.active_path.clone())?;
        let plan = project_sheets::plan_project_sheets_for(&self.document_state, &sheet);
        let stage = ReadStage {
            epoch: self.document_state.documents_epoch,
            child_sheets: self.child_sheet_signature(),
            project: self.document_state.document_project(&sheet).map(|p| p.id),
            root: plan.root().to_path_buf(),
            anchor,
        };
        Some((stage, plan))
    }

    /// Register `sheets` (the last one is the root) so a rename, delete or
    /// open of any of them is refused while the run reads them. A save is
    /// allowed and cancels the run instead.
    fn guard_run_paths(&mut self, sheets: &[PathBuf], fold: CaseFold) -> PathOpId {
        let (primary, rest) = sheets.split_last().expect("a run always has a root sheet");
        self.document_state.pending_path_ops.begin_with_dependents(
            PathOpKind::ProjectRead,
            vec![primary.clone()],
            rest.to_vec(),
            fold,
        )
    }

    pub(super) fn refuse_run(&mut self, label: &str, why: &str) {
        let detail = signex_i18n::Translatable::new(signex_i18n::ids::NOTICE_RUN_REFUSED)
            .arg("label", label)
            .arg("reason", why);
        crate::diagnostics::log_warning(detail.render_en_us());
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::annotate(detail));
    }

    /// Take the run's own job when `generation` is the live one.
    fn take_run(&mut self, generation: u64) -> Option<AnnotateJob> {
        let job = self.ui_state.annotate_runs.job.take()?;
        if job.generation == generation {
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
        let Some(mut job) = self.take_run(generation) else {
            return Task::none();
        };
        // Released first: the checks below ask whether ANOTHER operation holds
        // the sheets, and the run's own registration would answer yes.
        self.document_state.pending_path_ops.finish(job.guard);
        let Some(waiting) = job.waiting.take() else {
            return self.number_read_sheets(job, result).1;
        };
        // The operand form (#2470): its caller is answered with each change.
        let target = job.read.anchor.clone().unwrap_or_default();
        let mut follow = Task::none();
        let answered = self.finish_waiting_command(waiting, &target, |app| {
            let (outcome, task) = app.annotate_and_answer(job, result);
            follow = task;
            outcome
        });
        Task::batch([answered, follow])
    }

    /// Number the sheets of `result` as `job` asks, or change nothing and
    /// say why. Also returns the designators the sheets had before, when
    /// the numbering ran (`None`: it was refused).
    fn number_read_sheets(
        &mut self,
        mut job: AnnotateJob,
        result: &Result<SheetsRead, String>,
    ) -> (
        Option<super::annotate_command::SheetDesignators>,
        Task<Message>,
    ) {
        let label = job.kind.label();
        let set = match self.accept_read(job.kind, &job.read, result) {
            Ok(set) => set,
            Err(Refused::Reported) => return (None, Task::none()),
            Err(Refused::Because(why)) => {
                self.refuse_run(label, &format!("{why}. Nothing was changed; run it again."));
                return (None, Task::none());
            }
        };
        let before = self.designators_before(&set, job.read.anchor.as_deref());
        let task = match job.kind {
            PassKind::Annotate(mode) => self.apply_annotate(mode, set, job.read.anchor.as_deref()),
            PassKind::ResetDuplicates => self.apply_reset_duplicates(set),
            PassKind::Reprefix => {
                let Some(confirm) = job.reprefix.take() else {
                    return (None, Task::none());
                };
                self.apply_reprefix(confirm, set)
            }
        };
        let refresh = self.finish_run_view();
        (Some(before), Task::batch([task, refresh]))
    }

    /// [`Self::number_read_sheets`] for the operand form (#2470): what its
    /// caller is answered with, each designator the run changed. A run that
    /// changed nothing and raised a notice is refused with it.
    fn annotate_and_answer(
        &mut self,
        job: AnnotateJob,
        result: &Result<SheetsRead, String>,
    ) -> (crate::app::command::CommandOutcome, Task<Message>) {
        use crate::app::command::CommandOutcome;
        let label = job.kind.label();
        let mode = match job.kind {
            PassKind::Annotate(mode) => Some(mode),
            PassKind::ResetDuplicates | PassKind::Reprefix => None,
        };
        let notice = self.document_state.error_notice.clone();
        let (before, task) = self.number_read_sheets(job, result);
        let raised = self.document_state.error_notice != notice;
        let refused = |line: String| super::annotate_command::annotate_refused(line);
        let Some(before) = before else {
            return (refused(self.notice_line(label)), task);
        };
        let after = self.designators_now(&before);
        let changes = super::annotate_command::designator_changes(&before, &after);
        if changes.is_empty() && raised {
            return (refused(self.notice_line(label)), task);
        }
        let mut answer = super::annotate_command::answer(mode, &changes);
        if raised {
            answer["notice"] = serde_json::json!(self.notice_line(label));
        }
        (CommandOutcome::Answered(answer), task)
    }

    /// Everything the numbering relies on must still hold when the sheets
    /// arrive; otherwise the whole run is dropped (all or nothing: a partial
    /// run could hand two sheets the same designator). The returned set has
    /// every open sheet read from its live engine, so an edit made while the
    /// worker read is numbered, not overwritten.
    pub(super) fn accept_read(
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
            unreadable_causes: sheets.unreadable_causes.clone(),
            absent: sheets.absent.clone(),
            refused: sheets.refused.clone(),
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
        // A run a `document` operand named (#2470) follows that sheet, not
        // the active tab.
        let active = match (read.anchor.as_deref(), ds.active_path.as_deref()) {
            (Some(anchor), _) if ds.engines.contains_key(anchor) => anchor,
            (Some(_), _) => return stale("the sheet the run was named for is no longer open"),
            (None, Some(active)) => active,
            (None, None) => return stale("no sheet is open any more"),
        };
        let project = ds.document_project(active).map(|p| p.id);
        if project != read.project
            || project_sheets::resolve_netlist_root_for(ds, active) != read.root
        {
            return stale("the active project changed while the sheets were being read");
        }
        Ok(())
    }

    /// The run is over: the preview rows are stale, so reload them.
    pub(super) fn finish_run_view(&mut self) -> Task<Message> {
        self.refresh_panel_ctx();
        self.invalidate_annotate_preview();
        self.refresh_annotate_preview()
    }
}
