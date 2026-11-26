//! The end of a run of `update_pcb_from_schematic` from operands (#2470).
//! Once its list is ready, a dry run is answered with it and an apply is
//! executed and answered with what was done; a run that failed on the way
//! (the board did not open, the read failed, Execute was refused) is
//! answered why. The caller is the socket request or the headless run that
//! waits on the ticket (`agent::deferred`).

use std::path::{Path, PathBuf};

use iced::Task;
use signex_engine::pcb::eco::EcoChangeList;
use signex_i18n::{Translatable, ids};

use super::answer::{AnswerFor, UpdatePcbAnswer};
use super::command::did_not_run;
use super::state::{UpdatePcbJob, file_name};
use crate::app::agent::WaitingCommand;
use crate::app::command::CommandOutcome;
use crate::app::{Message, Signex};

fn refusal(reason: Translatable) -> CommandOutcome {
    CommandOutcome::Disabled { reason }
}

impl Signex {
    /// The list is ready and no dialog shows it: a run from operands is
    /// answered; a state no caller waits on executes as before.
    pub(super) fn finish_update_pcb_list(&mut self) -> Task<Message> {
        let Some(UpdatePcbJob { waiting, dry_run }) = self.update_pcb_run() else {
            return self.execute_update_pcb();
        };
        let target = self.update_pcb_run_target();
        let mut follow = Task::none();
        let answered = self.finish_waiting_command(waiting, &target, |app| {
            let (outcome, task) = app.answer_update_pcb_list(dry_run);
            follow = task;
            outcome
        });
        // Turned away at the gate (connections off meanwhile): the run ends.
        self.drop_update_pcb_run();
        Task::batch([answered, follow])
    }

    /// The pass after every update: a run from operands that failed on the
    /// way is answered why.
    pub(crate) fn answer_failed_update_pcb_run(&mut self) -> Task<Message> {
        let Some((waiting, why)) = self.failed_update_pcb_run() else {
            return Task::none();
        };
        let target = self.update_pcb_run_target();
        self.drop_update_pcb_run();
        self.finish_waiting_command(waiting, &target, |_| refusal(did_not_run(why)))
    }

    /// A run that failed before its command returned: its state goes and the
    /// command answers the reason itself.
    pub(super) fn take_failed_update_pcb_run(&mut self) -> Option<String> {
        let (_, why) = self.failed_update_pcb_run()?;
        self.drop_update_pcb_run();
        Some(why)
    }

    fn failed_update_pcb_run(&self) -> Option<(WaitingCommand, String)> {
        let run = self.update_pcb_run()?;
        let state = self.ui_state.update_pcb.as_ref()?;
        Some((run.waiting, state.failure()?.to_string()))
    }

    /// The run from operands the state in progress serves, if any: its job
    /// is held with the agent's deferred work (#2987).
    fn update_pcb_run(&self) -> Option<UpdatePcbJob> {
        self.ui_state.update_pcb.as_ref()?;
        self.agent.update_pcb_job.clone()
    }

    /// Whether the state in progress serves a run from operands.
    pub(super) fn is_update_pcb_run(&self) -> bool {
        self.ui_state.update_pcb.is_some() && self.agent.update_pcb_job.is_some()
    }

    fn drop_update_pcb_run(&mut self) {
        if self.agent.update_pcb_job.take().is_some() {
            self.ui_state.update_pcb = None;
        }
    }

    /// What the run acts on, for a turned-away journal line: the board, else
    /// the project file.
    fn update_pcb_run_target(&self) -> PathBuf {
        let Some(state) = self.ui_state.update_pcb.as_ref() else {
            return PathBuf::new();
        };
        let project = self.document_state.project_by_id(state.project);
        state
            .board
            .clone()
            .or_else(|| project.map(|p| p.path.clone()))
            .unwrap_or_default()
    }

    /// The answer to the ready list: the list itself for a dry run, else
    /// Execute runs it (one labelled undo step) and the list says what it
    /// did. The state goes either way.
    fn answer_update_pcb_list(&mut self, dry_run: bool) -> (CommandOutcome, Task<Message>) {
        let Some(state) = self.ui_state.update_pcb.as_ref() else {
            return (refusal(ids::UPDATE_PCB_NOT_IN_PROJECT.into()), Task::none());
        };
        let project = self
            .document_state
            .project_by_id(state.project)
            .map(|p| p.path.clone())
            .unwrap_or_default();
        let (Some(list), Some(board)) = (state.ready_list().cloned(), state.board.clone()) else {
            self.drop_update_pcb_run();
            return (refusal(ids::UPDATE_PCB_NO_BOARD.into()), Task::none());
        };
        // Exit criterion 3: the classes, read before an apply puts them.
        let mut net_classes = self.update_pcb_class_names(state);
        let snapshot = state.snapshot.clone();
        if dry_run {
            let blocked = self.update_pcb_execute_block_in(&signex_i18n::I18n::en_us());
            self.drop_update_pcb_run();
            let outcome =
                self.update_pcb_answer(&list, (&project, &board), true, blocked, net_classes);
            return (outcome, Task::none());
        }
        let task = self.execute_update_pcb();
        // A class step that failed after the board step is a Messages
        // warning, as for the rules; the answer leaves out what it did not
        // put.
        if let Some(edit) =
            (snapshot.as_deref()).and_then(|s| self.update_pcb_class_edit(&board, s))
        {
            let missed = super::classes_from_schematic::classes_in(&edit);
            net_classes.retain(|class| !missed.contains(class));
        }
        let outcome = match self.ui_state.update_pcb.take() {
            None => self.update_pcb_answer(&list, (&project, &board), false, None, net_classes),
            Some(left) => match left.failure() {
                Some(why) => refusal(did_not_run(why)),
                None => refusal(
                    Translatable::new(ids::UPDATE_PCB_BOARD_MISSING)
                        .arg("board", file_name(&board)),
                ),
            },
        };
        (outcome, task)
    }

    fn update_pcb_answer(
        &self,
        list: &EcoChangeList,
        (project, board): (&Path, &Path),
        dry_run: bool,
        execute_blocked: Option<String>,
        net_classes: Vec<String>,
    ) -> CommandOutcome {
        let board_revision = self
            .document_state
            .board_engine(board)
            .map_or(list.revision, |engine| engine.revision());
        let at = AnswerFor {
            dry_run,
            project,
            board,
            board_revision,
            execute_blocked,
            net_classes,
        };
        match serde_json::to_value(UpdatePcbAnswer::of(list, at)) {
            Ok(value) => CommandOutcome::Answered(value),
            Err(error) => refusal(did_not_run(error.to_string())),
        }
    }
}
