//! First-run tour overlay — a single dismissible card pinned to the
//! bottom-right of the main window on first launch. Shows the three
//! gestures a new user needs immediately (right-click pan, scroll
//! zoom, F1 for help). Closes UX_IMPROVEMENTS_OVER_ALTIUM §4.3.

use iced::widget::{Space, button, column, container, row, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;

use crate::app::{Message, OverlayMsg};
use crate::styles;

const CARD_W: f32 = 360.0;
const CARD_PAD_RIGHT: f32 = 24.0;
const CARD_PAD_BOTTOM: f32 = 36.0;

pub fn view<'a>(tokens: &'a ThemeTokens) -> Element<'a, Message> {
    let text_primary = styles::ti(tokens.text);
    let text_secondary = styles::ti(tokens.text_secondary);

    let header = container(
        row![
            text("Welcome to Signex").size(13).color(text_primary),
            Space::new().width(Length::Fill),
            button(text("✕").size(11).color(text_secondary))
                .on_press(Message::Overlay(OverlayMsg::DismissFirstRunTour))
                .style(styles::menu_item(tokens)),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(8),
    )
    .padding([8, 12])
    .style(styles::modal_header_strip(tokens));

    let body = container(
        column![
            text("Right-click + drag to pan")
                .size(11)
                .color(text_primary),
            text("Scroll to zoom").size(11).color(text_primary),
            text("F1 for keyboard shortcuts")
                .size(11)
                .color(text_primary),
        ]
        .spacing(4),
    )
    .padding([10, 12])
    .width(Length::Fill);

    let card = container(column![header, body].spacing(0))
        .width(Length::Fixed(CARD_W))
        .clip(true)
        .style(styles::modal_card(tokens));

    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(
            iced::Padding::default()
                .right(CARD_PAD_RIGHT)
                .bottom(CARD_PAD_BOTTOM),
        )
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Bottom)
        .into()
}
