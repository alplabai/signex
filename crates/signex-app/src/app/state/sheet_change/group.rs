//! A grouped undo or redo of the sheets (a project-wide run, an agent's
//! cross-sheet edit): every live member sheet moves in one step, or none
//! does. The rules and their Messages lines are in `app/undo_groups.rs`'s
//! module docs; a group with board steps is not stepped here (#2366).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use signex_engine::{DocumentPatch, UndoGroupId};
use signex_i18n_ids::ids;

use super::notices::{engine_error_notice, warning_notice};
use super::{DocumentState, Settlement};
use crate::app::UndoGroupSheets;
use crate::app::core::event::{ChangeCause, CoreEvent, DocumentChanged};

/// A grouped step run by the core half.
#[derive(Debug, Default)]
#[must_use = "project the events"]
pub(crate) struct GroupStep {
    /// What the wrapper returns: the active sheet took the step's change.
    pub(crate) stepped: bool,
    /// The step was not refused: the panel context follows it.
    pub(crate) ran: bool,
    /// Its notices and the other member sheets' changes, in order.
    pub(crate) members: Vec<CoreEvent>,
    /// The active sheet's change.
    pub(crate) active: Vec<CoreEvent>,
}

/// The sheets of a group, sorted by what the step does with them.
#[derive(Default)]
struct GroupSheets {
    members: Vec<PathBuf>,
    blocked: Vec<PathBuf>,
    /// Not loaded, or loaded without this step: the step is refused whole.
    out_of_reach: Vec<PathBuf>,
}

impl DocumentState {
    /// Undo (`undo`) or redo `group` across every live sheet it holds.
    /// `undocked` is the sheets an undocked window shows: with the tabs, what
    /// holds a parked sheet open, so an undo never drops its engine.
    pub(crate) fn step_undo_group(
        &mut self,
        group: UndoGroupId,
        undo: bool,
        undocked: &HashSet<PathBuf>,
    ) -> GroupStep {
        let mut step = GroupStep::default();
        let Some(active_path) = self.active_path.clone() else {
            return step;
        };
        let recorded = self.undo_groups.get(&group).cloned().unwrap_or_default();
        if !undo && !recorded.dropped.is_empty() {
            // All or nothing: redoing the other sheets would leave one part
            // under two designators.
            step.members.push(warning_notice(recorded.redo_dropped()));
            return step;
        }
        let sheets = self.sort_group_sheets(group, undo, &recorded, &active_path);
        if !sheets.blocked.is_empty() {
            step.members
                .push(warning_notice(recorded.blocked(undo, &sheets.blocked)));
            return step;
        }
        // Undo until you save (#2760): a project is never left half undone.
        if !sheets.out_of_reach.is_empty() {
            step.members.push(warning_notice(
                recorded.out_of_reach(undo, &sheets.out_of_reach),
            ));
            return step;
        }
        // Only now: a pass can move a sheet off its save point.
        for path in &sheets.members {
            self.pass_dead_class_picks(path, undo);
        }
        step.ran = true;
        let moved = self.step_group_members(group, undo, &recorded, sheets.members, undocked);
        let (moved_any, active_patch, events) = moved;
        step.members.extend(events);
        // A member sheet other than the active one changed too.
        self.note_sheet_edit();
        let settled = match active_patch {
            Some(patch) => {
                debug_assert_ne!(patch, DocumentPatch::NONE, "#2645: a step with no patch");
                let path = Some(active_path.as_path());
                self.settle(path, patch, ChangeCause::Step, &mut step.active)
            }
            None => Settlement::Unchanged,
        };
        step.stepped = settled != Settlement::Unchanged;
        if moved_any && settled != Settlement::Shown {
            // The history moved all the same: an empty patch on the active
            // canvas only asks for the library drift check.
            let (path, patch, cause) = (active_path, DocumentPatch::NONE, ChangeCause::Step);
            step.active
                .push(DocumentChanged { path, patch, cause }.into());
        }
        step
    }

    /// The group's live sheets and the active one, sorted into the members
    /// that move and the ones that refuse the step.
    fn sort_group_sheets(
        &self,
        group: UndoGroupId,
        undo: bool,
        recorded: &UndoGroupSheets,
        active_path: &Path,
    ) -> GroupSheets {
        let mut paths = recorded.live.clone();
        if !paths.iter().any(|path| path == active_path) {
            paths.push(active_path.to_path_buf());
        }
        paths.sort();
        let mut sheets = GroupSheets::default();
        for path in paths {
            let Some(engine) = self.engines.get(&path) else {
                // A redo owes nothing to a sheet the undo did not revert.
                if undo || recorded.undone.contains(&path) {
                    sheets.out_of_reach.push(path);
                }
                continue;
            };
            // A pick whose class edit was already undone is no later edit.
            // It is read past here and moved only once the step goes ahead.
            let top = engine
                .group_past_dead_picks(undo, |marker| !self.net_class_pick_is_live(marker, undo));
            if top == Some(group) {
                sheets.members.push(path);
                continue;
            }
            // Undo: gone from this sheet's undo stack (reopened, or history
            // trimmed) means it cannot go back with the others; a newer edit
            // above a present entry blocks. Redo: only the sheets the undo
            // reverted can owe one; with no history at all it was reopened,
            // not edited.
            let (expected, edited) = if undo {
                // The active sheet is only looked at: it owes the step only
                // if the run recorded it.
                (recorded.live.contains(&path), engine.has_undo_group(group))
            } else {
                let owed = recorded.undone.contains(&path);
                (owed, owed && (engine.can_undo() || engine.can_redo()))
            };
            if edited {
                sheets.blocked.push(path);
            } else if expected {
                sheets.out_of_reach.push(path);
            }
        }
        sheets
    }

