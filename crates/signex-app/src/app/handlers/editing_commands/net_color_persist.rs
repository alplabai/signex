//! Net colour overrides are saved with their sheet (#1041). The live map is
//! `UiState::wire_color_overrides`; these helpers move colours between it and
//! the sheet on load, and mark a sheet dirty when its colours change.

use super::super::super::*;

impl Signex {
    /// Take the colours `path`'s sheet was saved with into the live map.
    /// Call right after its engine is installed, and after
    /// [`Signex::forget_net_colors_of`] if an older engine stood in its place.
    pub(crate) fn adopt_net_colors_of(&mut self, path: &std::path::Path) {
        let Some(engine) = self.document_state.engines.get(path) else {
            return;
        };
        self.ui_state
            .wire_color_overrides
            .extend(engine.document().net_colors.iter().map(|(id, c)| (*id, *c)));
    }

    /// How many colour changes `path`'s sheet has seen this session.
    pub(crate) fn net_color_revision_of(&self, path: &std::path::Path) -> u64 {
        self.ui_state
            .net_color_revision
            .get(path)
            .copied()
            .unwrap_or(0)
    }

    /// A colour change is an unsaved edit: mark `path`'s sheet dirty so a
    /// plain Save writes it. Never saves by itself.
    pub(crate) fn mark_net_colors_dirty(&mut self, path: &std::path::Path) {
        *self
            .ui_state
            .net_color_revision
            .entry(path.to_path_buf())
            .or_default() += 1;
        self.mark_cached_sheet_dirty(path);
        self.refresh_panel_ctx();
    }
}
