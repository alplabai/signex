//! Apply: every manual number the dialog changed, written into its parent
//! sheet's sheet symbol, as ONE undo group in ONE `update`.
//!
//! Data-loss rules (the project Replace's, `handlers/find_replace`):
//! - a parent sheet no engine holds is loaded into a tabless engine and left
//!   unsaved; nothing is written to disk;
//! - each symbol's entries are re-read from the live document, so only the
//!   one entry that numbers the page changes;
//! - a stale read, a closed parent changed on disk since the read, a sheet
//!   being saved or moved, or a read-only sheet refuses the whole Apply, and
//!   a failure part-way puts every sheet back.

use std::collections::BTreeMap;
use std::path::PathBuf;

use iced::Task;
use signex_engine::Command;
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::SheetNumberingMsg;
use super::state::{NumberingProblem, PageEdit};
use crate::app::sheet_write::{SheetWrite, SheetWriteRefusal, disk_changed};
use crate::app::{Message, Signex};
use crate::library::closed_sheets::FileStamp;

const LABEL: &str = "Number Schematic Sheets";

/// One sheet symbol's new number: which placement, the page or `None`, and
/// the page's position (its automatic number).
type SymbolEdit = (Uuid, InstancePath, Option<usize>, usize);

impl Signex {
    /// Apply pressed. A closed parent sheet is installed from the copy the
    /// read took, so its file is stated again first, off the UI thread (as
    /// the symbol delete re-checks closed sheets): the write waits for
    /// [`Self::sheet_numbers_disk_checked`]. With no closed parent to change
    /// it runs now.
    pub(super) fn apply_sheet_numbers(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.sheet_numbering.as_ref() else {
            return Task::none();
        };
        if !state.can_apply() {
            return Task::none();
        }
        let Some(read) = state.read.clone() else {
            return Task::none();
        };
        if self.sheet_numbers_stale() {
            return self.refuse_sheet_numbers(NumberingProblem::Stale);
        }
        let to_check: Vec<(PathBuf, Option<FileStamp>)> = by_parent(&read.rows, &state.edits())
            .into_keys()
            .filter(|listed| {
                let path = self.resolve_held_spelling(listed.clone());
                !self.document_state.engines.contains_key(&path)
            })
            .map(|listed| {
                let then = read.stamps.get(&listed).copied().flatten();
                (listed, then)
            })
            .collect();
        let generation = state.generation;
        if to_check.is_empty() {
            return self.write_sheet_numbers_now();
        }
        if let Some(state) = self.ui_state.sheet_numbering.as_mut() {
            state.checking = true;
            state.error = None;
        }
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || disk_changed(&to_check)),
            move |result| Message::from(SheetNumberingMsg::DiskChecked { generation, result }),
        )
    }

    /// The closed parents were stated again: write, or refuse when one of
    /// them changed on disk since the read.
    pub(super) fn sheet_numbers_disk_checked(
        &mut self,
        generation: u64,
        result: Result<Vec<PathBuf>, String>,
    ) -> Task<Message> {
        let Some(state) = self.ui_state.sheet_numbering.as_mut() else {
            return Task::none();
        };
        if state.generation != generation || !state.checking {
            return Task::none();
        }
        state.checking = false;
        match result {
            Err(why) => self.refuse_sheet_numbers(NumberingProblem::ReadFailed(why)),
            Ok(changed) => match changed.into_iter().next() {
                Some(path) => self.refuse_sheet_numbers(NumberingProblem::DiskChanged(path)),
                None => self.write_sheet_numbers_now(),
            },
        }
    }

    /// An engine came or went, or a sheet symbol changed, since the read.
    fn sheet_numbers_stale(&self) -> bool {
        self.ui_state.sheet_numbering.as_ref().is_some_and(|state| {
            state.epoch != self.document_state.documents_epoch
                || state.child_sheets != self.child_sheet_signature()
        })
    }

    /// Check again what can have moved meanwhile, then write. On success the
    /// dialog closes; on a refusal it stays open and says why.
    fn write_sheet_numbers_now(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.sheet_numbering.as_ref() else {
            return Task::none();
        };
        let Some(read) = state.read.clone() else {
            return Task::none();
        };
        let project_name = state.project_name.clone();
        let edits = state.edits();
        if self.sheet_numbers_stale() {
            return self.refuse_sheet_numbers(NumberingProblem::Stale);
        }
        let edits = by_parent(&read.rows, &edits);
        match self.write_sheet_numbers(&read, &project_name, edits) {
            Ok(count) => {
                crate::diagnostics::log_info(format!(
                    "{LABEL}: renumbered pages on {count} sheet(s); save to keep them"
                ));
                self.ui_state.sheet_numbering = None;
            }
            Err(problem) => return self.refuse_sheet_numbers(problem),
        }
        Task::none()
    }

    /// Keep the dialog open with `problem` shown; the Messages panel gets the
    /// English line.
    fn refuse_sheet_numbers(&mut self, problem: NumberingProblem) -> Task<Message> {
        crate::diagnostics::log_warning(format!(
            "{LABEL}: nothing was changed: {}",
            problem.english()
        ));
        if let Some(state) = self.ui_state.sheet_numbering.as_mut() {
            state.error = Some(problem);
        }
        Task::none()
    }

    /// Write `edits` (per parent sheet), all or nothing, as one undo group
    /// (`app/sheet_write.rs`). The number of sheets changed, or why nothing
    /// was.
    fn write_sheet_numbers(
        &mut self,
        read: &super::NumberingRead,
        project_name: &str,
        edits: BTreeMap<PathBuf, Vec<SymbolEdit>>,
    ) -> Result<usize, NumberingProblem> {
        let write = SheetWrite {
            label: LABEL,
            outcome: "renumbered",
            kind: crate::app::UndoGroupKind::SheetNumbers,
            loaded: &read.loaded,
        };
        self.write_sheets_as_group(&write, edits, |document, list| {
            number_commands(document, project_name, list)
        })
        .map_err(|refusal| match refusal {
            SheetWriteRefusal::Busy(path) => NumberingProblem::Busy(path),
            SheetWriteRefusal::ReadOnly(path) => NumberingProblem::ReadOnly(path),
            SheetWriteRefusal::NotLoaded(path) => NumberingProblem::NotLoaded(path),
            SheetWriteRefusal::NotChanged(path) => NumberingProblem::NotChanged(path),
        })
    }
}

