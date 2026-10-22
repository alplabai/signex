use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GerberColorTarget {
    Layer(usize),
    Grid,
    DCode,
    NegativeObject,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct GerberLayerColorChoice {
    pub(super) palette_index: usize,
    pub(super) color: Color,
    pub(super) family_index: usize,
    pub(super) label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct GerberMaterialColor {
    pub(super) color: Color,
    pub(super) family_index: usize,
    pub(super) label: String,
}

impl GerberViewerState {
    pub(super) fn layer_color_choices(&self) -> Vec<GerberLayerColorChoice> {
        self.palette
            .iter()
            .enumerate()
            .map(|(palette_index, material_color)| GerberLayerColorChoice {
                palette_index,
                color: material_color.color,
                family_index: material_color.family_index,
                label: material_color.label.clone(),
            })
            .collect()
    }

    pub(super) fn selected_layer_color_choice(
        &self,
        layer_index: usize,
    ) -> Option<GerberLayerColorChoice> {
        let color = self.layers.get(layer_index)?.color;
        let palette_index = self
            .palette
            .iter()
            .position(|candidate| candidate.color == color)?;
        self.layer_color_choices()
            .into_iter()
            .find(|choice| choice.palette_index == palette_index)
    }

    pub(super) fn selected_color_choice(&self, color: Color) -> Option<GerberLayerColorChoice> {
        let palette_index = self
            .palette
            .iter()
            .position(|candidate| candidate.color == color)?;
        self.layer_color_choices()
            .into_iter()
            .find(|choice| choice.palette_index == palette_index)
    }

    pub fn toggle_color_picker(&mut self, target: GerberColorTarget) {
        self.open_color_picker = if self.open_color_picker == Some(target) {
            None
        } else {
            Some(target)
        };
    }

    pub fn close_color_picker(&mut self) {
        self.open_color_picker = None;
    }

    pub(super) fn color_picker_open(&self, target: GerberColorTarget) -> bool {
        self.open_color_picker == Some(target)
    }

    pub fn set_layer_color(&mut self, layer_index: usize, palette_index: usize) {
        self.close_color_picker();
        let Some(color) = self
            .palette
            .get(palette_index)
            .map(|material_color| material_color.color)
        else {
            return;
        };
        let Some(layer) = self.layers.get_mut(layer_index) else {
            return;
        };
        if layer.color == color {
            return;
        }

        layer.color = color;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = format!("Layer color: {} {}", layer.layer.name, color_hex(color),);
    }
}

fn color_hex(color: Color) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (color.r * 255.0).round() as u8,
        (color.g * 255.0).round() as u8,
        (color.b * 255.0).round() as u8,
    )
}
