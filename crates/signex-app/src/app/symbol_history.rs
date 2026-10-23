//! Undo / redo snapshots for the Symbol editor.
//!
//! A step captures the whole `.snxsym` container plus the active index,
//! not just the active symbol: an edit of symbol A, a switch to B and an
//! undo must put A's edit back into A, and deleting a symbol must be
//! undoable (#760). Mirrors `FootprintHistorySnapshot`.
//!
//! The symbols are held as `Arc<Symbol>` and a new snapshot reuses the
//! `Arc` of every symbol that equals its counterpart in a neighbouring
//! step, so an edit costs one cloned symbol, not a clone of the file.

use std::sync::Arc;

use signex_library::{Symbol, SymbolFile};

use super::documents::{FieldEditKey, SymbolEditorState};

/// One undo / redo step: the container and which symbol was active.
#[derive(Debug, Clone)]
pub struct SymbolHistorySnapshot {
    /// The container with its symbol list emptied; see [`Self::symbols`].
    header: SymbolFile,
    /// The file's symbols, structurally shared with neighbouring steps.
    pub symbols: Vec<Arc<Symbol>>,
    pub active_idx: usize,
}

impl SymbolHistorySnapshot {
    /// The step's symbol equal to `symbol`, looked up at `idx` first
    /// (the common case) and by uuid when a delete shifted the slots.
    fn shared_copy_of(&self, idx: usize, symbol: &Symbol) -> Option<&Arc<Symbol>> {
        let at = |candidate: &&Arc<Symbol>| ***candidate == *symbol;
        self.symbols.get(idx).filter(at).or_else(|| {
            self.symbols
                .iter()
                .filter(|s| s.uuid == symbol.uuid)
                .find(at)
        })
    }
}

impl SymbolEditorState {
    /// Depth cap for the undo / redo stacks.
    pub const HISTORY_DEPTH: usize = 100;

    /// Capture the current container and active index. Symbols equal to
    /// ones in the newest undo / redo step share that step's allocation.
    pub fn snapshot(&self) -> SymbolHistorySnapshot {
        let neighbours = [self.undo_snapshots.last(), self.redo_snapshots.last()];
        let symbols = self
            .file
            .symbols
            .iter()
            .enumerate()
            .map(|(idx, symbol)| {
                neighbours
                    .iter()
                    .flatten()
                    .find_map(|step| step.shared_copy_of(idx, symbol))
                    .map_or_else(|| Arc::new(symbol.clone()), Arc::clone)
            })
            .collect();
        SymbolHistorySnapshot {
            header: SymbolFile {
                format: self.file.format.clone(),
                file_uuid: self.file.file_uuid,
                display_name: self.file.display_name.clone(),
                symbols: Vec::new(),
                created: self.file.created,
                updated: self.file.updated,
            },
            symbols,
            active_idx: self.active_idx,
        }
    }

    /// Write a snapshot back. Selection and unit live on the editor, not
    /// in the snapshot: they are cleared, and when the step lands on a
    /// different symbol the unit and viewport reset as a symbol switch
    /// does, so new pins cannot land on a unit the symbol lacks.
    pub fn restore(&mut self, snap: SymbolHistorySnapshot) {
        let previous_idx = self.active_idx;
        self.file = SymbolFile {
            symbols: snap.symbols.into_iter().map(Arc::unwrap_or_clone).collect(),
            ..snap.header
        };
        self.active_idx = snap
            .active_idx
            .min(self.file.symbols.len().saturating_sub(1));
        self.mid_drag = false;
        self.last_edit_key = None;
        self.selected = None;
        if self.active_idx == previous_idx {
            let max = crate::library::editor::symbol::state::max_part_number(self.primitive());
            self.active_part = self.active_part.clamp(1, max);
            self.canvas_cache.clear();
        } else {
            self.reset_view_for_symbol_switch();
        }
        self.dirty = true;
    }

    /// Start the newly active symbol on unit 1 with a fresh viewport, so
    /// it does not inherit the previous symbol's unit, pan or zoom.
    pub(crate) fn reset_view_for_symbol_switch(&mut self) {
        self.selected = None;
        self.active_part = 1;
        self.reset_camera_origin_center();
        self.cursor_mm = None;
        self.canvas_cache.clear();
    }

    /// Record `snap` as an undo step: clears redo, evicts the oldest step
    /// past [`Self::HISTORY_DEPTH`] and ends any coalescing run.
    pub fn commit_history(&mut self, snap: SymbolHistorySnapshot) {
        self.undo_snapshots.push(snap);
        if self.undo_snapshots.len() > Self::HISTORY_DEPTH {
            self.undo_snapshots.remove(0);
        }
        self.redo_snapshots.clear();
        self.last_edit_key = None;
    }

    /// Run a Properties-panel edit as one undo step. `edit` returns
    /// whether it changed anything; the step is only recorded then, so a
    /// rejected edit (missing target) leaves history and redo untouched.
    /// With a `key`, repeated edits of the same field of the same symbol
    /// share the first one's step, so typing in a text input is one step.
    /// Mirrors `FootprintEditorState::record_edit`.
    pub fn record_edit(
        &mut self,
        key: Option<FieldEditKey>,
        edit: impl FnOnce(&mut Self) -> bool,
    ) -> bool {
        let key = key.map(|field| (field, self.active_idx));
        let coalesce = key.is_some() && self.last_edit_key == key;
        let snap = (!coalesce).then(|| self.snapshot());
        if !edit(self) {
            return false;
        }
        if let Some(snap) = snap {
            self.commit_history(snap);
        }
        self.last_edit_key = key;
        true
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
