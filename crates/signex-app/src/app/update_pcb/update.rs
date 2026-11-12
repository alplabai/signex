//! Opening the dialog, reading the design off the update thread and the
//! row ticks. Validate, Execute, Report and the fix links: `execute.rs`.

use std::path::Path;
use std::sync::Arc;

use iced::Task;
use signex_engine::pcb::eco::{EcoChangeList, refresh_nets};
use signex_i18n::{MessageId, ids, t};

use super::UpdatePcbMsg;
use super::design::{self, DesignRequest, LibraryRows};
use super::state::{Phase, UpdatePcbState, file_name};
use crate::app::project_sheets::{plan_project_sheets_for, project_root_sheet_path};
use crate::app::state::{ErrorNotice, ProjectId};
use crate::app::{Message, Signex};

impl Signex {
    pub(crate) fn dispatch_update_pcb_message(&mut self, msg: UpdatePcbMsg) -> Task<Message> {
        use UpdatePcbMsg as M;
        match msg {
            M::Requested { apply } => self
                .open_update_pcb_from_view(apply)
                .unwrap_or_else(|why| self.refuse_update_pcb(&why)),
            M::RequestedForProject(project) => self
                .open_update_pcb(project, false)
                .unwrap_or_else(|why| self.refuse_update_pcb(&why)),
            M::Computed { job, result } => self.update_pcb_computed(job, result),
            M::NetsRefreshed { job, list } => {
                if let Some(state) = self.ui_state.update_pcb.as_mut()
                    && state.job == job
                {
                    state.list = Some(Arc::unwrap_or_clone(list));
                    state.phase = Phase::Ready;
                }
                Task::none()
            }
            M::RowToggled { key, on } => self.tick_update_pcb(|list| list.set_on(&key, on)),
            M::GroupToggled { group, on } => {
                self.tick_update_pcb(|list| list.set_group_on(group, on))
            }
            M::RefreshRequested => {
                let previous = self
                    .ui_state
                    .update_pcb
                    .as_ref()
                    .and_then(|s| s.list.clone());
                self.start_update_pcb_read(previous)
            }
            M::ValidateRequested => {
                self.validate_update_pcb();
                Task::none()
            }
            M::ExecuteRequested => self.execute_update_pcb(),
            M::ReportRequested => self.pick_update_pcb_report(),
            M::ReportPathPicked(path) => self.write_update_pcb_report(path),
            M::ReportWritten(result) => {
                self.update_pcb_report_written(result);
                Task::none()
            }
            M::AnnotateRequested => self.annotate_from_update_pcb(),
            M::CrossProbeRequested(key) => {
                self.cross_probe_from_update_pcb(&key);
                Task::none()
            }
            M::ReopenRequested => self.reopen_update_pcb_board(),
            M::CreatePcbRequested => {
                let project = self.ui_state.update_pcb.as_ref().map(|s| s.project);
                self.open_document_setup(crate::app::SetupKind::Pcb, project)
            }
            M::CloseRequested => {
                self.ui_state.update_pcb = None;
                Task::none()
            }
            M::NewPcbRequested(project) => {
                self.open_document_setup(crate::app::SetupKind::Pcb, project)
            }
            M::NewPcbPathChecked {
                project,
                path,
                exists,
                rules,
            } => {
                self.new_pcb_path_checked(project, path, exists, rules);
                Task::none()
            }
        }
    }

    /// Open Update PCB for the project of the document in view.
    pub(crate) fn open_update_pcb_from_view(
        &mut self,
        apply: bool,
    ) -> Result<Task<Message>, String> {
        let project = self
            .update_pcb_source()
            .and_then(|path| self.owning_project(&path).map(|p| p.id))
            .ok_or_else(|| self.update_pcb_text(ids::UPDATE_PCB_NOT_IN_PROJECT))?;
        self.open_update_pcb(project, apply)
    }

