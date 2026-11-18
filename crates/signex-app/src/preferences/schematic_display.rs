//! Schematic ▸ Display — "Fill symbol bodies" and its colour.

use super::*;
use crate::panels::{ColorFieldProps, color_field};
use crate::symbol_fill_prefs::SymbolBodyFill;
use iced::widget::{column, row};
use iced::{Element, Length, Widget};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_widgets::{checkbox, text};
use std::rc::Rc;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// What the "Fill symbol bodies" rows changed. Drafts; persisted on Save.
#[derive(Debug, Clone)]
pub enum BodyFillMsg {
    /// The switch was turned on or off.
    Toggled(bool),
    /// The colour swatch was clicked: the preset palette opens or closes.
    PaletteToggled,
    /// "Custom…" was clicked: the colour picker opens.
    PickerOpened,
    /// The colour picker was closed without a colour.
    PickerCancelled,
    /// A colour was picked, from a preset or the picker (RGBA).
    ColorPicked([u8; 4]),
    /// Reset was clicked: the fill follows the theme's tint again.
    ThemeColorRestored,
}

/// What the Display page shows.
#[derive(Clone, Copy)]
pub(super) struct DisplayPage<'a> {
    pub(super) draft: SymbolBodyFill,
    /// The tint of the theme on screen, shown while no colour is picked.
    pub(super) theme_tint: Option<signex_types::theme::Color>,
    pub(super) palette_open: bool,
    pub(super) picker_open: bool,
    pub(super) tokens: &'a signex_types::theme::ThemeTokens,
    pub(super) i18n: &'a I18n,
}

pub(super) fn content_schematic_display<'a>(page: DisplayPage<'a>) -> Element<'a, PrefMsg> {
    let DisplayPage {
        draft,
        theme_tint,
        palette_open,
        picker_open,
        tokens,
        i18n,
    } = page;
    let described = |label: MessageId, help: MessageId| {
        column![
            text(t!(i18n, label)).size(TS.label).style(text_primary),
            text(t!(i18n, help)).size(TS.caption).style(text_muted),
        ]
        .spacing(SP.xs)
        .width(Length::Fill)
    };
    let switch = row![
        described(
            ids::PREFERENCES_SYMBOL_BODY_FILL,
            ids::PREFERENCES_SYMBOL_BODY_FILL_HELP
        ),
        checkbox(draft.enabled)
            .size(TS.heading)
            .on_toggle(|on| PrefMsg::SymbolBodyFill(BodyFillMsg::Toggled(on))),
    ]
    .spacing(SP.xl)
    .align_y(iced::Alignment::Center);

    let mut content = column![
        section_title(t!(i18n, ids::PREFERENCES_SECTION_SCHEMATIC_DISPLAY)),
        switch,
    ]
    .spacing(SP.l)
    .width(Length::Fill);
    if draft.enabled {
        let fill = PrefMsg::SymbolBodyFill;
        let rgba = |c: signex_types::theme::Color| [c.r, c.g, c.b, c.a];
        let on_pick: Rc<dyn Fn([u8; 4]) -> PrefMsg + 'static> =
            Rc::new(|rgba: [u8; 4]| PrefMsg::SymbolBodyFill(BodyFillMsg::ColorPicked(rgba)));
        content = content.push(color_field(
            ColorFieldProps {
                label: t!(i18n, ids::PREFERENCES_SYMBOL_BODY_FILL_COLOUR).into(),
                current: draft.color.or(theme_tint).map(rgba),
                none_label: t!(i18n, ids::PROPERTIES_COLOR_DEFAULT).into(),
                show_palette: palette_open,
                show_advanced: picker_open,
                muted: crate::styles::tokens::resolve(tokens).ink4,
                border_c: crate::styles::ti(tokens.border),
                on_toggle: fill(BodyFillMsg::PaletteToggled),
                on_advanced: fill(BodyFillMsg::PickerOpened),
                on_cancel: fill(BodyFillMsg::PickerCancelled),
                on_pick,
                on_clear: draft
                    .color
                    .is_some()
                    .then_some(fill(BodyFillMsg::ThemeColorRestored)),
            },
            i18n,
        ));
        content = content.push(
            text(t!(i18n, ids::PREFERENCES_SYMBOL_BODY_FILL_COLOUR_HELP))
                .size(TS.caption)
                .style(text_muted),
        );
    }
    content.padding(SP.xl).boxed()
}
