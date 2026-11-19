//! The Width and Color rows of a wire, shared by the Tab form of the wire
//! tool and the Properties of a selected wire.

use std::rc::Rc;

use iced::{Color, Element};
use signex_types::schematic::WireWidthPreset;

use signex_i18n::{I18n, MessageId, ids, t};

use super::{ColorFieldProps, PanelMsg, color_field, form_choice_row};
use crate::choice::{Choice, chosen};

/// Whether the wire colour picker is closed, shows the preset palette or the
/// full HSV / RGB picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireColorPicker {
    #[default]
    Closed,
    Palette,
    Advanced,
}

/// The wire colour as the colour field carries it.
pub fn color_to_rgba(color: signex_types::theme::Color) -> [u8; 4] {
    [color.r, color.g, color.b, color.a]
}

/// The colour a colour field pick means.
pub fn rgba_to_color(rgba: [u8; 4]) -> signex_types::theme::Color {
    signex_types::theme::Color::new(rgba[0], rgba[1], rgba[2], rgba[3])
}

/// The catalog name of a wire width preset.
pub(in crate::panels) fn preset_label(preset: WireWidthPreset) -> MessageId {
    match preset {
        WireWidthPreset::Smallest => ids::PROPERTIES_WIRE_WIDTH_SMALLEST,
        WireWidthPreset::Small => ids::PROPERTIES_WIRE_WIDTH_SMALL,
        WireWidthPreset::Medium => ids::PROPERTIES_WIRE_WIDTH_MEDIUM,
        WireWidthPreset::Large => ids::PROPERTIES_WIRE_WIDTH_LARGE,
    }
}

/// The Width pick-list. `on_pick` builds the message for the chosen preset.
pub fn wire_width_row<'a>(
    current: WireWidthPreset,
    on_pick: impl Fn(WireWidthPreset) -> PanelMsg + 'a,
    muted: Color,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let choices = Choice::named(&WireWidthPreset::ALL, preset_label, i18n);
    let selected = chosen(&choices, &current).unwrap_or_else(|| choices[0].clone());
    form_choice_row(
        t!(i18n, ids::PROPERTIES_WIRE_WIDTH),
        choices,
        selected,
        on_pick,
        muted,
    )
}

/// The Color swatch row. `on_set` builds the message for a pick (`Some`) or
/// for the reset to the theme's wire colour (`None`).
pub fn wire_color_row<'a>(
    current: Option<signex_types::theme::Color>,
    picker: WireColorPicker,
    on_set: Rc<dyn Fn(Option<signex_types::theme::Color>) -> PanelMsg + 'static>,
    muted: Color,
    border_c: Color,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let on_pick: Rc<dyn Fn([u8; 4]) -> PanelMsg + 'static> = {
        let on_set = Rc::clone(&on_set);
        Rc::new(move |rgba| on_set(Some(rgba_to_color(rgba))))
    };
    color_field(
        ColorFieldProps {
            label: t!(i18n, ids::PROPERTIES_WIRE_COLOR).into(),
            current: current.map(color_to_rgba),
            none_label: t!(i18n, ids::PROPERTIES_COLOR_DEFAULT).into(),
            show_palette: picker == WireColorPicker::Palette,
            show_advanced: picker == WireColorPicker::Advanced,
            muted,
            border_c,
            on_toggle: PanelMsg::ToggleWireColorPicker,
            on_advanced: PanelMsg::OpenWireColorAdvancedPicker,
            on_cancel: PanelMsg::CancelWireColorPicker,
            on_pick,
            on_clear: current.is_some().then(|| on_set(None)),
        },
        i18n,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each preset reads as `WireWidthPreset::label`, the English the list
    /// showed before, and a choice is found by its value.
    #[test]
    fn each_width_preset_keeps_its_english_name() {
        let en = I18n::en_us();
        let choices = Choice::named(&WireWidthPreset::ALL, preset_label, &en);
        for preset in WireWidthPreset::ALL {
            assert_eq!(en.message_plain(preset_label(preset), &[]), preset.label());
            assert_eq!(
                chosen(&choices, &preset).map(|c| c.label).as_deref(),
                Some(preset.label())
            );
        }
    }
}
