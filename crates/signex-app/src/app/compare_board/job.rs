//! A command-line run of Compare Board with Schematic (#2462): the dialog's
//! reads on the worker, its report built on the update thread, the net rows
//! (and an Apply's directions) computed back on a worker, then the answer.
//! An Apply plans both sides as the dialog does, states the closed sheets
//! the schematic side reaches, and applies through the dialog's own path
//! (`apply::apply_compare_plan`). The job travels in its messages; nothing
//! reaches the dialog. The caller waits on the ticket (`agent::deferred`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use serde_json::{Value, json};
use signex_engine::pcb::eco::{ChooseRefusal, CompareReport, Direction};
use signex_i18n::{ids, t};

use super::CompareBoardMsg;
use super::apply::AppliedCompare;
use super::command::command_refused;
use super::read::{CompareRead, compute};
use super::state::PendingApply;
use crate::app::agent::WaitingCommand;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::path_op_jobs::run_blocking;
use crate::app::sheet_write::disk_changed;
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// What the run answers.
#[derive(Debug, Clone)]
pub(crate) enum JobMode {
    /// `check_board_against_schematic`: the report.
    Check,
    /// `apply_board_schematic_differences`: the rows named, each with its
    /// direction; every other row is skipped.
    Apply(Vec<(String, Direction)>),
    /// `board_status`: the counters read when it was asked, given the
    /// board against the schematic once the report lands.
    Status(Value),
}

/// One run, from the request to the answer.
#[derive(Debug, Clone)]
pub(crate) struct CompareJob {
    pub(crate) waiting: WaitingCommand,
    pub(crate) project: ProjectId,
    pub(crate) board: PathBuf,
    pub(crate) mode: JobMode,
    pub(crate) read: Option<Arc<CompareRead>>,
    pub(crate) report: Option<CompareReport>,
    pub(crate) pending: Option<PendingApply>,
}

/// A run in a message: equal only to itself, as a read is.
#[derive(Clone)]
pub struct JobRef(Arc<CompareJob>);

impl std::fmt::Debug for JobRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobRef")
            .field("ticket", &self.0.waiting.ticket)
            .finish_non_exhaustive()
    }
}

impl PartialEq for JobRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// What landed for a command-line run, namespaced under
/// `CompareBoardMsg::Job`.
#[derive(Debug, Clone, PartialEq)]
pub enum CompareJobMsg {
    /// Both reads landed.
    Read {
        job: JobRef,
        result: Result<Arc<CompareRead>, String>,
    },
    /// The net rows (and an Apply's directions) were computed.
    Reported {
        job: JobRef,
        result: Result<CompareReport, String>,
    },
    /// The closed sheets an Apply writes were stated again: the files that
    /// changed on disk since the read.
    DiskChecked {
        job: JobRef,
        result: Result<Vec<PathBuf>, String>,
    },
}

impl From<CompareJobMsg> for Message {
    fn from(msg: CompareJobMsg) -> Self {
        Message::from(CompareBoardMsg::Job(msg))
    }
}

impl Signex {
    /// Start a run of `command` for the open `board` of `project`: the
    /// caller is answered when it ends ([`CommandOutcome::Deferred`]), or
    /// refused at once when the reads cannot start.
    pub(crate) fn start_compare_job(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
        target: (ProjectId, PathBuf),
        mode: JobMode,
    ) -> (CommandOutcome, Task<Message>) {
        let (project, board) = target;
        let request = match self.compare_request(project, &board) {
            Ok(request) => request,
            Err(why) => return (command_refused(why), Task::none()),
        };
        let waiting = self.wait_for_command(command, args);
        let ticket = waiting.ticket;
        let job = JobRef(Arc::new(CompareJob {
            waiting,
            project,
            board,
            mode,
            read: None,
            report: None,
            pending: None,
        }));
        let task = Task::perform(run_blocking(move || compute(request)), move |result| {
            let result = result.and_then(|read| read).map(Arc::new);
            Message::from(CompareJobMsg::Read { job, result })
        });
        (CommandOutcome::Deferred { ticket }, task)
    }

    pub(super) fn dispatch_compare_job(&mut self, msg: CompareJobMsg) -> Task<Message> {
        match msg {
            CompareJobMsg::Read { job, result } => self.compare_job_read(job, result),
            CompareJobMsg::Reported { job, result } => self.compare_job_reported(job, result),
            CompareJobMsg::DiskChecked { job, result } => {
                self.compare_job_disk_checked(job, result)
            }
        }
    }

