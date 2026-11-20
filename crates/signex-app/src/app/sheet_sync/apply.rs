//! Apply: exactly the changes the preview shows, as one undo step.
//!
//! The previews are worked out again from the live sheets first; when they
//! differ from what the dialog shows, nothing is changed and the dialog
//! shows the new ones. Sheet to Symbol edits the dialog's parent sheet (the
//! active one, or an undocked window's) through the mutation gateway, with
//! that sheet as the edit target. Symbol to Sheet edits the child sheet through
//! `write_sheets_as_group` (`app/sheet_write.rs`): a child no engine holds
//! is loaded into a tabless engine from the copy read for the preview,
//! after its file is stated again off the UI thread, and left unsaved.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use iced::Task;
use signex_engine::{SheetSyncDirection, SheetSyncPreview};
use signex_types::schematic::{ChildSheet, SchematicSheet};

use super::open::{LABEL, previews_of};
use super::{SheetSyncMsg, SheetSyncProblem};
use crate::app::mutation_gateway::MutationOutcome;
use crate::app::sheet_write::{SheetWrite, SheetWriteRefusal, disk_changed};
use crate::app::{Message, Signex, UndoGroupKind};

impl Signex {
    /// Apply pressed.
    pub(super) fn apply_sheet_sync(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.sheet_sync.as_ref() else {
            return Task::none();
        };
        if !state.can_apply() {
            return Task::none();
        }
        if !self.sync_parent_open() {
            return Task::none();
        }
        let Some(state) = self.ui_state.sheet_sync.as_ref() else {
            return Task::none();
        };
        let shown = state.previews.clone();
        let direction = state.direction;
        let generation = state.generation;
        let child_path = state.child_path.clone();
        let stamp = state.read.as_ref().and_then(|read| read.stamp);
        let (symbol, child) = match self.sheet_sync_sides(state) {
            Ok(sides) => sides,
            Err(problem) => return self.refuse_sheet_sync(problem),
        };
        let now = previews_of(&symbol, &child);
        if shown.as_ref() != Some(&now) {
            if let Some(state) = self.ui_state.sheet_sync.as_mut() {
                state.previews = Some(now);
            }
            return self.refuse_sheet_sync(SheetSyncProblem::Changed);
        }
        match direction {
            SheetSyncDirection::SheetToSymbol => {
                self.write_sync_to_symbol(&symbol, &child, &now.to_symbol);
                Task::none()
            }
            SheetSyncDirection::SymbolToSheet
                if self.document_state.engines.contains_key(&child_path) =>
            {
                self.write_sync_to_sheet(&symbol, &now.to_sheet);
                Task::none()
            }
            SheetSyncDirection::SymbolToSheet => {
                if let Some(state) = self.ui_state.sheet_sync.as_mut() {
                    state.checking = true;
                    state.problem = None;
                }
                let to_check = vec![(child_path, stamp)];
                Task::perform(
                    crate::app::path_op_jobs::run_blocking(move || disk_changed(&to_check)),
                    move |result| Message::from(SheetSyncMsg::DiskChecked { generation, result }),
                )
            }
        }
    }

    /// The closed child sheet was stated again: write, or refuse when it
    /// changed on disk since the read.
    pub(super) fn sheet_sync_disk_checked(
        &mut self,
        generation: u64,
        result: Result<Vec<PathBuf>, String>,
    ) -> Task<Message> {
        let Some(state) = self.ui_state.sheet_sync.as_mut() else {
            return Task::none();
        };
        if state.generation != generation || !state.checking {
            return Task::none();
        }
        state.checking = false;
        let changed = match result {
            Ok(changed) => changed,
            Err(why) => return self.refuse_sheet_sync(SheetSyncProblem::ReadFailed(why)),
        };
        if let Some(path) = changed.into_iter().next() {
            return self.refuse_sheet_sync(SheetSyncProblem::DiskChanged(path));
        }
        if !self.sync_parent_open() {
            return Task::none();
        }
        let Some(state) = self.ui_state.sheet_sync.as_ref() else {
            return Task::none();
        };
        let shown = state.previews.clone();
        let (symbol, child) = match self.sheet_sync_sides(state) {
            Ok(sides) => sides,
            Err(problem) => return self.refuse_sheet_sync(problem),
        };
        let now = previews_of(&symbol, &child);
        if shown.as_ref() != Some(&now) {
            return self.refuse_sheet_sync(SheetSyncProblem::Changed);
        }
        self.write_sync_to_sheet(&symbol, &now.to_sheet);
        Task::none()
    }

    /// Whether the dialog's parent sheet is still open. When it was closed,
    /// the Messages panel says so and the dialog shows it stale.
    fn sync_parent_open(&mut self) -> bool {
        let Some(state) = self.ui_state.sheet_sync.as_mut() else {
            return false;
        };
        if self.document_state.engines.contains_key(&state.parent) {
            return true;
        }
        crate::diagnostics::log_warning(format!(
            "{LABEL}: {} is no longer open; nothing was changed.",
            state.parent.display()
        ));
        state.checking = false;
        state.problem = Some(SheetSyncProblem::Stale);
        false
    }

