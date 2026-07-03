//! Footprint selection-filter preset apply/capture helpers.
//!
//! Task 6 — parallel to the schematic's `CustomFilterPreset` flow
//! (`crate::app::handlers::active_bar::filter_controls`), but scoped
//! to the footprint editor's `SelectionFilterKind` categories and
//! backed by `FootprintFilterPreset` (Task 5).

use crate::active_bar::FootprintFilterPreset;

use super::state::FootprintEditorState;

/// Replace the editor's active selection filter with exactly the
/// preset's kinds (everything else is switched off).
pub fn apply_preset(state: &mut FootprintEditorState, preset: &FootprintFilterPreset) {
    state.selection_filter.apply_kinds(&preset.kinds);
}

/// Snapshot the editor's currently-enabled filter kinds into a new
/// named preset, ready to be appended to the persisted list.
pub fn capture_preset(state: &FootprintEditorState, name: String) -> FootprintFilterPreset {
    FootprintFilterPreset {
        name,
        kinds: state.selection_filter.enabled_kinds(),
    }
}
