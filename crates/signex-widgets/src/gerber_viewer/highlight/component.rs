use std::collections::BTreeSet;

use super::*;

impl GerberViewerState {
    pub fn component_choices(&self) -> Vec<String> {
        let Some(viewer_layer) = self.active_layer.and_then(|index| self.layers.get(index)) else {
            return Vec::new();
        };

        viewer_layer
            .layer
            .geometry
            .primitive_attributes
            .iter()
            .filter_map(|attributes| attributes.component.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn highlighted_component(&self) -> Option<&str> {
        self.highlighted_component.as_deref()
    }

    pub fn set_highlighted_component(&mut self, component: String) {
        if self
            .component_choices()
            .iter()
            .any(|candidate| candidate == &component)
        {
            self.status = format!("Highlighted component: {component}");
            self.highlighted_component = Some(component);
            self.highlighted_net = None;
            self.highlighted_attribute = None;
            self.highlighted_d_code = None;
            self.redraw_generation = self.redraw_generation.wrapping_add(1);
        }
    }

    pub fn clear_component_highlight(&mut self) {
        if self.highlighted_component.take().is_some() {
            self.status = "Component highlight cleared.".into();
            self.redraw_generation = self.redraw_generation.wrapping_add(1);
        }
    }

    pub(in crate::gerber_viewer) fn retain_available_component_highlight(&mut self) {
        let Some(component) = self.highlighted_component.as_deref() else {
            return;
        };
        if !self
            .component_choices()
            .iter()
            .any(|candidate| candidate == component)
        {
            self.highlighted_component = None;
        }
    }
}

pub(in crate::gerber_viewer) fn component_highlight_color(
    layer_color: Color,
    attributes: Option<&signex_gerber::GerberObjectAttributes>,
    highlighted_component: Option<&str>,
) -> Color {
    let Some(highlighted_component) = highlighted_component else {
        return layer_color;
    };
    if attributes.and_then(|attributes| attributes.component.as_deref())
        == Some(highlighted_component)
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
#[path = "../../../tests/gerber_viewer/highlight.rs"]
mod gerber_highlight_test_definitions;

#[cfg(test)]
gerber_highlight_test_definitions::gerber_highlight_tests!();
