//! Apply: the confirmation when a chosen row deletes from the board, then
//! the PCB side as one labelled board step through Update PCB's Execute path
//! (refused, with nothing applied, once the board moved since the read),
//! then the schematic side as one Back Annotate undo group. A closed sheet
//! the schematic side reaches is stated again off the update thread first,
//! so nothing is applied when it changed on disk.

use std::path::PathBuf;

use iced::Task;
use signex_i18n::{ids, t};

use super::CompareBoardMsg;
use super::state::PendingApply;
use crate::app::sheet_write::disk_changed;
use crate::app::{Message, Signex};

impl Signex {
    /// The report no longer describes the board or the design: the board
    /// moved past its revision, or a sheet either read used was edited,
    /// closed or opened since.
    pub fn compare_board_stale(&self) -> bool {
        let Some(state) = self.ui_state.compare_board.as_ref() else {
            return false;
        };
        let (Some(report), Some(read)) = (state.report.as_ref(), state.read.as_ref()) else {
            return false;
        };
        let board_moved = self
            .document_state
            .board_engine(&state.board)
            .is_none_or(|engine| engine.revision() != report.revision());
        let edited = |revisions: &[(PathBuf, Option<u64>)]| {
            revisions.iter().any(|(path, then)| {
                let now = self.document_state.engines.get(path).map(|e| e.revision());
                now != *then
            })
        };
        board_moved
            || edited(read.pcb.snapshot.sheet_revisions.as_slice())
            || edited(read.back.sheet_revisions.as_slice())
    }

    /// Why Apply cannot run now: the board is parked, read-only or gone, or
    /// the report is stale.
    pub fn compare_board_apply_block(&self) -> Option<String> {
        let state = self.ui_state.compare_board.as_ref()?;
        if let Some(why) = self.board_block_in(&state.board, &self.ui_state.i18n) {
            return Some(why);
        }
        self.compare_board_stale()
            .then(|| t!(self.ui_state.i18n, ids::COMPARE_BOARD_STALE).into_string())
    }

    /// Apply: with a chosen row that deletes from the board, the
    /// confirmation listing the deletions opens first.
    pub(super) fn request_compare_apply(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.compare_board.as_mut() else {
            return Task::none();
        };
        let Some(report) = state.ready_report() else {
            return Task::none();
        };
        if report.counts().applied() == 0 {
            return Task::none();
        }
        if !report.board_deletions().is_empty() {
            state.confirming = true;
            return Task::none();
        }
        self.start_compare_apply()
    }

    /// Plan both sides from the directions now, state the closed sheets the
    /// schematic side reaches, then apply exactly that plan.
    pub(super) fn start_compare_apply(&mut self) -> Task<Message> {
        if let Some(why) = self.compare_board_apply_block() {
            return self.refuse_compare_apply(why);
        }
        let Some(state) = self.ui_state.compare_board.as_ref() else {
            return Task::none();
        };
        let (Some(report), Some(read)) = (state.ready_report(), state.read.clone()) else {
            return Task::none();
        };
        let (eco, back) = report.ticks();
        let edits = match self.back_annotate_edits(state.project, &back, &read.back) {
            Ok(edits) => edits,
            Err(why) => return self.refuse_compare_apply(why),
        };
        let to_check = self.closed_sheets_to_check(&read.back, &edits);
        let pending = PendingApply {
            eco,
            back,
            edits,
            checked: to_check.iter().map(|(file, _)| file.clone()).collect(),
        };
        if to_check.is_empty() {
            return self.finish_compare_apply(pending);
        }
        let Some(state) = self.ui_state.compare_board.as_mut() else {
            return Task::none();
        };
        state.checking = true;
        state.pending = Some(pending);
        state.error = None;
        let job = state.job;
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || disk_changed(&to_check)),
            move |result| Message::from(CompareBoardMsg::DiskChecked { job, result }),
        )
    }

    /// The closed sheets were stated again: apply the plan, or refuse when
    /// one of them changed on disk since the read.
    pub(super) fn compare_disk_checked(
        &mut self,
        job: u64,
        result: Result<Vec<PathBuf>, String>,
    ) -> Task<Message> {
        let Some(state) = self.ui_state.compare_board.as_mut() else {
            return Task::none();
        };
        if state.job != job || !state.checking {
            return Task::none();
        }
        state.checking = false;
        let pending = state.pending.take();
        match (result, pending) {
            (Err(why), _) => self.refuse_compare_apply(why),
            (Ok(changed), _) if !changed.is_empty() => {
                let why = t!(
                    self.ui_state.i18n,
                    ids::COMPARE_BOARD_DISK_CHANGED,
                    "path" => changed[0].display().to_string()
                )
                .into_string();
                self.refuse_compare_apply(why)
            }
            (Ok(_), Some(pending)) => self.finish_compare_apply(pending),
            (Ok(_), None) => Task::none(),
        }
    }

    /// Apply `pending`: the PCB side, then the schematic side, each one undo
    /// step. A refused PCB side applies nothing; a refused schematic side
    /// after the board changed says so. On success the dialog closes.
    fn finish_compare_apply(&mut self, pending: PendingApply) -> Task<Message> {
        if let Some(why) = self.compare_board_apply_block() {
            return self.refuse_compare_apply(why);
        }
        let Some(state) = self.ui_state.compare_board.as_ref() else {
            return Task::none();
        };
        let (board, board_name) = (state.board.clone(), state.board_name());
        let Some(read) = state.read.clone() else {
            return Task::none();
        };
        let to_pcb = pending
            .eco
            .rows
            .iter()
            .filter(|r| pending.eco.applies(r))
            .count();
        let to_schematic = pending.back.rows.iter().filter(|r| r.applies()).count();
        if to_pcb > 0 {
            let label = t!(self.ui_state.i18n, ids::COMPARE_BOARD_TITLE).into_string();
            if let Err(why) = self.execute_eco_list(&board, &pending.eco, &label) {
                return self.refuse_compare_apply(why);
            }
        }
        if !pending.edits.is_empty()
            && let Err(why) =
                self.write_back_annotation(&read.back, pending.edits, &pending.checked)
        {
            let why = if to_pcb > 0 {
                t!(
                    self.ui_state.i18n,
                    ids::COMPARE_BOARD_SCHEMATIC_REFUSED,
                    "why" => why
                )
                .into_string()
            } else {
                why
            };
            return self.refuse_compare_apply(why);
        }
        crate::diagnostics::log_info(
            t!(
                self.ui_state.i18n,
                ids::COMPARE_BOARD_DONE,
                "pcb" => to_pcb,
                "board" => board_name,
                "schematic" => to_schematic
            )
            .into_string(),
        );
        self.ui_state.compare_board = None;
        Task::none()
    }

    /// Keep the dialog open with `why` shown; Messages gets it too.
    fn refuse_compare_apply(&mut self, why: String) -> Task<Message> {
        if why.is_empty() {
            return Task::none();
        }
        crate::diagnostics::log_warning(
            t!(self.ui_state.i18n, ids::COMPARE_BOARD_REFUSED, "why" => why.as_str()).into_string(),
        );
        if let Some(state) = self.ui_state.compare_board.as_mut() {
            state.error = Some(why);
            state.confirming = false;
            state.checking = false;
            state.pending = None;
        }
        Task::none()
    }
}
