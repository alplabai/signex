//! PCB ▸ Display (#2514): how much the copper of every other net dims while
//! a board net is highlighted, on the `setting_row` grid. A draft; persisted
//! on Save.

use iced::widget::{row, slider};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;

use super::PrefMsg;
use super::pages_view::controls::{readout, slider_style};
use super::setting_row::{SettingRow, help_text, page, section_header, setting_row};
use crate::styles;

const SP: styles::Spacing = styles::Spacing::DEFAULT;

/// The page: one slider, 0 to 100 %.
pub(super) fn content_pcb_display<'a>(
    dim_percent: u8,
    t: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, PrefMsg> {
    let control = row![
        slider(0..=100u8, dim_percent, PrefMsg::DraftPcbDimOthersPercent)
            .width(Length::Fill)
            .style(slider_style(t)),
        readout(
            t!(
                i18n,
                ids::PREFERENCES_PCB_DIM_OTHERS_VALUE,
                "percent" => u32::from(dim_percent)
            ),
            t,
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    page(vec![
        section_header(t!(i18n, ids::PREFERENCES_SECTION_PCB_DISPLAY), None, t),
        setting_row(
            SettingRow {
                label: t!(i18n, ids::PREFERENCES_PCB_DIM_OTHERS),
                pills: Vec::new(),
                help: Some(help_text(t!(i18n, ids::PREFERENCES_PCB_DIM_OTHERS_HELP), t)),
                control: control.boxed(),
                error: None,
            },
            t,
        ),
    ])
}
