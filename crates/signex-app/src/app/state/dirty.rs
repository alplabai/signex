//! The one owner of "this document has unsaved edits".
//!
//! One stored flag per document, and every other answer is derived from it:
//!
//! - a symbol or footprint draft stores it in its own editor, which flips it
//!   on every edit (the editor update modules hold no workspace state); a
//!   save snapshot resets that flag, so the path then rides in the set
//!   until the write lands;
//! - every other document (a schematic sheet, a PCB, a project file, a
//!   library manifest) is a path in [`DirtyDocuments`], whether or not a
//!   tab shows it, so a parked engine stays marked after its tab closed;
//! - a Component editor keeps its flag in `LibraryState.editors`, keyed by
//!   row, and reaches the tab bar through `tab_bar::dirty_tab_paths`.
//!
//! Tabs, the title bar count, the quit and close gates, Save All and the
//! project tree all ask [`DocumentState::is_document_dirty`] or one of its
//! siblings, and every write goes through [`DocumentState::mark_document_dirty`],
//! [`DocumentState::mark_document_clean`] or [`DocumentState::rekey_document_dirty`].
//! The set's contents are private to this module, so the compiler keeps it that way. Rationale and the
//! failures the old three-way mirror caused: docs/DIRTY_STATE.md.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::DocumentState;

/// The paths of documents with unsaved edits that no editor draft holds.
#[derive(Debug, Default)]
pub(crate) struct DirtyDocuments {
    paths: HashSet<PathBuf>,
    /// The subset of `paths` marked by something other than the engine's
    /// undo history (a project edit, a net colour, an annotate pass, a
    /// restore). Undoing back to the engine's save point cannot clear these:
    /// only a save does.
    external: HashSet<PathBuf>,
}

impl DocumentState {
    /// Does the document at `path` hold edits that are not on disk?
    pub fn is_document_dirty(&self, path: &Path) -> bool {
        self.dirty_docs.paths.contains(path) || self.editor_draft_is_dirty(path)
    }

    /// Is any document dirty? Cheaper than counting.
    pub fn has_dirty_documents(&self) -> bool {
        !self.dirty_docs.paths.is_empty()
            || self.symbol_editors.values().any(|e| e.is_dirty())
            || self.footprint_editors.values().any(|e| e.is_dirty())
    }

    /// How many documents are dirty (the title bar's "N unsaved").
    pub fn dirty_document_count(&self) -> usize {
        self.dirty_document_paths().len()
    }

    /// Every dirty document's path: the stored set plus the dirty drafts.
    pub fn dirty_document_paths(&self) -> HashSet<PathBuf> {
        let symbols = self.symbol_editors.iter().filter(|(_, e)| e.is_dirty());
        let footprints = self.footprint_editors.iter().filter(|(_, e)| e.is_dirty());
        self.dirty_docs
            .paths
            .iter()
            .cloned()
            .chain(symbols.map(|(path, _)| path.clone()))
            .chain(footprints.map(|(path, _)| path.clone()))
            .collect()
    }

    /// Record unsaved edits for `path`. A draft open at `path` owns the flag;
    /// any other document is added to the stored set.
    /// A schematic sheet or a project file also counts as a sheet edit for ERC
    /// (see [`Self::note_sheet_edit`]); any other document (PCB, library
    /// manifest) does not.
    pub fn mark_document_dirty(&mut self, path: &Path) {
        self.mark_dirty_inner(path, can_change_erc_results(path));
    }

    /// [`Self::mark_document_dirty`] for an edit that only changes how a sheet or
    /// project is shown (net colours, net classes): no finding can change.
    pub fn mark_document_dirty_display_only(&mut self, path: &Path) {
        self.mark_dirty_inner(path, false);
    }

    fn mark_dirty_inner(&mut self, path: &Path, affects_erc: bool) {
        if let Some(editor) = self.symbol_editors.get_mut(path) {
            editor.mark_dirty();
        } else if let Some(editor) = self.footprint_editors.get_mut(path) {
            editor.mark_dirty();
        } else {
            self.dirty_docs.paths.insert(path.to_path_buf());
            self.dirty_docs.external.insert(path.to_path_buf());
            if affects_erc {
                self.note_sheet_edit();
            }
        }
    }

