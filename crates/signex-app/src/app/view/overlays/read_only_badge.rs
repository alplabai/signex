//! The "read-only" badge drawn over the canvas of a sheet written by a newer
//! Signex (#1402), so the state is visible before the user tries an edit.

use iced::widget::{container, text};
use iced::{Background, Border, Element, Length};
use signex_types::theme::ThemeTokens;

use crate::app::Message;
use crate::styles;

/// Text of the badge; the tab says the same word.
const BADGE_TEXT: &str = "Read-only: written by a newer Signex";

/// Top-right badge. A plain container with no `mouse_area`, so every pointer
/// event falls through to the canvas below it.
pub(in crate::app::view) fn read_only_badge<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let panel_bg = styles::ti(tokens.panel_bg);
    let border = styles::ti(tokens.border);
    let muted = styles::ti(tokens.text_secondary);
    let chip = container(text(BADGE_TEXT).size(11).color(muted))
        .padding([3, 8])
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(panel_bg)),
            border: Border {
                color: border,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        });
    container(chip)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Top)
        .padding(8)
        .into()
}
