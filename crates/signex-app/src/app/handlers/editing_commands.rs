use super::super::*;

impl Signex {
    pub(crate) fn handle_selection_delete_requested(&mut self) {
        if let Some(engine) = self.document_state.engine.as_ref()
            && engine.has_selected_items(&self.interaction_state.canvas.selected)
            && self.apply_engine_command(
                signex_engine::Command::DeleteSelection {
                    items: self.interaction_state.canvas.selected.clone(),
                },
                true,
                true,
            )
        {
            self.interaction_state.canvas.selected.clear();
        }
    }

    pub(crate) fn handle_undo_requested(&mut self) {
        let undone = self.apply_engine_undo(true);

        if undone {
            self.interaction_state.canvas.selected.clear();
        }
    }

    pub(crate) fn handle_redo_requested(&mut self) {
        let redone = self.apply_engine_redo(true);

        if redone {
            self.interaction_state.canvas.selected.clear();
        }
    }

    pub(crate) fn handle_selection_rotate_requested(&mut self) {
        if let Some(engine) = self.document_state.engine.as_ref()
            && engine.selection_is_single_symbol(&self.interaction_state.canvas.selected)
        {
            self.apply_engine_command(
                signex_engine::Command::RotateSelection {
                    items: self.interaction_state.canvas.selected.clone(),
                    angle_degrees: 90.0,
                },
                true,
                true,
            );
        }
    }

    pub(crate) fn handle_selection_mirror_x_requested(&mut self) {
        if let Some(engine) = self.document_state.engine.as_ref()
            && engine.selection_is_single_symbol(&self.interaction_state.canvas.selected)
        {
            self.apply_engine_command(
                signex_engine::Command::MirrorSelection {
                    items: self.interaction_state.canvas.selected.clone(),
                    axis: signex_engine::MirrorAxis::Vertical,
                },
                true,
                true,
            );
        }
    }

    pub(crate) fn handle_selection_mirror_y_requested(&mut self) {
        if let Some(engine) = self.document_state.engine.as_ref()
            && engine.selection_is_single_symbol(&self.interaction_state.canvas.selected)
        {
            self.apply_engine_command(
                signex_engine::Command::MirrorSelection {
                    items: self.interaction_state.canvas.selected.clone(),
                    axis: signex_engine::MirrorAxis::Horizontal,
                },
                true,
                true,
            );
        }
    }
}