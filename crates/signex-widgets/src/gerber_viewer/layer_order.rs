use super::*;

impl GerberViewerState {
    pub fn move_active_layer_up(&mut self) {
        let Some(active_layer) = self.active_layer else {
            return;
        };
        let target_layer = active_layer + 1;
        if target_layer >= self.layers.len() {
            return;
        }

        self.move_active_layer(active_layer, target_layer);
    }

    pub fn move_active_layer_down(&mut self) {
        let Some(active_layer) = self.active_layer else {
            return;
        };
        let Some(target_layer) = active_layer.checked_sub(1) else {
            return;
        };

        self.move_active_layer(active_layer, target_layer);
    }

    fn move_active_layer(&mut self, active_layer: usize, target_layer: usize) {
        self.layers.swap(active_layer, target_layer);
        self.active_layer = Some(target_layer);
        self.remap_selected_layer_after_swap(active_layer, target_layer);
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = format!(
            "Moved {} to layer position {}.",
            self.layers[target_layer].layer.name,
            target_layer + 1,
        );
    }

    fn remap_selected_layer_after_swap(&mut self, first_layer: usize, second_layer: usize) {
        let remap = |selection: &mut GerberItemSelection| {
            if selection.layer_index == first_layer {
                selection.layer_index = second_layer;
            } else if selection.layer_index == second_layer {
                selection.layer_index = first_layer;
            }
        };
        if let Some(selection) = self.selected_item.as_mut() {
            remap(selection);
        }
        for selection in &mut self.region_selection {
            remap(selection);
        }
    }
}
