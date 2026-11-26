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

use super::SavePoint;
use super::SymbolDraft;
use super::documents::{FieldEditKey, SymbolDocument};

/// One undo / redo step: the container and which symbol was active.
#[derive(Debug, Clone)]
pub struct SymbolHistorySnapshot {
    /// The container with its symbol list emptied; see [`Self::symbols`].
    header: SymbolFile,
    /// The file's symbols, structurally shared with neighbouring steps.
    pub symbols: Vec<Arc<Symbol>>,
    pub active_idx: usize,
    /// The save point when the step was captured; see
    /// [`SymbolDocument::commit_history`].
    point: SavePoint,
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

impl SymbolDraft {
    /// Capture the current container and active index. Symbols equal to
    /// ones in the newest undo / redo step share that step's allocation.
    pub fn snapshot(&self) -> SymbolHistorySnapshot {
        let neighbours = [
            self.history.undo_steps().last(),
            self.history.redo_steps().last(),
        ];
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
                copied_from: self.file.copied_from,
                symbols: Vec::new(),
                created: self.file.created,
                updated: self.file.updated,
            },
            symbols,
            active_idx: self.active_idx,
            point: self.history.point(),
        }
    }

    /// Write a snapshot's content back. The file's id is the live one: a
    /// step brings back content, not identity (#2172).
    fn restore(&mut self, snap: SymbolHistorySnapshot) {
        let live_id = self.file.file_uuid;
        self.file = SymbolFile {
            symbols: snap.symbols.into_iter().map(Arc::unwrap_or_clone).collect(),
            file_uuid: live_id,
            ..snap.header
        };
        self.active_idx = snap
            .active_idx
            .min(self.file.symbols.len().saturating_sub(1));
        self.last_edit_key = None;
    }

    /// Dirty exactly when the history is away from the saved depth (or rows
    /// are still owed): the flag after an Undo or Redo (#1854).
    fn sync_dirty_to_history(&mut self) {
        self.set_dirty(self.history.is_dirty() || !self.cascade_owed.is_empty());
    }

    /// Record `snap` as an undo step: clears redo, evicts the oldest step
    /// past the depth cap and ends any coalescing run. The save
    /// point is the one `snap` captured, so an edit's own `mark_dirty`
    /// between the capture and this call cannot cost it; a point on the
    /// cleared redo branch or on the evicted step is forgotten.
    pub fn commit_history(&mut self, snap: SymbolHistorySnapshot) {
        self.history.set_point(snap.point);
        self.history.record(snap);
        self.last_edit_key = None;
        self.history_generation += 1;
    }

    /// Apply one Undo (`back`) or Redo step to the content. Returns the
    /// symbol index active before it, or `None` when there is no step.
    fn step(&mut self, back: bool) -> Option<usize> {
        let snap = self.history.begin_step(back)?;
        let current = self.snapshot();
        self.history.finish_step(current, back);
        self.history_generation += 1;
        let previous_idx = self.active_idx;
        self.restore(snap);
        self.sync_dirty_to_history();
        Some(previous_idx)
    }
}

impl SymbolDocument {
    /// Depth cap for the undo / redo stacks.
    pub const HISTORY_DEPTH: usize = signex_history::DEFAULT_UNDO_DEPTH;

    /// See [`SymbolDraft::snapshot`].
    pub fn snapshot(&self) -> SymbolHistorySnapshot {
        self.draft.snapshot()
    }

    /// See [`SymbolDraft::commit_history`].
    pub fn commit_history(&mut self, snap: SymbolHistorySnapshot) {
        self.draft.commit_history(snap);
    }

    /// Start the newly active symbol on unit 1 with a fresh viewport, so
    /// it does not inherit the previous symbol's unit, pan or zoom.
    pub(crate) fn reset_view_for_symbol_switch(&mut self) {
        self.gui.selected = None;
        self.draft.active_part = 1;
        self.draft.active_body = signex_types::schematic::BodyStyle::Normal;
        self.reset_camera_origin_center();
        self.gui.cursor_mm = None;
        self.gui.canvas_cache.clear();
    }

    /// Run a Properties-panel edit as one undo step. `edit` returns
    /// whether it changed anything; the step is only recorded then, so a
    /// rejected edit (missing target) leaves history and redo untouched.
    /// With a `key`, repeated edits of the same field of the same symbol
    /// share the first one's step, so typing in a text input is one step.
    /// Mirrors `FootprintDocument::record_edit`.
    pub fn record_edit(
        &mut self,
        key: Option<FieldEditKey>,
        edit: impl FnOnce(&mut Self) -> bool,
    ) -> bool {
        let key = key.map(|field| (field, self.draft.active_idx));
        let coalesce = key.is_some() && self.draft.last_edit_key == key;
        let snap = (!coalesce).then(|| self.snapshot());
        if !edit(self) {
            return false;
        }
        if let Some(snap) = snap {
            self.commit_history(snap);
        } else {
            self.draft.history_generation += 1;
        }
        self.draft.last_edit_key = key;
        true
    }

    /// Step back. Returns whether a step was applied.
    pub fn undo(&mut self) -> bool {
        self.apply_history_step(true)
    }

    /// Step forward. Returns whether a step was applied.
    pub fn redo(&mut self) -> bool {
        self.apply_history_step(false)
    }

    /// Selection and the drag live on the editor, not in the snapshot: a
    /// step clears them, and when it lands on a different symbol the unit
    /// and viewport reset as a symbol switch does, so new pins cannot land
    /// on a unit the symbol lacks.
    fn apply_history_step(&mut self, back: bool) -> bool {
        let Some(previous_idx) = self.draft.step(back) else {
            return false;
        };
        self.gui.mid_drag = false;
        self.gui.selected = None;
        if self.draft.active_idx == previous_idx {
            let max = crate::library::editor::symbol::state::max_part_number(self.primitive());
            self.draft.active_part = self.draft.active_part.clamp(1, max);
            self.gui.canvas_cache.clear();
        } else {
            self.reset_view_for_symbol_switch();
        }
        true
    }
}
