//! The footprint editor's grid picker popup (`G`): the standard grid
//! ladder as menu rows, the active step checked.

use super::*;

use super::items::text;
use signex_i18n::{I18n, menu_ids};
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
            grid_picker_entries(&self.ui_state.i18n, active_step),
            &panel_ctx.tokens,
            &panel_ctx.ui_font_name,
        )
        .element
    }
}

/// The grid picker's rows: the Altium-standard ladder, mil steps first
/// (imperial designs anchor on 50 mil), then metric; the active step is
/// checked. Clicking a row sends `UiMsg::GridPickerSelect(step_mm)`.
fn grid_picker_entries(i18n: &I18n, active_step: Option<f64>) -> Vec<DropdownEntry<Message>> {
    const LADDER: &[(signex_i18n::MessageId, f64)] = &[
        (menu_ids::CTX_GRID_1_MIL, 0.0254),
        (menu_ids::CTX_GRID_5_MIL, 0.127),
        (menu_ids::CTX_GRID_10_MIL, 0.254),
        (menu_ids::CTX_GRID_20_MIL, 0.508),
        (menu_ids::CTX_GRID_25_MIL, 0.635),
        (menu_ids::CTX_GRID_50_MIL, 1.27),
        (menu_ids::CTX_GRID_100_MIL, 2.54),
        (menu_ids::CTX_GRID_0_025_MM, 0.025),
        (menu_ids::CTX_GRID_0_100_MM, 0.100),
        (menu_ids::CTX_GRID_0_250_MM, 0.250),
        (menu_ids::CTX_GRID_0_500_MM, 0.500),
        (menu_ids::CTX_GRID_1_000_MM, 1.000),
        (menu_ids::CTX_GRID_2_500_MM, 2.500),
    ];
    LADDER
        .iter()
        .map(|&(label, step_mm)| {
            let is_active = active_step.is_some_and(|s| (s - step_mm).abs() < 1e-9);
            DropdownEntry::Item(
                DropdownItem::new(
                    text(i18n, label),
                    Message::Ui(UiMsg::GridPickerSelect(step_mm)),
                )
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
        let rows = grid_picker_entries(I18n::en_us_ref(), Some(1.27));
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
