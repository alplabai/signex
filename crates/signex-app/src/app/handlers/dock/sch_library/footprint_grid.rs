//! Footprint-editor grid / guide / snap-view handlers — the methods
//! behind the `FpEditor*` dock-panel messages that manage the Snap
//! Options sub-tab + snapping mode, the multi-grid Manager (add /
//! properties / delete / activate), and the guide Manager (add /
//! delete / toggle / reposition) on the active `.snxfpt` editor. The
//! dispatcher in `mod.rs` routes these panel messages here.
//!
//! Pure code motion out of the former `sch_library.rs` god-file
//! (ADR-0001 #163); zero behaviour change.

use iced::Task;

use super::*;

impl Signex {
    pub(super) fn handle_fp_editor_set_snap_subtab(
        &mut self,
        tab: &crate::library::editor::footprint::state::SnapSubTab,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.snap_subtab = *tab;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn handle_fp_editor_set_snapping_mode(
        &mut self,
        mode: &crate::library::editor::footprint::state::SnappingMode,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.snapping_mode = *mode;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn handle_fp_editor_grid_manager_add(&mut self) -> bool {
        // v0.18.21 — append a fresh `GridDef` clone of the
        // active row (so the new grid inherits the user's last
        // step + display picks). The new row activates so the
        // user can immediately retune via Ctrl+G.
        //
        // v0.18.25.1 — fall back to the live `snap_options`
        // (not `GridDef::default()`) when `active_grid_idx`
        // is out of range, so a misindex doesn't drop the
        // user's current step/display pickers on the floor.
        if let Some(editor) = self.active_footprint_editor_mut() {
            let seed = editor
                .state
                .grids
                .get(editor.state.active_grid_idx)
                .cloned()
                .unwrap_or_else(|| {
                    crate::library::editor::footprint::state::GridDef::from_snap_options(
                        &editor.state.snap_options,
                    )
                });
            let mut next = seed;
            next.name = format!("Grid {}", editor.state.grids.len() + 1);
            editor.state.grids.push(next);
            let new_idx = editor.state.grids.len() - 1;
            editor.state.active_grid_idx = new_idx;
            // Mirror onto SnapOptions so the canvas picks up
            // the new active row immediately.
            let row = &editor.state.grids[new_idx];
            editor.state.snap_options.grid_step_mm = row.step_mm;
            editor.state.snap_options.fine_grid_display = row.fine_display;
            editor.state.snap_options.coarse_grid_display = row.coarse_display;
            editor.state.snap_options.coarse_multiplier = row.coarse_multiplier;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn handle_fp_editor_grid_manager_properties(&mut self) -> Task<Message> {
        // Reuses the Ctrl+G modal so the user can edit the
        // active grid via the same dialog. The modal open
        // handler reads `snap_options.grid_step_mm` and seeds
        // the buffers; the commit path mirrors back to
        // `grids[active_grid_idx]` (see GridPropertiesCommit).
        self.update(Message::GridProperties(GridPropertiesMsg::Open))
    }

    pub(super) fn handle_fp_editor_grid_manager_delete(&mut self) -> bool {
        // v0.18.21 — remove the active row. Always keep at
        // least one grid (UI gates the button when only one
        // remains, so this branch should normally only fire
        // when len > 1).
        if let Some(editor) = self.active_footprint_editor_mut() {
            if editor.state.grids.len() > 1 {
                let idx = editor.state.active_grid_idx;
                editor.state.grids.remove(idx);
                if editor.state.active_grid_idx >= editor.state.grids.len() {
                    editor.state.active_grid_idx = editor.state.grids.len() - 1;
                }
                // Mirror new active onto SnapOptions.
                let row = &editor.state.grids[editor.state.active_grid_idx];
                editor.state.snap_options.grid_step_mm = row.step_mm;
                editor.state.snap_options.fine_grid_display = row.fine_display;
                editor.state.snap_options.coarse_grid_display = row.coarse_display;
                editor.state.snap_options.coarse_multiplier = row.coarse_multiplier;
                editor.canvas_cache.clear();
            }
        }
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn handle_fp_editor_grid_set_active(&mut self, idx: &usize) -> bool {
        let idx = *idx;
        if let Some(editor) = self.active_footprint_editor_mut() {
            if idx < editor.state.grids.len() {
                editor.state.active_grid_idx = idx;
                let row = &editor.state.grids[idx];
                editor.state.snap_options.grid_step_mm = row.step_mm;
                editor.state.snap_options.fine_grid_display = row.fine_display;
                editor.state.snap_options.coarse_grid_display = row.coarse_display;
                editor.state.snap_options.coarse_multiplier = row.coarse_multiplier;
                editor.canvas_cache.clear();
            }
        }
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn handle_fp_editor_guide_manager_add(&mut self) -> bool {
        // v0.18.20 — bare "Add" button defaults to a vertical
        // guide at world X = 0; users can flip via the row's
        // axis label and edit the position field afterwards.
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor
                .state
                .guides
                .push(crate::library::editor::footprint::state::Guide {
                    axis: crate::library::editor::footprint::state::GuideAxis::Vertical,
                    position_mm: 0.0,
                    enabled: true,
                });
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn handle_fp_editor_guide_add_vertical(&mut self) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor
                .state
                .guides
                .push(crate::library::editor::footprint::state::Guide {
                    axis: crate::library::editor::footprint::state::GuideAxis::Vertical,
                    position_mm: 0.0,
                    enabled: true,
                });
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn handle_fp_editor_guide_add_horizontal(&mut self) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor
                .state
                .guides
                .push(crate::library::editor::footprint::state::Guide {
                    axis: crate::library::editor::footprint::state::GuideAxis::Horizontal,
                    position_mm: 0.0,
                    enabled: true,
                });
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn handle_fp_editor_guide_delete(&mut self, idx: &usize) -> bool {
        let idx = *idx;
        if let Some(editor) = self.active_footprint_editor_mut() {
            if idx < editor.state.guides.len() {
                editor.state.guides.remove(idx);
                editor.canvas_cache.clear();
            }
        }
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn handle_fp_editor_guide_toggle(&mut self, idx: &usize) -> bool {
        let idx = *idx;
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(g) = editor.state.guides.get_mut(idx) {
                g.enabled = !g.enabled;
                editor.canvas_cache.clear();
            }
        }
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn handle_fp_editor_guide_set_position(&mut self, idx: &usize, raw: &str) -> bool {
        let idx = *idx;
        if let Ok(parsed) = raw.trim().parse::<f64>() {
            if let Some(editor) = self.active_footprint_editor_mut() {
                if let Some(g) = editor.state.guides.get_mut(idx) {
                    g.position_mm = parsed;
                    editor.canvas_cache.clear();
                }
            }
            self.refresh_panel_ctx();
        }
        // Invalid float (e.g. user typing "-") — silently drop
        // so the input keeps capturing keystrokes.
        true
    }
}
