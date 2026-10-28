//! Help ▸ About Signex dialog — product name, version, licence and the
//! "Copy version info" action. Styled from `ThemeTokens` only.

use iced::widget::{Space, button, column, container, row, text};
use iced::{Element, Length};
use signex_types::theme::{ThemeId, ThemeTokens};

use crate::about::{ExternalLink, LICENSE_NAME, VERSION};
use crate::app::view::dialogs::{
    MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE, close_x_button,
};
use crate::app::{Message, OverlayMsg};
use crate::styles;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

const MODAL_W: f32 = 380.0;

pub fn view<'a>(tokens: &'a ThemeTokens, theme_id: ThemeId) -> Element<'a, Message> {
    let text_primary = styles::ti(tokens.text);
    let text_secondary = styles::ti(tokens.text_secondary);

    let header = container(
        row![
            text("About Signex")
                .size(MODAL_HEADER_TITLE_SIZE)
                .color(text_primary),
            Space::new().width(Length::Fill),
            close_x_button(
                Message::Overlay(OverlayMsg::CloseAbout),
                theme_id,
                text_secondary,
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .height(MODAL_HEADER_HEIGHT)
    .padding(MODAL_HEADER_PADDING)
    .style(styles::modal_header_strip(tokens));

    let license_link = button(text(format!("{LICENSE_NAME} license")).size(TS.label))
        .padding(0)
        .on_press(Message::Overlay(OverlayMsg::OpenLink(
            ExternalLink::License,
        )))
        .style(styles::menu_item(tokens));
    let copy_button = button(text("Copy version info").size(TS.label))
        .on_press(Message::Overlay(OverlayMsg::CopyAboutInfo))
        .style(styles::menu_item(tokens));

    let body = container(
        column![
            text("Signex").size(20).color(text_primary),
            text(format!("Version {VERSION}"))
                .size(TS.label)
                .color(text_primary),
            text("AI-first electronics design automation")
                .size(TS.body)
                .color(text_secondary),
            row![
                text("Licensed under").size(TS.label).color(text_secondary),
                license_link
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center),
            copy_button,
        ]
        .spacing(SP.m),
    )
    .padding([SP.l, SP.xl])
    .width(Length::Fill);

    let card = container(column![header, body].spacing(0))
        .width(Length::Fixed(MODAL_W))
        .clip(true)
        .style(styles::modal_card(tokens));

    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
