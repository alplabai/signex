//! Tools ▸ PCB Trace Calculator modal — the IPC-2221 trace width
//! calculator, rendered as an in-app modal like every other Signex dialog.
//!
//! Like the passive calculator (`passive_calculator_modal.rs`, which has
//! the reasoning) it does NOT open its own OS window: Signex runs
//! borderless everywhere, and a native title bar would exist nowhere else
//! in the app. Header strip, close X, centred card.

use iced::widget::{Space, column, container, row, text};
use iced::{Element, Length};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::trace_calculator::TraceCalculatorControl;

use crate::app::view::dialogs::{
    MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE, close_x_button,
};
use crate::app::{Message, OverlayMsg};
use crate::styles;

/// Card width, wide enough for the label / field / unit rows and the two
/// result panels side by side. The height follows the content up to
/// `MODAL_MAX_H`; the control scrolls only past that.
const MODAL_W: f32 = 640.0;
const MODAL_MAX_H: f32 = 840.0;

pub fn view<'a>(
    tokens: &'a ThemeTokens,
    theme_id: ThemeId,
    control: &'a TraceCalculatorControl,
) -> Element<'a, Message> {
    let text_primary = styles::ti(tokens.text);
    let text_secondary = styles::ti(tokens.text_secondary);

    let header = container(
        row![
            text("PCB Trace Calculator")
                .size(MODAL_HEADER_TITLE_SIZE)
                .color(text_primary),
            Space::new().width(Length::Fill),
            close_x_button(
                Message::Overlay(OverlayMsg::ClosePcbTraceCalculator),
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

    // The control scrolls its own body; a second scrollable here would
    // swallow the wheel events before the inner one saw them.
    let body = container(
        control
            .view(tokens)
            .map(|message| Message::Overlay(OverlayMsg::PcbTraceCalculatorEdited(message))),
    )
    .width(Length::Fill);

    let card = container(column![header, body].spacing(0))
        .width(Length::Fixed(MODAL_W))
        .max_height(MODAL_MAX_H)
        .clip(true)
        .style(styles::modal_card(tokens));

    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
