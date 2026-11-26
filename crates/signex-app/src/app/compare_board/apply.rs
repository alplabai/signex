//! Apply: the confirmation when a chosen row deletes from the board, then
//! the PCB side as one labelled board step through Update PCB's Execute path
//! (refused, with nothing applied, once the board moved since the read),
//! then the schematic side as one Back Annotate undo group. A closed sheet
//! the schematic side reaches is stated again off the update thread first,
//! so nothing is applied when it changed on disk.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{ids, t};

use super::CompareBoardMsg;
use super::read::CompareRead;
use super::state::PendingApply;
use crate::app::sheet_write::disk_changed;
use crate::app::update_pcb::file_name;
use crate::app::{Message, Signex};

/// What an Apply changed: the rows taken to the board and to the
/// schematic, and the sheets the schematic side wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AppliedCompare {
    pub(crate) to_pcb: usize,
    pub(crate) to_schematic: usize,
    pub(crate) sheets: usize,
}

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
        self.compare_read_stale(&state.board, report, read)
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
        let board = state.board.clone();
        let Some(read) = state.read.clone() else {
            return Task::none();
        };
        match self.apply_compare_plan(&board, &read, pending) {
            Ok(_) => {
                self.ui_state.compare_board = None;
                Task::none()
            }
            Err(why) => self.refuse_compare_apply(why),
        }
    }

    /// The PCB side of `pending` as one labelled board step, then its
    /// schematic side as one Back Annotate undo group, and the line in
    /// Messages: the dialog's Apply and `apply_board_schematic_differences`
    /// both end here. A refused PCB side applies nothing; a refused
    /// schematic side after the board changed says so.
    pub(crate) fn apply_compare_plan(
        &mut self,
        board: &Path,
        read: &CompareRead,
        pending: PendingApply,
    ) -> Result<AppliedCompare, String> {
        let to_pcb = pending
            .eco
            .rows
            .iter()
            .filter(|r| pending.eco.applies(r))
            .count();
        let to_schematic = pending.back.rows.iter().filter(|r| r.applies()).count();
        if to_pcb > 0 {
            let label = t!(self.ui_state.i18n, ids::COMPARE_BOARD_TITLE).into_string();
            self.execute_eco_list(board, &pending.eco, &label)?;
        }
        let mut sheets = 0;
        if !pending.edits.is_empty() {
            match self.write_back_annotation(&read.back, pending.edits, &pending.checked) {
                Ok(written) => sheets = written,
                Err(why) if to_pcb > 0 => {
                    return Err(t!(
                        self.ui_state.i18n,
                        ids::COMPARE_BOARD_SCHEMATIC_REFUSED,
                        "why" => why
                    )
                    .into_string());
                }
                Err(why) => return Err(why),
            }
        }
        crate::diagnostics::log_info(
            t!(
                self.ui_state.i18n,
                ids::COMPARE_BOARD_DONE,
                "pcb" => to_pcb,
                "board" => file_name(board),
                "schematic" => to_schematic
            )
            .into_string(),
        );
        Ok(AppliedCompare {
            to_pcb,
            to_schematic,
            sheets,
        })
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
