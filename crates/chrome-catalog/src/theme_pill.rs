use iced::widget::{button, text};
use iced::{Background, Border, Color, Element, Theme};
use signex_types::theme::{ThemeId, ThemeTokens};

use crate::catalog::Message;
use crate::theme;

pub(crate) fn view<'a>(
    theme_id: ThemeId,
    is_selected: bool,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let accent = theme::color(tokens.accent);
    let text_color = theme::color(tokens.text);
    let border = theme::color(tokens.border);
    button(
        text(theme_id.label().to_string())
            .size(11)
            .color(text_color),
    )
    .padding([4, 10])
    .on_press(Message::SelectTheme(theme_id))
    .style(move |_: &Theme, status: button::Status| {
        let background = match (is_selected, status) {
            (true, _) => accent,
            (false, button::Status::Hovered | button::Status::Pressed) => {
                Color { a: 0.18, ..accent }
            }
            _ => Color::from_rgba(1.0, 1.0, 1.0, 0.04),
        };
        button::Style {
            background: Some(Background::Color(background)),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            text_color,
            ..button::Style::default()
        }
    })
    .into()
}
