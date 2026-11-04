//! The dock panels of an undocked document window show and edit that
//! window's sheet: each such window keeps a [`PanelContext`] of its own
//! (#1966). `run_in_window` swaps it in beside the window's canvas, so the
//! same builders and handlers that serve the main window fill it.

use super::super::*;
use crate::panels::PanelContext;

impl PanelContext {
    /// Copy what is the app's rather than a document's: theme, units, grid,
    /// fonts, filters, diagnostics and the ERC results.
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
        self.ui_font_name.clone_from(&main.ui_font_name);
        self.component_classes.clone_from(&main.component_classes);
        self.canvas_font_name.clone_from(&main.canvas_font_name);
        self.canvas_font_size = main.canvas_font_size;
        self.canvas_font_bold = main.canvas_font_bold;
        self.canvas_font_italic = main.canvas_font_italic;
        self.diagnostics_level.clone_from(&main.diagnostics_level);
        if self.diagnostics.last().map(|e| e.id) != main.diagnostics.last().map(|e| e.id) {
            self.diagnostics.clone_from(&main.diagnostics);
        }
        self.selection_filters.clone_from(&main.selection_filters);
        self.custom_filter_presets
            .clone_from(&main.custom_filter_presets);
        self.active_custom_filter_tab = main.active_custom_filter_tab;
        self.footprint_filter_presets
            .clone_from(&main.footprint_filter_presets);
        self.erc_diagnostics.clone_from(&main.erc_diagnostics);
        self.erc_waivers.clone_from(&main.erc_waivers);
        self.erc_violation_waivers
            .clone_from(&main.erc_violation_waivers);
        self.project_tree_selected
            .clone_from(&main.project_tree_selected);
        self.erc_running = main.erc_running;
        self.erc_results_stale = main.erc_results_stale;
    }
}

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

    /// Bring the contexts of the undocked document windows up to date with
    /// the main window's refresh: the app-wide fields are copied across, then
    /// the same builder that fills the main context runs against each
    /// window's sheet. `skip` is the window that has just refreshed its own.
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
            // The closure returns no task, so there is nothing to run after.
            drop(self.run_in_window(window, true, |app| {
                app.refresh_main_panel_ctx();
                Task::none()
            }));
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
