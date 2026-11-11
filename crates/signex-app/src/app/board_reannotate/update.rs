use iced::Task;
use signex_engine::pcb::designators::{ReannotateOptions, plan_reannotate};

use super::{BoardReannotateMsg, BoardReannotateState, ReannotateProblem};
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, Signex};
use signex_i18n::{ids, t};

impl Signex {
    pub(crate) fn dispatch_board_reannotate_message(
        &mut self,
        msg: BoardReannotateMsg,
    ) -> Task<Message> {
        match msg {
            BoardReannotateMsg::Requested => self.open_board_reannotate(),
            BoardReannotateMsg::OrderPicked(order) => {
                self.replan_board_reannotate(|o| o.order = order);
            }
            BoardReannotateMsg::BottomPicked(bottom) => {
                self.replan_board_reannotate(|o| o.bottom = bottom);
            }
            BoardReannotateMsg::ExecuteRequested => self.execute_board_reannotate(),
            BoardReannotateMsg::CancelRequested => self.ui_state.board_reannotate = None,
        }
        Task::none()
    }

    /// Open the dialog on the board in view, with the options it was last
    /// used with on this run of the app.
    fn open_board_reannotate(&mut self) {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            let line = t!(self.ui_state.i18n, ids::PCB_REANNOTATE_NO_PCB);
            crate::diagnostics::log_info(line.into_string());
            return;
        };
        let board_name = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.title.clone())
            .unwrap_or_default();
        let options = self.ui_state.board_reannotate_options;
        let Some(state) = self.board_reannotate_state(path, board_name, options) else {
            return;
        };
        self.ui_state.board_reannotate = Some(state);
    }

    /// A fresh plan of the board at `path`, or `None` when it is not open.
    fn board_reannotate_state(
        &self,
        board: std::path::PathBuf,
        board_name: String,
        options: ReannotateOptions,
    ) -> Option<BoardReannotateState> {
        let engine = self.document_state.board_engine(&board)?;
        let read_only = engine.board().unknown.newer_minor_source.is_some();
        Some(BoardReannotateState {
            plan: plan_reannotate(engine.board(), options),
            revision: engine.revision(),
            problem: read_only.then_some(ReannotateProblem::ReadOnly),
            board,
            board_name,
            options,
        })
    }

    /// Change the options and remake the plan from the board as it is now.
    fn replan_board_reannotate(&mut self, change: impl FnOnce(&mut ReannotateOptions)) {
        let Some(state) = self.ui_state.board_reannotate.take() else {
            return;
        };
        let mut options = state.options;
        change(&mut options);
        self.ui_state.board_reannotate_options = options;
        self.ui_state.board_reannotate =
            self.board_reannotate_state(state.board, state.board_name, options);
    }

    /// Run the plan as one labelled step at the revision it was made from.
    /// A board that moved on gets a new plan to check first; a refusal
    /// keeps the dialog open with the reason.
    fn execute_board_reannotate(&mut self) {
        let Some(state) = self.ui_state.board_reannotate.take() else {
            return;
        };
        // A read-only board keeps the dialog open with its reason.
        if !state.can_execute() {
            self.ui_state.board_reannotate = Some(state);
            return;
        }
        let Some(engine) = self.document_state.board_engine(&state.board) else {
            let line = t!(self.ui_state.i18n, ids::PCB_REANNOTATE_BOARD_GONE);
            crate::diagnostics::log_warning(line.into_string());
            return;
        };
        if engine.revision() != state.revision {
            let fresh = self.board_reannotate_state(state.board, state.board_name, state.options);
            self.ui_state.board_reannotate = fresh.map(|s| BoardReannotateState {
                problem: s.problem.clone().or(Some(ReannotateProblem::Stale)),
                ..s
            });
            return;
        }
        let commands = match state.plan.commands(engine.board()) {
            Ok(commands) => commands,
            Err(error) => {
                self.ui_state.board_reannotate = Some(BoardReannotateState {
                    problem: Some(ReannotateProblem::Refused(error.to_string())),
                    ..state
                });
                return;
            }
        };
        let label = t!(self.ui_state.i18n, ids::PCB_REANNOTATE_TITLE).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision: state.revision,
        };
        match self.execute_board_commands(&state.board, commands, Some(step)) {
            Ok(_) => {
                let line = t!(
                    self.ui_state.i18n,
                    ids::PCB_REANNOTATE_DONE,
                    "count" => state.plan.renames.len(),
                    "board" => state.board_name.as_str()
                );
                crate::diagnostics::log_info(line.into_string());
                self.refresh_pcb_highlight();
            }
            Err(error) => {
                self.ui_state.board_reannotate = Some(BoardReannotateState {
                    problem: Some(ReannotateProblem::Refused(error.to_string())),
                    ..state
                });
            }
        }
    }
}