    /// Step every member: whether any moved, the active sheet's patch, and
    /// the other members' events. Records which sheets the undo reverted and
    /// which it took back to disk.
    fn step_group_members(
        &mut self,
        group: UndoGroupId,
        undo: bool,
        recorded: &UndoGroupSheets,
        members: Vec<PathBuf>,
        undocked: &HashSet<PathBuf>,
    ) -> (bool, Option<DocumentPatch>, Vec<CoreEvent>) {
        let (mut moved, mut active_patch, mut events) = (false, None, Vec::new());
        let mut dropped_now = Vec::new();
        for path in &members {
            let Some(engine) = self.engines.get_mut(path) else {
                continue;
            };
            let patch = match if undo { engine.undo() } else { engine.redo() } {
                Ok(Some(pair)) => pair.document,
                Ok(None) => continue,
                // `Engine::undo` / `redo` return `Err` never today, and the
                // pre-check guarantees every member has an entry, so this arm
                // is unreachable. If either ever starts failing, the sheets
                // already stepped stay stepped; it is logged.
                Err(error) => {
                    debug_assert!(false, "group step failed after the pre-check: {error}");
                    events.push(engine_error_notice(
                        ids::NOTICE_EDIT_GROUP_STEP_FAILED,
                        error,
                    ));
                    continue;
                }
            };
            moved = true;
            if self.active_path.as_deref() == Some(path.as_path()) {
                active_patch = Some(patch);
            } else if undo && self.is_back_at_disk(path, recorded, undocked) {
                // A clean tabless engine would own the path and shadow the
                // file; drop it so the next use re-reads the disk. Redo is
                // then refused whole (`dropped`).
                self.clear_document_dirty(path);
                self.engines.remove(path);
                self.document_saves.forget_disk_records(path);
                dropped_now.push(path.clone());
            } else {
                self.settle_group_member(path, patch, &mut events);
            }
        }
        if undo && let Some(record) = self.undo_groups.get_mut(&group) {
            record.undone = members;
            record.dropped.extend(dropped_now);
        }
        (moved, active_patch, events)
    }

    /// A parked sheet an undo took back to what its file holds (no step left,
    /// or at the save point of a sheet whose tab closed) has nothing to save;
    /// a redo marks it dirty again.
    fn is_back_at_disk(
        &self,
        path: &Path,
        recorded: &UndoGroupSheets,
        undocked: &HashSet<PathBuf>,
    ) -> bool {
        recorded.parked.iter().any(|parked| parked == path)
            && self
                .engines
                .get(path)
                .is_some_and(|engine| !engine.has_document_step(true) || engine.is_at_save_point())
            // A save of the re-prefixed bytes may still be landing; its
            // `mark_saved` refuses on the revision mismatch, so clearing the
            // mark now would lose this undo on disk.
            && !self.document_saves.is_saving(path)
            && !self.tabs.iter().any(|tab| tab.path == path)
            && !undocked.contains(path)
    }

    /// A member other than the active sheet: its dirty mark follows the
    /// history, and the windows showing it repaint (owed to the end of an
    /// agent's edit list that targets it).
    fn settle_group_member(
        &mut self,
        path: &Path,
        patch: DocumentPatch,
        events: &mut Vec<CoreEvent>,
    ) {
        self.sync_history_dirty(path);
        self.show_member(path, patch, ChangeCause::Step, events);
    }

    /// A member other than the active sheet changed: the windows showing it
    /// repaint, or the refresh is owed to the end of an agent's edit list
    /// that targets it.
    pub(super) fn show_member(
        &mut self,
        path: &Path,
        patch: DocumentPatch,
        cause: ChangeCause,
        events: &mut Vec<CoreEvent>,
    ) {
        if self.agent_edits.target.as_deref() == Some(path) {
            self.agent_edits.owed |= patch;
            return;
        }
        let path = path.to_path_buf();
        events.push(DocumentChanged { path, patch, cause }.into());
    }

    /// Pass the dead net-class pick markers on top of the sheet at `path`'s
    /// timeline: a project-wide step below them is then the sheet's next one.
    /// A live marker stays and blocks it, like any later edit.
    fn pass_dead_class_picks(&mut self, path: &Path, undoing: bool) {
        while self
            .engines
            .get(path)
            .and_then(|engine| engine.next_class_pick(undoing))
            .is_some_and(|marker| !self.net_class_pick_is_live(marker, undoing))
        {
            if let Some(engine) = self.engines.get_mut(path) {
                engine.pass_dead_class_pick(undoing);
            }
        }
        // A pass can push the saved step off a full stack.
        self.sync_history_dirty(path);
    }
}