    /// The reads landed: the report, then its net rows on the worker.
    fn compare_job_read(
        &mut self,
        job: JobRef,
        result: Result<Arc<CompareRead>, String>,
    ) -> Task<Message> {
        let mut job = Arc::unwrap_or_clone(job.0);
        let read = match result {
            Ok(read) => read,
            Err(why) => return self.end_compare_job(job.waiting, &job.board, Err(why)),
        };
        let report = self.compare_report_of(job.project, &read);
        let picks = match &job.mode {
            JobMode::Apply(picks) => Some(picks.clone()),
            JobMode::Check | JobMode::Status(_) => None,
        };
        job.read = Some(Arc::clone(&read));
        let job = JobRef(Arc::new(job));
        Task::perform(
            run_blocking(move || report_for(report, &read, picks.as_deref())),
            move |result| {
                let result = result.and_then(|report| report);
                Message::from(CompareJobMsg::Reported { job, result })
            },
        )
    }

    /// The report is ready: answer it, or plan the Apply.
    fn compare_job_reported(
        &mut self,
        job: JobRef,
        result: Result<CompareReport, String>,
    ) -> Task<Message> {
        let job = Arc::unwrap_or_clone(job.0);
        let report = match result {
            Ok(report) => report,
            Err(why) => return self.end_compare_job(job.waiting, &job.board, Err(why)),
        };
        match job.mode.clone() {
            JobMode::Check => {
                let answer = self.compare_answer(job.project, &job.board, &report);
                self.end_compare_job(job.waiting, &job.board, Ok(answer))
            }
            JobMode::Status(mut status) => {
                status["schematic"] = json!({
                    "matches": report.in_sync(),
                    "differences": report.rows.len(),
                    "summary": report.summary(),
                    "revision": report.revision(),
                });
                self.end_compare_job(job.waiting, &job.board, Ok(status))
            }
            JobMode::Apply(_) => self.plan_compare_job(job, report),
        }
    }

    /// The query's answer: the report with the board and project it read.
    fn compare_answer(&self, project: ProjectId, board: &Path, report: &CompareReport) -> Value {
        let mut answer = report.answer();
        answer["board"] = json!(board.display().to_string());
        answer["project"] = json!(
            self.document_state
                .project_by_id(project)
                .map(|p| p.path.display().to_string())
        );
        answer
    }

    /// Plan both sides from the directions, as the dialog's Apply does, and
    /// state the closed sheets the schematic side reaches.
    fn plan_compare_job(&mut self, mut job: CompareJob, report: CompareReport) -> Task<Message> {
        let Some(read) = job.read.clone() else {
            let why = "the reads of the comparison were lost".to_string();
            return self.end_compare_job(job.waiting, &job.board, Err(why));
        };
        if let Some(row) = report.rows.iter().find(|row| report.waits(row)) {
            let why = format!(
                "the difference {} goes to the board only with a difference it needs; name that one in `to_board` too",
                row.token()
            );
            return self.end_compare_job(job.waiting, &job.board, Err(why));
        }
        let (eco, back) = report.ticks();
        let edits = match self.back_annotate_edits(job.project, &back, &read.back) {
            Ok(edits) => edits,
            Err(why) => return self.end_compare_job(job.waiting, &job.board, Err(why)),
        };
        let to_check = self.closed_sheets_to_check(&read.back, &edits);
        let pending = PendingApply {
            eco,
            back,
            edits,
            checked: to_check.iter().map(|(file, _)| file.clone()).collect(),
        };
        job.report = Some(report);
        if to_check.is_empty() {
            return self.apply_compare_job(job, pending);
        }
        job.pending = Some(pending);
        let job = JobRef(Arc::new(job));
        Task::perform(
            run_blocking(move || disk_changed(&to_check)),
            move |result| Message::from(CompareJobMsg::DiskChecked { job, result }),
        )
    }

