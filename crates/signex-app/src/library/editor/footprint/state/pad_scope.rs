//! The "This pad / All pads" question (#192 D1, decision 9, Caner
//! 2026-11-20): an edit of a Pads-mode attribute of a patterned pad (not a
//! pattern's source pad) waits here until the user says where it goes. It
//! is asked every time; no answer is remembered.

use signex_sketch::array::ArrayId;

use super::EditorPad;

/// Where a held pad edit goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadScope {
    /// Only this instance: a per-instance override in the sketch.
    ThisPad,
    /// Every instance: the pattern's source pad.
    AllPads,
    /// Drop the edit.
    Cancel,
}

/// A pad edit held for the question.
#[derive(Debug, Clone, PartialEq)]
pub struct PadScopePrompt {
    /// The edited pad, an index into the editor's pad list.
    pub pad_idx: usize,
    /// The instance the pad is: `(pattern, i, j)`, as overrides are keyed.
    pub instance: (ArrayId, u32, u32),
    /// The pad before the first held keystroke; Cancel puts it back.
    pub before: EditorPad,
    /// The pad as the edit leaves it, shown in the panel while the question
    /// waits.
    pub after: EditorPad,
}
