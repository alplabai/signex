//! Opening the dialog, opening the board when it is not, both reads on the
//! worker, the directions and their net refresh, Export report… and the
//! jump links. Apply: `apply.rs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_engine::pcb::eco::{CompareJump, CompareReport, Direction};
use signex_i18n::{MessageId, ids, t};
use signex_net::cross_probe::PartOccurrence;

use super::read::{CompareRead, compute};
use super::{CompareBoardMsg, CompareBoardState, JumpSide};
use crate::app::project_sheets::project_root_sheet_path;
use crate::app::state::ErrorNotice;
use crate::app::state::cross_select::{CrossSelection, PendingProbe};
use crate::app::update_pcb::{Phase, file_name};
use crate::app::{Message, Signex};

/// The file name Export report… suggests.
const REPORT_FILE: &str = "compare-board-report.txt";

impl Signex {
    pub(crate) fn dispatch_compare_board_message(&mut self, msg: CompareBoardMsg) -> Task<Message> {
        use CompareBoardMsg as M;
        // While Apply states the closed sheets, its plan is fixed.
        let checking = self
            .ui_state
            .compare_board
            .as_ref()
            .is_some_and(|s| s.checking);
        if checking
            && matches!(
                msg,
                M::RowDirectionPicked { .. }
                    | M::ShownRowsDirectionPicked(_)
                    | M::CompareAgainRequested
                    | M::ApplyRequested
                    | M::ApplyConfirmed
            )
        {
            return Task::none();
        }
        // #2749: a pick made while a job runs would be lost when it lands,
        // or would supersede a Compare again.
        let directions_live = self
            .ui_state
            .compare_board
            .as_ref()
            .is_some_and(|s| s.directions_live());
        if !directions_live
            && matches!(
                msg,
                M::RowDirectionPicked { .. } | M::ShownRowsDirectionPicked(_)
            )
        {
            return Task::none();
        }
        match msg {
            M::Requested => self.open_compare_board(),
            M::Read { job, result } => self.compare_board_read(job, result),
            M::NetsRefreshed { job, report } => {
                if let Some(state) = self.ui_state.compare_board.as_mut()
                    && state.job == job
                {
                    state.report = Some(Arc::unwrap_or_clone(report));
                    state.phase = Phase::Ready;
                }
                Task::none()
            }
            M::KindFilterPicked(kind) => {
                if let Some(state) = self.ui_state.compare_board.as_mut() {
                    state.filter = kind;
                    state.shown_choice = None;
                }
                Task::none()
            }
            M::RowDirectionPicked { row, direction } => {
                self.pick_compare_direction(None, |report| report.set_direction(&row, direction))
            }
            M::ShownRowsDirectionPicked(direction) => {
                let filter = self.ui_state.compare_board.as_ref().and_then(|s| s.filter);
                self.pick_compare_direction(Some(direction), |report| {
                    report.set_shown(filter, direction)
                })
            }
            M::CompareAgainRequested => self.start_compare_read(),
            M::ExportRequested => self.pick_compare_report(),
            M::ExportPathPicked(path) => self.write_compare_report(path),
            M::Exported(result) => {
                self.compare_report_written(result);
                Task::none()
            }
            M::ApplyRequested => self.request_compare_apply(),
            M::ApplyConfirmed => {
                if let Some(state) = self.ui_state.compare_board.as_mut() {
                    state.confirming = false;
                }
                self.start_compare_apply()
            }
            M::ApplyCancelled => {
                if let Some(state) = self.ui_state.compare_board.as_mut() {
                    state.confirming = false;
                }
                Task::none()
            }
            M::DiskChecked { job, result } => self.compare_disk_checked(job, result),
            M::JumpRequested { side, target } => {
                self.jump_from_compare(side, &target);
                Task::none()
            }
            M::CloseRequested => {
                self.ui_state.compare_board = None;
                Task::none()
            }
            M::Job(msg) => self.dispatch_compare_job(msg),
        }
    }

    /// Open the dialog for the project of the document in view and read
    /// it, opening the project's board first when it is not open.
    fn open_compare_board(&mut self) -> Task<Message> {
        let id = self
            .update_pcb_source()
            .and_then(|path| self.owning_project(&path).map(|p| p.id));
        let Some(project) = id.and_then(|id| self.document_state.project_by_id(id)) else {
            return self.refuse_compare_board(ids::COMPARE_BOARD_NOT_IN_PROJECT);
        };
        let (id, board, root) = (
            project.id,
            Self::project_board(project),
            project_root_sheet_path(project),
        );
        let Some(board) = board else {
            return self.refuse_compare_board(ids::COMPARE_BOARD_NO_BOARD);
        };
        let Some(root) = root else {
            return self.refuse_compare_board(ids::COMPARE_BOARD_NO_SCHEMATIC);
        };
        self.interaction_state.context_menu = None;
        self.ui_state.compare_board = Some(Box::new(CompareBoardState::new(
            id,
            file_name(&root),
            board.clone(),
        )));
        if self.document_state.board_engine(&board).is_some() {
            return self.start_compare_read();
        }
        if self.is_board_opening(&board) {
            return Task::none();
        }
        match self.open_path_reported(board.clone()) {
            Ok(task) => task,
            Err(_) => {
                self.fail_compare_board(self.compare_board_missing(&board));
                Task::none()
            }
        }
    }

