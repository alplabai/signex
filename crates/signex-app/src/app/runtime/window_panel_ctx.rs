//! The dock panels of an undocked document window show and edit that
//! window's sheet: each such window keeps a [`PanelContext`] of its own
//! (#1966). `run_in_window` swaps it in beside the window's canvas, so the
//! same builders and handlers that serve the main window fill it.

use super::super::*;
use crate::app::state::WindowKind;
use crate::panels::PanelContext;

impl PanelContext {
    /// Copy what is the app's rather than a document's: theme, units, grid,
    /// fonts, filters, diagnostics and the ERC results. The scalars are copied
    /// every time; a list is compared by value (or by its newest id) and
    /// cloned only when it differs, so a steady state costs no allocation.
    /// The Navigator is not copied here: see `sync_window_panel_ctx_shared`.
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
        if self.component_classes != main.component_classes {
            self.component_classes.clone_from(&main.component_classes);
        }
        if self.selection_filters != main.selection_filters {
            self.selection_filters.clone_from(&main.selection_filters);
        }
        if self.custom_filter_presets != main.custom_filter_presets {
            self.custom_filter_presets
                .clone_from(&main.custom_filter_presets);
        }
        if self.footprint_filter_presets != main.footprint_filter_presets {
            self.footprint_filter_presets
                .clone_from(&main.footprint_filter_presets);
        }
        if self.erc_diagnostics != main.erc_diagnostics {
            self.erc_diagnostics.clone_from(&main.erc_diagnostics);
        }
        self.adopt_net_classes_from(main);
        if self.erc_waivers != main.erc_waivers {
            self.erc_waivers.clone_from(&main.erc_waivers);
        }
        if self.erc_violation_waivers != main.erc_violation_waivers {
            self.erc_violation_waivers
                .clone_from(&main.erc_violation_waivers);
        }
    }

    /// Net classes are project-wide, and `PanelMsg::NetClass` runs on the
    /// main window against the main window's panel state, so a window shows
    /// that state (selection, draft, table) and never keeps its own. Only
    /// the net label's class picker follows the window's own selection.
    fn adopt_net_classes_from(&mut self, main: &PanelContext) {
        let (own, shared) = (&mut self.net_classes, &main.net_classes);
        own.available = shared.available;
        own.project = shared.project;
        own.selected = shared.selected;
        own.can_undo = shared.can_undo;
        own.can_redo = shared.can_redo;
        if own.classes != shared.classes {
            own.classes.clone_from(&shared.classes);
        }
        if own.draft != shared.draft {
            own.draft.clone_from(&shared.draft);
        }
        if own.notice != shared.notice {
            own.notice.clone_from(&shared.notice);
        }
    }
}

/// What a window's context was built from; unchanged means it is current.
///
/// The window's sheet revision; a stamp of what its panels read from the
/// app (the open tabs, the projects' names, sheets, parameters and net
/// classes, which documents are dirty); the library data version; and
/// whether ERC is running. Not the global edit epochs: an edit to another
/// sheet changes none of these.
pub(crate) type WindowCtxKey = (Option<u64>, u64, u64, bool);

impl Signex {
    /// The context the panels of `window` render: that window's own when it
    /// shows a document of its own, the main window's otherwise. A document
    /// window without a context yet would draw main's History rows; a click on
    /// one is refused (`history_lists_target`), never restored.
    pub(crate) fn panel_ctx_for(&self, window: iced::window::Id) -> &PanelContext {
        self.document_state
            .window_panel_ctx
            .get(&window)
            .filter(|_| self.is_document_window(window))
            .unwrap_or(&self.document_state.panel_ctx)
    }

    /// A context for a window that has none yet: the main window's, as the
    /// starting point for `init_window_panel_ctx`.
    pub(crate) fn new_window_panel_ctx(&self, window: iced::window::Id) -> PanelContext {
        let mut ctx = self.document_state.panel_ctx.clone();
        // Main's History rows belong to main's file; the window lists its own
        // once its first load lands (see `refresh_window_histories`).
        ctx.history = Default::default();
        if !self.window_shares_main_navigator(window) {
            ctx.navigator = Default::default();
        }
        ctx
    }

    /// Fill a new window context from the window's own sheet and selection.
    /// Runs with the window swapped in (see `run_in_window`).
    pub(crate) fn init_window_panel_ctx(&mut self) {
        self.sync_panel_ctx_from_visible_document();
        self.update_selection_info();
    }

