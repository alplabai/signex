//! Sheets a project-wide run loaded into tabless engines (#2760). Each one is
//! an open document while unsaved: saves compare with the bytes the run read,
//! and its earlier recovery copy is checked before autosave may write. A clean
//! save releases the engine, so the run's undo ends there for that sheet; see
//! `docs/architecture/path-ops.md`, "Parked sheets".

use std::path::Path;

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
}

impl DocumentState {
    /// The file at `path` now holds a write of ours, so an undo no longer
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
    }
}
