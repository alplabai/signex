//! Help ▸ Keyboard Shortcuts modal — a single page listing every
//! binding registered in `crate::shortcuts::SHORTCUTS`. Reachable from
//! the Help menu and from F1.
//!
//! Closes UX_IMPROVEMENTS_OVER_ALTIUM §4.2 ("Hotkey discoverability").

use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;

use crate::app::Message;
use crate::shortcuts::SHORTCUTS;
use crate::styles;

const MODAL_W: f32 = 460.0;
const MODAL_H: f32 = 540.0;
const KEY_COL_W: f32 = 160.0;

pub fn view<'a>(tokens: &'a ThemeTokens) -> Element<'a, Message> {
    let text_primary = styles::ti(tokens.text);
    let text_secondary = styles::ti(tokens.text_secondary);

    let header = container(
        row![
            text("Keyboard Shortcuts").size(14).color(text_primary),
            Space::new().width(Length::Fill),
            button(text("Close").size(11).color(text_secondary))
                .on_press(Message::CloseKeyboardShortcuts)
                .style(styles::menu_item(tokens)),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(8),
    )
    .padding([10, 14])
    .style(styles::modal_header_strip(tokens));

    let mut rows: Vec<Element<'a, Message>> = Vec::with_capacity(SHORTCUTS.len() + 1);
    rows.push(
        row![
            text("Shortcut")
                .size(10)
                .color(text_secondary)
                .width(Length::Fixed(KEY_COL_W)),
            text("Action").size(10).color(text_secondary),
        ]
        .padding([4, 0])
        .into(),
    );
    for shortcut in SHORTCUTS {
        let key_label = if shortcut.modifiers.is_empty() {
            shortcut.key.to_string()
        } else {
            format!("{}+{}", shortcut.modifiers, shortcut.key)
        };
        rows.push(
            row![
                text(key_label)
                    .size(11)
                    .color(text_primary)
                    .width(Length::Fixed(KEY_COL_W)),
                text(shortcut.description).size(11).color(text_primary),
            ]
            .padding([3, 0])
            .into(),
        );
    }

    let body = container(scrollable(column(rows).spacing(0)).height(Length::Fill))
        .padding([10, 14])
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
