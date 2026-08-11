use super::*;

impl GerberViewerState {
    pub fn set_grid_color(&mut self, palette_index: usize) {
        self.close_color_picker();
        let Some(color) = self
            .palette
            .get(palette_index)
            .map(|material_color| material_color.color)
        else {
            return;
        };
        if self.grid_color == color {
            return;
        }

        self.grid_color = color;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = "Updated Gerber grid color.".to_owned();
    }

    pub fn set_d_code_color(&mut self, palette_index: usize) {
        self.close_color_picker();
        let Some(color) = self
            .palette
            .get(palette_index)
            .map(|material_color| material_color.color)
        else {
            return;
        };
        if self.d_code_color == color {
            return;
        }

        self.d_code_color = color;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = "Updated Gerber D-code color.".to_owned();
    }

    pub fn set_negative_object_color(&mut self, palette_index: usize) {
        self.close_color_picker();
        let Some(color) = self
            .palette
            .get(palette_index)
            .map(|material_color| material_color.color)
        else {
            return;
        };
        if self.negative_ghost_color == color {
            return;
        }

        self.negative_ghost_color = color;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = "Updated Gerber negative-object color.".to_owned();
    }
}
