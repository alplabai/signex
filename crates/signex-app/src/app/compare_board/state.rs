//! The Compare Board with Schematic dialog's state: the project and board it
//! compares, where the report is in its life, the report with its
//! directions, the kind filter and the confirmation before Apply.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_engine::pcb::eco::{
    BackChangeList, CompareCounts, CompareKind, CompareReport, CompareRow, Direction, EcoChangeList,
};

use super::read::CompareRead;
use crate::app::back_annotate::FileEdits;
use crate::app::state::ProjectId;
use crate::app::update_pcb::{Phase, file_name};

/// What Apply planned before stating the closed sheets: both lists with only
/// the chosen rows ticked, the schematic edits by sheet file, and the closed
/// files stated again.
#[derive(Debug, Clone)]
pub(crate) struct PendingApply {
    pub(crate) eco: EcoChangeList,
    pub(crate) back: BackChangeList,
    pub(crate) edits: BTreeMap<PathBuf, FileEdits>,
    pub(crate) checked: BTreeSet<PathBuf>,
}

/// The dialog, while open.
#[derive(Debug, Clone)]
pub struct CompareBoardState {
    pub(crate) project: ProjectId,
    /// The root sheet's file name, for the subtitle.
    pub(crate) schematic_name: String,
    pub(crate) board: PathBuf,
    pub(crate) phase: Phase,
    /// The newest worker job; a result of another job is dropped.
    pub(crate) job: u64,
    pub(crate) read: Option<Arc<CompareRead>>,
    /// The report shown, with the directions chosen.
    pub(crate) report: Option<CompareReport>,
    /// The kind chip in force; `None`: All.
    pub(crate) filter: Option<CompareKind>,
    /// The "Set shown rows" choice last made, until a row is set by hand.
    pub(crate) shown_choice: Option<Direction>,
    /// When the report was read, `HH:MM:SS` local time.
    pub(crate) compared_at: Option<String>,
    /// The confirmation listing the board deletions is open.
    pub(crate) confirming: bool,
    /// Apply is stating the closed sheets; the directions wait.
    pub(crate) checking: bool,
    pub(crate) pending: Option<PendingApply>,
    /// Why the last Apply did nothing (or did only the board side).
    pub(crate) error: Option<String>,
    /// Libraries whose open failure was already logged.
    pub(crate) logged_library_errors: HashSet<PathBuf>,
}

impl CompareBoardState {
    pub(crate) fn new(project: ProjectId, schematic_name: String, board: PathBuf) -> Self {
        Self {
            project,
            schematic_name,
            board,
            phase: Phase::Opening,
            job: 0,
            read: None,
            report: None,
            filter: None,
            shown_choice: None,
            compared_at: None,
            confirming: false,
            checking: false,
            pending: None,
            error: None,
            logged_library_errors: HashSet::new(),
        }
    }

    /// The board's file name, for labels.
    pub(crate) fn board_name(&self) -> String {
        file_name(&self.board)
    }

    /// The board compared.
    pub fn board(&self) -> &Path {
        &self.board
    }

    /// The report, while the dialog shows one ready to apply.
    pub(crate) fn ready_report(&self) -> Option<&CompareReport> {
        matches!(self.phase, Phase::Ready)
            .then_some(self.report.as_ref())
            .flatten()
    }

    /// The report shown, directions included.
    pub fn report(&self) -> Option<&CompareReport> {
        self.report.as_ref()
    }

    /// The report is ready (not opening, reading or failed).
    pub fn is_ready(&self) -> bool {
        matches!(self.phase, Phase::Ready)
    }

    /// Why there is no report, when a read failed.
    pub fn failure(&self) -> Option<&str> {
        match &self.phase {
            Phase::Failed(why) => Some(why),
            _ => None,
        }
    }

    /// Why the last Apply did nothing, or did only the board side.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The confirmation before Apply is open.
    pub fn is_confirming(&self) -> bool {
        self.confirming
    }

    /// The kind chip in force; `None`: All.
    pub fn filter(&self) -> Option<CompareKind> {
        self.filter
    }

    /// The rows the table lists under the kind filter, in report order.
    pub fn shown_rows(&self) -> Vec<&CompareRow> {
        self.report
            .iter()
            .flat_map(|report| &report.rows)
            .filter(|row| self.filter.is_none_or(|kind| row.kind == kind))
            .collect()
    }

    /// Where the rows go with the directions chosen now.
    pub fn counts(&self) -> CompareCounts {
        self.report
            .as_ref()
            .map(CompareReport::counts)
            .unwrap_or_default()
    }

    /// Start a new worker job: results of earlier jobs are dropped.
    pub(crate) fn next_job(&mut self) -> u64 {
        self.job += 1;
        self.job
    }
}
