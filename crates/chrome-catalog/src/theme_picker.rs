use iced::widget::{Row, container, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::{ThemeId, ThemeTokens};

use crate::catalog::Message;
use crate::theme;
use crate::theme_pill;

pub(crate) fn view<'a>(selected_theme: ThemeId, tokens: &ThemeTokens) -> Element<'a, Message> {
    let toolbar_bg = theme::color(tokens.toolbar_bg);
    let text_color = theme::color(tokens.text);
    let mut pills: Row<'a, Message> = Row::new().spacing(6);
    pills = pills.push(text("Theme:").size(11).color(text_color));
    for &theme_id in ThemeId::BUILTINS {
        pills = pills.push(theme_pill::view(
            theme_id,
            theme_id == selected_theme,
            tokens,
        ));
    }
    container(pills.align_y(iced::Alignment::Center))
        .width(Length::Fill)
        .padding([10, 14])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(toolbar_bg)),
            border: Border {
                width: 0.0,
                radius: 0.0.into(),
                color: Color::TRANSPARENT,
            },
            ..container::Style::default()
        })
        .into()
}
