//! The Width and Color rows of a wire, shared by the Tab form of the wire
//! tool and the Properties of a selected wire.

use std::rc::Rc;

use iced::{Color, Element};
use signex_types::schematic::WireWidthPreset;

use super::{ColorFieldProps, PanelMsg, color_field, form_pick_row};

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

/// The Width pick-list. `on_pick` builds the message for the chosen preset.
pub fn wire_width_row<'a>(
    current: WireWidthPreset,
    on_pick: impl Fn(WireWidthPreset) -> PanelMsg + 'a,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let labels: Vec<String> = WireWidthPreset::ALL
        .iter()
        .map(|p| p.label().to_string())
        .collect();
    form_pick_row(
        "Width",
        labels,
        current.label().to_string(),
        move |label: String| on_pick(WireWidthPreset::from_label(&label).unwrap_or_default()),
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
) -> Element<'a, PanelMsg> {
    let on_pick: Rc<dyn Fn([u8; 4]) -> PanelMsg + 'static> = {
        let on_set = Rc::clone(&on_set);
        Rc::new(move |rgba| on_set(Some(rgba_to_color(rgba))))
    };
    color_field(ColorFieldProps {
        label: "Color",
        current: current.map(color_to_rgba),
        none_label: "Default",
        show_palette: picker == WireColorPicker::Palette,
        show_advanced: picker == WireColorPicker::Advanced,
        muted,
        border_c,
        on_toggle: PanelMsg::ToggleWireColorPicker,
        on_advanced: PanelMsg::OpenWireColorAdvancedPicker,
        on_cancel: PanelMsg::CancelWireColorPicker,
        on_pick,
        on_clear: current.is_some().then(|| on_set(None)),
    })
}
