//! The visible board (or no document), drawn into the PCB canvases and the
//! panel context.

use crate::app::Signex;

impl Signex {
    /// The active tab's path and board revision: what the ratsnest cache
    /// is keyed by.
    fn active_pcb_ratsnest_key(&self) -> Option<crate::app::state::RatsnestKey> {
        let path = &self
            .document_state
            .tabs
            .get(self.document_state.active_tab)?
            .path;
        let revision = *self.document_state.pcb_revisions.get(path)?;
        Some((path.clone(), revision))
    }

    pub(crate) fn sync_pcb_canvas_from_visible_board(&mut self) {
        // The ratsnest is recomputed only when the board's revision moved.
        let ratsnest_key = self.active_pcb_ratsnest_key();
        let board_path = self.active_tab_path().unwrap_or_default();
        let mut ratsnest = std::mem::take(&mut self.interaction_state.pcb_ratsnest);
        let ratsnest_visible = self.pcb_layer_view(&board_path).ratsnest_visible();
        let renderer_snapshot = self.active_pcb().map(|board| {
            ratsnest.refresh(ratsnest_key, board);
            let snapshot = self
                .pcb_snapshot_for(&board_path, board)
                .with_zone_fills(ratsnest.zone_fills());
            if ratsnest_visible {
                ratsnest.apply(&board_path, snapshot)
            } else {
                snapshot
            }
        });
        self.interaction_state.pcb_ratsnest = ratsnest;
        let board_grid = self
            .active_pcb()
            .map(signex_engine::pcb::placement::grid_step_mm);
        let view = self.pcb_layer_view(&board_path);
        let (origin_marker, flipped) = (view.origin_marker_visible(), view.flipped());
        let marks = self
            .active_pcb()
            .map(crate::pcb_canvas::BoardMarks::of)
            .unwrap_or_default();

        // An undocked window showing the same tab draws the same board
        // (#494). Its camera stays its own.
        let active_path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.path.as_path());
        for (id, canvas) in &mut self.interaction_state.pcb_canvases {
            let shows_active = matches!(
                self.ui_state.windows.get(id),
                Some(crate::app::state::WindowKind::UndockedTab { path, .. })
                    if Some(path.as_path()) == active_path
            );
            if shows_active {
                canvas.set_renderer_snapshot(renderer_snapshot.clone());
                canvas.set_board_grid(board_grid);
                canvas.set_board_marks(marks.clone());
                canvas.set_origin_marker_visible(origin_marker);
                canvas.set_view_flipped(flipped);
                canvas.clear_content_cache();
            }
        }
        let main = &mut self.interaction_state.pcb_canvas;
        main.set_renderer_snapshot(renderer_snapshot);
        main.set_board_grid(board_grid);
        main.set_board_marks(marks);
        main.set_origin_marker_visible(origin_marker);
        main.set_view_flipped(flipped);
        self.note_pcb_board_changed();
    }

    pub(crate) fn apply_loaded_pcb_document(
        &mut self,
        fit_to_board: bool,
        refresh_panel_ctx: bool,
    ) {
        self.clear_schematic_ui_state();
        self.sync_pcb_canvas_from_visible_board();
        if fit_to_board {
            // A board opened into the tab starts upright (#2316).
            if self.interaction_state.pcb_canvas.set_view_rotation(0.0) {
                self.close_view_angle_entry_in(None);
            }
            self.interaction_state.pcb_canvas.fit_to_board();
        }
        self.interaction_state.pcb_canvas.clear_bg_cache();
        self.interaction_state.pcb_canvas.clear_content_cache();

        if refresh_panel_ctx {
            self.refresh_panel_ctx();
        } else {
            self.sync_panel_ctx_from_visible_document();
        }
    }

    pub(crate) fn apply_loaded_empty_document(&mut self, refresh_panel_ctx: bool) {
        self.clear_schematic_ui_state();
        self.interaction_state
            .pcb_canvas
            .set_renderer_snapshot(None);
        self.note_pcb_board_changed();
        self.interaction_state.pcb_canvas.clear_bg_cache();
        self.interaction_state.pcb_canvas.clear_content_cache();

        if refresh_panel_ctx {
            self.refresh_panel_ctx();
        } else {
            self.sync_panel_ctx_from_visible_document();
        }
    }
}