    /// The closed sheets were stated again: apply, or refuse when one
    /// changed on disk since the read.
    fn compare_job_disk_checked(
        &mut self,
        job: JobRef,
        result: Result<Vec<PathBuf>, String>,
    ) -> Task<Message> {
        let mut job = Arc::unwrap_or_clone(job.0);
        let refusal = match (result, job.pending.take()) {
            (Err(why), _) => why,
            (Ok(changed), _) if !changed.is_empty() => t!(
                self.ui_state.i18n,
                ids::COMPARE_BOARD_DISK_CHANGED,
                "path" => changed[0].display().to_string()
            )
            .into_string(),
            (Ok(_), Some(pending)) => return self.apply_compare_job(job, pending),
            (Ok(_), None) => "the plan of the Apply was lost".to_string(),
        };
        self.end_compare_job(job.waiting, &job.board, Err(refusal))
    }

    /// Apply `pending` through the dialog's path, after the gate of the
    /// waiting caller, and answer what changed.
    fn apply_compare_job(&mut self, job: CompareJob, pending: PendingApply) -> Task<Message> {
        let CompareJob {
            waiting,
            board,
            read,
            report,
            ..
        } = job;
        let (Some(read), Some(report)) = (read, report) else {
            let why = "the report of the comparison was lost".to_string();
            return self.end_compare_job(waiting, &board, Err(why));
        };
        let target = board.clone();
        self.finish_waiting_command(waiting, &target, move |app| {
            let applied = match app.compare_job_block(&board, &report, &read) {
                Some(why) => Err(why),
                None => app.apply_compare_plan(&board, &read, pending),
            };
            match applied {
                Ok(applied) => {
                    CommandOutcome::Answered(app.applied_answer(&board, &report, applied))
                }
                Err(why) => command_refused(why),
            }
        })
    }

    /// Why an Apply of `report` cannot run now: the board is parked,
    /// read-only or gone, or the report is stale.
    fn compare_job_block(
        &self,
        board: &Path,
        report: &CompareReport,
        read: &CompareRead,
    ) -> Option<String> {
        if let Some(why) = self.board_block_in(board, &self.ui_state.i18n) {
            return Some(why);
        }
        self.compare_read_stale(board, report, read)
            .then(|| t!(self.ui_state.i18n, ids::COMPARE_BOARD_STALE).into_string())
    }

    /// What an Apply answers: the rows taken each way, the board deletions
    /// among them, the changes and sheets written, and the board's revision
    /// after it.
    fn applied_answer(
        &self,
        board: &Path,
        report: &CompareReport,
        applied: AppliedCompare,
    ) -> Value {
        let tokens = |direction: Direction| -> Vec<String> {
            report
                .rows
                .iter()
                .filter(|row| row.direction == direction)
                .map(|row| row.token())
                .collect()
        };
        let removed: Vec<String> = report
            .board_deletions()
            .iter()
            .map(|row| row.token())
            .collect();
        json!({
            "board": board.display().to_string(),
            "read_at": report.revision(),
            "revision": self.document_state.board_engine(board).map(|engine| engine.revision()),
            "to_board": tokens(Direction::UpdatePcb),
            "to_schematic": tokens(Direction::UpdateSchematic),
            "removed_from_board": removed,
            "board_changes": applied.to_pcb,
            "schematic_changes": applied.to_schematic,
            "sheets": applied.sheets,
        })
    }

    /// Answer the caller waiting on `waiting` with `answer` (`Err`: the
    /// refusal).
    fn end_compare_job(
        &mut self,
        waiting: WaitingCommand,
        board: &Path,
        answer: Result<Value, String>,
    ) -> Task<Message> {
        self.finish_waiting_command(waiting, board, move |_| match answer {
            Ok(value) => CommandOutcome::Answered(value),
            Err(why) => command_refused(why),
        })
    }
}

/// The report's net rows for its directions, on the worker. With `picks`,
/// every row is skipped but those named; when a footprint row they name
/// changes the net rows, those are computed again and the choice made
/// again on them.
fn report_for(
    mut report: CompareReport,
    read: &CompareRead,
    picks: Option<&[(String, Direction)]>,
) -> Result<CompareReport, String> {
    let input = read.pcb.snapshot.input();
    report.refresh_nets(&input);
    let Some(picks) = picks else {
        return Ok(report);
    };
    if report
        .choose(picks)
        .map_err(|refusal| refusal.to_string())?
    {
        report.refresh_nets(&input);
        report.choose(picks).map_err(|refusal| match refusal {
            ChooseRefusal::UnknownRow(row) => format!(
                "the difference {row} changed with the footprint differences named; apply those first, then check again"
            ),
            other => other.to_string(),
        })?;
    }
    Ok(report)
}
