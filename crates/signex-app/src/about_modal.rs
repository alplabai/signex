//! Help ▸ About Signex dialog — product name, version, licence and the
//! "Copy version info" action. Styled from `ThemeTokens` only.

use iced::widget::{Widget, column, container, row};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};

use crate::about::{ExternalLink, LICENSE_NAME, VERSION};
use crate::app::view::dialogs::modal_title_header;
use crate::app::{Message, OverlayMsg};
use crate::styles;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

const MODAL_W: f32 = 380.0;

/// The dialog painted as a card in the main window.
pub fn view<'a>(tokens: &'a ThemeTokens) -> Element<'a, Message> {
    build(tokens, false)
}

/// The dialog as the content of its own OS window: no header (the native
/// title bar has the title and the close) and filling the window.
pub fn window_view<'a>(tokens: &'a ThemeTokens) -> Element<'a, Message> {
    build(tokens, true)
}

fn build<'a>(tokens: &'a ThemeTokens, windowed: bool) -> Element<'a, Message> {
    let text_primary = styles::ti(tokens.text);
    let text_secondary = styles::ti(tokens.text_secondary);

    let header = modal_title_header(
        "About Signex",
        Message::Overlay(OverlayMsg::CloseAbout),
        tokens,
    );

    let license_link = button(text(raw(format!("{LICENSE_NAME} license"))).size(TS.label))
        .padding(0)
        .on_press(Message::Overlay(OverlayMsg::OpenLink(
            ExternalLink::License,
        )))
        .style(styles::menu_item(tokens));
    let copy_button = button(text(raw("Copy version info")).size(TS.label))
        .on_press(Message::Overlay(OverlayMsg::CopyAboutInfo))
        .style(styles::menu_item(tokens));

    let body = container(
        column![
            text(raw("Signex")).size(TS.display).color(text_primary),
            text(raw(format!("Version {VERSION}")))
                .size(TS.label)
                .color(text_primary),
            text(raw("AI-first electronics design automation"))
                .size(TS.body)
                .color(text_secondary),
            row![
                text(raw("Licensed under"))
                    .size(TS.label)
                    .color(text_secondary),
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

    if windowed {
        return container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(styles::modal_card(tokens))
            .boxed();
    }
    let card = container(column![header, body].spacing(0))
        .width(Length::Fixed(MODAL_W))
        .clip(true)
        .style(styles::modal_card(tokens));

    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .boxed()
}
