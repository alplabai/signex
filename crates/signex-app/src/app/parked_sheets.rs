//! Sheets a project-wide run loaded into tabless engines (#2760). Each one is
//! an open document while unsaved: saves compare with the bytes the run read,
//! and its earlier recovery copy is checked before autosave may write.
//!
//! Once saved, the engine leaves `engines` for `held_for_undo`: it is only an
//! undo holder, never a source of content, so every reader takes the file.
//! See `docs/architecture/path-ops.md`, "Parked sheets".

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::Signex;
use super::state::DocumentState;
use crate::app::core::held_engine::HeldEngine;
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

    /// A read of `path` for a new tab took `read`. When that is still the
    /// file the held engine saved, the tab gets the engine and its undo; true
    /// then. Otherwise the engine is released and the read is installed.
    pub(crate) fn attach_held_engine(
        &mut self,
        path: &Path,
        title: &str,
        read: ContentHash,
    ) -> bool {
        let docs = &mut self.document_state;
        if !docs.held_for_undo.contains_key(path) {
            return false;
        }
        if docs.document_saves.baseline(path) != Some(read) {
            docs.release_held(path);
            return false;
        }
        let Some(held) = docs.held_for_undo.remove(path) else {
            return false;
        };
        docs.insert_engine(path.to_path_buf(), held.engine);
        self.attach_parked_schematic_tab(path.to_path_buf(), title.to_string());
        true
    }

    /// Release the held engines no undo group needs any more. Runs after
    /// every update, so a history the depth setting trimmed is seen too.
    pub(crate) fn release_engines_past_their_undo(&mut self) {
        let docs = &mut self.document_state;
        if docs.held_for_undo.is_empty() {
            return;
        }
        let depth = docs.undo_depth;
        for held in docs.held_for_undo.values_mut() {
            held.engine.set_undo_depth(depth);
        }
        let past: Vec<PathBuf> = (docs.held_for_undo.iter())
            .filter(|(path, held)| {
                let project_gone =
                    (held.project).is_some_and(|id| !docs.projects.iter().any(|p| p.id == id));
                project_gone || !docs.undo_group_holds(path)
            })
            .map(|(path, _)| path.clone())
            .collect();
        for path in past {
            docs.release_held(&path);
        }
    }
}

impl DocumentState {
    /// Whether a recorded undo group still has its step in `path`'s history,
    /// on a live engine or a held one.
    pub(crate) fn undo_group_holds(&self, path: &Path) -> bool {
        let engine = (self.engines.get(path))
            .or_else(|| self.held_for_undo.get(path).map(|held| &held.engine));
        let Some(engine) = engine else {
            return false;
        };
        self.undo_groups.iter().any(|(group, sheets)| {
            sheets.live.iter().any(|sheet| sheet == path)
                && (engine.has_undo_group(*group) || engine.next_redo_group() == Some(*group))
        })
    }

    /// `path` is saved and nothing holds it open: move its engine out of
    /// `engines` when an undo group needs it. True when kept.
    pub(crate) fn keep_engine_for_undo(&mut self, path: &Path) -> bool {
        if !self.undo_group_holds(path) {
            return false;
        }
        let project = self.document_project(path).map(|project| project.id);
        if let Some(engine) = self.remove_engine(path) {
            self.held_for_undo
                .insert(path.to_path_buf(), HeldEngine { engine, project });
        }
        true
    }

    /// Drop the held engine of `path` and what it knew of the file. A later
    /// undo of its group names the sheet as closed.
    pub(crate) fn release_held(&mut self, path: &Path) {
        if self.held_for_undo.remove(path).is_some() {
            self.document_saves.forget_disk_records(path);
        }
    }

    /// Put the held members of `group` back in `engines` for one step.
    pub(crate) fn wake_held_members(&mut self, group: signex_engine::UndoGroupId) -> Vec<PathBuf> {
        let live = (self.undo_groups.get(&group))
            .map(|sheets| sheets.live.clone())
            .unwrap_or_default();
        let mut woken = Vec::new();
        for path in live {
            if let Some(held) = self.held_for_undo.remove(&path) {
                self.insert_engine(path.clone(), held.engine);
                woken.push(path);
            }
        }
        woken
    }

    /// The step is over. A woken sheet it left unsaved stays a parked
    /// document; one back at its saved state is only an undo holder again.
    pub(crate) fn rest_woken(&mut self, woken: &[PathBuf], undocked: &HashSet<PathBuf>) {
        for path in woken {
            if !self.engines.contains_key(path)
                || self.is_document_dirty(path)
                || self.document_saves.is_saving(path)
                || undocked.contains(path)
                || self.tabs.iter().any(|tab| tab.path == *path)
            {
                continue;
            }
            if !self.keep_engine_for_undo(path) {
                self.remove_engine(path);
                self.document_saves.forget_disk_records(path);
            }
        }
    }

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
        if let Some(mut held) = self.held_for_undo.remove(old) {
            held.engine.set_path(Some(new.to_path_buf()));
            self.held_for_undo.insert(new.to_path_buf(), held);
        }
    }
}
