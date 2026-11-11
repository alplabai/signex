//! Opening the dialog, opening the board when it is not, the read on the
//! worker and the ticks. Validate and Execute: `apply.rs`.

use std::collections::HashSet;
use std::sync::Arc;

use iced::Task;
use signex_engine::pcb::eco::BackChangeList;
use signex_i18n::{MessageId, ids, t};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::read::{BackRequest, compute};
use super::{BackAnnotateMsg, BackAnnotateState};
use crate::app::project_sheets::{plan_project_sheets_for, project_root_sheet_path};
use crate::app::state::ErrorNotice;
use crate::app::update_pcb::{Phase, file_name};
use crate::app::{Message, Signex};

impl Signex {
    pub(crate) fn dispatch_back_annotate_message(&mut self, msg: BackAnnotateMsg) -> Task<Message> {
        use BackAnnotateMsg as M;
        // While Execute states the closed sheets, its plan is fixed: no
        // tick, Refresh, Validate or second Execute until the check lands.
        let checking = self
            .ui_state
            .back_annotate
            .as_ref()
            .is_some_and(|s| s.checking);
        if checking
            && matches!(
                msg,
                M::RowToggled { .. }
                    | M::GroupToggled { .. }
                    | M::RefreshRequested
                    | M::ValidateRequested
                    | M::ExecuteRequested
            )
        {
            return Task::none();
        }
        match msg {
            M::Requested => self.open_back_annotate(),
            M::Read { job, result } => {
                self.back_annotate_read(job, result);
                Task::none()
            }
            M::RowToggled { key, on } => {
                self.tick_back_annotate(|list| list.set_on(&key, on));
                Task::none()
            }
            M::GroupToggled { group, on } => {
                self.tick_back_annotate(|list| list.set_group_on(group, on));
                Task::none()
            }
            M::RefreshRequested => {
                let previous = self
                    .ui_state
                    .back_annotate
                    .as_ref()
                    .and_then(|s| s.list.clone());
                self.start_back_annotate_read(previous)
            }
            M::ValidateRequested => {
                self.validate_back_annotate();
                Task::none()
            }
            M::ExecuteRequested => self.execute_back_annotate(),
            M::DiskChecked { job, result } => self.back_annotate_disk_checked(job, result),
            M::CloseRequested => {
                self.ui_state.back_annotate = None;
                Task::none()
            }
        }
    }

    /// Open the dialog for the project of the document in view and read
    /// it, opening the project's board first when it is not open.
    fn open_back_annotate(&mut self) -> Task<Message> {
        let project = self
            .update_pcb_source()
            .and_then(|path| self.owning_project(&path).map(|p| (p.id, p)));
        let Some((id, project)) = project else {
            return self.refuse_back_annotate(ids::BACK_ANNOTATE_NOT_IN_PROJECT);
        };
        let name = file_name(&project.path);
        let board = Self::project_board(project);
        self.interaction_state.context_menu = None;
        self.ui_state.back_annotate = Some(BackAnnotateState::new(id, name, board.clone()));
        let Some(board) = board else {
            return Task::none();
        };
        if self.document_state.board_engine(&board).is_some() {
            return self.start_back_annotate_read(None);
        }
        if self.is_board_opening(&board) {
            return Task::none();
        }
        match self.open_path_reported(board.clone()) {
            Ok(task) => task,
            Err(_) => {
                self.fail_back_annotate(self.back_annotate_board_missing(&file_name(&board)));
                Task::none()
            }
        }
    }

    /// The pass after every update: a board the dialog waits for has opened
    /// (start the read) or its open failed (say so).
    pub(crate) fn sync_back_annotate(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.back_annotate.as_ref() else {
            return Task::none();
        };
        let (Phase::Opening, Some(board)) = (&state.phase, state.board.clone()) else {
            return Task::none();
        };
        if self.document_state.board_engine(&board).is_some() {
            return self.start_back_annotate_read(None);
        }
        if !self.is_board_opening(&board) {
            self.fail_back_annotate(self.back_annotate_board_missing(&file_name(&board)));
        }
        Task::none()
    }

