use std::collections::BTreeSet;

use super::*;

impl GerberViewerState {
    pub fn net_choices(&self) -> Vec<String> {
        let Some(viewer_layer) = self.active_layer.and_then(|index| self.layers.get(index)) else {
            return Vec::new();
        };

        viewer_layer
            .layer
            .geometry
            .primitive_attributes
            .iter()
            .flat_map(|attributes| attributes.nets.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn highlighted_net(&self) -> Option<&str> {
        self.highlighted_net.as_deref()
    }

    pub fn set_highlighted_net(&mut self, net: String) {
        if self.net_choices().iter().any(|candidate| candidate == &net) {
            self.status = format!("Highlighted net: {net}");
            self.highlighted_net = Some(net);
            self.highlighted_component = None;
            self.highlighted_attribute = None;
            self.highlighted_d_code = None;
            self.redraw_generation = self.redraw_generation.wrapping_add(1);
        }
    }

    pub fn clear_net_highlight(&mut self) {
        if self.highlighted_net.take().is_some() {
            self.status = "Net highlight cleared.".into();
            self.redraw_generation = self.redraw_generation.wrapping_add(1);
        }
    }

    pub(in crate::gerber_viewer) fn retain_available_net_highlight(&mut self) {
        let Some(net) = self.highlighted_net.as_deref() else {
            return;
        };
        if !self.net_choices().iter().any(|candidate| candidate == net) {
            self.highlighted_net = None;
        }
    }
}

pub(in crate::gerber_viewer) fn net_highlight_color(
    layer_color: Color,
    attributes: Option<&signex_gerber::GerberObjectAttributes>,
    highlighted_net: Option<&str>,
) -> Color {
    let Some(highlighted_net) = highlighted_net else {
        return layer_color;
    };
    if attributes.is_some_and(|attributes| attributes.nets.iter().any(|net| net == highlighted_net))
    {
        HIGHLIGHT_COLOR
    } else {
        Color {
            a: NON_MATCHING_ALPHA,
            ..layer_color
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/gerber_viewer/net_highlight.rs"]
mod gerber_net_highlight_test_definitions;

#[cfg(test)]
gerber_net_highlight_test_definitions::gerber_net_highlight_tests!();
