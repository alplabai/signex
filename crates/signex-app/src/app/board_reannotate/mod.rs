//! Tools ▸ Re-Annotate Board (#1312): Positional Re-Annotate. The dialog
//! numbers the board's parts by position within each prefix, shows the
//! renames, and Execute runs them as one labelled board step. The plan is
//! `signex_engine::pcb::designators::plan_reannotate`; the board is read in
//! memory, so nothing runs on a worker.

mod update;

use std::path::PathBuf;

use signex_engine::pcb::designators::{
    BottomSideOrder, ReannotateOptions, ReannotateOrder, ReannotatePlan,
};

impl From<BoardReannotateMsg> for crate::app::Message {
    fn from(msg: BoardReannotateMsg) -> Self {
        Self::Pcb(crate::app::PcbMsg::Reannotate(msg))
    }
}

/// What happened in the Re-Annotate Board dialog, namespaced under
/// `PcbMsg::Reannotate` and routed to `dispatch_board_reannotate_message`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardReannotateMsg {
    /// `reannotate_board` ran on the board in view.
    Requested,
    OrderPicked(ReannotateOrder),
    BottomPicked(BottomSideOrder),
    ExecuteRequested,
    CancelRequested,
}

/// Why Execute is off or did not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReannotateProblem {
    /// The board opened read-only (a newer format).
    ReadOnly,
    /// The board changed after the list was made; the list is remade.
    Stale,
    /// The board engine refused the step; the reason as it gave it.
    Refused(String),
}

/// The open dialog: the board it numbers, the options and the plan made
/// from them at `revision`.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardReannotateState {
    pub board: PathBuf,
    pub board_name: String,
    pub options: ReannotateOptions,
    pub plan: ReannotatePlan,
    pub revision: u64,
    pub problem: Option<ReannotateProblem>,
}

impl BoardReannotateState {
    /// Whether Execute can run: there is something to rename and the board
    /// is not read-only. After a stale list or a refusal it may run again.
    pub fn can_execute(&self) -> bool {
        !self.plan.renames.is_empty() && self.problem != Some(ReannotateProblem::ReadOnly)
    }
}

#[cfg(test)]
mod tests;
