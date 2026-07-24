use iced::widget::{Space, column, container, text};
use iced::{Background, Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use crate::catalog::Message;
use crate::theme;

pub(crate) fn view<'a>(
    title: &'static str,
    tokens: &ThemeTokens,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    let text_color = theme::color(tokens.text);
    let muted = theme::color(tokens.text_secondary);
    let border = theme::color(tokens.border);
    let panel = theme::color(tokens.panel_bg);
    container(column![
        text(title).size(13).color(text_color),
        Space::new().height(8),
        container(body)
            .padding(16)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(panel)),
                border: Border {
                    width: 1.0,
                    radius: 4.0.into(),
                    color: border,
                },
                ..container::Style::default()
            }),
    ])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        text_color: Some(muted),
        ..container::Style::default()
    })
    .into()
}