    /// The pass after every update: a board the dialog waits for has opened
    /// (start the read) or its open failed (say so).
    pub(crate) fn sync_compare_board(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.compare_board.as_ref() else {
            return Task::none();
        };
        if !matches!(state.phase, Phase::Opening) {
            return Task::none();
        }
        let board = state.board.clone();
        if self.document_state.board_engine(&board).is_some() {
            return self.start_compare_read();
        }
        if !self.is_board_opening(&board) {
            self.fail_compare_board(self.compare_board_missing(&board));
        }
        Task::none()
    }

    /// Read the design both ways on the worker. The report shown so far is
    /// kept until the new one lands, which keeps its directions.
    pub(super) fn start_compare_read(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.compare_board.as_ref() else {
            return Task::none();
        };
        let (board_path, project_id) = (state.board.clone(), state.project);
        let request = match self.compare_request(project_id, &board_path) {
            Ok(request) => request,
            Err(why) => {
                self.fail_compare_board(why);
                return Task::none();
            }
        };
        let Some(state) = self.ui_state.compare_board.as_mut() else {
            return Task::none();
        };
        let job = state.next_job();
        state.phase = Phase::Computing;
        state.error = None;
        state.confirming = false;
        // A check of the old report can no longer land (its job is stale).
        state.checking = false;
        state.pending = None;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || compute(request))
                    .await
                    .unwrap_or_else(|e| Err(format!("the worker stopped: {e}")))
            },
            move |result| {
                Message::from(CompareBoardMsg::Read {
                    job,
                    result: result.map(Arc::new),
                })
            },
        )
    }

    /// Both reads landed: build the report (the shared-sheet check needs the
    /// open projects), keep the directions of the report it replaces, and
    /// compute its net rows on the worker.
    fn compare_board_read(
        &mut self,
        job: u64,
        result: Result<Arc<CompareRead>, String>,
    ) -> Task<Message> {
        let Some(owner) = self
            .ui_state
            .compare_board
            .as_ref()
            .filter(|state| state.job == job)
            .map(|state| state.project)
        else {
            return Task::none();
        };
        let read = match result {
            Ok(read) => read,
            Err(why) => {
                self.fail_compare_board(why);
                return Task::none();
            }
        };
        self.log_compare_library_errors(&read);
        let mut report = self.compare_report_of(owner, &read);
        let Some(state) = self.ui_state.compare_board.as_mut() else {
            return Task::none();
        };
        if let Some(previous) = &state.report {
            report.keep_choices_from(previous);
        }
        state.read = Some(Arc::clone(&read));
        state.compared_at = Some(chrono::Local::now().format("%H:%M:%S").to_string());
        refresh_on_worker(state, read, report)
    }

    /// Each library whose adapter did not open, once per dialog.
    fn log_compare_library_errors(&mut self, read: &CompareRead) {
        let Some(state) = self.ui_state.compare_board.as_mut() else {
            return;
        };
        for (library, why) in &read.pcb.snapshot.library_errors {
            if !state.logged_library_errors.insert(library.clone()) {
                continue;
            }
            crate::diagnostics::log_warning(
                t!(
                    self.ui_state.i18n,
                    ids::COMPARE_BOARD_LIBRARY_NOT_OPENED,
                    "library" => library.display().to_string(),
                    "why" => why.clone()
                )
                .into_string(),
            );
        }
    }

    /// Change directions through `change`. `shown` is the "Set shown rows"
    /// choice to highlight (`None`: a row set by hand). When a footprint row
    /// changed, the net rows are recomputed on the worker; a newer job
    /// supersedes it.
    fn pick_compare_direction(
        &mut self,
        shown: Option<Direction>,
        change: impl FnOnce(&mut CompareReport) -> bool,
    ) -> Task<Message> {
        let Some(state) = self.ui_state.compare_board.as_mut() else {
            return Task::none();
        };
        let Some(report) = state.report.as_mut() else {
            return Task::none();
        };
        let refresh = change(report);
        state.shown_choice = shown;
        state.error = None;
        if !refresh {
            return Task::none();
        }
        let (Some(read), Some(report)) = (state.read.clone(), state.report.clone()) else {
            return Task::none();
        };
        refresh_on_worker(state, read, report)
    }

    /// Export report…: ask where to save the report as text.
    fn pick_compare_report(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.compare_board.as_ref() else {
            return Task::none();
        };
        if state.ready_report().is_none() {
            return Task::none();
        }
        let dir = state.board.parent().map(Path::to_path_buf);
        let i18n = &self.ui_state.i18n;
        let base = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::COMPARE_BOARD_EXPORT_TITLE)
            .filter_in(i18n, ids::FILE_FILTER_TEXT, &["txt"]);
        Task::perform(
            async move {
                let mut dialog = base.set_file_name(REPORT_FILE);
                if let Some(dir) = dir {
                    dialog = dialog.set_directory(dir);
                }
                dialog.save_file().await
            },
            |path| Message::from(CompareBoardMsg::ExportPathPicked(path)),
        )
    }

    /// The user picked where the report goes: write it there, off the
    /// update thread, directions included.
    fn write_compare_report(&mut self, path: Option<PathBuf>) -> Task<Message> {
        let (Some(path), Some(report)) = (
            path,
            self.ui_state
                .compare_board
                .as_ref()
                .and_then(|s| s.report.as_ref()),
        ) else {
            return Task::none();
        };
        let text = report.report_text();
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || {
                signex_types::atomic_io::atomic_write(&path, text.as_bytes())
                    .map(|()| path)
                    .map_err(|e| e.to_string())
            }),
            |result| Message::from(CompareBoardMsg::Exported(result.unwrap_or_else(Err))),
        )
    }

    fn compare_report_written(&self, result: Result<PathBuf, String>) {
        let i18n = &self.ui_state.i18n;
        match result {
            Ok(path) => crate::diagnostics::log_info(
                t!(i18n, ids::COMPARE_BOARD_REPORT_SAVED, "path" => path.display().to_string())
                    .into_string(),
            ),
            Err(why) => crate::diagnostics::log_warning(
                t!(i18n, ids::COMPARE_BOARD_REPORT_FAILED, "why" => why).into_string(),
            ),
        }
    }

    /// A jump link: show the row's object on the sheet (zoomed and
    /// selected) or on the board, through the pending Cross Probe the pass
    /// after the update runs. The dialog stays open.
    fn jump_from_compare(&mut self, side: JumpSide, target: &CompareJump) {
        let Some(board) = self
            .ui_state
            .compare_board
            .as_ref()
            .map(|s| s.board.clone())
        else {
            return;
        };
        let linked = target.footprint.and_then(|id| self.linked_part(&board, id));
        let (part, net) = match side {
            JumpSide::Schematic => (target.part.clone().or(linked), target.schematic_net.clone()),
            JumpSide::Board => (
                linked.or_else(|| target.part.clone()),
                target.board_net.clone(),
            ),
        };
        self.interaction_state.cross_select.pending = Some(PendingProbe {
            what: CrossSelection {
                parts: part.into_iter().collect(),
                nets: net.into_iter().collect(),
            },
            to_board: side == JumpSide::Board,
            from: board,
            opening: None,
        });
    }

    /// The part the footprint `id` of `board` is linked to, by its first
    /// unit.
    fn linked_part(&self, board: &Path, id: uuid::Uuid) -> Option<PartOccurrence> {
        let engine = self.document_state.board_engine(board)?;
        let footprint = engine.board().footprints.iter().find(|f| f.uuid == id)?;
        let link = footprint.schematic_link.as_ref()?;
        link.units.first().map(|unit| PartOccurrence {
            sheet: link.sheet.clone(),
            symbol: *unit,
        })
    }

    /// The read or the board open failed: Messages and the dialog say why.
    pub(super) fn fail_compare_board(&mut self, why: String) {
        let line = t!(self.ui_state.i18n, ids::COMPARE_BOARD_FAILED, "why" => why.as_str());
        crate::diagnostics::log_warning(line.into_string());
        if let Some(state) = self.ui_state.compare_board.as_mut() {
            state.phase = Phase::Failed(why);
            state.report = None;
            state.read = None;
            state.confirming = false;
            state.checking = false;
            state.pending = None;
        }
    }

    /// No dialog can open: Messages and an error notice say why.
    fn refuse_compare_board(&mut self, id: MessageId) -> Task<Message> {
        // Messages stays en-US whatever the UI language.
        let why = signex_i18n::I18n::en_us().message_plain(id, &[]);
        crate::diagnostics::log_warning(
            signex_i18n::I18n::en_us()
                .message_plain(ids::COMPARE_BOARD_FAILED, &[("why", why.into())]),
        );
        self.document_view.error_notice = Some(ErrorNotice::new(ids::COMPARE_BOARD_TITLE, id));
        Task::none()
    }

    pub(super) fn compare_board_missing(&self, board: &Path) -> String {
        t!(self.ui_state.i18n, ids::UPDATE_PCB_BOARD_MISSING, "board" => file_name(board))
            .into_string()
    }
}

/// Compute the net rows of `report` for its directions on the worker, as a
/// new job of the dialog; the report shows again once they land.
fn refresh_on_worker(
    state: &mut CompareBoardState,
    read: Arc<CompareRead>,
    report: CompareReport,
) -> Task<Message> {
    let job = state.next_job();
    state.phase = Phase::Computing;
    Task::perform(
        crate::app::path_op_jobs::run_blocking(move || {
            let mut report = report;
            report.refresh_nets(&read.pcb.snapshot.input());
            report
        }),
        move |result| {
            Message::from(match result {
                Ok(report) => CompareBoardMsg::NetsRefreshed {
                    job,
                    report: Arc::new(report),
                },
                Err(why) => CompareBoardMsg::Read {
                    job,
                    result: Err(why),
                },
            })
        },
    )
}
