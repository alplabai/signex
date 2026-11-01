use signex_types::schematic::SchematicSheet;

use crate::EngineError;
use crate::patch::{DocumentPatch, PatchPair};

use super::Engine;
use super::history_snapshot::SheetSnapshot;

pub(crate) const MAX_HISTORY_ENTRIES: usize = 100;

#[derive(Debug)]
pub(super) struct HistoryEntry {
    pub snapshot: SheetSnapshot,
    pub patch_pair: PatchPair,
    /// Set when this entry is one sheet's share of a project-wide step; the
    /// app undoes/redoes every sheet carrying the same id together.
    pub group: Option<UndoGroupId>,
}

/// Identifies one project-wide undo step (e.g. an annotate run) whose
/// per-sheet history entries must be undone and redone together. The engine
/// only stamps and reports it; coordinating sheets is the app's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UndoGroupId(u64);

impl UndoGroupId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

/// The parameter cell a run of keystrokes is editing, and where the history
/// stood right after the run's entry was recorded.
#[derive(Debug)]
pub(super) struct FieldEditRun {
    symbol_id: uuid::Uuid,
    key: String,
    position: usize,
}

impl Engine {
    /// Record a `SetSymbolField` edit. Consecutive edits of the same cell
    /// share the one history entry the first of them recorded, so typing a
    /// value is one undo step while every keystroke is already in the
    /// document. The run ends when anything else lands in the history, when
    /// another cell is edited, on undo/redo, or on
    /// [`Engine::end_field_edit_run`].
    pub(crate) fn record_field_edit(
        &mut self,
        before: SchematicSheet,
        patch_pair: PatchPair,
        symbol_id: uuid::Uuid,
        key: &str,
    ) {
        if self.history_suppressed {
            self.record_history(before, patch_pair);
            return;
        }
        let continues = self.redo_stack.is_empty()
            && self.field_edit_run.as_ref().is_some_and(|run| {
                run.symbol_id == symbol_id && run.key == key && run.position == self.undo_position()
            });
        if continues {
            // The run rewrites the entry the history already ends in, so a
            // save taken at this position no longer matches the document.
            if self.saved_point == Some(self.undo_position()) {
                self.saved_point = None;
            }
        } else {
            self.record_history(before, patch_pair);
            self.field_edit_run = Some(FieldEditRun {
                symbol_id,
                key: key.to_string(),
                position: self.undo_position(),
            });
        }
    }

    /// Close the current cell-typing run: the next edit of the same cell
    /// starts a new undo step. Enter, closing the Parameter Manager.
    pub fn end_field_edit_run(&mut self) {
        self.field_edit_run = None;
    }

    /// The document now matches the file just written: remember this point
    /// of the history, so undoing or redoing back to it is clean again.
    pub fn mark_saved(&mut self) {
        self.saved_point = Some(self.undo_position());
    }

    /// The file holds something no step of this history reproduces (a save
    /// whose snapshot the document has moved past): nothing is clean until
    /// the next [`Engine::mark_saved`].
    pub fn forget_save_point(&mut self) {
        self.saved_point = None;
    }

