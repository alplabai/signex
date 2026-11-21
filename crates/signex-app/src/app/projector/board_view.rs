//! The visible board, drawn into the PCB canvases.

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
}
