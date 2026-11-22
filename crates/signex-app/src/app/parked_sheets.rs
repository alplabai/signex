//! Sheets a project-wide run loaded into tabless engines (#2760). Each one is
//! an open document from then on: saves compare with the bytes the run read,
//! its earlier recovery copy is checked before autosave may write, and its
//! engine stays while a live undo group needs it.

use std::path::{Path, PathBuf};

use super::Signex;
use super::state::DocumentState;
use crate::persist::ContentHash;

impl Signex {
    /// A run installed a tabless engine for `path` from a copy with
    /// fingerprint `read_hash`. Autosave is held until the recovery check,
    /// which `update` starts, has reported.
    pub(crate) fn settle_parked_sheet(&mut self, path: &Path, read_hash: Option<[u8; 32]>) {
        if let Some(hash) = read_hash {
            self.record_disk_baseline(path, ContentHash::from_raw(hash));
        }
        let docs = &mut self.document_state;
        docs.autosave.begin_check(path);
        docs.recovery_check_due.push(path.to_path_buf());
    }

    /// The run that parked `path` changed nothing there or was rolled back:
    /// forget the engine and what [`Self::settle_parked_sheet`] recorded.
    pub(crate) fn abandon_parked_sheet(&mut self, path: &Path) {
        let docs = &mut self.document_state;
        docs.remove_engine(path);
        docs.document_saves.forget_disk_records(path);
        let due = docs.recovery_check_due.len();
        docs.recovery_check_due.retain(|queued| queued != path);
        if docs.recovery_check_due.len() != due {
            docs.autosave.finish_check(path);
        }
    }

    /// Release the saved engines no undo group needs any more. Runs after
    /// every update; a sheet that is open or unsaved keeps its engine.
    pub(crate) fn release_engines_past_their_undo(&mut self) {
        if self.document_state.held_for_undo.is_empty() {
            return;
        }
        let held: Vec<PathBuf> = self.document_state.held_for_undo.iter().cloned().collect();
        for path in held {
            if self.document_state.undo_group_holds(&path) {
                continue;
            }
            self.document_state.held_for_undo.remove(&path);
            let docs = &self.document_state;
            if docs.engines.contains_key(&path)
                && !docs.is_document_dirty(&path)
                && !docs.document_saves.is_saving(&path)
                && !self.document_is_held_open(&path)
            {
                self.document_state.remove_engine(&path);
                self.document_state
                    .document_saves
                    .forget_disk_records(&path);
            }
        }
    }
}

impl DocumentState {
    /// Whether a recorded undo group still has its step in `path`'s history.
    pub(crate) fn undo_group_holds(&self, path: &Path) -> bool {
        let Some(engine) = self.engines.get(path) else {
            return false;
        };
        self.undo_groups.iter().any(|(group, sheets)| {
            sheets.live.iter().any(|sheet| sheet == path)
                && (engine.has_undo_group(*group) || engine.next_redo_group() == Some(*group))
        })
    }

    /// `path` was saved and nothing holds it open: keep its engine when an
    /// undo group needs it. True when kept.
    pub(crate) fn keep_engine_for_undo(&mut self, path: &Path) -> bool {
        let keep = self.undo_group_holds(path);
        if keep {
            self.held_for_undo.insert(path.to_path_buf());
        }
        keep
    }

    /// The file at `path` now holds the run's result, so an undo no longer
    /// goes back to what is on disk and must leave the sheet unsaved.
    pub(crate) fn forget_parked_mark(&mut self, path: &Path) {
        for sheets in self.undo_groups.values_mut() {
            sheets.parked.retain(|sheet| sheet != path);
        }
    }

    /// The document at `old` moved to `new`: its undo groups follow it.
    pub(crate) fn rekey_undo_sheets(&mut self, old: &Path, new: &Path) {
        for sheets in self.undo_groups.values_mut() {
            for list in [
                &mut sheets.live,
                &mut sheets.undone,
                &mut sheets.parked,
                &mut sheets.dropped,
                &mut sheets.boards,
            ] {
                for sheet in list.iter_mut().filter(|sheet| sheet.as_path() == old) {
                    *sheet = new.to_path_buf();
                }
            }
        }
        if self.held_for_undo.remove(old) {
            self.held_for_undo.insert(new.to_path_buf());
        }
    }
}
