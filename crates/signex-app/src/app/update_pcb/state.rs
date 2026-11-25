//! The Update PCB dialog's state: the project and board it updates, where
//! the list is in its life, and the list itself.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_engine::pcb::eco::{EcoChangeList, EcoRow, GroupTicks};

use super::design::DesignSnapshot;
use super::table::{TableGroup, TableState};
use crate::app::state::ProjectId;

/// Where the change list is.
#[derive(Debug, Clone)]
pub(crate) enum Phase {
    /// The project names no board: Create PCB.
    NoBoard,
    /// The project's board is being opened; the read starts once it is.
    Opening,
    /// The design read (or a net refresh) is running on the worker.
    Computing,
    /// The read failed, or the board could not be opened.
    Failed(String),
    /// The list is ready to review.
    Ready,
}

/// What the board lets Execute do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoardStatus {
    Editable,
    /// Its tab closed with unsaved edits; the parked board is read-only.
    Parked,
    /// It opened read-only (a newer format).
    ReadOnly,
}

/// A run of `update_pcb_from_schematic` from operands, waiting for its list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HeadlessRun {
    pub(crate) waiting: crate::app::agent::WaitingCommand,
    /// `dry_run=true`: answer the list and change nothing.
    pub(crate) dry_run: bool,
}

/// The Update PCB dialog, while open (or a run from operands).
#[derive(Debug, Clone)]
pub struct UpdatePcbState {
    pub(crate) project: ProjectId,
    /// The project's file name, for the subtitle.
    pub(crate) project_name: String,
    /// The project's board; `None` when it names none.
    pub(crate) board: Option<PathBuf>,
    pub(crate) phase: Phase,
    /// The newest worker job; a result of another job is dropped.
    pub(crate) job: u64,
    /// No dialog: the run of `update_pcb_from_schematic` from operands. It
    /// executes the default ticks once the list is ready, or answers the
    /// list (`headless` says which).
    pub(crate) apply: bool,
    /// The command run from operands that waits for this list (#2470): its
    /// ticket is answered with the change list, or why it did not run.
    pub(crate) headless: Option<HeadlessRun>,
    /// The design as read and the list built from it.
    pub(crate) snapshot: Option<Arc<DesignSnapshot>>,
    pub(crate) list: Option<EcoChangeList>,
    /// The last Validate: `Ok` or why it refused.
    pub(crate) validation: Option<Result<(), String>>,
    /// Filters, folds and the focused row of the table.
    pub(crate) table: TableState,
    /// Libraries whose open failure was already logged: a recompute logs
    /// each once per dialog.
    pub(crate) logged_library_errors: std::collections::HashSet<PathBuf>,
    /// Schematic rules left out that were already logged, likewise once.
    pub(crate) logged_rule_problems:
        std::collections::HashSet<super::rules_from_schematic::RuleProblem>,
}

impl UpdatePcbState {
    pub(crate) fn new(
        project: ProjectId,
        project_name: String,
        board: Option<PathBuf>,
        apply: bool,
    ) -> Self {
        Self {
            project,
            project_name,
            phase: if board.is_some() {
                Phase::Opening
            } else {
                Phase::NoBoard
            },
            board,
            job: 0,
            apply,
            headless: None,
            snapshot: None,
            logged_library_errors: std::collections::HashSet::new(),
            logged_rule_problems: std::collections::HashSet::new(),
            list: None,
            validation: None,
            table: TableState::default(),
        }
    }

    /// The board's file name, for labels.
    pub(crate) fn board_name(&self) -> String {
        self.board.as_deref().map(file_name).unwrap_or_default()
    }

    /// The list, while the dialog shows one.
    pub(crate) fn ready_list(&self) -> Option<&EcoChangeList> {
        matches!(self.phase, Phase::Ready)
            .then_some(self.list.as_ref())
            .flatten()
    }

    /// The list is ready to review (not opening, reading or failed).
    pub fn is_ready(&self) -> bool {
        matches!(self.phase, Phase::Ready)
    }

    /// The project names no board yet.
    pub fn lacks_board(&self) -> bool {
        matches!(self.phase, Phase::NoBoard)
    }

    /// Why there is no list, when the read failed.
    pub fn failure(&self) -> Option<&str> {
        match &self.phase {
            Phase::Failed(why) => Some(why),
            _ => None,
        }
    }

    /// The board the dialog updates.
    pub fn board(&self) -> Option<&Path> {
        self.board.as_deref()
    }

    /// The change list shown, ticks included.
    pub fn list(&self) -> Option<&EcoChangeList> {
        self.list.as_ref()
    }

    /// The last Validate: `Ok` or why it refused.
    pub fn validation(&self) -> Option<&Result<(), String>> {
        self.validation.as_ref()
    }

    /// The rows the table lists under its filters, in list order.
    pub fn shown_rows(&self) -> Vec<&EcoRow> {
        self.list
            .iter()
            .flat_map(|list| &list.rows)
            .filter(|row| self.table.shows(row))
            .collect()
    }

    /// The rows the Problems card lifts above the list: every blocked row,
    /// in list order.
    pub fn problem_rows(&self) -> Vec<&EcoRow> {
        self.list
            .as_ref()
            .map(super::table::problems)
            .unwrap_or_default()
    }

    /// The tick state of a table group.
    pub fn group_ticks(&self, group: TableGroup) -> Option<GroupTicks> {
        Some(super::table::group_ticks(self.list.as_ref()?, group))
    }

    /// Whether a table group is folded now.
    pub fn is_folded(&self, group: TableGroup) -> bool {
        self.list
            .as_ref()
            .is_some_and(|list| self.table.is_folded(list, group))
    }

    /// How many rows Execute applies now.
    pub fn execute_count(&self) -> usize {
        self.list.as_ref().map_or(0, super::table::applied_count)
    }

    /// Start a new worker job: results of earlier jobs are dropped.
    pub(crate) fn next_job(&mut self) -> u64 {
        self.job += 1;
        self.job
    }
}

/// The last component of `path`, lossy.
pub(crate) fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
