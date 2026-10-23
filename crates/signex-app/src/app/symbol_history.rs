//! Undo / redo snapshots for the Symbol editor.
//!
//! A step captures the whole `.snxsym` container plus the active index,
//! not just the active symbol: an edit of symbol A, a switch to B and an
//! undo must put A's edit back into A, and deleting a symbol must be
//! undoable (#760). Mirrors `FootprintHistorySnapshot`.

use super::documents::SymbolEditorState;

/// One undo / redo step: the container and which symbol was active.
#[derive(Debug, Clone)]
pub struct SymbolHistorySnapshot {
    pub file: signex_library::SymbolFile,
    pub active_idx: usize,
}

impl SymbolEditorState {
    /// Depth cap for the undo / redo stacks.
    pub const HISTORY_DEPTH: usize = 100;

    /// Capture the current container and active index by value.
    pub fn snapshot(&self) -> SymbolHistorySnapshot {
        SymbolHistorySnapshot {
            file: self.file.clone(),
            active_idx: self.active_idx,
        }
    }

    /// Write a snapshot back. `active_part` and the selection live on the
    /// editor, not in the snapshot, so they are re-clamped / cleared.
    pub fn restore(&mut self, snap: SymbolHistorySnapshot) {
        self.file = snap.file;
        self.active_idx = snap
            .active_idx
            .min(self.file.symbols.len().saturating_sub(1));
        self.mid_drag = false;
        self.selected = None;
        let max = crate::library::editor::symbol::state::max_part_number(self.primitive());
        self.active_part = self.active_part.clamp(1, max);
        self.canvas_cache.clear();
        self.dirty = true;
    }

    /// Record `snap` as an undo step: clears redo, evicts the oldest step
    /// past [`Self::HISTORY_DEPTH`].
    pub fn commit_history(&mut self, snap: SymbolHistorySnapshot) {
        self.undo_snapshots.push(snap);
        if self.undo_snapshots.len() > Self::HISTORY_DEPTH {
            self.undo_snapshots.remove(0);
        }
        self.redo_snapshots.clear();
    }

    /// Step back. Returns whether a step was applied.
    pub fn undo(&mut self) -> bool {
        let Some(snap) = self.undo_snapshots.pop() else {
            return false;
        };
        self.redo_snapshots.push(self.snapshot());
        self.restore(snap);
        true
    }

    /// Step forward. Returns whether a step was applied.
    pub fn redo(&mut self) -> bool {
        let Some(snap) = self.redo_snapshots.pop() else {
            return false;
        };
        self.undo_snapshots.push(self.snapshot());
        self.restore(snap);
        true
    }
}
