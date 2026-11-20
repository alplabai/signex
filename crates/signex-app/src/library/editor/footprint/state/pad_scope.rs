//! The "This pad / All pads" question (mockup row 192 D1, decision 9, Caner
//! 2026-11-20): an edit of a Pads-mode attribute of a patterned pad (not a
//! pattern's source pad) waits here until the user says where it goes. It
//! is asked every time; no answer is remembered.
//!
//! The held value lives only in the question, never in the pad list, so a
//! save, an autosave or an undo while the question waits cannot write it.

use signex_sketch::array::ArrayId;

use super::{EditorPad, FootprintEditorState};

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
    /// The pad as it is in the pad list, before the held edit.
    pub before: EditorPad,
    /// The pad as the edit leaves it, shown in the panel while the question
    /// waits.
    pub after: EditorPad,
}

impl FootprintEditorState {
    /// Pad `idx` as the Properties panel shows it: the held value while the
    /// question waits on that pad and the pad is still as it was held.
    pub fn shown_pad(&self, idx: usize) -> Option<&EditorPad> {
        let pad = self.pads.get(idx)?;
        match self.pad_scope.as_deref() {
            Some(prompt) if prompt.pad_idx == idx && prompt.before == *pad => Some(&prompt.after),
            _ => Some(pad),
        }
    }
}