    /// Open Update PCB for `project`: open its board when it is not, then
    /// read the design. `apply` executes the default ticks without a dialog.
    fn open_update_pcb(
        &mut self,
        project: ProjectId,
        apply: bool,
    ) -> Result<Task<Message>, String> {
        let loaded = self
            .document_state
            .project_by_id(project)
            .ok_or_else(|| self.update_pcb_text(ids::UPDATE_PCB_NOT_IN_PROJECT))?;
        let name = file_name(&loaded.path);
        let board = Self::project_board(loaded);
        if apply && board.is_none() {
            return Err(self.update_pcb_text(ids::UPDATE_PCB_NO_BOARD));
        }
        self.ui_state.update_pcb = Some(UpdatePcbState::new(project, name, board.clone(), apply));
        match board {
            Some(board) => Ok(self.begin_update_pcb_board(&board)),
            None => Ok(Task::none()),
        }
    }

    /// The board is open: read the design. Else open it; the pass after the
    /// update starts the read once it is ([`Self::sync_update_pcb`]).
    fn begin_update_pcb_board(&mut self, board: &Path) -> Task<Message> {
        if self.document_state.board_engine(board).is_some() {
            return self.start_update_pcb_read(None);
        }
        if self.is_board_opening(board) {
            return Task::none();
        }
        match self.open_path_reported(board.to_path_buf()) {
            Ok(task) => task,
            Err(_) => {
                let why = self.board_missing(board);
                self.fail_update_pcb(why);
                Task::none()
            }
        }
    }

    pub(crate) fn is_board_opening(&self, board: &Path) -> bool {
        self.document_state.pending_opens.contains(board)
            || self.ui_state.pending_sheet_opens.contains_key(board)
    }

    fn board_missing(&self, board: &Path) -> String {
        t!(self.ui_state.i18n, ids::UPDATE_PCB_BOARD_MISSING, "board" => file_name(board))
            .into_string()
    }

    /// The pass after every update: a board the dialog waits for has opened
    /// (start the read) or its open failed (say so).
    pub(crate) fn sync_update_pcb(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.update_pcb.as_ref() else {
            return Task::none();
        };
        let (Phase::Opening, Some(board)) = (&state.phase, state.board.clone()) else {
            return Task::none();
        };
        if self.document_state.board_engine(&board).is_some() {
            return self.start_update_pcb_read(None);
        }
        if !self.is_board_opening(&board) {
            let why = self.board_missing(&board);
            self.fail_update_pcb(why);
        }
        Task::none()
    }

