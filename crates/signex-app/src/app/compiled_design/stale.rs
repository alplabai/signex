//! Out of date: an edit to a sheet a compile read marks that compile's row
//! amber. Called from the edit tails (`state::sheet_change`) with the
//! engine's `DocumentPatch`; a display-only change (a colour, a font) leaves
//! the counts as they are.

use std::path::Path;

use crate::app::state::DocumentState;

impl DocumentState {
    /// Mark the sheet at `path` changed since every compile that read it.
    pub(crate) fn mark_compiled_stale(&mut self, path: &Path, patch: signex_engine::DocumentPatch) {
        if patch == signex_engine::DocumentPatch::NONE || patch.is_display_only() {
            return;
        }
        for result in self.compiled_design.results.values_mut() {
            if result.covers(path) {
                result.stale.insert(path.to_path_buf());
            }
        }
    }
}
