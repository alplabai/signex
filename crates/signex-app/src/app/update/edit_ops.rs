use super::super::*;

impl Signex {
    pub(crate) fn handle_delete_selected(&mut self) {
        if let Some(engine) = self.engine.as_ref()
            && engine.has_selected_items(&self.canvas.selected)
            && self.apply_engine_command(
                signex_engine::Command::DeleteSelection {
                    items: self.canvas.selected.clone(),
                },
                true,
                true,
            )
        {
            self.canvas.selected.clear();
        }
    }

    pub(crate) fn handle_undo(&mut self) {
        let undone = self.apply_engine_undo(true);

        if undone {
            self.canvas.selected.clear();
        }
    }

    pub(crate) fn handle_redo(&mut self) {
        let redone = self.apply_engine_redo(true);

        if redone {
            self.canvas.selected.clear();
        }
    }

    pub(crate) fn handle_rotate_selected(&mut self) {
        if let Some(engine) = self.engine.as_ref()
            && engine.selection_is_single_symbol(&self.canvas.selected)
        {
            self.apply_engine_command(
                signex_engine::Command::RotateSelection {
                    items: self.canvas.selected.clone(),
                    angle_degrees: 90.0,
                },
                true,
                true,
            );
        }
    }

    pub(crate) fn handle_mirror_selected_x(&mut self) {
        if let Some(engine) = self.engine.as_ref()
            && engine.selection_is_single_symbol(&self.canvas.selected)
        {
            self.apply_engine_command(
                signex_engine::Command::MirrorSelection {
                    items: self.canvas.selected.clone(),
                    axis: signex_engine::MirrorAxis::Vertical,
                },
                true,
                true,
            );
        }
    }

    pub(crate) fn handle_mirror_selected_y(&mut self) {
        if let Some(engine) = self.engine.as_ref()
            && engine.selection_is_single_symbol(&self.canvas.selected)
        {
            self.apply_engine_command(
                signex_engine::Command::MirrorSelection {
                    items: self.canvas.selected.clone(),
                    axis: signex_engine::MirrorAxis::Horizontal,
                },
                true,
                true,
            );
        }
    }
}