    /// Does the document equal the last saved or loaded state, judged by
    /// the history alone? Undo and redo move this both ways; an edit made
    /// below the save point makes it unreachable until the next save.
    pub fn is_at_save_point(&self) -> bool {
        self.saved_point == Some(self.undo_position())
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Whether any undo entry still belongs to `group`.
    pub fn has_undo_group(&self, group: UndoGroupId) -> bool {
        self.history.iter().any(|entry| entry.group == Some(group))
    }

    /// Group of the entry the next [`Engine::undo`] would revert, if any.
    pub fn next_undo_group(&self) -> Option<UndoGroupId> {
        self.history.back().and_then(|entry| entry.group)
    }

    /// Group of the entry the next [`Engine::redo`] would restore, if any.
    pub fn next_redo_group(&self) -> Option<UndoGroupId> {
        self.redo_stack.last().and_then(|entry| entry.group)
    }

    /// Monotonic position in the edit timeline: how many recorded edits
    /// are currently applied, counting any evicted by the history cap.
    /// Undo lowers it, redo raises it, a new edit raises it and drops
    /// the redo branch. App-level state that interleaves with engine
    /// history (net colours) stamps itself with this to order undo.
    pub fn undo_position(&self) -> usize {
        self.history_evicted + self.history.len()
    }

    /// An undo or redo swaps in a snapshot; connectivity moves with it when
    /// the step being replayed touched any electrical collection.
    fn note_history_step(&mut self, patch_pair: PatchPair) {
        if patch_pair.document.intersects(DocumentPatch::ELECTRICAL) {
            self.bump_connectivity_revision();
        }
    }

    pub fn undo(&mut self) -> Result<Option<PatchPair>, EngineError> {
        self.field_edit_run = None;
        self.bump_revision();
        let Some(entry) = self.history.pop_back() else {
            return Ok(None);
        };

        let patch_pair = entry.patch_pair;
        self.note_history_step(patch_pair);
        let group = entry.group;
        let redo_snapshot = std::mem::replace(&mut self.document, entry.snapshot.into_sheet());
        let redo_snapshot =
            SheetSnapshot::capture(redo_snapshot, self.redo_stack.last().map(|e| &e.snapshot));
        self.redo_stack.push(HistoryEntry {
            snapshot: redo_snapshot,
            patch_pair,
            group,
        });
        Ok(Some(patch_pair))
    }

    /// Undo the top entry when it belongs to `group`, and forget it: it does
    /// not land on the redo stack. For a grouped run that failed halfway and
    /// was reported as never having happened, so no Redo can bring half of it
    /// back. `None` (nothing changed) when the top entry is not `group`'s.
    pub fn discard_undo_group_top(
        &mut self,
        group: UndoGroupId,
    ) -> Result<Option<PatchPair>, EngineError> {
        if self.next_undo_group() != Some(group) {
            return Ok(None);
        }
        let position = self.undo_position();
        let patch_pair = self.undo()?;
        self.redo_stack.pop();
        if self.saved_point == Some(position) {
            self.saved_point = None;
        }
        Ok(patch_pair)
    }

    pub fn redo(&mut self) -> Result<Option<PatchPair>, EngineError> {
        self.field_edit_run = None;
        self.bump_revision();
        let Some(entry) = self.redo_stack.pop() else {
            return Ok(None);
        };

        let patch_pair = entry.patch_pair;
        self.note_history_step(patch_pair);
        let group = entry.group;
        let undo_snapshot = std::mem::replace(&mut self.document, entry.snapshot.into_sheet());
        let undo_snapshot =
            SheetSnapshot::capture(undo_snapshot, self.history.back().map(|e| &e.snapshot));
        self.history.push_back(HistoryEntry {
            snapshot: undo_snapshot,
            patch_pair,
            group,
        });
        Ok(Some(patch_pair))
    }

    pub(crate) fn record_history(&mut self, snapshot: SchematicSheet, patch_pair: PatchPair) {
        self.record_grouped_history(snapshot, patch_pair, None);
    }

    pub(crate) fn record_grouped_history(
        &mut self,
        snapshot: SchematicSheet,
        patch_pair: PatchPair,
        group: Option<UndoGroupId>,
    ) {
        // Inside `execute_batch` the individual commands must not each land
        // an entry: the batch pushes one coalesced entry of its own once the
        // loop is done. Skipping the `redo_stack.clear()` below along with
        // the push is deliberate — a batch that turns out to have changed
        // nothing records nothing, so it must not consume a redoable step
        // either. See `exec/batch.rs` for why this is suppressed up front
        // rather than truncated away afterwards.
        if self.history_suppressed {
            return;
        }

        // Share every collection the step left unchanged with the entry
        // before it, so a long history holds one copy of each.
        let snapshot = SheetSnapshot::capture(snapshot, self.history.back().map(|e| &e.snapshot));
        // The new entry drops the redo branch; a save point inside it can
        // never be reached again.
        if self
            .saved_point
            .is_some_and(|saved| saved > self.undo_position())
        {
            self.saved_point = None;
        }
        if self.history.len() >= MAX_HISTORY_ENTRIES {
            self.history.pop_front();
            self.history_evicted += 1;
        }
        self.history.push_back(HistoryEntry {
            snapshot,
            patch_pair,
            group,
        });
        self.redo_stack.clear();
    }
}
