//! Appearance ▸ Language: "System default" plus each available language by
//! its own name.

use super::widgets::{h_sep, section_title, text_muted, text_primary};
use super::*;
use iced::Widget;
use iced::widget::{Space, column, row, text};
use signex_i18n::{I18n, LanguagePref, ids, t};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;

/// The section's rows, ready to push onto the Appearance column.
pub(super) fn language_section<'a>(draft: &LanguagePref, i18n: &I18n) -> Vec<Element<'a, PrefMsg>> {
    let label = t!(i18n, ids::SETTINGS_LANGUAGE).into_string();
    let help = t!(i18n, ids::SETTINGS_LANGUAGE_HELP).into_string();
    let picker = iced::widget::pick_list(
        Some(i18n.option_for(draft)),
        i18n.language_options().to_vec(),
        |option: &signex_i18n::LanguageOption| option.to_string(),
    )
    .on_select(|option: signex_i18n::LanguageOption| PrefMsg::DraftLanguage(option.pref))
    .text_size(12)
    .width(200);
    vec![
        h_sep(),
        Space::new().height(16).boxed(),
        section_title(&label),
        Space::new().height(10).boxed(),
        row![
            column![
                text(label).size(TS.label).style(text_primary),
                text(help).size(TS.caption).style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            picker,
        ]
        .align_y(iced::Alignment::Center)
        .boxed(),
        Space::new().height(20).boxed(),
    ]
}
