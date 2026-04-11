//! Document tab bar — tabs for open schematic sheets and PCB.

use iced::widget::{button, container, row, text, Row};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::app::TabInfo;
use crate::styles;

#[derive(Debug, Clone)]
pub enum TabMessage {
    Select(usize),
    Close(usize),
}

pub fn view<'a>(tabs: &[TabInfo], active: usize) -> Element<'a, TabMessage> {
    let mut bar = Row::new().spacing(2.0);

    for (i, tab) in tabs.iter().enumerate() {
        let label = if tab.dirty {
            format!("{} \u{2022}", tab.title) // bullet for dirty
        } else {
            tab.title.clone()
        };

        let is_active = i == active;
        let text_c = if is_active {
            Color::WHITE
        } else {
            styles::TEXT_MUTED
        };

        // Close button — visible "×" with hover highlight
        let close_btn = button(text("\u{00D7}").size(14).color(styles::TEXT_MUTED))
            .padding([0, 4])
            .style(|_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered => Some(Background::Color(
                        Color::from_rgb(0.35, 0.35, 0.38),
                    )),
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
            row![
                text(label).size(11).color(text_c),
                close_btn,
            ]
            .spacing(8.0)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 10])
        .on_press(TabMessage::Select(i))
        .style(move |_: &Theme, status: button::Status| {
            let bg = match (is_active, status) {
                (true, _) => Some(Background::Color(styles::TAB_ACTIVE_BG)),
                (false, button::Status::Hovered) => {
                    Some(Background::Color(styles::TAB_ACTIVE_BG))
                }
                _ => None,
            };
            button::Style {
                background: bg,
                border: Border {
                    width: 1.0,
                    radius: 0.0.into(),
                    color: styles::BORDER_SUBTLE,
                },
                ..button::Style::default()
            }
        });

        bar = bar.push(tab_btn);
    }

    container(bar)
        .width(Length::Fill)
        .padding([2, 6])
        .style(styles::toolbar_strip)
        .into()
}