    /// Read the design and diff it against the board on the worker.
    /// `previous` is the list a Refresh replaces; the new one keeps its ticks.
    pub(super) fn start_back_annotate_read(
        &mut self,
        previous: Option<BackChangeList>,
    ) -> Task<Message> {
        let Some(state) = self.ui_state.back_annotate.as_ref() else {
            return Task::none();
        };
        let (Some(board_path), project_id) = (state.board.clone(), state.project) else {
            return Task::none();
        };
        // A Refresh after the project or its board closed says why there
        // is no list, rather than keep the old one.
        let Some(project) = self.document_state.project_by_id(project_id) else {
            let why = t!(self.ui_state.i18n, ids::BACK_ANNOTATE_PROJECT_CLOSED).into_string();
            self.fail_back_annotate(why);
            return Task::none();
        };
        let Some(engine) = self.document_state.board_engine(&board_path) else {
            let why = self.back_annotate_board_missing(&file_name(&board_path));
            self.fail_back_annotate(why);
            return Task::none();
        };
        let (board, revision) = (engine.board().clone(), engine.revision());
        let Some(root) = project_root_sheet_path(project) else {
            let why = t!(self.ui_state.i18n, ids::UPDATE_PCB_NO_SCHEMATIC).into_string();
            self.fail_back_annotate(why);
            return Task::none();
        };
        let request = BackRequest {
            plan: plan_project_sheets_for(&self.document_state, &root),
            project_dir: project.dir().to_path_buf(),
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
        let Some(state) = self.ui_state.back_annotate.as_mut() else {
            return Task::none();
        };
        let job = state.next_job();
        state.phase = Phase::Computing;
        state.validation = None;
        // A check of the old list can no longer land (its job is stale).
        state.checking = false;
        state.pending = None;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || compute(request))
                    .await
                    .unwrap_or_else(|e| Err(format!("the worker stopped: {e}")))
            },
            move |result| {
                Message::from(BackAnnotateMsg::Read {
                    job,
                    result: result.map(Arc::new),
                })
            },
        )
    }

    fn back_annotate_read(&mut self, job: u64, result: Result<Arc<super::BackRead>, String>) {
        let Some(owner) = self
            .ui_state
            .back_annotate
            .as_ref()
            .filter(|state| state.job == job)
            .map(|state| state.project)
        else {
            return;
        };
        match result {
            Ok(read) => {
                for (library, why) in &read.library_errors {
                    crate::diagnostics::log_warning(format!(
                        "Back Annotate: library {} did not open: {why}",
                        library.display()
                    ));
                }
                // A sheet file another open project reaches holds that
                // project's part too: its values are not carried (#1224).
                let shared: HashSet<(InstancePath, Vec<Uuid>)> = read
                    .files
                    .iter()
                    .filter(|(_, part)| self.shared_with_project_other_than(owner, &part.file))
                    .map(|(key, _)| key.clone())
                    .collect();
                let mut list = read.list.clone();
                list.block_shared_values(|sheet, units| {
                    shared.contains(&(sheet.clone(), units.to_vec()))
                });
                let Some(state) = self.ui_state.back_annotate.as_mut() else {
                    return;
                };
                state.list = Some(list);
                state.read = Some(read);
                state.phase = Phase::Ready;
            }
            Err(why) => self.fail_back_annotate(why),
        }
    }

    fn tick_back_annotate(&mut self, change: impl FnOnce(&mut BackChangeList)) {
        if let Some(state) = self.ui_state.back_annotate.as_mut()
            && let Some(list) = state.list.as_mut()
        {
            change(list);
            state.validation = None;
        }
    }

    /// The read or the board open failed: the dialog says why.
    pub(super) fn fail_back_annotate(&mut self, why: String) {
        let line = t!(self.ui_state.i18n, ids::BACK_ANNOTATE_FAILED_LINE, "why" => why.as_str());
        crate::diagnostics::log_warning(line.into_string());
        if let Some(state) = self.ui_state.back_annotate.as_mut() {
            state.phase = Phase::Failed(why);
            state.list = None;
            state.read = None;
            state.checking = false;
            state.pending = None;
        }
    }

    /// No dialog can open: an error notice says why.
    fn refuse_back_annotate(&mut self, id: MessageId) -> Task<Message> {
        let why = t!(self.ui_state.i18n, id).into_string();
        crate::diagnostics::log_warning(format!("Back Annotate: {why}"));
        let title = t!(self.ui_state.i18n, ids::BACK_ANNOTATE_TITLE).into_string();
        self.document_state.error_notice = Some(ErrorNotice::new(title, why));
        Task::none()
    }

    pub(super) fn back_annotate_board_missing(&self, board: &str) -> String {
        t!(self.ui_state.i18n, ids::UPDATE_PCB_BOARD_MISSING, "board" => board).into_string()
    }
}