    /// Read the design and diff the board on the worker. `previous` is the
    /// list a Refresh replaces; the new one keeps its ticks.
    pub(super) fn start_update_pcb_read(
        &mut self,
        previous: Option<EcoChangeList>,
    ) -> Task<Message> {
        let Some(state) = self.ui_state.update_pcb.as_ref() else {
            return Task::none();
        };
        let (Some(board_path), project_id) = (state.board.clone(), state.project) else {
            return Task::none();
        };
        let Some(engine) = self.document_state.board_engine(&board_path) else {
            return Task::none();
        };
        let (board, revision) = (engine.board().clone(), engine.revision());
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return Task::none();
        };
        let Some(root) = project_root_sheet_path(project) else {
            let why = self.update_pcb_text(ids::UPDATE_PCB_NO_SCHEMATIC);
            self.fail_update_pcb(why);
            return Task::none();
        };
        let project_dir = project.dir().to_path_buf();
        let request = DesignRequest {
            plan: plan_project_sheets_for(&self.document_state, &root),
            project_dir,
            board,
            revision,
            libraries: self.library_rows(),
            open_revisions: self
                .document_state
                .engines
                .iter()
                .map(|(path, engine)| (path.clone(), engine.revision()))
                .collect(),
            previous,
        };
        let Some(state) = self.ui_state.update_pcb.as_mut() else {
            return Task::none();
        };
        let job = state.next_job();
        state.phase = Phase::Computing;
        state.validation = None;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || design::compute(request))
                    .await
                    .unwrap_or_else(|e| Err(format!("the worker stopped: {e}")))
            },
            move |result| Message::from(UpdatePcbMsg::Computed { job, result }),
        )
    }

    /// Every open library's rows, for the worker to resolve footprints from.
    pub(crate) fn library_rows(&self) -> Vec<LibraryRows> {
        self.library
            .open_libraries
            .iter()
            .map(|lib| LibraryRows {
                id: lib.library_id,
                root: lib.root.clone(),
                rows: lib.tables.values().flatten().cloned().collect(),
            })
            .collect()
    }

    fn update_pcb_computed(
        &mut self,
        job: u64,
        result: Result<Arc<super::Computed>, String>,
    ) -> Task<Message> {
        let Some(state) = self.ui_state.update_pcb.as_mut() else {
            return Task::none();
        };
        if state.job != job {
            return Task::none();
        }
        match result {
            Ok(computed) => {
                for (library, why) in &computed.snapshot.library_errors {
                    if !state.logged_library_errors.insert(library.clone()) {
                        continue;
                    }
                    crate::diagnostics::log_warning(
                        t!(
                            self.ui_state.i18n,
                            ids::UPDATE_PCB_LIBRARY_NOT_OPENED,
                            "library" => library.display().to_string(),
                            "why" => why.clone()
                        )
                        .into_string(),
                    );
                }
                state.snapshot = Some(Arc::clone(&computed.snapshot));
                state.list = Some(computed.list.clone());
                state.phase = Phase::Ready;
                if state.apply {
                    return self.execute_update_pcb();
                }
            }
            Err(why) => self.fail_update_pcb(why),
        }
        Task::none()
    }

    /// Tick through `change`; when a footprint row changed, the net rows
    /// are recomputed on the worker for the board the ticks now make.
    fn tick_update_pcb(
        &mut self,
        change: impl FnOnce(&mut EcoChangeList) -> bool,
    ) -> Task<Message> {
        let Some(state) = self.ui_state.update_pcb.as_mut() else {
            return Task::none();
        };
        let Some(list) = state.list.as_mut() else {
            return Task::none();
        };
        state.validation = None;
        if !change(list) {
            return Task::none();
        }
        let (Some(snapshot), Some(list)) = (state.snapshot.clone(), state.list.clone()) else {
            return Task::none();
        };
        let job = state.next_job();
        state.phase = Phase::Computing;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut list = list;
                    refresh_nets(&mut list, &snapshot.input());
                    list
                })
                .await
            },
            move |result| {
                Message::from(match result {
                    Ok(list) => UpdatePcbMsg::NetsRefreshed {
                        job,
                        list: Arc::new(list),
                    },
                    Err(e) => UpdatePcbMsg::Computed {
                        job,
                        result: Err(format!("the worker stopped: {e}")),
                    },
                })
            },
        )
    }

    /// The read failed: Messages and the dialog say why; a headless run
    /// ends here.
    pub(super) fn fail_update_pcb(&mut self, why: String) {
        if self.ui_state.update_pcb.is_none() {
            return;
        }
        self.log_update_pcb_refused(&why);
        let Some(state) = self.ui_state.update_pcb.as_mut() else {
            return;
        };
        if state.apply {
            self.ui_state.update_pcb = None;
        } else {
            state.phase = Phase::Failed(why);
        }
    }

    pub(super) fn log_update_pcb_refused(&self, why: &str) {
        crate::diagnostics::log_warning(
            t!(self.ui_state.i18n, ids::UPDATE_PCB_REFUSED, "why" => why).into_string(),
        );
    }

    /// Update PCB cannot start: say why in Messages and on the notice card.
    fn refuse_update_pcb(&mut self, why: &str) -> Task<Message> {
        self.log_update_pcb_refused(why);
        let title = self.update_pcb_text(ids::UPDATE_PCB_TITLE);
        self.document_state.error_notice = Some(ErrorNotice::new(title, why.to_string()));
        Task::none()
    }

    /// A catalog message with no arguments, as a string.
    pub(super) fn update_pcb_text(&self, id: MessageId) -> String {
        t!(self.ui_state.i18n, id).into_string()
    }
}
