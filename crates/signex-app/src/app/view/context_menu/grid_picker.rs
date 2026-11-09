//! The footprint editor's grid picker popup (`G`): the standard grid
//! ladder as menu rows, the active step checked.

use super::*;

use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

impl Signex {
    /// v0.18.10 — Altium-style grid picker popup body. Renders the
    /// standard 1mil…2.5mm ladder; clicking a row sends
    /// `Message::Ui(UiMsg::GridPickerSelect(step_mm))` and closes the popup.
    pub(in crate::app::view) fn view_grid_picker_menu(&self) -> Element<'_, Message> {
        let active_step = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| match &t.kind {
                crate::app::TabKind::FootprintEditor(p) => {
                    self.document_state.footprint_editors.get(p)
                }
                _ => None,
            })
            .map(|e| e.state.snap_options.grid_step_mm);
        let panel_ctx = &self.document_state.panel_ctx;
        super::placement::menu_card(
            grid_picker_entries(active_step),
            &panel_ctx.tokens,
            &panel_ctx.ui_font_name,
        )
        .element
    }
}

/// The grid picker's rows: the Altium-standard ladder, mil steps first
/// (imperial designs anchor on 50 mil), then metric; the active step is
/// checked. Clicking a row sends `UiMsg::GridPickerSelect(step_mm)`.
fn grid_picker_entries(active_step: Option<f64>) -> Vec<DropdownEntry<Message>> {
    const LADDER: &[(&str, f64)] = &[
        ("1 Mil", 0.0254),
        ("5 Mil", 0.127),
        ("10 Mil", 0.254),
        ("20 Mil", 0.508),
        ("25 Mil", 0.635),
        ("50 Mil", 1.27),
        ("100 Mil", 2.54),
        ("0.025 mm", 0.025),
        ("0.100 mm", 0.100),
        ("0.250 mm", 0.250),
        ("0.500 mm", 0.500),
        ("1.000 mm", 1.000),
        ("2.500 mm", 2.500),
    ];
    LADDER
        .iter()
        .map(|&(label, step_mm)| {
            let is_active = active_step.is_some_and(|s| (s - step_mm).abs() < 1e-9);
            DropdownEntry::Item(
                DropdownItem::new(label, Message::Ui(UiMsg::GridPickerSelect(step_mm)))
                    .checked(is_active),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every step is a row that picks it; only the active one is checked.
    #[test]
    fn each_row_picks_its_step_and_the_active_one_is_checked() {
        let rows = grid_picker_entries(Some(1.27));
        assert_eq!(rows.len(), 13);
        for row in &rows {
            let DropdownEntry::Item(item) = row else {
                panic!("a grid row is an item row");
            };
            let Some(Message::Ui(UiMsg::GridPickerSelect(step))) = item.on_press else {
                panic!("{} picks no step", item.label);
            };
            assert_eq!(item.checked, (step - 1.27).abs() < 1e-9, "{}", item.label);
        }
    }
}
