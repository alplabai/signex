//! Tools ▸ Passive Network Calculator modal — the resistor / capacitor /
//! inductor network calculator plus the RKM encoder, rendered as an
//! in-app modal like every other Signex dialog.
//!
//! It deliberately does NOT open its own OS window. Signex runs
//! borderless everywhere (`bootstrap/new.rs` — `decorations: false`),
//! so a default `iced::window::open` would arrive wearing a native
//! title bar that exists nowhere else in the app. Same chrome as
//! `keyboard_shortcuts_modal`: header strip, close X, centred card.

use iced::widget::{Space, column, container, row, text};
use iced::{Element, Length};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::passive_calculator::CalculatorControl;

use crate::app::view::dialogs::{
    MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE, close_x_button,
};
use crate::app::{Message, OverlayMsg};
use crate::styles;

/// Card size. Wide enough for the four-control input row and the
/// component cards the solver emits, without the empty acreage the
/// standalone window had.
const MODAL_W: f32 = 640.0;
const MODAL_H: f32 = 520.0;

pub fn view<'a>(
    tokens: &'a ThemeTokens,
    theme_id: ThemeId,
    control: &'a CalculatorControl,
) -> Element<'a, Message> {
    let text_primary = styles::ti(tokens.text);
    let text_secondary = styles::ti(tokens.text_secondary);

    let header = container(
        row![
            text("Passive Network Calculator")
                .size(MODAL_HEADER_TITLE_SIZE)
                .color(text_primary),
            Space::new().width(Length::Fill),
            close_x_button(
                Message::Overlay(OverlayMsg::ClosePassiveCalculator),
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

    // The control already scrolls its own body, so this wrapper only
    // bounds it — nesting a second scrollable would swallow wheel
    // events before the inner one sees them.
    let body = container(control.view(tokens).map(Message::PassiveCalculator))
        .width(Length::Fill)
        .height(Length::Fill);

    let card = container(column![header, body].spacing(0))
        .width(Length::Fixed(MODAL_W))
        .height(Length::Fixed(MODAL_H))
        .clip(true)
        .style(styles::modal_card(tokens));

    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
