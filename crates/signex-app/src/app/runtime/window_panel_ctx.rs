//! The dock panels of an undocked document window show and edit that
//! window's sheet: each such window keeps a [`PanelContext`] of its own
//! (#1966). `run_in_window` swaps it in beside the window's canvas, so the
//! same builders and handlers that serve the main window fill it.

use super::super::*;
use crate::app::state::WindowKind;
use crate::panels::PanelContext;

impl PanelContext {
    /// Copy what is the app's rather than a document's: theme, units, grid,
    /// fonts, filters, diagnostics, the ERC results and the Navigator. The
    /// scalars are copied every time; the lists only when a cheap check (a
    /// length, a newest id, a fingerprint) says they changed, so a steady
    /// state costs no deep clone. A change the check misses is caught by
    /// the next full refresh.
    fn adopt_shared_from(&mut self, main: &PanelContext) {
        self.tokens = main.tokens;
        self.canvas_colors = main.canvas_colors;
        self.device_px_per_logical_px = main.device_px_per_logical_px;
        self.theme_id = main.theme_id;
        self.unit = main.unit;
        self.grid_visible = main.grid_visible;
        self.snap_enabled = main.snap_enabled;
        self.grid_size_mm = main.grid_size_mm;
        self.visible_grid_mm = main.visible_grid_mm;
        self.symbol_grid_style = main.symbol_grid_style;
        self.snap_hotspots = main.snap_hotspots;
        self.canvas_font_size = main.canvas_font_size;
        self.canvas_font_bold = main.canvas_font_bold;
        self.canvas_font_italic = main.canvas_font_italic;
        self.active_custom_filter_tab = main.active_custom_filter_tab;
        self.erc_running = main.erc_running;
        self.erc_results_stale = main.erc_results_stale;
        self.project_tree_selected
            .clone_from(&main.project_tree_selected);
        if self.ui_font_name != main.ui_font_name {
            self.ui_font_name.clone_from(&main.ui_font_name);
        }
        if self.canvas_font_name != main.canvas_font_name {
            self.canvas_font_name.clone_from(&main.canvas_font_name);
        }
        if self.diagnostics_level != main.diagnostics_level {
            self.diagnostics_level.clone_from(&main.diagnostics_level);
        }
        if self.diagnostics.last().map(|e| e.id) != main.diagnostics.last().map(|e| e.id) {
            self.diagnostics.clone_from(&main.diagnostics);
        }
        if self.component_classes.len() != main.component_classes.len() {
            self.component_classes.clone_from(&main.component_classes);
        }
        if self.selection_filters != main.selection_filters {
            self.selection_filters.clone_from(&main.selection_filters);
        }
        if self.custom_filter_presets.len() != main.custom_filter_presets.len() {
            self.custom_filter_presets
                .clone_from(&main.custom_filter_presets);
        }
        if self.footprint_filter_presets.len() != main.footprint_filter_presets.len() {
            self.footprint_filter_presets
                .clone_from(&main.footprint_filter_presets);
        }
        if self.erc_diagnostics.len() != main.erc_diagnostics.len() {
            self.erc_diagnostics.clone_from(&main.erc_diagnostics);
        }
        if self.erc_waivers.len() != main.erc_waivers.len() {
            self.erc_waivers.clone_from(&main.erc_waivers);
        }
        if self.erc_violation_waivers.len() != main.erc_violation_waivers.len() {
            self.erc_violation_waivers
                .clone_from(&main.erc_violation_waivers);
        }
        if self.navigator.fingerprint != main.navigator.fingerprint {
            self.navigator.clone_from(&main.navigator);
        }
    }

    /// Take over the per-sheet drafts of a window context that is going away,
    /// so what was typed and not yet committed survives the window.
    pub(crate) fn take_drafts_from(&mut self, window: PanelContext) {
        for draft in window.title_block_drafts {
            if !self
                .title_block_drafts
                .iter()
                .any(|d| d.sheet == draft.sheet)
            {
                self.title_block_drafts.push(draft);
            }
        }
        for draft in window.paper_edge_drafts {
            if !self
                .paper_edge_drafts
                .iter()
                .any(|d| d.sheet == draft.sheet)
            {
                self.paper_edge_drafts.push(draft);
            }
        }
        if self.numeric_draft.is_none() {
            self.numeric_draft = window.numeric_draft;
        }
        if self.drawing_edit_buf_for.is_none() {
            self.drawing_edit_buf = window.drawing_edit_buf;
            self.drawing_edit_buf_for = window.drawing_edit_buf_for;
        }
    }
}

