//! Tools ▸ Passive Network Calculator — the resistor / capacitor / inductor
//! network calculator plus the RKM encoder. It opens in its own OS window
//! like the other tool dialogs (docs/DETACHED_MODALS.md); the in-window card
//! below is the fallback when that window is not available.

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

/// The calculator painted as a card in the main window (the fallback when
/// its own window is not available).
pub fn view<'a>(
    tokens: &'a ThemeTokens,
    theme_id: ThemeId,
    control: &'a CalculatorControl,
) -> Element<'a, Message> {
    build(tokens, theme_id, control, false)
}

/// The calculator as the content of its own OS window: no header, filling
/// the window.
pub fn window_view<'a>(
    tokens: &'a ThemeTokens,
    theme_id: ThemeId,
    control: &'a CalculatorControl,
) -> Element<'a, Message> {
    build(tokens, theme_id, control, true)
}

fn build<'a>(
    tokens: &'a ThemeTokens,
    theme_id: ThemeId,
    control: &'a CalculatorControl,
    windowed: bool,
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

    if windowed {
        return container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(styles::modal_card(tokens))
            .into();
    }
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
