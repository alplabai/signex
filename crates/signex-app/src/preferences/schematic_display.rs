//! Schematic ▸ Display — "Fill symbol bodies" and its colour, on the
//! `setting_row` grid every Preferences page draws on.

use super::pages_view::controls::toggle;
use super::setting_row::{SettingRow, help_text, page, section_header, setting_row};
use super::*;
use crate::panels::{ColorFieldProps, color_field};
use crate::symbol_fill_prefs::SymbolBodyFill;
use iced::Element;
use signex_i18n::{I18n, ids, t};
use std::rc::Rc;

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
    /// The "Cross out not-fitted parts" draft (#1294).
    pub(super) cross_out_not_fitted: bool,
    pub(super) tokens: &'a signex_types::theme::ThemeTokens,
    pub(super) i18n: &'a I18n,
}

pub(super) fn content_schematic_display<'a>(page_in: DisplayPage<'a>) -> Element<'a, PrefMsg> {
    let DisplayPage {
        draft,
        theme_tint,
        palette_open,
        picker_open,
        cross_out_not_fitted,
        tokens,
        i18n,
    } = page_in;
    let mut items = vec![
        section_header(
            t!(i18n, ids::PREFERENCES_SECTION_SCHEMATIC_DISPLAY),
            None,
            tokens,
        ),
        setting_row(
            SettingRow {
                label: t!(i18n, ids::PREFERENCES_SYMBOL_BODY_FILL),
                pills: Vec::new(),
                help: Some(help_text(
                    t!(i18n, ids::PREFERENCES_SYMBOL_BODY_FILL_HELP),
                    tokens,
                )),
                control: toggle(draft.enabled, tokens, |on| {
                    PrefMsg::SymbolBodyFill(BodyFillMsg::Toggled(on))
                }),
                error: None,
            },
            tokens,
        ),
    ];
    if draft.enabled {
        let fill = PrefMsg::SymbolBodyFill;
        let rgba = |c: signex_types::theme::Color| [c.r, c.g, c.b, c.a];
        let on_pick: Rc<dyn Fn([u8; 4]) -> PrefMsg + 'static> =
            Rc::new(|rgba: [u8; 4]| PrefMsg::SymbolBodyFill(BodyFillMsg::ColorPicked(rgba)));
        items.push(color_field(
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
        items.push(help_text(
            t!(i18n, ids::PREFERENCES_SYMBOL_BODY_FILL_COLOUR_HELP),
            tokens,
        ));
    }
    items.push(setting_row(
        SettingRow {
            label: t!(i18n, ids::PREFERENCES_CROSS_OUT_NOT_FITTED),
            pills: Vec::new(),
            help: Some(help_text(
                t!(i18n, ids::PREFERENCES_CROSS_OUT_NOT_FITTED_HELP),
                tokens,
            )),
            control: toggle(
                cross_out_not_fitted,
                tokens,
                PrefMsg::DraftCrossOutNotFitted,
            ),
            error: None,
        },
        tokens,
    ));
    page(items)
}
