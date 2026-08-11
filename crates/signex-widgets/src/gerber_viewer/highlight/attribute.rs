use std::collections::BTreeSet;

use super::*;

impl GerberViewerState {
    pub fn attribute_choices(&self) -> Vec<GerberAttributeValue> {
        let Some(viewer_layer) = self.active_layer.and_then(|index| self.layers.get(index)) else {
            return Vec::new();
        };

        viewer_layer
            .layer
            .geometry
            .primitive_attributes
            .iter()
            .flat_map(|attributes| attributes.attributes.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn highlighted_attribute(&self) -> Option<&GerberAttributeValue> {
        self.highlighted_attribute.as_ref()
    }

    pub fn set_highlighted_attribute(&mut self, attribute: GerberAttributeValue) {
        if self
            .attribute_choices()
            .iter()
            .any(|candidate| candidate == &attribute)
        {
            self.status = format!("Highlighted attribute: {attribute}");
            self.highlighted_attribute = Some(attribute);
            self.highlighted_component = None;
            self.highlighted_net = None;
            self.highlighted_d_code = None;
            self.redraw_generation = self.redraw_generation.wrapping_add(1);
        }
    }

    pub fn clear_attribute_highlight(&mut self) {
        if self.highlighted_attribute.take().is_some() {
            self.status = "Attribute highlight cleared.".into();
            self.redraw_generation = self.redraw_generation.wrapping_add(1);
        }
    }

    pub(in crate::gerber_viewer) fn retain_available_attribute_highlight(&mut self) {
        let Some(attribute) = self.highlighted_attribute.as_ref() else {
            return;
        };
        if !self
            .attribute_choices()
            .iter()
            .any(|candidate| candidate == attribute)
        {
            self.highlighted_attribute = None;
        }
    }
}

pub(in crate::gerber_viewer) fn attribute_highlight_color(
    layer_color: Color,
    attributes: Option<&signex_gerber::GerberObjectAttributes>,
    highlighted_attribute: Option<&GerberAttributeValue>,
) -> Color {
    let Some(highlighted_attribute) = highlighted_attribute else {
        return layer_color;
    };
    if attributes.is_some_and(|attributes| {
        attributes
            .attributes
            .iter()
            .any(|attribute| attribute == highlighted_attribute)
    }) {
        HIGHLIGHT_COLOR
    } else {
        Color {
            a: NON_MATCHING_ALPHA,
            ..layer_color
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/gerber_viewer/attribute_highlight.rs"]
mod gerber_attribute_highlight_test_definitions;

#[cfg(test)]
gerber_attribute_highlight_test_definitions::gerber_attribute_highlight_tests!();