    /// Sheet to Symbol: one `ReconcileChildSheetPins` for every sheet symbol
    /// of the file on the dialog's parent sheet, with the preview's renames
    /// on `symbol` only. The parent is the edit target, so the change lands
    /// on it whichever sheet the main window shows.
    fn write_sync_to_symbol(
        &mut self,
        symbol: &ChildSheet,
        child: &SchematicSheet,
        preview: &SheetSyncPreview,
    ) {
        let Some(parent_path) = self
            .ui_state
            .sheet_sync
            .as_ref()
            .map(|state| state.parent.clone())
        else {
            return;
        };
        let Some(parent) = self
            .document_state
            .engines
            .get(&parent_path)
            .map(|e| e.document())
        else {
            self.note_sheet_sync_problem(SheetSyncProblem::Stale);
            return;
        };
        let ports = signex_engine::ports_for_pin_sync(child, parent, &symbol.filename);
        let command = signex_engine::sync_from_sheet_command(symbol, ports, preview);
        let (outcome, reason) = self.on_edit_target(&parent_path, |app| {
            app.apply_engine_commands_reporting(vec![command], true, true)
        });
        if outcome == MutationOutcome::Failed {
            if let Some(reason) = reason {
                crate::diagnostics::log_warning(format!("{LABEL}: {reason}"));
            }
            self.note_sheet_sync_problem(SheetSyncProblem::NotChanged(parent_path));
            return;
        }
        crate::diagnostics::log_info(format!(
            "{LABEL}: updated the entries of sheet symbol '{}'{}",
            symbol.name,
            summary(preview)
        ));
        self.ui_state.sheet_sync = None;
    }

    /// Symbol to Sheet: the preview's port changes on the child sheet, as
    /// one undo step, left unsaved.
    fn write_sync_to_sheet(&mut self, symbol: &ChildSheet, preview: &SheetSyncPreview) {
        let Some(state) = self.ui_state.sheet_sync.as_ref() else {
            return;
        };
        let child_path = state.child_path.clone();
        let loaded: HashMap<PathBuf, SchematicSheet> = state
            .read
            .as_ref()
            .map(|read| (child_path.clone(), read.sheet.clone()))
            .into_iter()
            .collect();
        let write = SheetWrite {
            label: LABEL,
            outcome: "synced",
            kind: UndoGroupKind::SheetEntrySync,
            loaded: &loaded,
        };
        let edits = BTreeMap::from([(child_path.clone(), ())]);
        let result = self.write_sheets_as_group(&write, edits, |document, _| {
            signex_engine::port_sync_commands(
                document,
                symbol,
                preview,
                &mut signex_types::object_id::UuidV7Ids,
            )
        });
        match result {
            Ok(_) => {
                crate::diagnostics::log_info(format!(
                    "{LABEL}: updated the ports of {}{}; save it to keep them",
                    child_path.display(),
                    summary(preview)
                ));
                self.ui_state.sheet_sync = None;
            }
            Err(refusal) => {
                let problem = match refusal {
                    SheetWriteRefusal::Busy(path) => SheetSyncProblem::Busy(path),
                    SheetWriteRefusal::ReadOnly(path) => SheetSyncProblem::ReadOnly(path),
                    SheetWriteRefusal::NotLoaded(path) => SheetSyncProblem::NotLoaded(path),
                    SheetWriteRefusal::NotChanged(path) => SheetSyncProblem::NotChanged(path),
                };
                self.note_sheet_sync_problem(problem);
            }
        }
    }

    /// Keep the dialog open with `problem` shown; the Messages panel gets the
    /// English line.
    fn refuse_sheet_sync(&mut self, problem: SheetSyncProblem) -> Task<Message> {
        self.note_sheet_sync_problem(problem);
        Task::none()
    }

    /// [`Self::refuse_sheet_sync`] where no task is returned.
    fn note_sheet_sync_problem(&mut self, problem: SheetSyncProblem) {
        crate::diagnostics::log_warning(format!(
            "{LABEL}: nothing was changed: {}",
            english(&problem)
        ));
        if let Some(state) = self.ui_state.sheet_sync.as_mut() {
            state.checking = false;
            state.problem = Some(problem);
        }
    }
}

/// `: added A, B; removed C; renamed D to E` for the Messages line.
fn summary(preview: &SheetSyncPreview) -> String {
    let mut parts = Vec::new();
    if !preview.added.is_empty() {
        parts.push(format!("added {}", preview.added.join(", ")));
    }
    if !preview.removed.is_empty() {
        parts.push(format!("removed {}", preview.removed.join(", ")));
    }
    if !preview.renamed.is_empty() {
        let renamed: Vec<String> = preview
            .renamed
            .iter()
            .map(|r| format!("{} to {}", r.from, r.to))
            .collect();
        parts.push(format!("renamed {}", renamed.join(", ")));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(": {}", parts.join("; "))
    }
}

/// `problem` in English, for the Messages panel.
fn english(problem: &SheetSyncProblem) -> String {
    match problem {
        SheetSyncProblem::ReadFailed(why) => format!("the sheet could not be read ({why})"),
        SheetSyncProblem::Stale => {
            "the sheet, the sheet symbol or the open sheets changed".to_string()
        }
        SheetSyncProblem::Changed => "the sheets changed since the preview".to_string(),
        SheetSyncProblem::Busy(path) => format!("{} is being saved or moved", path.display()),
        SheetSyncProblem::ReadOnly(path) => format!("{} opens read-only", path.display()),
        SheetSyncProblem::NotLoaded(path) => format!("{} could not be loaded", path.display()),
        SheetSyncProblem::NotChanged(path) => format!("{} refused the change", path.display()),
        SheetSyncProblem::DiskChanged(path) => {
            format!("{} changed on disk after it was read", path.display())
        }
    }
}
