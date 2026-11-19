//! The "Fill symbol bodies" rows of Preferences ▸ Schematic ▸ Display: they
//! edit the draft only; Save commits and persists it with every other draft.

use crate::app::UiState;
use crate::preferences::BodyFillMsg;

impl UiState {
    /// Apply one change of the fill rows to the draft and the colour field.
    pub(crate) fn apply_body_fill_message(&mut self, msg: BodyFillMsg) {
        let draft = &mut self.preferences_draft_symbol_body_fill;
        match msg {
            BodyFillMsg::Toggled(enabled) => draft.enabled = enabled,
            BodyFillMsg::PaletteToggled => {
                self.preferences_body_fill_palette_open = !self.preferences_body_fill_palette_open;
                self.preferences_body_fill_picker_open = false;
            }
            BodyFillMsg::PickerOpened => {
                self.preferences_body_fill_palette_open = false;
                self.preferences_body_fill_picker_open = true;
            }
            BodyFillMsg::PickerCancelled => self.preferences_body_fill_picker_open = false,
            BodyFillMsg::ColorPicked([r, g, b, a]) => {
                draft.color = Some(signex_types::theme::Color::new(r, g, b, a));
                self.close_body_fill_colour_field();
            }
            BodyFillMsg::ThemeColorRestored => {
                draft.color = None;
                self.close_body_fill_colour_field();
            }
        }
    }

    fn close_body_fill_colour_field(&mut self) {
        self.preferences_body_fill_palette_open = false;
        self.preferences_body_fill_picker_open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol_fill_prefs::SymbolBodyFill;
    use signex_types::theme::Color;

    #[test]
    fn the_rows_edit_the_draft_and_never_the_saved_preference() {
        let (mut app, _t) = crate::app::Signex::new();
        let saved = app.ui_state.symbol_body_fill;
        let ui = &mut app.ui_state;
        ui.apply_body_fill_message(BodyFillMsg::Toggled(false));
        ui.apply_body_fill_message(BodyFillMsg::PaletteToggled);
        assert!(ui.preferences_body_fill_palette_open);
        ui.apply_body_fill_message(BodyFillMsg::ColorPicked([1, 2, 3, 255]));
        assert_eq!(
            ui.preferences_draft_symbol_body_fill,
            SymbolBodyFill {
                enabled: false,
                color: Some(Color::new(1, 2, 3, 255)),
            }
        );
        assert!(!ui.preferences_body_fill_palette_open);
        assert!(ui.preferences_draft_differs());
        assert_eq!(ui.symbol_body_fill, saved);
        ui.apply_body_fill_message(BodyFillMsg::ThemeColorRestored);
        assert_eq!(ui.preferences_draft_symbol_body_fill.color, None);
    }
}
