use super::super::*;

impl SchematicCanvas {
    /// Ghost power-port / symbol preview following the cursor.
    pub(in crate::canvas) fn draw_ghost_symbol(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) {
        // Ghost power-port symbol preview at cursor position.
        // While placement is paused (TAB → properties form open),
        // hide the ghosts so the user isn't distracted by a preview
        // that can't be committed until they confirm.
        if let Some(ref ghost_sym) = self.ghost_symbol
            && !self.placement_paused
        {
            let cursor_world = state.camera.screen_to_world(cursor_pos, bounds);
            let (sx, sy) = if self.snap_enabled && self.snap_grid_mm > 0.0 {
                let g = self.snap_grid_mm;
                (
                    (cursor_world.x as f64 / g).round() * g,
                    (cursor_world.y as f64 / g).round() * g,
                )
            } else {
                (cursor_world.x as f64, cursor_world.y as f64)
            };
            let mut preview = ghost_sym.clone();
            preview.position = signex_types::schematic::Point::new(sx, sy);
            let ghost_transform = crate::schematic_runtime::ScreenTransform {
                offset_x: state.camera.offset.x,
                offset_y: state.camera.offset.y,
                scale: state.camera.scale,
            };
            let ghost_color = Color::from_rgba(0.3, 0.8, 1.0, 0.7);
            crate::schematic_runtime::draw_power_port_preview(
                &mut *frame,
                &preview,
                &ghost_transform,
                ghost_color,
            );
        }
    }

    /// Ghost text-note preview following the cursor.
    pub(in crate::canvas) fn draw_ghost_text(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) {
        // Ghost text-note preview at cursor position.
        if let Some(ref ghost_tn) = self.ghost_text
            && !self.placement_paused
        {
            let cursor_world = state.camera.screen_to_world(cursor_pos, bounds);
            let (sx, sy) = if self.snap_enabled && self.snap_grid_mm > 0.0 {
                let g = self.snap_grid_mm;
                (
                    (cursor_world.x as f64 / g).round() * g,
                    (cursor_world.y as f64 / g).round() * g,
                )
            } else {
                (cursor_world.x as f64, cursor_world.y as f64)
            };
            let mut preview = ghost_tn.clone();
            preview.position = signex_types::schematic::Point::new(sx, sy);
            let ghost_transform = crate::schematic_runtime::ScreenTransform {
                offset_x: state.camera.offset.x,
                offset_y: state.camera.offset.y,
                scale: state.camera.scale,
            };
            let ghost_color = Color::from_rgba(0.3, 0.8, 1.0, 0.7);
            crate::schematic_runtime::text::draw_text_note_preview(
                &mut *frame,
                &preview,
                &ghost_transform,
                ghost_color,
            );
        }
    }

    /// Ghost label / port preview following the cursor.
    pub(in crate::canvas) fn draw_ghost_label(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) {
        // Ghost label/port preview at cursor position
        if let Some(ref ghost) = self.ghost_label
            && !self.placement_paused
        {
            let cursor_world = state.camera.screen_to_world(cursor_pos, bounds);
            let snap_world = if self.snap_enabled && self.snap_grid_mm > 0.0 {
                let g = self.snap_grid_mm;
                (
                    (cursor_world.x as f64 / g).round() * g,
                    (cursor_world.y as f64 / g).round() * g,
                )
            } else {
                (cursor_world.x as f64, cursor_world.y as f64)
            };
            let mut preview_label = ghost.clone();
            preview_label.position =
                signex_types::schematic::Point::new(snap_world.0, snap_world.1);
            let ghost_transform = crate::schematic_runtime::ScreenTransform {
                offset_x: state.camera.offset.x,
                offset_y: state.camera.offset.y,
                scale: state.camera.scale,
            };
            let ghost_color = Color::from_rgba(0.3, 0.8, 1.0, 0.7);
            let ghost_fill = Color::from_rgba(0.3, 0.8, 1.0, 0.15);
            crate::schematic_runtime::label::draw_label_preview(
                &mut *frame,
                &preview_label,
                &ghost_transform,
                ghost_color,
                ghost_fill,
            );
        }
    }
}