    /// The engine's history moved (an edit, an undo, a redo) on the sheet at
    /// `path`: it is dirty unless the history sits at its save point and
    /// nothing outside the history marked it. Undoing the only edit since the
    /// last save is clean again; redoing it is dirty again.
    pub fn sync_history_dirty(&mut self, path: &Path) {
        let Some(engine) = self.engines.get(path) else {
            return;
        };
        // A write that has not landed holds the sheet dirty: an Undo back to
        // the old save point during it would otherwise read clean while the
        // file is about to receive the newer snapshot. The completion
        // settles the mark (clean on a matching revision, re-synced else).
        let clean = engine.is_at_save_point()
            && !self.dirty_docs.external.contains(path)
            && !self.document_saves.is_saving(path);
        if clean {
            self.dirty_docs.paths.remove(path);
        } else {
            self.dirty_docs.paths.insert(path.to_path_buf());
        }
    }

    /// A schematic sheet changed: ERC results taken earlier no longer describe it.
    pub fn note_sheet_edit(&mut self) {
        self.sheet_edit_epoch += 1;
    }

    /// A draft's save snapshot reset its edit flag, but the write has not
    /// landed: keep `path` dirty until [`Self::mark_document_clean`] says
    /// it did, so a quit, a tab close or a failed write still sees it.
    pub fn keep_document_dirty_until_saved(&mut self, path: &Path) {
        self.dirty_docs.paths.insert(path.to_path_buf());
    }

    /// A save landed for a snapshot older than the draft's newest edit: the
    /// editor flag now carries the dirt, so release the hold. A later
    /// rejected edit that gives the flag back as clean then makes the draft
    /// clean. A path no draft backs keeps its entry.
    pub fn release_save_hold(&mut self, path: &Path) {
        if self.symbol_editors.contains_key(path) || self.footprint_editors.contains_key(path) {
            self.dirty_docs.paths.remove(path);
        }
    }

    /// The document at `path` matches the disk (a save landed) or its edits
    /// were discarded: clear every place that knew it dirty.
    pub fn mark_document_clean(&mut self, path: &Path) {
        self.dirty_docs.paths.remove(path);
        self.dirty_docs.external.remove(path);
        // The engine's document is what is (now) on disk: its history
        // position is the point undo and redo return to as clean.
        if let Some(engine) = self.engines.get_mut(path) {
            engine.mark_saved();
        }
        if let Some(editor) = self.symbol_editors.get_mut(path) {
            editor.set_dirty(false);
        }
        if let Some(editor) = self.footprint_editors.get_mut(path) {
            editor.set_dirty(false);
        }
    }

    /// Clear every dirty document whose path satisfies `matches`: one file
    /// kept under several spellings, or everything under a directory.
    pub fn mark_documents_clean_where(&mut self, matches: impl Fn(&Path) -> bool) {
        let hit: Vec<PathBuf> = self
            .dirty_document_paths()
            .into_iter()
            .filter(|path| matches(path))
            .collect();
        for path in hit {
            self.mark_document_clean(&path);
        }
    }

    /// Move `from`'s stored dirty mark to `to` (a Save As or a rename re-keyed
    /// the document). Returns whether `from` was dirty. A draft's flag
    /// travels with its editor, which the caller re-keys; this moves the rest.
    pub fn rekey_document_dirty(&mut self, from: &Path, to: &Path) -> bool {
        let was_dirty = self.dirty_docs.paths.remove(from);
        if was_dirty {
            self.dirty_docs.paths.insert(to.to_path_buf());
        }
        if self.dirty_docs.external.remove(from) {
            self.dirty_docs.external.insert(to.to_path_buf());
        }
        was_dirty
    }

    fn editor_draft_is_dirty(&self, path: &Path) -> bool {
        self.symbol_editors.get(path).is_some_and(|e| e.is_dirty())
            || self
                .footprint_editors
                .get(path)
                .is_some_and(|e| e.is_dirty())
    }
}

/// Whether a change to the document at `path` can change an ERC finding. A
/// project file may (rules, the sheet list, a rename), so it counts; when in
/// doubt a path counts.
fn can_change_erc_results(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("snxsch") || e.eq_ignore_ascii_case("snxprj"))
}
