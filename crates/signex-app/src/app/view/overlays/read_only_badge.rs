//! The "read-only" badge drawn over the canvas of a sheet written by a newer
//! Signex (#1402), so the state is visible before the user tries an edit.

use iced::widget::container;
use iced::{Background, Border, Element, Length, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use crate::app::Message;
use crate::styles;
const TS: styles::TypeScale = styles::TypeScale::DEFAULT;

/// Top-right badge. A plain container with no `mouse_area`, so every pointer
/// event falls through to the canvas below it. Its text is
/// `overlay-read-only-badge`; the tab says the same word.
pub(in crate::app::view) fn read_only_badge<'a>(
    tokens: &ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, Message> {
    let panel_bg = styles::ti(tokens.panel_bg);
    let border = styles::ti(tokens.border);
    let muted = styles::ti(tokens.text_secondary);
    let badge = signex_i18n::t!(i18n, signex_i18n::ids::OVERLAY_READ_ONLY_BADGE);
    let chip = container(text(badge).size(TS.body).color(muted))
        .padding([3, 8])
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(panel_bg)),
            border: Border {
                color: border,
                width: 1.0,
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    container(chip)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Top)
        .padding(8.0)
        .boxed()
}