    /// Whether the dock shows any panel, so a window's context is worth
    /// building.
    pub(super) fn dock_has_panels(&self) -> bool {
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
            self.window_panel_inputs_stamp(),
            self.library.data_version,
            self.ui_state.erc_running,
        )
    }

    /// A hash of what a window's panels read from the app besides its sheet.
    pub(crate) fn window_panel_inputs_stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let document = &self.document_state;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for tab in &document.tabs {
            tab.path.hash(&mut hasher);
        }
        for project in &document.projects {
            let data = &project.data;
            (project.id, &data.name, &data.dir).hash(&mut hasher);
            for sheet in &data.sheets {
                (&sheet.name, &sheet.filename).hash(&mut hasher);
            }
            for class in &data.net_classes {
                (&class.name, &class.nets, &class.patterns).hash(&mut hasher);
                (class.style.color, class.style.line).hash(&mut hasher);
                class
                    .style
                    .wire_width_mm
                    .map(f64::to_bits)
                    .hash(&mut hasher);
                for rule in [
                    class.clearance,
                    class.trace_width,
                    class.via_diameter,
                    class.via_drill,
                    class.diff_pair_gap,
                    class.diff_pair_width,
                ] {
                    rule.to_bits().hash(&mut hasher);
                }
            }
            data.parameters.hash(&mut hasher);
        }
        // Order-independent: the set of dirty documents.
        let dirty = document
            .dirty_document_paths()
            .iter()
            .map(|path| {
                let mut one = std::collections::hash_map::DefaultHasher::new();
                path.hash(&mut one);
                one.finish()
            })
            .fold(0u64, u64::wrapping_add);
        dirty.hash(&mut hasher);
        hasher.finish()
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

    /// Drop the context of `window`, which has closed or been reattached.
    /// Drafts live in the app's one store, so none go with it.
    pub(crate) fn retire_window_panel_ctx(&mut self, window: iced::window::Id) {
        self.document_state.window_panel_ctx_keys.remove(&window);
        self.document_state.window_history.remove(&window);
        self.document_state.window_panel_ctx.remove(&window);
    }

    /// The project the main window's Navigator is built for.
    fn main_navigator_project(&self) -> Option<crate::app::state::ProjectId> {
        self.document_state
            .active_document_project()
            .or_else(|| self.document_state.active_loaded_project())
            .map(|p| p.id)
    }

    /// Whether the sheet `window` shows belongs to the project the main
    /// window's Navigator describes. A window of another project must not
    /// read that hierarchy: it would take a sheet placed twice for placed
    /// once and rewrite a shared designator.
    fn window_shares_main_navigator(&self, window: iced::window::Id) -> bool {
        let Some(WindowKind::UndockedTab { path, .. }) = self.ui_state.windows.get(&window) else {
            return false;
        };
        let window_project = self.document_state.document_project(path).map(|p| p.id);
        window_project.is_some() && window_project == self.main_navigator_project()
    }

    /// Keep the app-wide fields of every window context current; cheap
    /// enough for every update. A window of the main window's project
    /// follows main's Navigator; a window of another project has none, so
    /// nothing there is "known placed once" and its designator edits take
    /// the placement-entry path.
    pub(in crate::app) fn sync_window_panel_ctx_shared(&mut self) {
        // Inside a window's swap the main slot holds that window's context.
        if self.interaction_state.acting_window.is_some() {
            return;
        }
        let shares: Vec<(iced::window::Id, bool)> = self
            .document_state
            .window_panel_ctx
            .keys()
            .map(|w| (*w, self.window_shares_main_navigator(*w)))
            .collect();
        let document = &mut self.document_state;
        for (window, shares_navigator) in shares {
            let Some(ctx) = document.window_panel_ctx.get_mut(&window) else {
                continue;
            };
            ctx.adopt_shared_from(&document.panel_ctx);
            if !shares_navigator {
                if ctx.navigator.hierarchy.is_some() {
                    ctx.navigator = Default::default();
                }
            } else if ctx.navigator.fingerprint != document.panel_ctx.navigator.fingerprint {
                ctx.navigator.adopt_from(&document.panel_ctx.navigator);
            }
        }
    }
}
