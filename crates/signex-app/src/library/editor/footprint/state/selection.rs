//! The one accessor that resolves "the current pad selection".
//!
//! `selected_pad` (the primary) and `selected_pads_extra` (the
//! ctrl-click additions) are two fields, and every op that acts on
//! the selection has to union them. Hand-rolling that union at each
//! call site is how the active-bar Rotate / Flip / Align-to-Grid
//! transforms ended up reading `selected_pad` alone and silently
//! acting on one pad out of N — a mixed-layer flip is a fab error, not
//! a cosmetic one. Read the selection through here.

use super::FootprintEditorState;

impl FootprintEditorState {
    /// Combined pad selection — primary plus ctrl-click extras —
    /// sorted, deduped, and clamped to the live pad list. Empty when
    /// nothing is selected.
    pub fn selected_pad_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = Vec::new();
        if let Some(p) = self.selected_pad {
            indices.push(p);
        }
        indices.extend(self.selected_pads_extra.iter().copied());
        indices.sort_unstable();
        indices.dedup();
        indices.retain(|&i| i < self.pads.len());
        indices
    }
}
