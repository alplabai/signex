use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::gerber_viewer) struct DCodeChoice {
    pub(in crate::gerber_viewer) code: i32,
    label: String,
}

impl std::fmt::Display for DCodeChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

impl GerberViewerState {
    pub(in crate::gerber_viewer) fn d_code_choices(&self) -> Vec<DCodeChoice> {
        let Some(layer) = self.active_layer.and_then(|index| self.layers.get(index)) else {
            return Vec::new();
        };
        let group = layer.layer.definition_group();
        if group.definition_label != "D-codes" {
            return Vec::new();
        }

        group
            .definitions
            .into_iter()
            .filter_map(|definition| {
                let code = definition.code.strip_prefix('D')?.parse::<i32>().ok()?;
                let item_suffix = if definition.usage_count == 1 { "" } else { "s" };
                Some(DCodeChoice {
                    code,
                    label: format!(
                        "{} — {} ({} item{})",
                        definition.code,
                        definition.description,
                        definition.usage_count,
                        item_suffix,
                    ),
                })
            })
            .collect()
    }

    pub fn highlighted_d_code(&self) -> Option<i32> {
        self.highlighted_d_code
    }

    pub fn set_highlighted_d_code(&mut self, d_code: i32) {
        if self
            .d_code_choices()
            .iter()
            .any(|choice| choice.code == d_code)
        {
            self.status = format!("Highlighted D-code: D{d_code}");
            self.highlighted_d_code = Some(d_code);
            self.highlighted_component = None;
            self.highlighted_net = None;
            self.highlighted_attribute = None;
            self.redraw_generation = self.redraw_generation.wrapping_add(1);
        }
    }

    pub fn clear_d_code_highlight(&mut self) {
        if self.highlighted_d_code.take().is_some() {
            self.status = "D-code highlight cleared.".into();
            self.redraw_generation = self.redraw_generation.wrapping_add(1);
        }
    }

    pub(in crate::gerber_viewer) fn retain_available_d_code_highlight(&mut self) {
        let Some(d_code) = self.highlighted_d_code else {
            return;
        };
        if !self
            .d_code_choices()
            .iter()
            .any(|choice| choice.code == d_code)
        {
            self.highlighted_d_code = None;
        }
    }
}

pub(in crate::gerber_viewer) fn d_code_highlight_color(
    layer_color: Color,
    primitive: &GerberPrimitive,
    highlighted_d_code: Option<i32>,
) -> Color {
    let Some(highlighted_d_code) = highlighted_d_code else {
        return layer_color;
    };
    let matches = match primitive {
        GerberPrimitive::Stroke { d_code, .. } | GerberPrimitive::Flash { d_code, .. } => {
            *d_code == Some(highlighted_d_code)
        }
        _ => false,
    };
    if matches {
        HIGHLIGHT_COLOR
    } else {
        Color {
            a: NON_MATCHING_ALPHA,
            ..layer_color
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/gerber_viewer/d_code_highlight.rs"]
mod gerber_d_code_highlight_test_definitions;

#[cfg(test)]
gerber_d_code_highlight_test_definitions::gerber_d_code_highlight_tests!();
