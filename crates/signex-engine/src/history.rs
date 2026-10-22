use signex_types::schematic::SchematicSheet;

use crate::EngineError;
use crate::patch::PatchPair;

use super::Engine;

pub(super) const MAX_HISTORY_ENTRIES: usize = 100;

#[derive(Debug)]
pub(super) struct HistoryEntry {
    pub snapshot: SchematicSheet,
    pub patch_pair: PatchPair,
}

impl Engine {
    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> Result<Option<PatchPair>, EngineError> {
        let Some(entry) = self.history.pop() else {
            return Ok(None);
        };

        let patch_pair = entry.patch_pair;
        let redo_snapshot = std::mem::replace(&mut self.document, entry.snapshot);
        self.redo_stack.push(HistoryEntry {
            snapshot: redo_snapshot,
            patch_pair,
        });
        Ok(Some(patch_pair))
    }

    pub fn redo(&mut self) -> Result<Option<PatchPair>, EngineError> {
        let Some(entry) = self.redo_stack.pop() else {
            return Ok(None);
        };

        let patch_pair = entry.patch_pair;
        let undo_snapshot = std::mem::replace(&mut self.document, entry.snapshot);
        self.history.push(HistoryEntry {
            snapshot: undo_snapshot,
            patch_pair,
        });
        Ok(Some(patch_pair))
    }

    pub(crate) fn record_history(&mut self, snapshot: SchematicSheet, patch_pair: PatchPair) {
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

        if self.history.len() >= MAX_HISTORY_ENTRIES {
            self.history.remove(0);
        }
        self.history.push(HistoryEntry {
            snapshot,
            patch_pair,
        });
        self.redo_stack.clear();
    }
}
