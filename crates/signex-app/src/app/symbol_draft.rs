//! The core-bound half of a `.snxsym` editor (ADR-0006, "Before 4a: the
//! symbol draft"): the content, its history and its save bookkeeping. The
//! GUI half is `SymbolEditorView`; both live in `SymbolDocument`. Nothing
//! here may name an iced or canvas type (`scripts/check_uifree_state.sh`).

use signex_library::{Symbol, SymbolFile};

use super::SymbolHistorySnapshot;
use super::documents::FieldEditKey;

#[derive(Debug)]
pub struct SymbolDraft {
    /// Multi-symbol container backing this `.snxsym` tab. The editor
    /// works against `file.symbols[active_idx]`; access via
    /// [`primitive`](Self::primitive) / [`primitive_mut`](Self::primitive_mut).
    pub file: SymbolFile,
    /// The container as this editor last loaded or saved it. A save
    /// writes only the symbols that differ from it, so another window's
    /// saves to the other symbols of the file survive (#902).
    pub baseline: SymbolFile,
    /// Which symbol within the file is currently being edited. The
    /// SCH-Library left-dock panel will eventually drive this index;
    /// for now it always lands on the first symbol.
    pub active_idx: usize,
    /// Active sub-part for multi-part components (Altium "Part X / N"
    /// arrows). `1` is the single-part default; higher values let the
    /// user step through parts. Pins with `part_number == 0` (Part
    /// Zero) render on every part; pins with `part_number == active_part`
    /// render on the active part only.
    pub active_part: u8,
    /// Which body of a De Morgan symbol the canvas shows and new items are
    /// stamped with. Items tagged body 0 show on both.
    pub active_body: signex_types::schematic::BodyStyle,
    /// Unsaved edits in this draft. Private: edits call [`Self::mark_dirty`];
    /// the workspace reads and clears it through `DocumentState`.
    dirty: bool,
    /// The stamp of the last container save started from this editor
    /// (`DocumentSaves::mint_generation`); `0` until one starts.
    pub save_generation: u64,
    /// Primitives of this container whose component rows a failed cascade
    /// left behind and a retry can still move; the next Save cascades them
    /// even with no new edit.
    pub cascade_owed: Vec<uuid::Uuid>,
    /// Undo / redo snapshots (each the whole `.snxsym` container, symbols
    /// shared structurally between steps) and the depth at which this draft
    /// last equalled its file, so Undo back to it reads clean (#1854).
    /// Capped at `SymbolDocument::HISTORY_DEPTH`; unknown stays dirty.
    pub history: signex_history::History<SymbolHistorySnapshot>,
    /// Bumped by every history change: commit, undo, redo and a coalesced
    /// field edit. Unlike the stack depths it moves at `HISTORY_DEPTH` and
    /// while typing in one field, so it tells "the document changed". Starts
    /// at a per-editor base from `DocumentState::next_symbol_history_base`.
    pub history_generation: u64,
    /// The field the last `record_edit` changed, tagged with the symbol
    /// it belonged to, so a run of keystrokes in one text input shares
    /// one undo step. Cleared by every other history push, undo and redo.
    pub(in crate::app) last_edit_key: Option<(FieldEditKey, usize)>,
    /// A drag's undo step is recorded and its later ticks join it. Opened by
    /// the first movement tick, closed by the drag commit or an Undo / Redo
    /// step; another history push leaves it open. It groups undo steps, like
    /// `last_edit_key`, so it is history state, not a front-end operand.
    move_run_open: bool,
}

impl SymbolDraft {
    /// A clean draft of `file`, on its first symbol, unit 1, normal body.
    pub fn new(file: SymbolFile, history_depth: usize) -> Self {
        Self {
            baseline: file.clone(),
            file,
            active_idx: 0,
            active_part: 1,
            active_body: signex_types::schematic::BodyStyle::Normal,
            dirty: false,
            save_generation: 0,
            cascade_owed: Vec::new(),
            history: signex_history::History::at_start(history_depth),
            history_generation: 0,
            last_edit_key: None,
            move_run_open: false,
        }
    }

    /// Is a drag's undo step open, so a movement tick records nothing?
    pub(crate) fn is_moving(&self) -> bool {
        self.move_run_open
    }

    /// Open the move run. The caller records the drag's undo step first.
    pub(crate) fn begin_move_run(&mut self) {
        self.move_run_open = true;
    }

    /// Close the move run: the next movement tick records a new step.
    pub(crate) fn end_move_run(&mut self) {
        self.move_run_open = false;
    }

    /// Does this draft hold edits that are not on disk?
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// An edit changed the draft. A change at the saved depth that no
    /// undo step records leaves no step that reproduces the file, so the
    /// save point is forgotten; a recorded edit has already moved the depth.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.history.forget_if_current();
    }

    /// Set the flag outright: a save or discard clears it.
    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    /// A save is writing the current content: it is the state Undo and Redo
    /// return to as clean, and the flag clears until the next edit.
    pub fn mark_saved(&mut self) {
        self.history.mark_saved();
        self.dirty = false;
    }

    /// Borrow the symbol currently being edited. Falls back to the
    /// first symbol in the file when `active_idx` is out of range
    /// (defensive — should never happen in practice). Panics only
    /// when the file has zero symbols, which the loader rejects.
    pub fn primitive(&self) -> &Symbol {
        let idx = self
            .active_idx
            .min(self.file.symbols.len().saturating_sub(1));
        &self.file.symbols[idx]
    }

    /// Mutable borrow of the active symbol. Same out-of-range fallback as
    /// [`primitive`](Self::primitive).
    pub fn primitive_mut(&mut self) -> &mut Symbol {
        let idx = self
            .active_idx
            .min(self.file.symbols.len().saturating_sub(1));
        &mut self.file.symbols[idx]
    }
}