/// What a window's context was built from; unchanged means it is current.
pub(crate) type WindowCtxKey = (Option<u64>, u64, u64, u64, usize, usize, bool);

impl Signex {
    /// The context the panels of `window` render: that window's own when it
    /// shows a document of its own, the main window's otherwise.
    pub(crate) fn panel_ctx_for(&self, window: iced::window::Id) -> &PanelContext {
        self.document_state
            .window_panel_ctx
            .get(&window)
            .filter(|_| self.is_document_window(window))
            .unwrap_or(&self.document_state.panel_ctx)
    }

    /// A context for a window that has none yet: the main window's, as the
    /// starting point for `init_window_panel_ctx`.
    pub(crate) fn new_window_panel_ctx(&self) -> PanelContext {
        self.document_state.panel_ctx.clone()
    }

    /// Fill a new window context from the window's own sheet and selection.
    /// Runs with the window swapped in (see `run_in_window`).
    pub(crate) fn init_window_panel_ctx(&mut self) {
        self.sync_panel_ctx_from_visible_document();
        self.update_selection_info();
    }

    /// Whether the dock shows any panel, so a window's context is worth
    /// building.
    fn dock_has_panels(&self) -> bool {
        use crate::dock::PanelPosition::{Bottom, Left, Right};
        let dock = &self.document_state.dock;
        [Left, Right, Bottom]
            .into_iter()
            .any(|p| dock.has_panels(p))
            || !dock.floating.is_empty()
    }

    /// What the context of `window` is built from, for a cheap "has anything
    /// changed" test.
    fn window_ctx_key(&self, window: iced::window::Id) -> WindowCtxKey {
        let revision = match self.ui_state.windows.get(&window) {
            Some(WindowKind::UndockedTab { path, .. }) => self
                .document_state
                .engines
                .get(path)
                .map(signex_engine::Engine::revision),
            _ => None,
        };
        (
            revision,
            self.document_state.sheet_edit_epoch,
            self.document_state.documents_epoch,
            self.library.data_version,
            self.document_state.tabs.len(),
            self.document_state.projects.len(),
            self.ui_state.erc_running,
        )
    }

    /// Bring the contexts of the undocked document windows up to date with
    /// the main window's refresh: the app-wide fields are copied across, then
    /// the same document-scoped builder that fills the main context runs
    /// against each window's sheet. A window whose inputs are unchanged since
    /// its last build is left alone. `skip` is the window that has just
    /// refreshed its own.
    pub(crate) fn refresh_window_panel_ctxs(&mut self, skip: Option<iced::window::Id>) {
        if self.interaction_state.acting_window.is_some() || !self.dock_has_panels() {
            return;
        }
        let windows: Vec<iced::window::Id> = self
            .ui_state
            .windows
            .keys()
            .copied()
            .filter(|w| Some(*w) != skip && self.is_document_window(*w))
            .collect();
        self.sync_window_panel_ctx_shared();
        for window in windows {
            let key = self.window_ctx_key(window);
            let previous = self
                .document_state
                .window_panel_ctx_keys
                .get(&window)
                .copied();
            if previous == Some(key) && self.document_state.window_panel_ctx.contains_key(&window) {
                continue;
            }
            let revision_changed = previous.is_some_and(|p| p.0 != key.0);
            // The run's own edits move the revision; the key is read again
            // after it so they do not look like a change next time.
            drop(self.run_in_window(window, true, |app| {
                app.refresh_main_panel_ctx();
                if revision_changed {
                    app.update_selection_info();
                }
                Task::none()
            }));
            let key = self.window_ctx_key(window);
            self.document_state
                .window_panel_ctx_keys
                .insert(window, key);
        }
    }

    /// Drop the context of `window`, which has closed or been reattached,
    /// keeping its unsaved drafts in the main window's context.
    pub(crate) fn retire_window_panel_ctx(&mut self, window: iced::window::Id) {
        self.document_state.window_panel_ctx_keys.remove(&window);
        if let Some(ctx) = self.document_state.window_panel_ctx.remove(&window) {
            self.document_state.panel_ctx.take_drafts_from(ctx);
        }
    }

    /// Keep the app-wide fields of every window context current; cheap
    /// enough for every update.
    pub(in crate::app) fn sync_window_panel_ctx_shared(&mut self) {
        // Inside a window's swap the main slot holds that window's context.
        if self.interaction_state.acting_window.is_some() {
            return;
        }
        let document = &mut self.document_state;
        for ctx in document.window_panel_ctx.values_mut() {
            ctx.adopt_shared_from(&document.panel_ctx);
        }
    }
}