/// The edits grouped by the parent sheet that stores them, in path order.
fn by_parent(
    rows: &[super::NumberingRow],
    edits: &[PageEdit],
) -> BTreeMap<PathBuf, Vec<SymbolEdit>> {
    let mut grouped: BTreeMap<PathBuf, Vec<SymbolEdit>> = BTreeMap::new();
    for edit in edits {
        let Some(placement) = rows.get(edit.row).and_then(|r| r.placement.as_ref()) else {
            continue;
        };
        let position = rows[edit.row].position;
        grouped.entry(placement.parent.clone()).or_default().push((
            placement.symbol,
            placement.parent_path.clone(),
            edit.page,
            position,
        ));
    }
    grouped
}

/// One `SetChildSheetInstances` per sheet symbol `list` changes, each from
/// the symbol's live entries. A symbol placed in several parent placements
/// takes all of its edits in one command.
fn number_commands(
    document: &signex_types::schematic::SchematicSheet,
    project_name: &str,
    list: &[SymbolEdit],
) -> Vec<Command> {
    let mut entries: BTreeMap<Uuid, Vec<signex_types::schematic::SheetInstance>> = BTreeMap::new();
    for (symbol, parent_path, page, position) in list {
        let current = match entries.get(symbol) {
            Some(edited) => edited.clone(),
            None => match document.child_sheets.iter().find(|cs| cs.uuid == *symbol) {
                Some(cs) => cs.instances.clone(),
                None => continue,
            },
        };
        let next =
            signex_net::with_page_number(&current, parent_path, project_name, *page, *position);
        entries.insert(*symbol, next);
    }
    entries
        .into_iter()
        .map(|(sheet_id, instances)| Command::SetChildSheetInstances {
            sheet_id,
            instances,
        })
        .collect()
}
