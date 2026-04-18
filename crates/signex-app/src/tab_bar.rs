//! Document tab bar — tabs for open schematic sheets and PCB.

use iced::widget::{Row, button, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use crate::app::TabInfo;
use crate::styles;

#[derive(Debug, Clone)]
pub enum TabMessage {
    Select(usize),
    Close(usize),
}

pub fn view<'a>(tabs: &[TabInfo], active: usize, tokens: &ThemeTokens) -> Element<'a, TabMessage> {
    let mut bar = Row::new().spacing(2.0);

    let text_primary = styles::ti(tokens.text);
    let text_muted = styles::ti(tokens.text_secondary);
    let tab_active_bg = styles::ti(tokens.hover);
    let border = styles::ti(tokens.border);

    for (i, tab) in tabs.iter().enumerate() {
        let label = if tab.dirty {
            format!("{} \u{2022}", tab.title) // bullet for dirty
        } else {
            tab.title.clone()
        };

        let is_active = i == active;
        let text_c = if is_active { text_primary } else { text_muted };

        // Close button — visible "×" with hover highlight
        let hover_close = Color::from_rgb(0.35, 0.35, 0.38);
        let close_btn = button(text("\u{00D7}").size(14).color(text_muted))
            .padding([0, 4])
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered => Some(Background::Color(hover_close)),
                    _ => None,
                };
                button::Style {
                    background: bg,
                    border: Border {
                        radius: 2.0.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }
            })
            .on_press(TabMessage::Close(i));

        let tab_btn = button(
            row![text(label).size(11).color(text_c), close_btn,]
                .spacing(8.0)
                .align_y(iced::Alignment::Center),
        )
        .padding([4, 10])
        .on_press(TabMessage::Select(i))
        .style(move |_: &Theme, status: button::Status| {
            let bg = match (is_active, status) {
                (true, _) => Some(Background::Color(tab_active_bg)),
                (false, button::Status::Hovered) => Some(Background::Color(tab_active_bg)),
                _ => None,
            };
            button::Style {
                background: bg,
                border: Border {
                    width: 1.0,
                    radius: 0.0.into(),
                    color: border,
                },
                ..button::Style::default()
            }
        });

        bar = bar.push(tab_btn);
    }

    container(bar)
        .width(Length::Fill)
        .padding([2, 6])
        .style(styles::toolbar_strip(tokens))
        .into()
}
