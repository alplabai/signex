use super::*;

impl GerberViewerState {
    pub fn has_active_highlight(&self) -> bool {
        self.highlighted_component.is_some()
            || self.highlighted_net.is_some()
            || self.highlighted_attribute.is_some()
            || self.highlighted_d_code.is_some()
    }

    pub fn clear_highlight(&mut self) {
        if !self.has_active_highlight() {
            return;
        }

        self.highlighted_component = None;
        self.highlighted_net = None;
        self.highlighted_attribute = None;
        self.highlighted_d_code = None;
        self.status = "Gerber highlight cleared.".to_owned();
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
    }
}
